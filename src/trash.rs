//! Undo for deletions.
//!
//! `remove_dir_all` cannot be undone, so an undoable clean *moves* the artifact
//! into a per-run session under the klean dir and journals the original path.
//! `klean undo` moves the newest session back; sessions older than
//! [`KEEP_DAYS`] are purged on the next trash move, so the space does come back.

use crate::config::dirs;
use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Sessions older than this are purged on the next move into the trash.
pub const KEEP_DAYS: u64 = 7;

const JOURNAL: &str = "journal.tsv";

/// `$KLEAN_TRASH`, or `<config>/klean/trash` next to the history file.
pub fn trash_root() -> Result<PathBuf> {
    if let Ok(custom) = std::env::var("KLEAN_TRASH") {
        if !custom.is_empty() {
            return Ok(PathBuf::from(custom));
        }
    }
    let config = dirs::config_dir().context("cannot determine the config directory")?;
    Ok(config.join("klean").join("trash"))
}

struct Entry {
    original: PathBuf,
    stored: String,
    size: u64,
}

/// Items moved during one clean run.
pub struct Session {
    dir: PathBuf,
    entries: Vec<Entry>,
}

impl Session {
    /// Opens a new session, purging sessions past the keep window on the way.
    pub fn open() -> Result<Self> {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .context("system clock before 1970")?
            .as_secs();
        let dir = trash_root()?.join(stamp.to_string());
        fs::create_dir_all(&dir).with_context(|| format!("Failed to create {}", dir.display()))?;
        // Best effort: a purge failure must not block the clean itself.
        let _ = purge(KEEP_DAYS);
        Ok(Session {
            dir,
            entries: Vec::new(),
        })
    }

    /// Moves one artifact in, keeping a unique name per item.
    pub fn store(&mut self, path: &Path, size: u64) -> Result<()> {
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("item");
        let stored = format!("{}-{}", self.entries.len(), name);
        fs::rename(path, self.dir.join(&stored))
            .with_context(|| format!("Failed to move {} into the trash", path.display()))?;
        self.entries.push(Entry {
            original: path.to_path_buf(),
            stored,
            size,
        });
        Ok(())
    }

    /// Writes the journal `klean undo` reads; an unused session is removed.
    pub fn finish(&self) -> Result<()> {
        if self.entries.is_empty() {
            let _ = fs::remove_dir_all(&self.dir);
            return Ok(());
        }

        let mut journal = String::new();
        for entry in &self.entries {
            journal.push_str(&format!(
                "{}\t{}\t{}\n",
                entry.stored,
                entry.original.display(),
                entry.size
            ));
        }
        fs::write(self.dir.join(JOURNAL), journal)?;
        Ok(())
    }
}

/// What `klean undo` managed to move back.
#[derive(Debug, Default)]
pub struct UndoReport {
    /// Restored path and the bytes it held (the TUI rebuilds rows from this).
    pub restored: Vec<(PathBuf, u64)>,
    pub failed: Vec<(PathBuf, String)>,
    pub bytes: u64,
}

/// Restores the newest session. An original path that exists again is left in
/// the trash instead of being overwritten.
pub fn undo_last() -> Result<Option<UndoReport>> {
    let Some(session) = newest_session()? else {
        return Ok(None);
    };

    let mut report = UndoReport::default();
    let mut leftovers = String::new();

    for line in fs::read_to_string(session.join(JOURNAL))?.lines() {
        let mut fields = line.split('\t');
        let (Some(stored), Some(original), size) = (fields.next(), fields.next(), fields.next())
        else {
            continue;
        };
        let size: u64 = size.unwrap_or("0").parse().unwrap_or(0);
        let original = PathBuf::from(original);

        if original.exists() {
            report
                .failed
                .push((original.clone(), "já existe de novo".to_string()));
            leftovers.push_str(&format!("{stored}\t{}\t{size}\n", original.display()));
            continue;
        }

        if let Some(parent) = original.parent() {
            fs::create_dir_all(parent)?;
        }

        match fs::rename(session.join(stored), &original) {
            Ok(()) => {
                report.bytes += size;
                report.restored.push((original, size));
            }
            Err(error) => {
                report.failed.push((original.clone(), error.to_string()));
                leftovers.push_str(&format!("{stored}\t{}\t{size}\n", original.display()));
            }
        }
    }

    if leftovers.is_empty() {
        fs::remove_dir_all(&session)?;
    } else {
        fs::write(session.join(JOURNAL), leftovers)?;
    }

    Ok(Some(report))
}

/// One line per session, for `klean trash`.
pub struct SessionInfo {
    pub age_secs: u64,
    pub items: usize,
    pub bytes: u64,
}

pub fn list() -> Result<Vec<SessionInfo>> {
    let root = trash_root()?;
    let mut sessions = Vec::new();

    for dir in session_dirs(&root)? {
        let (items, bytes) = journal_totals(&dir)?;
        sessions.push(SessionInfo {
            age_secs: age_of(&dir).unwrap_or(0),
            items,
            bytes,
        });
    }

    sessions.sort_by_key(|session| session.age_secs);
    Ok(sessions)
}

/// Drops sessions older than `days`, returning how many went away.
pub fn purge(days: u64) -> Result<usize> {
    let root = trash_root()?;
    let limit = days.saturating_mul(24 * 60 * 60);
    let mut purged = 0;

    for dir in session_dirs(&root)? {
        if age_of(&dir).is_some_and(|age| age >= limit) {
            fs::remove_dir_all(&dir)?;
            purged += 1;
        }
    }

    Ok(purged)
}

/// Empties the whole trash: sessions and items.
pub fn empty() -> Result<(usize, u64)> {
    let root = trash_root()?;
    let dirs = session_dirs(&root)?;
    let mut bytes = 0;

    for dir in &dirs {
        bytes += journal_totals(dir)?.1;
        fs::remove_dir_all(dir)?;
    }

    Ok((dirs.len(), bytes))
}

fn session_dirs(root: &Path) -> Result<Vec<PathBuf>> {
    if !root.exists() {
        return Ok(Vec::new());
    }

    let mut dirs: Vec<PathBuf> = fs::read_dir(root)?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| path.join(JOURNAL).exists())
        .collect();
    dirs.sort();
    Ok(dirs)
}

/// Newest session by its timestamp directory name.
fn newest_session() -> Result<Option<PathBuf>> {
    let root = trash_root()?;
    Ok(session_dirs(&root)?
        .into_iter()
        .filter_map(|dir| Some((dir.file_name()?.to_str()?.parse::<u64>().ok()?, dir)))
        .max_by_key(|(stamp, _)| *stamp)
        .map(|(_, dir)| dir))
}

fn journal_totals(dir: &Path) -> Result<(usize, u64)> {
    let mut items = 0;
    let mut bytes = 0;

    for line in fs::read_to_string(dir.join(JOURNAL))?.lines() {
        let mut fields = line.split('\t');
        if let (Some(_), Some(_), Some(size)) = (fields.next(), fields.next(), fields.next()) {
            items += 1;
            bytes += size.parse::<u64>().unwrap_or(0);
        }
    }

    Ok((items, bytes))
}

fn age_of(dir: &Path) -> Option<u64> {
    let stamp = dir.file_name()?.to_str()?.parse::<u64>().ok()?;
    let now = SystemTime::now().duration_since(UNIX_EPOCH).ok()?.as_secs();
    Some(now.saturating_sub(stamp))
}

/// "há 3h 12min" — enough for a trash listing, no date library needed.
pub fn human_age(secs: u64) -> String {
    const MINUTE: u64 = 60;
    const HOUR: u64 = 60 * MINUTE;
    const DAY: u64 = 24 * HOUR;

    match secs {
        s if s < MINUTE => "agora mesmo".to_string(),
        s if s < HOUR => format!("há {}min", s / MINUTE),
        s if s < DAY => format!("há {}h {}min", s / HOUR, (s % HOUR) / MINUTE),
        s => format!("há {} dia(s)", s / DAY),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `KLEAN_TRASH` is process-wide, so these tests must not overlap.
    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn with_temp_trash<T>(body: impl FnOnce(&Path) -> T) -> T {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|error| error.into_inner());
        let dir = std::env::temp_dir().join(format!(
            "klean-trash-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        std::env::set_var("KLEAN_TRASH", &dir);
        let out = body(&dir);
        let _ = fs::remove_dir_all(&dir);
        out
    }

    #[test]
    fn a_trashed_artifact_comes_back_with_undo() {
        with_temp_trash(|trash| {
            let project = std::env::temp_dir().join("klean-trash-project");
            let _ = fs::remove_dir_all(&project);
            let artifact = project.join("app/node_modules");
            fs::create_dir_all(&artifact).unwrap();
            fs::write(artifact.join("index.js"), b"conteudo").unwrap();

            let mut session = Session::open().unwrap();
            session.store(&artifact, 8).unwrap();
            session.finish().unwrap();

            assert!(!artifact.exists(), "o original foi movido");
            assert_eq!(list().unwrap().len(), 1);

            let report = undo_last().unwrap().unwrap();
            assert_eq!(report.restored, vec![(artifact.clone(), 8)]);
            assert_eq!(report.bytes, 8);
            assert!(report.failed.is_empty());
            assert_eq!(
                fs::read_to_string(artifact.join("index.js")).unwrap(),
                "conteudo"
            );
            assert!(session_dirs(trash).unwrap().is_empty(), "sessão consumida");
            assert!(undo_last().unwrap().is_none(), "nada mais para desfazer");

            let _ = fs::remove_dir_all(&project);
        });
    }

    #[test]
    fn an_original_that_came_back_is_kept_in_the_trash() {
        with_temp_trash(|_| {
            let artifact = std::env::temp_dir().join("klean-trash-conflict/node_modules");
            let _ = fs::remove_dir_all(artifact.parent().unwrap());
            fs::create_dir_all(&artifact).unwrap();

            let mut session = Session::open().unwrap();
            session.store(&artifact, 1).unwrap();
            session.finish().unwrap();

            // Something recreated the path before the undo.
            fs::create_dir_all(artifact.join("novo")).unwrap();

            let report = undo_last().unwrap().unwrap();
            assert!(report.restored.is_empty());
            assert_eq!(report.failed.len(), 1);
            assert_eq!(list().unwrap().len(), 1, "a sessão continua lá");

            let _ = fs::remove_dir_all(artifact.parent().unwrap());
        });
    }

    #[test]
    fn purge_and_empty_clear_the_sessions() {
        with_temp_trash(|_| {
            let artifact = std::env::temp_dir().join("klean-trash-purge/node_modules");
            let _ = fs::remove_dir_all(artifact.parent().unwrap());
            fs::create_dir_all(&artifact).unwrap();

            let mut session = Session::open().unwrap();
            session.store(&artifact, 2).unwrap();
            session.finish().unwrap();

            assert_eq!(purge(0).unwrap(), 1, "idade 0 >= limite 0");
            assert!(list().unwrap().is_empty());

            let _ = fs::remove_dir_all(artifact.parent().unwrap());
        });
    }

    #[test]
    fn ages_read_like_english() {
        assert_eq!(human_age(30), "agora mesmo");
        assert_eq!(human_age(120), "há 2min");
        assert_eq!(human_age(3600 + 600), "há 1h 10min");
        assert_eq!(human_age(3 * 24 * 3600), "há 3 dia(s)");
    }
}
