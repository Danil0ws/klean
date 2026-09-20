//! End-to-end checks for the web API (the part with a trust boundary).

use klean::ignore::IgnoreRules;
use klean::patterns::get_default_patterns;
use klean::scanner::ArtifactScanner;
use klean::web::{self, WebConfig};
use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::time::Duration;
use tempfile::TempDir;

fn fixture() -> (TempDir, PathBuf) {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path().canonicalize().unwrap();
    fs::create_dir_all(root.join("proj/node_modules/pkg")).unwrap();
    fs::write(root.join("proj/node_modules/pkg/index.js"), vec![0u8; 4096]).unwrap();
    fs::write(root.join("proj/package.json"), "{}").unwrap();
    (tmp, root)
}

fn scanner(root: &Path) -> ArtifactScanner {
    let ignore = IgnoreRules::from_path(root, true).unwrap();
    ArtifactScanner::new(root.to_path_buf(), ignore, get_default_patterns())
}

fn free_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.local_addr().unwrap().port()
}

fn spawn_server(root: PathBuf, token: Option<String>) -> u16 {
    let port = free_port();
    let scanner = scanner(&root);
    std::thread::spawn(move || {
        let _ = web::serve(
            scanner,
            WebConfig {
                host: "127.0.0.1".to_string(),
                port,
                token,
                root,
                allow_system_paths: false,
                backup_dir: None,
                quiet: true,
            },
        );
    });
    // ponytail: fixed sleep instead of polling a readiness endpoint; the server
    // is already listening by the time serve() returns from TcpListener::bind.
    std::thread::sleep(Duration::from_millis(300));
    port
}

fn request(port: u16, raw: &str) -> String {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    stream.write_all(raw.as_bytes()).unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    response
}

fn get(port: u16, path: &str, auth: Option<&str>) -> String {
    let mut raw = format!("GET {path} HTTP/1.1\r\nHost: localhost\r\n");
    if let Some(token) = auth {
        raw.push_str(&format!("Authorization: Bearer {token}\r\n"));
    }
    raw.push_str("Connection: close\r\n\r\n");
    request(port, &raw)
}

fn post(port: u16, path: &str, body: &str, auth: Option<&str>) -> String {
    let mut raw = format!(
        "POST {path} HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/json\r\nContent-Length: {}\r\n",
        body.len()
    );
    if let Some(token) = auth {
        raw.push_str(&format!("Authorization: Bearer {token}\r\n"));
    }
    raw.push_str("Connection: close\r\n\r\n");
    raw.push_str(body);
    request(port, &raw)
}

#[test]
fn serves_scan_json_and_the_page() {
    let (_tmp, root) = fixture();
    let port = spawn_server(root.clone(), None);

    let page = get(port, "/", None);
    assert!(page.starts_with("HTTP/1.1 200 OK"), "{page}");
    assert!(page.contains("<title>klean</title>"));

    let scan = get(port, "/api/scan", None);
    assert!(scan.contains("200 OK"), "{scan}");
    let body = scan.split("\r\n\r\n").nth(1).unwrap();
    let doc: serde_json::Value = serde_json::from_str(body).unwrap();
    assert_eq!(doc["artifact_count"], 1);
    assert_eq!(doc["projects"][0]["artifacts"][0]["name"], "node_modules");
    assert_eq!(doc["total_bytes"], 4096);
    assert_eq!(
        doc["projects"][0]["artifacts"][0]["relative_path"],
        "proj/node_modules"
    );

    assert!(get(port, "/api/nope", None).starts_with("HTTP/1.1 404"));
}

#[test]
fn clean_only_accepts_paths_from_the_current_scan() {
    let (tmp, root) = fixture();
    // keep the history side effect out of the real config dir
    std::env::set_var("KLEAN_HISTORY", tmp.path().join("history.tsv"));

    let port = spawn_server(root.clone(), None);
    let victim = root.join("proj/package.json");

    // A forged path outside the scan must not be deleted.
    let response = post(port, "/api/clean", r#"{"paths":["/etc"]}"#, None);
    assert!(response.starts_with("HTTP/1.1 409"), "{response}");
    assert!(victim.exists());

    let good = root.join("proj/node_modules");
    let response = post(
        port,
        "/api/clean",
        &format!(r#"{{"paths":["{}"]}}"#, good.display()),
        None,
    );
    assert!(response.starts_with("HTTP/1.1 200"), "{response}");
    assert!(!good.exists(), "artifact should be gone");
    assert!(victim.exists(), "project file must survive");
}

#[test]
fn token_is_required_for_the_api_when_configured() {
    let (_tmp, root) = fixture();
    let port = spawn_server(root, Some("s3cr3t".to_string()));

    // The static page stays public; the API does not.
    assert!(get(port, "/", None).starts_with("HTTP/1.1 200"));
    assert!(get(port, "/api/scan", None).starts_with("HTTP/1.1 401"));
    assert!(get(port, "/api/scan", Some("wrong")).starts_with("HTTP/1.1 401"));
    assert!(get(port, "/api/scan", Some("s3cr3t")).starts_with("HTTP/1.1 200"));
    assert!(post(port, "/api/clean", r#"{"paths":["/etc"]}"#, None).starts_with("HTTP/1.1 401"));
}
