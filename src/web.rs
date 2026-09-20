//! Minimal web UI for remote management: scan, review and clean from a browser.
//!
//! `klean serve --host 0.0.0.0 --token <token>` exposes:
//!   GET  /            single-page UI (vanilla JS, no build step)
//!   GET  /api/scan    current artifacts as JSON
//!   POST /api/clean   {"paths": [...]} — only paths from a fresh scan are accepted
//!
//! ponytail: hand-rolled single-threaded HTTP over std TcpListener (no new dep,
//! one request at a time, Connection: close). Swap for axum/tiny_http when
//! concurrent clients matter.

use crate::cleaner::{Cleaner, CleanerAction};
use crate::scanner::{Artifact, ArtifactScanner, ScanOutcome};
use anyhow::{bail, Context, Result};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::time::Duration;

pub struct WebConfig {
    pub host: String,
    pub port: u16,
    pub token: Option<String>,
    pub root: PathBuf,
    pub allow_system_paths: bool,
    pub backup_dir: Option<PathBuf>,
    pub quiet: bool,
}

pub fn serve(scanner: ArtifactScanner, config: WebConfig) -> Result<()> {
    let loopback = matches!(config.host.as_str(), "127.0.0.1" | "localhost" | "::1");
    if !loopback && config.token.is_none() {
        // A cleaning API reachable from the network without auth deletes other
        // people's directories. Refuse instead of relying on good intentions.
        bail!(
            "recusando expor {} sem --token (ou KLEAN_TOKEN): uma API que apaga pastas precisa de autenticação",
            config.host
        );
    }

    let listener = TcpListener::bind((config.host.as_str(), config.port)).context(format!(
        "não consegui escutar em {}:{}",
        config.host, config.port
    ))?;
    let addr = listener.local_addr()?;

    if !config.quiet {
        println!("🌐 klean serve em http://{}/", addr);
        println!("   raiz:  {}", config.root.display());
        println!(
            "   token: {}",
            if config.token.is_some() {
                "obrigatório (/api/*)"
            } else {
                "não"
            }
        );
        if config.token.is_some() {
            println!("   abra http://{}/?token=<token> no navegador", addr);
        }
    }

    for connection in listener.incoming() {
        match connection {
            Ok(stream) => {
                if let Err(err) = handle(stream, &scanner, &config) {
                    eprintln!("⚠️  requisição falhou: {err}");
                }
            }
            Err(err) => eprintln!("⚠️  conexão recusada: {err}"),
        }
    }

    Ok(())
}

fn handle(mut stream: TcpStream, scanner: &ArtifactScanner, config: &WebConfig) -> Result<()> {
    stream.set_read_timeout(Some(Duration::from_secs(15)))?;
    stream.set_write_timeout(Some(Duration::from_secs(30)))?;

    let mut reader = BufReader::new(stream.try_clone()?);

    let mut request_line = String::new();
    reader.read_line(&mut request_line)?;
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or_default().to_string();
    let target = parts.next().unwrap_or("/").to_string();

    let mut content_length = 0usize;
    let mut authorization = String::new();
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line)? == 0 {
            break;
        }
        let trimmed = line.trim_end();
        if trimmed.is_empty() {
            break;
        }
        let lower = trimmed.to_ascii_lowercase();
        if let Some(value) = lower.strip_prefix("content-length:") {
            content_length = value.trim().parse().unwrap_or(0);
        }
        if let Some(value) = lower.strip_prefix("authorization:") {
            authorization = value.trim().to_string();
        }
    }

    let path = target.split('?').next().unwrap_or("/").to_string();

    if path.starts_with("/api/") {
        if let Some(expected) = &config.token {
            let provided = authorization
                .strip_prefix("bearer ")
                .unwrap_or(&authorization)
                .to_string();
            if provided != *expected {
                return respond(&mut stream, 401, "text/plain", "unauthorized");
            }
        }
    }

    match (method.as_str(), path.as_str()) {
        ("GET", "/") | ("GET", "/index.html") => respond(&mut stream, 200, "text/html", PAGE),
        ("GET", "/api/scan") => {
            let outcome = scanner.scan()?;
            let body = scan_json(&outcome, &config.root)?;
            respond(&mut stream, 200, "application/json", &body)
        }
        ("POST", "/api/clean") => {
            let mut body = vec![0u8; content_length.min(1 << 20)];
            reader.read_exact(&mut body)?;
            let text = String::from_utf8_lossy(&body).to_string();
            let requested: Vec<String> = serde_json::from_str::<serde_json::Value>(&text)
                .ok()
                .and_then(|value| value.get("paths").cloned())
                .and_then(|value| serde_json::from_value(value).ok())
                .unwrap_or_default();

            if requested.is_empty() {
                return respond(&mut stream, 400, "text/plain", "nenhum path informado");
            }

            // Trust boundary: only paths that the current scan reports are
            // deletable, so a forged request cannot erase anything else.
            let outcome = scanner.scan()?;
            let allowed: Vec<Artifact> = outcome
                .artifacts
                .into_iter()
                .filter(|artifact| {
                    requested
                        .iter()
                        .any(|path| artifact.path.to_string_lossy() == *path)
                })
                .collect();

            if allowed.is_empty() {
                return respond(&mut stream, 409, "text/plain", "paths fora do scan atual");
            }

            let action = if config.backup_dir.is_some() {
                CleanerAction::Backup
            } else {
                CleanerAction::Delete
            };
            let cleaner =
                Cleaner::new(action, config.backup_dir.clone(), config.allow_system_paths)
                    .with_root(config.root.clone());
            cleaner.verify_safety(&allowed)?;
            let result = cleaner.clean(allowed, false)?;

            crate::history::record(
                &config.root,
                "delete",
                result.total_size_freed,
                result.deleted,
                0,
            )?;

            let body = serde_json::json!({
                "deleted": result.deleted,
                "failed": result.failed,
                "freed_bytes": result.total_size_freed,
                "freed_human": humansize::format_size(result.total_size_freed, humansize::BINARY),
                "errors": result.errors,
            });
            respond(
                &mut stream,
                200,
                "application/json",
                &serde_json::to_string(&body)?,
            )
        }
        _ => respond(&mut stream, 404, "text/plain", "not found"),
    }
}

fn scan_json(outcome: &ScanOutcome, root: &std::path::Path) -> Result<String> {
    let groups = outcome.groups();
    let total: u64 = outcome.artifacts.iter().map(|a| a.size).sum();

    let doc = serde_json::json!({
        "root": root.display().to_string(),
        "total_bytes": total,
        "total_human": humansize::format_size(total, humansize::BINARY),
        "artifact_count": outcome.artifacts.len(),
        "project_count": groups.len(),
        "projects": groups
            .iter()
            .map(|group| serde_json::json!({
                "root": group.root.display().to_string(),
                "total_bytes": group.total_size,
                "total_human": group.size_string(),
                "artifacts": group
                    .artifacts
                    .iter()
                    .map(|artifact| serde_json::json!({
                        "name": artifact.name,
                        "pattern": artifact.pattern_name,
                        "path": artifact.path.display().to_string(),
                        "relative_path": artifact.relative_to(root).display().to_string(),
                        "size_bytes": artifact.size,
                        "size_human": artifact.size_string(),
                        "safe": artifact.is_safe,
                    }))
                    .collect::<Vec<_>>(),
            }))
            .collect::<Vec<_>>(),
        "blocked": outcome
            .blocked
            .iter()
            .map(|blocked| serde_json::json!({
                "path": blocked.path.display().to_string(),
                "name": blocked.name,
                "project": blocked.project.display().to_string(),
                "reason": blocked.reason,
            }))
            .collect::<Vec<_>>(),
    });

    Ok(serde_json::to_string(&doc)?)
}

fn respond(stream: &mut TcpStream, status: u16, content_type: &str, body: &str) -> Result<()> {
    let reason = match status {
        200 => "OK",
        400 => "Bad Request",
        401 => "Unauthorized",
        404 => "Not Found",
        409 => "Conflict",
        _ => "Internal Server Error",
    };

    let head = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: {content_type}; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\nCache-Control: no-store\r\n\r\n",
        body.len()
    );

    stream.write_all(head.as_bytes())?;
    stream.write_all(body.as_bytes())?;
    stream.flush()?;
    Ok(())
}

const PAGE: &str = r#"<!doctype html>
<html lang="pt-BR">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>klean</title>
<style>
:root { color-scheme: dark; }
body { margin:0; font:14px/1.4 ui-monospace,SFMono-Regular,Menlo,monospace; background:#11141a; color:#e6e6e6; }
header { padding:14px 18px; background:#161a22; border-bottom:1px solid #263041; display:flex; gap:16px; align-items:baseline; flex-wrap:wrap; }
h1 { font-size:16px; margin:0; }
#root { color:#7dd3fc; }
#total { color:#9ca3af; margin-left:auto; }
.bar { padding:10px 18px; display:flex; gap:14px; align-items:center; flex-wrap:wrap; border-bottom:1px solid #263041; }
button { background:#1f2937; color:#e6e6e6; border:1px solid #374151; padding:6px 12px; border-radius:6px; cursor:pointer; font:inherit; }
button:hover { background:#273244; }
button.primary { background:#1d4ed8; border-color:#2563eb; }
table { width:100%; border-collapse:collapse; }
th,td { text-align:left; padding:6px 18px; border-bottom:1px solid #1d2430; vertical-align:top; }
th { color:#93a4bd; font-weight:600; position:sticky; top:0; background:#11141a; }
td.p { color:#8b9bb4; }
td.num { text-align:right; white-space:nowrap; }
tr:hover td { background:#151a24; }
#msg { padding:8px 18px; color:#fbbf24; min-height:20px; }
.empty { padding:24px 18px; color:#8b9bb4; }
</style>
</head>
<body>
<header><h1>klean</h1><span id="root"></span><span id="total"></span></header>
<div class="bar">
  <button id="refresh">↻ Atualizar</button>
  <label><input type="checkbox" id="all"> tudo</label>
  <button id="clean" class="primary">🧹 Limpar selecionados</button>
  <button id="stats">📈 Histórico</button>
</div>
<div id="msg"></div>
<table>
  <thead><tr><th></th><th>Artefato</th><th>Tamanho</th><th>Projeto</th><th>Caminho</th></tr></thead>
  <tbody id="rows"></tbody>
</table>
<div id="empty" class="empty" hidden>Nenhum artefato encontrado.</div>
<pre id="statsout" class="empty" hidden></pre>
<script>
var token = new URLSearchParams(location.search).get('token') || '';
function api(path, opts) {
  opts = opts || {};
  var headers = opts.headers || {};
  if (token) { headers['Authorization'] = 'Bearer ' + token; }
  opts.headers = headers;
  return fetch(path, opts).then(function (res) {
    if (!res.ok) { return res.text().then(function (t) { throw new Error(t || res.status); }); }
    return res.json();
  });
}
function esc(s) { return String(s).replace(/[&<>"]/g, function (c) { return ({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;'})[c]; }); }
function load() {
  return api('/api/scan').then(function (data) {
    document.getElementById('root').textContent = data.root;
    document.getElementById('total').textContent = data.total_human + ' — ' + data.artifact_count + ' item(ns), ' + data.project_count + ' projeto(s)';
    var rows = document.getElementById('rows');
    rows.innerHTML = '';
    data.projects.forEach(function (p) {
      p.artifacts.forEach(function (a) {
        var tr = document.createElement('tr');
        tr.innerHTML = '<td><input type="checkbox" data-path="' + esc(a.path) + '"></td>'
          + '<td>' + esc(a.name) + '</td>'
          + '<td class="num">' + esc(a.size_human) + '</td>'
          + '<td>' + esc(p.root) + '</td>'
          + '<td class="p">' + esc(a.path) + '</td>';
        rows.appendChild(tr);
      });
    });
    document.getElementById('empty').hidden = data.artifact_count > 0;
    document.getElementById('msg').textContent = data.blocked.length
      ? data.blocked.length + ' item(ns) protegido(s) por regra de projeto'
      : '';
  }).catch(show);
}
function show(err) { document.getElementById('msg').textContent = 'erro: ' + err.message; }
function clean() {
  var paths = Array.prototype.map.call(document.querySelectorAll('input[data-path]:checked'), function (c) { return c.dataset.path; });
  if (!paths.length) { return; }
  if (!confirm('Apagar ' + paths.length + ' pasta(s)? Isso não tem volta.')) { return; }
  api('/api/clean', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ paths: paths })
  }).then(function (res) {
    document.getElementById('msg').textContent = 'Removidos ' + res.deleted + ' — liberado ' + res.freed_human
      + (res.failed ? ' — ' + res.failed + ' falha(s)' : '');
    return load();
  }).catch(show);
}
function stats() {
  api('/api/scan').then(function () { return load(); });
  var out = document.getElementById('statsout');
  out.hidden = false;
  out.textContent = 'Histórico: rode `klean stats` no host (o arquivo fica em ~/.config/klean/history.tsv).';
}
document.getElementById('refresh').onclick = load;
document.getElementById('clean').onclick = clean;
document.getElementById('stats').onclick = stats;
document.getElementById('all').onchange = function (e) {
  Array.prototype.forEach.call(document.querySelectorAll('input[data-path]'), function (c) { c.checked = e.target.checked; });
};
load();
</script>
</body>
</html>
"#;
