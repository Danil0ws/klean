use crate::scanner::Artifact;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
    Frame,
};

pub mod interactive;

pub use interactive::InteractiveMode;

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq)]
pub enum InputMode {
    Normal,
    Selection,
    Confirmation,
    Exiting,
}

pub struct TuiState {
    pub selected: usize,
    pub artifacts: Vec<Artifact>,
    pub selected_artifacts: Vec<bool>,
    pub input_mode: InputMode,
    pub total_selected_size: u64,
    pub list_state: ListState,
}

impl TuiState {
    pub fn new(artifacts: Vec<Artifact>) -> Self {
        let count = artifacts.len();
        TuiState {
            selected: 0,
            artifacts,
            selected_artifacts: vec![false; count],
            input_mode: InputMode::Normal,
            total_selected_size: 0,
            list_state: {
                let mut s = ListState::default();
                s.select(Some(0));
                s
            },
        }
    }

    pub fn select_all(&mut self) {
        self.selected_artifacts.iter_mut().for_each(|s| *s = true);
        self.update_total_size();
    }

    pub fn deselect_all(&mut self) {
        self.selected_artifacts.iter_mut().for_each(|s| *s = false);
        self.total_selected_size = 0;
    }

    pub fn toggle_selected(&mut self) {
        if self.selected < self.selected_artifacts.len() {
            self.selected_artifacts[self.selected] = !self.selected_artifacts[self.selected];
            self.update_total_size();
        }
    }

    fn update_total_size(&mut self) {
        self.total_selected_size = self
            .artifacts
            .iter()
            .enumerate()
            .filter(|(i, _)| self.selected_artifacts.get(*i).copied().unwrap_or(false))
            .map(|(_, a)| a.size)
            .sum();
    }

    pub fn move_up(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
            self.list_state.select(Some(self.selected));
        }
    }

    pub fn move_down(&mut self) {
        if self.selected < self.artifacts.len().saturating_sub(1) {
            self.selected += 1;
            self.list_state.select(Some(self.selected));
        }
    }

    pub fn get_selected_artifacts(&self) -> Vec<Artifact> {
        self.artifacts
            .iter()
            .enumerate()
            .filter(|(i, _)| self.selected_artifacts.get(*i).copied().unwrap_or(false))
            .map(|(_, a)| a.clone())
            .collect()
    }
}

pub struct Tui;

impl Tui {
    pub fn render_list(f: &mut Frame, state: &mut TuiState) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([Constraint::Min(0), Constraint::Length(3)])
            .split(f.area());

        // List of artifacts
        let items: Vec<ListItem> = state
            .artifacts
            .iter()
            .enumerate()
            .map(|(i, artifact)| {
                let is_selected = state.selected_artifacts.get(i).copied().unwrap_or(false);
                let is_highlighted = i == state.selected;

                let prefix = if is_selected { "✓" } else { " " };
                // Show the artifact name and size, and display the parent folder
                // where the artifact (e.g. node_modules) was found. This makes the
                // UI similar to tools like npkill which show the project folder
                // containing the large artifact instead of listing many nested
                // paths inside it.
                let parent_display = artifact
                    .path
                    .parent()
                    .unwrap_or_else(|| std::path::Path::new("."))
                    .display();

                let label = format!("{}  {} ({}) - {}", prefix, artifact.name, artifact.size_string(), parent_display);

                let style = if is_highlighted {
                    Style::default()
                        .bg(Color::Blue)
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD)
                } else if is_selected {
                    Style::default().fg(Color::Green)
                } else {
                    Style::default()
                };

                ListItem::new(label).style(style)
            })
            .collect();

        let list = List::new(items)
            .block(Block::default().title("Artifacts Found").borders(Borders::ALL))
            .style(Style::default());

        // Render statefully so the List scrolls when selection moves off-screen
        f.render_stateful_widget(list, chunks[0], &mut state.list_state);

        // Status bar
        let status_text = vec![
            Line::from(vec![
                Span::styled("↑↓", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(" Navigate  "),
                Span::styled("Space", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(" Select  "),
                Span::styled("A", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(" All  "),
                Span::styled("D", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(" Deselect All  "),
                Span::styled("Enter", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(" Clean  "),
                Span::styled("Q", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(" Quit"),
            ]),
            Line::from(format!(
                "Selected: {} items, {} to be freed",
                state.selected_artifacts.iter().filter(|s| **s).count(),
                humansize::format_size(state.total_selected_size, humansize::BINARY)
            )),
        ];

        let status = Paragraph::new(status_text)
            .block(Block::default().borders(Borders::ALL))
            .alignment(Alignment::Left);

        f.render_widget(status, chunks[1]);
    }

    pub fn render_confirmation(f: &mut Frame, state: &TuiState) {
        let area = f.area();
        let dialog_width = 60;
        let dialog_height = 10;
        let x = (area.width.saturating_sub(dialog_width)) / 2;
        let y = (area.height.saturating_sub(dialog_height)) / 2;

        let dialog_area = ratatui::layout::Rect {
            x,
            y,
            width: dialog_width,
            height: dialog_height,
        };

        let inner_text = vec![
            Line::from("Are you sure you want to clean?"),
            Line::from(""),
            Line::from(format!(
                "Items: {}",
                state.selected_artifacts.iter().filter(|s| **s).count()
            )),
            Line::from(format!(
                "Size: {}",
                humansize::format_size(state.total_selected_size, humansize::BINARY)
            )),
            Line::from(""),
            Line::from(vec![
                Span::styled(
                    "Y",
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw("es  "),
                Span::styled(
                    "N",
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                ),
                Span::raw("o"),
            ]),
        ];

        let dialog = Paragraph::new(inner_text)
            .block(Block::default().title("Confirm").borders(Borders::ALL))
            .alignment(Alignment::Center);

        f.render_widget(dialog, dialog_area);
    }
}
