use crate::cleaner::{CleanResult, CleanerAction};
use crate::scanner::Artifact;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Clear, Paragraph, Row, Table, TableState},
    Frame,
};
use std::path::{Path, PathBuf};

pub mod interactive;

pub use interactive::InteractiveMode;

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq)]
pub enum InputMode {
    Normal,
    Selection,
    Confirmation,
    /// Last clean result is on screen; any key returns to the list.
    Summary,
    Exiting,
}

/// Everything the TUI needs to clean without leaving the screen.
pub struct CleanContext {
    pub backup_dir: Option<PathBuf>,
    pub root: PathBuf,
    pub allow_system_paths: bool,
}

impl CleanContext {
    /// Backup when a backup directory is configured, delete otherwise — the
    /// same rule the CLI path uses.
    pub fn action(&self) -> CleanerAction {
        if self.backup_dir.is_some() {
            CleanerAction::Backup
        } else {
            CleanerAction::Delete
        }
    }
}

/// Numbers of one clean run, kept on screen until the user dismisses them.
#[derive(Debug, Clone)]
pub struct LastResult {
    pub deleted: usize,
    pub backed_up: usize,
    pub failed: usize,
    pub freed: u64,
    pub errors: Vec<String>,
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
    /// Rows that fit in the table; refreshed on every render so paging moves
    /// exactly one screenful.
    pub page_size: usize,
    /// What the last clean run freed, shown over the list.
    pub last_result: Option<LastResult>,
    /// Bytes freed and items removed since the TUI opened.
    pub session_freed: u64,
    pub session_cleaned: usize,
    /// A clean that could not even start (safety refusal, IO error).
    pub notice: Option<String>,
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
            page_size: 10,
            last_result: None,
            session_freed: 0,
            session_cleaned: 0,
            notice: None,
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

    fn sync_selection(&mut self) {
        self.table_state.select(Some(self.selected));
    }

    pub fn move_up(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
            self.sync_selection();
        }
    }

    pub fn move_down(&mut self) {
        if self.selected < self.artifacts.len().saturating_sub(1) {
            self.selected += 1;
            self.sync_selection();
        }
    }

    /// One screenful up/down: PgUp/PgDn and the mouse wheel.
    pub fn move_page(&mut self, down: bool) {
        let last = self.artifacts.len().saturating_sub(1);
        let step = self.page_size.max(1);
        self.selected = if down {
            (self.selected + step).min(last)
        } else {
            self.selected.saturating_sub(step)
        };
        self.sync_selection();
    }

    pub fn move_to_first(&mut self) {
        self.selected = 0;
        self.sync_selection();
    }

    pub fn move_to_last(&mut self) {
        self.selected = self.artifacts.len().saturating_sub(1);
        self.sync_selection();
    }

    pub fn get_selected_artifacts(&self) -> Vec<Artifact> {
        self.artifacts
            .iter()
            .enumerate()
            .filter(|(i, _)| self.selected_artifacts.get(*i).copied().unwrap_or(false))
            .map(|(_, a)| a.clone())
            .collect()
    }

    /// Fold a finished clean run back into the screen: whatever is still on
    /// disk failed, so it stays listed (and selected) for a retry, and the
    /// freed total accumulates for the rest of the session.
    pub fn apply_clean_result(&mut self, result: &CleanResult) {
        self.artifacts.retain(|artifact| artifact.path.exists());
        let count = self.artifacts.len();
        self.selected_artifacts = vec![true; count];
        self.selected = self.selected.min(count.saturating_sub(1));
        if count == 0 {
            self.table_state.select(None);
        } else {
            self.sync_selection();
        }
        self.total_selected_size = self.artifacts.iter().map(|a| a.size).sum();
        self.session_freed += result.total_size_freed;
        self.session_cleaned += result.deleted + result.backed_up;
        self.notice = None;
        self.last_result = Some(LastResult {
            deleted: result.deleted,
            backed_up: result.backed_up,
            failed: result.failed,
            freed: result.total_size_freed,
            errors: result.errors.clone(),
        });
        self.input_mode = InputMode::Summary;
    }
}

pub struct Tui;

impl Tui {
    pub fn render_list(f: &mut Frame, state: &mut TuiState) {
        // The footer gains a line per extra message (session total, notice).
        let extra_lines =
            usize::from(state.session_cleaned > 0) + usize::from(state.notice.is_some());
        let footer_height = (4 + extra_lines) as u16;

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([Constraint::Min(3), Constraint::Length(footer_height)])
            .split(f.area());

        // Borders (2) + header (1) are not rows: keep the offset inside the rows
        // that are really visible, so paging lands on whole screens and the
        // highlight never hides behind the footer.
        let visible = usize::from(chunks[0].height.saturating_sub(3)).max(1);
        state.page_size = visible;
        if state.artifacts.len() > visible {
            let offset = state.table_state.offset_mut();
            if state.selected < *offset {
                *offset = state.selected;
            } else if state.selected >= *offset + visible {
                *offset = state.selected + 1 - visible;
            }
        }

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

        if state.artifacts.is_empty() {
            let inner = Block::default().borders(Borders::ALL).inner(chunks[0]);
            f.render_widget(Clear, inner);
            f.render_widget(
                Paragraph::new("✨ nada mais para limpar — Q para sair")
                    .alignment(Alignment::Center)
                    .style(Style::default().fg(Color::Green)),
                inner,
            );
        }

        let mut status_text = vec![
            Line::from(vec![
                Span::styled("↑↓ / wheel", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(" move  "),
                Span::styled("PgUp PgDn", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(" page  "),
                Span::styled("Home End", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(" ends  "),
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

        if state.session_cleaned > 0 {
            status_text.push(Line::from(Span::styled(
                format!(
                    "✓ {} libertados nesta sessão ({} itens)",
                    humansize::format_size(state.session_freed, humansize::BINARY),
                    state.session_cleaned
                ),
                Style::default().fg(Color::Green),
            )));
        }

        if let Some(notice) = &state.notice {
            status_text.push(Line::from(Span::styled(
                notice.clone(),
                Style::default().fg(Color::Red),
            )));
        }

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

    /// What the run just freed, on screen until the user presses a key.
    pub fn render_summary(f: &mut Frame, state: &TuiState) {
        let Some(result) = &state.last_result else {
            return;
        };

        let area = f.area();
        let width = 64.min(area.width.saturating_sub(2)).max(24);
        let height = 13.min(area.height.saturating_sub(2)).max(7);
        let dialog_area = centered_rect(area, width, height);

        let mut lines = vec![
            Line::from(Span::styled(
                "limpeza concluída",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(format!("Apagados        {}", result.deleted)),
            Line::from(format!("Backups         {}", result.backed_up)),
            Line::from(format!("Falhas          {}", result.failed)),
            Line::from(format!(
                "Espaço liberado {}",
                humansize::format_size(result.freed, humansize::BINARY)
            )),
            Line::from(Span::styled(
                format!(
                    "Total da sessão {}",
                    humansize::format_size(state.session_freed, humansize::BINARY)
                ),
                Style::default().add_modifier(Modifier::BOLD),
            )),
        ];

        if let Some(error) = result.errors.first() {
            lines.push(Line::from(Span::styled(
                truncate_middle(error, width.saturating_sub(4) as usize),
                Style::default().fg(Color::Red),
            )));
        }

        lines.push(Line::from(""));
        lines.push(Line::from("qualquer tecla volta para a lista — Q sai"));

        let dialog = Paragraph::new(lines)
            .block(
                Block::default()
                    .title("RESUMO")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Green)),
            )
            .alignment(Alignment::Center);

        f.render_widget(Clear, dialog_area);
        f.render_widget(dialog, dialog_area);
    }
}

/// Keep both ends of a long path so the error stays identifiable.
fn truncate_middle(text: &str, max: usize) -> String {
    if max < 8 || text.chars().count() <= max {
        return text.to_string();
    }
    let keep = (max - 1) / 2;
    let head: String = text.chars().take(keep).collect();
    let tail: String = text.chars().skip(text.chars().count() - keep).collect();
    format!("{head}…{tail}")
}

fn centered_rect(area: Rect, width: u16, height: u16) -> Rect {
    Rect {
        x: area.x + (area.width.saturating_sub(width)) / 2,
        y: area.y + (area.height.saturating_sub(height)) / 2,
        width,
        height,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(count: usize) -> TuiState {
        let artifacts = (0..count)
            .map(|i| Artifact {
                path: std::path::PathBuf::from(format!("/tmp/a{i}/node_modules")),
                size: 1,
                name: "node_modules".to_string(),
                pattern_name: "node_modules".to_string(),
                modified: None,
                is_safe: true,
                project: std::path::PathBuf::from(format!("/tmp/a{i}")),
            })
            .collect();
        TuiState::new(artifacts, std::path::PathBuf::from("/tmp"))
    }

    #[test]
    fn paging_moves_one_screenful_and_clamps() {
        let mut s = state(30);
        s.page_size = 10;

        s.move_page(true);
        assert_eq!(s.selected, 10);
        assert_eq!(s.table_state.selected(), Some(10));

        s.move_page(true);
        assert_eq!(s.selected, 20);

        // clamping at the last row
        s.move_page(true);
        assert_eq!(s.selected, 29);
        s.move_page(true);
        assert_eq!(s.selected, 29);

        s.move_page(false);
        assert_eq!(s.selected, 19);
        s.move_page(false);
        s.move_page(false);
        assert_eq!(s.selected, 0);
        s.move_page(false);
        assert_eq!(s.selected, 0);
    }

    #[test]
    fn home_and_end_jump_to_the_bounds() {
        let mut s = state(30);
        s.move_to_last();
        assert_eq!(s.selected, 29);
        s.move_to_first();
        assert_eq!(s.selected, 0);
        assert_eq!(s.table_state.selected(), Some(0));
    }

    #[test]
    fn movement_on_an_empty_list_is_a_no_op() {
        let mut s = state(0);
        s.move_down();
        s.move_page(true);
        s.move_to_last();
        assert_eq!(s.selected, 0);
    }

    #[test]
    fn a_clean_keeps_only_the_failures_and_accumulates_the_session() {
        let base = std::env::temp_dir().join("klean-tui-apply");
        let _ = std::fs::remove_dir_all(&base);
        let gone = base.join("gone/node_modules");
        let kept = base.join("kept/node_modules");
        std::fs::create_dir_all(&kept).unwrap();

        let mut s = TuiState::new(vec![artifact_at(&gone), artifact_at(&kept)], base.clone());
        s.select_all();

        let result = CleanResult {
            deleted: 1,
            backed_up: 0,
            failed: 1,
            total_size_freed: 4096,
            errors: vec![format!("{}: boom", kept.display())],
        };
        s.apply_clean_result(&result);

        assert_eq!(s.artifacts.len(), 1);
        assert_eq!(s.artifacts[0].path, kept);
        assert!(s.selected_artifacts[0], "a falha fica marcada para retry");
        assert_eq!(s.selected, 0);
        assert_eq!(s.total_selected_size, 1);
        assert_eq!(s.session_freed, 4096);
        assert_eq!(s.session_cleaned, 1);
        assert_eq!(s.input_mode, InputMode::Summary);
        assert_eq!(s.last_result.as_ref().map(|r| r.failed), Some(1));

        let _ = std::fs::remove_dir_all(&base);
    }

    fn artifact_at(path: &Path) -> Artifact {
        Artifact {
            path: path.to_path_buf(),
            size: 1,
            name: "node_modules".to_string(),
            pattern_name: "node_modules".to_string(),
            modified: None,
            is_safe: true,
            project: path.parent().unwrap_or(path).to_path_buf(),
        }
    }
}
