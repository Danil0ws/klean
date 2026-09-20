use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io::stdout;
use std::path::PathBuf;
use std::time::Duration;

use super::{InputMode, Tui, TuiState};
use crate::scanner::Artifact;

pub struct InteractiveMode;

/// Restores the terminal on every exit path, including errors (`?`).
struct TerminalGuard;

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(stdout(), LeaveAlternateScreen);
    }
}

impl InteractiveMode {
    pub fn run(artifacts: Vec<Artifact>, root: PathBuf) -> Result<Option<Vec<Artifact>>> {
        // Setup terminal
        enable_raw_mode()?;
        let mut stdout = stdout();
        execute!(stdout, EnterAlternateScreen)?;
        let guard = TerminalGuard;

        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        let mut state = TuiState::new(artifacts, root);
        let mut result = None;

        loop {
            terminal.draw(|f| match state.input_mode {
                InputMode::Confirmation => Tui::render_confirmation(f, &state),
                _ => Tui::render_list(f, &mut state),
            })?;

            // Handle events
            if event::poll(Duration::from_millis(250))? {
                if let Event::Key(key) = event::read()? {
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
                            KeyCode::Char(' ') => state.toggle_selected(),
                            KeyCode::Char('a') | KeyCode::Char('A') => state.select_all(),
                            KeyCode::Char('d') | KeyCode::Char('D') => state.deselect_all(),
                            KeyCode::Enter if !state.get_selected_artifacts().is_empty() => {
                                state.input_mode = InputMode::Confirmation;
                            }
                            _ => {}
                        },
                        InputMode::Confirmation => match key.code {
                            KeyCode::Char('y') | KeyCode::Char('Y') => {
                                result = Some(state.get_selected_artifacts());
                                break;
                            }
                            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                                state.input_mode = InputMode::Normal;
                            }
                            _ => {}
                        },
                        InputMode::Exiting => break,
                        _ => {}
                    }
                }
            }
        }

        // Restore terminal before the caller prints anything.
        drop(guard);

        Ok(result)
    }
}
