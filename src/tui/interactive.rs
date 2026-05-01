use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io::stdout;
use std::time::Duration;

use super::{InputMode, Tui, TuiState};
use crate::scanner::Artifact;

pub struct InteractiveMode;

impl InteractiveMode {
    pub fn run(artifacts: Vec<Artifact>) -> Result<Option<Vec<Artifact>>> {
        // Setup terminal
        enable_raw_mode()?;
        let mut stdout = stdout();
        execute!(stdout, EnterAlternateScreen)?;

        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        let mut state = TuiState::new(artifacts);
        let mut result = None;

        loop {
            terminal.draw(|f| match state.input_mode {
                InputMode::Confirmation => {
                    Tui::render_confirmation(f, &state);
                }
                _ => {
                    Tui::render_list(f, &mut state);
                }
            })?;

            // Handle events
            if event::poll(Duration::from_millis(250))? {
                if let Event::Key(key) = event::read()? {
                    match state.input_mode {
                        InputMode::Normal => match key.code {
                            KeyCode::Char('q') | KeyCode::Esc => {
                                state.input_mode = InputMode::Exiting;
                                break;
                            }
                            KeyCode::Up => state.move_up(),
                            KeyCode::Down => state.move_down(),
                            KeyCode::Char(' ') => state.toggle_selected(),
                            KeyCode::Char('a') | KeyCode::Char('A') => state.select_all(),
                            KeyCode::Char('d') | KeyCode::Char('D') => state.deselect_all(),
                            KeyCode::Enter => {
                                if !state.get_selected_artifacts().is_empty() {
                                    state.input_mode = InputMode::Confirmation;
                                }
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

        // Restore terminal
        disable_raw_mode()?;
        execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
        terminal.show_cursor()?;

        Ok(result)
    }
}
