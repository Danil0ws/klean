use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, MouseEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::collections::BTreeSet;
use std::io::stdout;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use super::{CleanContext, InputMode, ScanFeed, Tui, TuiState};
use crate::cleaner::{CleanResult, Cleaner, CleanerAction};
use crate::history;
use crate::scanner::ArtifactScanner;
use crate::trash;

/// What a TUI session did, so the shell can say it once the screen is restored.
#[derive(Debug, Default, Clone, Copy)]
pub struct SessionReport {
    pub freed: u64,
    pub cleaned: usize,
}

pub struct InteractiveMode;

/// Restores the terminal on every exit path, including errors (`?`).
struct TerminalGuard;

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        // Mouse capture must be turned off too, or the shell keeps receiving
        // mouse reports after klean exits.
        let _ = execute!(stdout(), DisableMouseCapture, LeaveAlternateScreen);
    }
}

impl InteractiveMode {
    /// Scans on a worker thread behind the loading screen, then runs the list on
    /// the alternate screen. Cleaning happens here too, so the result stays on
    /// screen instead of flashing past after the UI closes.
    pub fn run_scan(scanner: ArtifactScanner, ctx: CleanContext) -> Result<SessionReport> {
        // The scan runs off-thread so the loading screen can breathe: it names
        // the directory being read and counts folders while it works.
        let feed = Arc::new(Mutex::new(ScanFeed::new()));
        let worker = Arc::clone(&feed);
        let handle = std::thread::spawn(move || {
            let progress = Arc::clone(&worker);
            let scanner = scanner.with_progress(Box::new(move |path: &Path| {
                if let Ok(mut feed) = progress.lock() {
                    feed.current = Some(path.to_path_buf());
                    feed.dirs += 1;
                }
            }));

            let outcome = scanner.scan();
            let mut feed = worker.lock().unwrap_or_else(|error| error.into_inner());
            feed.done = true;
            feed.outcome = Some(match outcome {
                Ok(outcome) => Ok(outcome.artifacts),
                Err(error) => Err(error.to_string()),
            });
        });

        // Setup terminal. Mouse capture is what makes the wheel scroll the list
        // instead of the terminal scrollback.
        enable_raw_mode()?;
        let mut stdout = stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
        let guard = TerminalGuard;

        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        let mut state = TuiState::new(Vec::new(), ctx.root.clone());
        let mut scanning = true;

        loop {
            // Fold the worker's result in as soon as it lands.
            if scanning {
                let mut feed = feed.lock().unwrap_or_else(|error| error.into_inner());
                if feed.done {
                    match feed.outcome.take() {
                        Some(Ok(artifacts)) => state = TuiState::new(artifacts, ctx.root.clone()),
                        Some(Err(error)) => state.notice = Some(format!("✗ {error}")),
                        None => {}
                    }
                    scanning = false;
                }
            }

            terminal.draw(|f| {
                if scanning {
                    let feed = feed.lock().unwrap_or_else(|error| error.into_inner());
                    Tui::render_scanning(f, &feed, &ctx.root);
                } else {
                    match state.input_mode {
                        InputMode::Confirmation => Tui::render_confirmation(f, &state),
                        InputMode::Summary => Tui::render_summary(f, &state),
                        _ => Tui::render_list(f, &mut state),
                    }
                }
            })?;

            // Handle events
            if event::poll(Duration::from_millis(if scanning { 60 } else { 250 }))? {
                match event::read()? {
                    Event::Key(key) => {
                        if key.kind == event::KeyEventKind::Release {
                            continue;
                        }
                        match state.input_mode {
                            InputMode::Normal => match key.code {
                                KeyCode::Char('q') | KeyCode::Esc => {
                                    state.input_mode = InputMode::Exiting;
                                    break;
                                }
                                KeyCode::Up | KeyCode::Char('k') => state.move_up(),
                                KeyCode::Down | KeyCode::Char('j') => state.move_down(),
                                KeyCode::PageUp => state.move_page(false),
                                KeyCode::PageDown => state.move_page(true),
                                KeyCode::Home => state.move_to_first(),
                                KeyCode::End => state.move_to_last(),
                                KeyCode::Char(' ') => state.toggle_selected(),
                                KeyCode::Char('a') | KeyCode::Char('A') => state.select_all(),
                                KeyCode::Char('d') | KeyCode::Char('D') => state.deselect_all(),
                                KeyCode::Enter if !state.get_selected_artifacts().is_empty() => {
                                    state.input_mode = InputMode::Confirmation;
                                }
                                _ => {}
                            },
                            InputMode::Confirmation => match key.code {
                                KeyCode::Enter | KeyCode::Char('y') | KeyCode::Char('Y') => {
                                    let action = if ctx.backup_dir.is_some() {
                                        CleanerAction::Backup
                                    } else {
                                        CleanerAction::Delete
                                    };
                                    run_clean(&mut state, &ctx, action);
                                }
                                // Trash instead of delete: `U` (or `klean undo`)
                                // brings it all back.
                                KeyCode::Char('t') | KeyCode::Char('T') => {
                                    run_clean(&mut state, &ctx, CleanerAction::Trash);
                                }
                                KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                                    state.input_mode = InputMode::Normal;
                                }
                                _ => {}
                            },
                            // The result stays until it is dismissed; Q still exits.
                            InputMode::Summary => match key.code {
                                KeyCode::Char('q') | KeyCode::Esc => break,
                                KeyCode::Char('u') | KeyCode::Char('U') => {
                                    match trash::undo_last() {
                                        Ok(Some(report)) if !report.restored.is_empty() => {
                                            state.apply_undo(&report.restored)
                                        }
                                        Ok(_) => {
                                            state.last_result = None;
                                            state.notice =
                                                Some("nada na lixeira para desfazer".to_string());
                                            state.input_mode = InputMode::Normal;
                                        }
                                        Err(error) => {
                                            state.last_result = None;
                                            state.notice = Some(format!("✗ {error}"));
                                            state.input_mode = InputMode::Normal;
                                        }
                                    }
                                }
                                _ => {
                                    state.last_result = None;
                                    state.input_mode = InputMode::Normal;
                                }
                            },
                            InputMode::Exiting => break,
                            _ => {}
                        }
                    }
                    // Wheel scrolling, available because of EnableMouseCapture.
                    Event::Mouse(mouse) if state.input_mode == InputMode::Normal => {
                        match mouse.kind {
                            MouseEventKind::ScrollDown => state.move_down(),
                            MouseEventKind::ScrollUp => state.move_up(),
                            MouseEventKind::ScrollRight => state.move_page(true),
                            MouseEventKind::ScrollLeft => state.move_page(false),
                            _ => {}
                        }
                    }
                    _ => {}
                }
            }
        }

        let report = SessionReport {
            freed: state.session_freed,
            cleaned: state.session_cleaned,
        };
        let _ = handle.join();
        // Restore terminal before the caller prints anything.
        drop(guard);

        Ok(report)
    }
}

/// Runs one clean for the current selection and folds the result back in.
fn run_clean(state: &mut TuiState, ctx: &CleanContext, action: CleanerAction) {
    match clean_selection(state, ctx, action) {
        Ok(result) => state.apply_clean_result(&result),
        Err(error) => {
            state.notice = Some(format!("✗ {error}"));
            state.input_mode = InputMode::Normal;
        }
    }
}

/// Cleans the current selection and records the run in the history file.
fn clean_selection(
    state: &TuiState,
    ctx: &CleanContext,
    action: CleanerAction,
) -> Result<CleanResult> {
    let to_clean = state.get_selected_artifacts();
    let action_label = match &action {
        CleanerAction::Trash => "trash",
        CleanerAction::Backup => "backup",
        CleanerAction::Delete => "delete",
    };

    let cleaner = Cleaner::new(action, ctx.backup_dir.clone(), ctx.allow_system_paths)
        .with_root(ctx.root.clone())
        // The TUI owns the screen: a progress bar here would be painted on top
        // of the table.
        .with_silent(true);
    cleaner.verify_safety(&to_clean)?;

    let projects = to_clean
        .iter()
        .map(|artifact| artifact.project.clone())
        .collect::<BTreeSet<_>>()
        .len();

    let result = cleaner.clean(to_clean, false)?;

    history::record(
        &ctx.root,
        action_label,
        result.total_size_freed,
        result.deleted + result.backed_up,
        projects,
    )?;

    Ok(result)
}
