use crate::scanner::Artifact;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Clear, Paragraph, Row, Table, TableState},
    Frame,
};
use std::path::Path;

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
    pub table_state: TableState,
    /// Root used to shorten displayed paths.
    pub root: std::path::PathBuf,
}

impl TuiState {
    pub fn new(artifacts: Vec<Artifact>, root: std::path::PathBuf) -> Self {
        let count = artifacts.len();
        let mut table_state = TableState::default();
        table_state.select(if count == 0 { None } else { Some(0) });
        TuiState {
            selected: 0,
            artifacts,
            selected_artifacts: vec![false; count],
            input_mode: InputMode::Normal,
            total_selected_size: 0,
            table_state,
            root,
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

    pub fn selected_count(&self) -> usize {
        self.selected_artifacts.iter().filter(|s| **s).count()
    }

    /// Number of distinct projects in the current list.
    pub fn project_count(&self) -> usize {
        Self::distinct_projects(self.artifacts.iter())
    }

    /// Number of distinct projects among the selected artifacts.
    pub fn selected_project_count(&self) -> usize {
        Self::distinct_projects(
            self.artifacts
                .iter()
                .enumerate()
                .filter(|(i, _)| self.selected_artifacts.get(*i).copied().unwrap_or(false))
                .map(|(_, a)| a),
        )
    }

    fn distinct_projects<'a>(artifacts: impl Iterator<Item = &'a Artifact>) -> usize {
        let mut projects: Vec<&Path> = artifacts.map(|a| a.project.as_path()).collect();
        projects.sort();
        projects.dedup();
        projects.len()
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
            self.table_state.select(Some(self.selected));
        }
    }

    pub fn move_down(&mut self) {
        if self.selected < self.artifacts.len().saturating_sub(1) {
            self.selected += 1;
            self.table_state.select(Some(self.selected));
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
            .constraints([Constraint::Min(3), Constraint::Length(4)])
            .split(f.area());

        let header = Row::new(vec![
            Cell::from(" "),
            Cell::from("ARTIFACT").style(Style::default().add_modifier(Modifier::BOLD)),
            Cell::from("SIZE").style(Style::default().add_modifier(Modifier::BOLD)),
            Cell::from("PROJECT / PATH").style(Style::default().add_modifier(Modifier::BOLD)),
        ])
        .style(Style::default().fg(Color::Cyan))
        .height(1);

        let rows: Vec<Row> = state
            .artifacts
            .iter()
            .enumerate()
            .map(|(i, artifact)| {
                let is_selected = state.selected_artifacts.get(i).copied().unwrap_or(false);

                let project = artifact.project_relative_to(&state.root);
                // Path of the artifact's parent inside the project (usually the
                // project root itself, in which case only the project is shown).
                let inside = artifact
                    .path
                    .parent()
                    .and_then(|p| p.strip_prefix(&artifact.project).ok())
                    .filter(|p| !p.as_os_str().is_empty());

                let location = match (project.as_os_str().is_empty(), inside) {
                    (true, Some(inside)) => inside.display().to_string(),
                    (true, None) => artifact.relative_to(&state.root).display().to_string(),
                    (false, Some(inside)) => {
                        format!("{} / {}", project.display(), inside.display())
                    }
                    (false, None) => project.display().to_string(),
                };

                let style = if is_selected {
                    Style::default().fg(Color::Green)
                } else {
                    Style::default()
                };

                Row::new(vec![
                    Cell::from(if is_selected { "✓" } else { " " }),
                    Cell::from(artifact.name.clone()),
                    Cell::from(artifact.size_string()),
                    Cell::from(location),
                ])
                .style(style)
            })
            .collect();

        let total: u64 = state.artifacts.iter().map(|a| a.size).sum();
        let title = format!(
            " klean — {} artifacts in {} projects, {} total ",
            state.artifacts.len(),
            state.project_count(),
            humansize::format_size(total, humansize::BINARY)
        );

        let table = Table::new(
            rows,
            [
                Constraint::Length(1),
                Constraint::Min(16),
                Constraint::Length(11),
                Constraint::Min(20),
            ],
        )
        .header(header)
        .block(Block::default().title(title).borders(Borders::ALL))
        .highlight_style(
            Style::default()
                .bg(Color::Blue)
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶ ");

        f.render_stateful_widget(table, chunks[0], &mut state.table_state);

        let status_text = vec![
            Line::from(vec![
                Span::styled("↑↓", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(" move  "),
                Span::styled("Space", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(" toggle  "),
                Span::styled("A", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(" all  "),
                Span::styled("D", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(" none  "),
                Span::styled("Enter", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(" clean  "),
                Span::styled("Q", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(" quit"),
            ]),
            Line::from(format!(
                "Selected {} of {} items — {} to free",
                state.selected_count(),
                state.artifacts.len(),
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
        // Fit inside the terminal even when it is small.
        let dialog_width = 60.min(area.width.saturating_sub(2)).max(20);
        let dialog_height = 10.min(area.height.saturating_sub(2)).max(5);
        let dialog_area = centered_rect(area, dialog_width, dialog_height);

        let inner_text = vec![
            Line::from("Are you sure you want to clean?"),
            Line::from(""),
            Line::from(format!("Projects: {}", state.selected_project_count())),
            Line::from(format!("Items: {}", state.selected_count())),
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

        // Clear the area behind the dialog so the table does not bleed through.
        f.render_widget(Clear, dialog_area);
        f.render_widget(dialog, dialog_area);
    }
}

fn centered_rect(area: Rect, width: u16, height: u16) -> Rect {
    Rect {
        x: area.x + (area.width.saturating_sub(width)) / 2,
        y: area.y + (area.height.saturating_sub(height)) / 2,
        width,
        height,
    }
}
