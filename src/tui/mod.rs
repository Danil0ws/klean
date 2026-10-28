use crate::cleaner::{CleanEvent, CleanResult, CleanerAction};
use crate::scanner::Artifact;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Clear, Gauge, Paragraph, Row, Table, TableState},
    Frame,
};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

pub mod globals;
pub mod interactive;
pub mod menu;

pub use globals::run_globals;
pub use interactive::InteractiveMode;
pub use menu::{run_menu, MenuChoice};

/// How long a toggled row stays lit.
const FLASH: Duration = Duration::from_millis(450);
/// How long a footer toast stays up.
const TOAST: Duration = Duration::from_millis(2500);
/// How long the "confirmed" badge stays on the cleaning screen.
const CONFIRMED_BADGE: Duration = Duration::from_millis(1500);

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq)]
pub enum InputMode {
    Normal,
    Selection,
    Confirmation,
    /// The scan runs on a worker thread and the loading screen is up.
    Scanning,
    /// A clean runs on a worker thread and its live feed is on screen.
    Cleaning,
    /// Last clean result is on screen; any key returns to the list.
    Summary,
    Exiting,
}

/// Buttons of the confirmation dialog, in screen order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfirmChoice {
    /// Delete for good (or move to the backup dir when one is configured).
    Clean,
    Trash,
    Cancel,
}

impl ConfirmChoice {
    const ALL: [ConfirmChoice; 3] = [
        ConfirmChoice::Clean,
        ConfirmChoice::Trash,
        ConfirmChoice::Cancel,
    ];

    fn position(self) -> usize {
        Self::ALL.iter().position(|c| *c == self).unwrap_or(0)
    }

    pub fn next(self) -> Self {
        Self::ALL[(self.position() + 1) % Self::ALL.len()]
    }

    pub fn previous(self) -> Self {
        Self::ALL[(self.position() + Self::ALL.len() - 1) % Self::ALL.len()]
    }
}

/// Everything the TUI needs to clean without leaving the screen.
pub struct CleanContext {
    pub backup_dir: Option<PathBuf>,
    pub root: PathBuf,
    pub allow_system_paths: bool,
}

/// Numbers of one clean run, kept on screen until the user dismisses them.
#[derive(Debug, Clone)]
pub struct LastResult {
    pub action: CleanerAction,
    pub deleted: usize,
    pub backed_up: usize,
    pub failed: usize,
    pub freed: u64,
    /// Items sitting in the klean trash: `U` can bring them back.
    pub trashed: usize,
    pub errors: Vec<String>,
    pub elapsed: Duration,
    /// Items the run was asked to clean (more than were touched if cancelled).
    pub planned: usize,
    pub cancelled: bool,
    /// Distinct projects among the items removed.
    pub projects: usize,
    /// (artifact name, count, bytes) of what was removed, biggest first.
    pub breakdown: Vec<(String, usize, u64)>,
    /// Location and size of the biggest item removed.
    pub largest: Option<(String, u64)>,
}

impl LastResult {
    /// Items that really left their place (deleted, backed up or trashed).
    pub fn removed(&self) -> usize {
        self.deleted + self.backed_up + self.trashed
    }
}

/// How a finished run went, as seen from the cleaning screen.
#[derive(Debug, Clone)]
pub struct CleanRun {
    pub action: CleanerAction,
    pub elapsed: Duration,
    pub planned: usize,
    pub cancelled: bool,
    /// Artifacts that were removed successfully.
    pub cleaned: Vec<Artifact>,
}

/// Live scan progress: written by the worker thread, read once per frame.
#[derive(Debug)]
pub struct ScanFeed {
    pub started: std::time::Instant,
    /// Directory being read right now.
    pub current: Option<PathBuf>,
    pub dirs: usize,
    pub done: bool,
    /// Artifacts, or the message to show instead of a list.
    pub outcome: Option<Result<Vec<Artifact>, String>>,
}

impl Default for ScanFeed {
    fn default() -> Self {
        ScanFeed::new()
    }
}

impl ScanFeed {
    pub fn new() -> Self {
        ScanFeed {
            started: std::time::Instant::now(),
            current: None,
            dirs: 0,
            done: false,
            outcome: None,
        }
    }

    pub fn elapsed(&self) -> std::time::Duration {
        self.started.elapsed()
    }
}

/// Live clean progress: written by the worker thread, read once per frame.
pub struct CleanFeed {
    pub started: Instant,
    pub action: CleanerAction,
    pub items: Vec<Artifact>,
    pub total_bytes: u64,
    /// Index into `items` of the artifact being removed right now.
    pub current: Option<usize>,
    /// Entry inside the current artifact that is going right now.
    pub entry: Option<PathBuf>,
    pub entry_done: usize,
    pub entry_total: usize,
    /// Finished artifacts in completion order, with the error when one failed.
    pub finished: Vec<(usize, Option<String>)>,
    /// Bytes of the artifacts removed so far.
    pub freed: u64,
    /// Bytes of the artifacts finished so far, failed or not (drives the gauge).
    processed: u64,
    /// Set by Esc: the worker stops before the next artifact.
    pub cancel: Arc<AtomicBool>,
    pub done: bool,
    pub outcome: Option<Result<CleanResult, String>>,
    ended: Option<Duration>,
}

impl CleanFeed {
    pub fn new(action: CleanerAction, items: Vec<Artifact>) -> Self {
        CleanFeed {
            started: Instant::now(),
            action,
            total_bytes: items.iter().map(|a| a.size).sum(),
            items,
            current: None,
            entry: None,
            entry_done: 0,
            entry_total: 0,
            finished: Vec::new(),
            freed: 0,
            processed: 0,
            cancel: Arc::new(AtomicBool::new(false)),
            done: false,
            outcome: None,
            ended: None,
        }
    }

    pub fn record(&mut self, event: CleanEvent<'_>) {
        match event {
            CleanEvent::Started { index, .. } => {
                self.current = Some(index);
                self.entry = None;
                self.entry_done = 0;
                self.entry_total = 0;
            }
            CleanEvent::Entry { path, done, total } => {
                self.entry = Some(path.to_path_buf());
                self.entry_done = done;
                self.entry_total = total;
            }
            CleanEvent::Finished { index, error } => {
                let size = self.items.get(index).map_or(0, |a| a.size);
                self.processed += size;
                if error.is_none() {
                    self.freed += size;
                }
                self.finished.push((index, error.map(String::from)));
                self.current = None;
                self.entry = None;
            }
        }
    }

    pub fn finish(&mut self, outcome: Result<CleanResult, String>) {
        self.ended = Some(self.started.elapsed());
        self.outcome = Some(outcome);
        self.done = true;
    }

    pub fn elapsed(&self) -> Duration {
        self.ended.unwrap_or_else(|| self.started.elapsed())
    }

    pub fn cancel_requested(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }

    /// 0.0..=1.0 by bytes, counting the share of the current artifact whose
    /// entries are already gone, so a single huge `node_modules` still moves.
    pub fn ratio(&self) -> f64 {
        if self.total_bytes == 0 {
            if self.items.is_empty() {
                return 1.0;
            }
            return self.finished.len() as f64 / self.items.len() as f64;
        }
        let partial = match self.current {
            Some(index) if self.entry_total > 0 => {
                self.items.get(index).map_or(0.0, |a| a.size as f64)
                    * (self.entry_done as f64 / self.entry_total as f64)
            }
            _ => 0.0,
        };
        ((self.processed as f64 + partial) / self.total_bytes as f64).clamp(0.0, 1.0)
    }

    pub fn run(&self) -> CleanRun {
        CleanRun {
            action: self.action,
            elapsed: self.elapsed(),
            planned: self.items.len(),
            cancelled: self.cancel_requested() && self.finished.len() < self.items.len(),
            cleaned: self
                .finished
                .iter()
                .filter(|(_, error)| error.is_none())
                .filter_map(|(index, _)| self.items.get(*index).cloned())
                .collect(),
        }
    }
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
    /// Where the table was drawn last, so a click can find its row.
    pub table_area: Rect,
    /// What the last clean run freed, shown over the list.
    pub last_result: Option<LastResult>,
    /// Bytes freed and items removed since the TUI opened.
    pub session_freed: u64,
    pub session_cleaned: usize,
    /// A clean that could not even start (safety refusal, IO error).
    pub notice: Option<String>,
    /// Short-lived feedback line in the footer ("✓ marcado node_modules").
    pub toast: Option<(String, Color, Instant)>,
    /// Row that was just toggled; it lights up for a moment.
    pub flash: Option<(usize, Instant)>,
    /// Focused button of the confirmation dialog.
    pub confirm_choice: ConfirmChoice,
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
            table_area: Rect::default(),
            last_result: None,
            session_freed: 0,
            session_cleaned: 0,
            notice: None,
            toast: None,
            flash: None,
            confirm_choice: ConfirmChoice::Clean,
        }
    }

    pub fn show_toast(&mut self, text: impl Into<String>, color: Color) {
        self.toast = Some((text.into(), color, Instant::now()));
    }

    /// Something on screen is moving: the loop redraws faster while it lasts.
    pub fn animating(&self) -> bool {
        self.input_mode == InputMode::Cleaning
            || self.flash.is_some_and(|(_, at)| at.elapsed() < FLASH)
            || self
                .toast
                .as_ref()
                .is_some_and(|(_, _, at)| at.elapsed() < TOAST)
    }

    pub fn select_all(&mut self) {
        self.selected_artifacts.iter_mut().for_each(|s| *s = true);
        self.update_total_size();
        if !self.artifacts.is_empty() {
            self.show_toast(
                format!(
                    "✓ todos os {} marcados · {}",
                    self.artifacts.len(),
                    bytes(self.total_selected_size)
                ),
                Color::Green,
            );
        }
    }

    pub fn deselect_all(&mut self) {
        let had = self.selected_count();
        self.selected_artifacts.iter_mut().for_each(|s| *s = false);
        self.total_selected_size = 0;
        if had > 0 {
            self.show_toast(format!("○ {had} desmarcado(s)"), Color::Yellow);
        }
    }

    pub fn invert_selection(&mut self) {
        self.selected_artifacts.iter_mut().for_each(|s| *s = !*s);
        self.update_total_size();
        self.show_toast(
            format!(
                "⇄ seleção invertida · {} marcados · {}",
                self.selected_count(),
                bytes(self.total_selected_size)
            ),
            Color::Cyan,
        );
    }

    pub fn toggle_selected(&mut self) {
        if self.selected < self.selected_artifacts.len() {
            let marked = !self.selected_artifacts[self.selected];
            self.selected_artifacts[self.selected] = marked;
            self.update_total_size();
            self.flash = Some((self.selected, Instant::now()));

            let artifact = &self.artifacts[self.selected];
            let text = format!(
                "{} {} · {} — {}",
                if marked {
                    "✓ marcado"
                } else {
                    "○ desmarcado"
                },
                artifact.name,
                artifact.size_string(),
                location(artifact, &self.root)
            );
            self.show_toast(text, if marked { Color::Green } else { Color::Yellow });
        }
    }

    /// Mouse click on a table row: move the cursor there and toggle it.
    pub fn click(&mut self, column: u16, row: u16) {
        let area = self.table_area;
        // Border + header above the first row, border below the last one.
        let first_row = area.y + 2;
        let inside = column > area.x
            && column < area.x + area.width.saturating_sub(1)
            && row >= first_row
            && row < area.y + area.height.saturating_sub(1);
        if !inside {
            return;
        }
        let index = self.table_state.offset() + usize::from(row - first_row);
        if index < self.artifacts.len() {
            self.selected = index;
            self.sync_selection();
            self.toggle_selected();
        }
    }

    pub fn selected_count(&self) -> usize {
        self.selected_artifacts.iter().filter(|s| **s).count()
    }

    /// How many of the selected artifacts are heuristic matches (`bin`, `out`,
    /// `Library`, ...) that may hold hand-written work instead of build output.
    pub fn selected_unsafe_count(&self) -> usize {
        self.artifacts
            .iter()
            .enumerate()
            .filter(|(i, _)| self.selected_artifacts.get(*i).copied().unwrap_or(false))
            .filter(|(_, artifact)| !artifact.is_safe)
            .count()
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

    /// Drop whatever is no longer on disk and clear the marks: the run is over,
    /// and leaving everything marked makes the whole list look deleted (and
    /// Enter would clean it again).
    pub fn prune_missing(&mut self) {
        self.artifacts.retain(|artifact| artifact.path.exists());
        let count = self.artifacts.len();
        self.selected_artifacts = vec![false; count];
        self.selected = self.selected.min(count.saturating_sub(1));
        if count == 0 {
            self.table_state.select(None);
        } else {
            self.sync_selection();
        }
        self.total_selected_size = 0;
        self.flash = None;
    }

    /// Fold a finished clean run back into the screen: whatever is still on
    /// disk failed, so it stays listed for a retry, and the freed total
    /// accumulates for the rest of the session.
    pub fn apply_clean_result(&mut self, result: &CleanResult, run: CleanRun) {
        self.prune_missing();
        self.session_freed += result.total_size_freed;
        self.session_cleaned += result.deleted + result.backed_up;
        self.notice = None;
        self.toast = None;

        let mut breakdown: Vec<(String, usize, u64)> = Vec::new();
        for artifact in &run.cleaned {
            match breakdown
                .iter_mut()
                .find(|(name, _, _)| *name == artifact.name)
            {
                Some((_, count, size)) => {
                    *count += 1;
                    *size += artifact.size;
                }
                None => breakdown.push((artifact.name.clone(), 1, artifact.size)),
            }
        }
        breakdown.sort_by(|a, b| b.2.cmp(&a.2).then_with(|| a.0.cmp(&b.0)));

        let largest = run
            .cleaned
            .iter()
            .max_by_key(|a| a.size)
            .map(|a| (a.relative_to(&self.root).display().to_string(), a.size));

        self.last_result = Some(LastResult {
            action: run.action,
            deleted: result.deleted,
            backed_up: result.backed_up,
            failed: result.failed,
            freed: result.total_size_freed,
            trashed: result.trashed,
            errors: result.errors.clone(),
            elapsed: run.elapsed,
            planned: run.planned,
            cancelled: run.cancelled,
            projects: Self::distinct_projects(run.cleaned.iter()),
            breakdown,
            largest,
        });
        self.input_mode = InputMode::Summary;
    }

    /// After an undo (`U`): the restored items go back into the list and the
    /// session totals give back what they had taken.
    pub fn apply_undo(&mut self, restored: &[(PathBuf, u64)]) {
        let freed: u64 = restored.iter().map(|(_, size)| size).sum();

        for (path, size) in restored {
            let name = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("item")
                .to_string();
            self.artifacts.push(Artifact {
                path: path.clone(),
                size: *size,
                pattern_name: name.clone(),
                name,
                modified: None,
                is_safe: true,
                project: path.parent().unwrap_or(path.as_path()).to_path_buf(),
            });
        }

        self.artifacts
            .sort_by(|a, b| b.size.cmp(&a.size).then_with(|| a.path.cmp(&b.path)));
        self.selected_artifacts = vec![false; self.artifacts.len()];
        self.selected = self.selected.min(self.artifacts.len().saturating_sub(1));
        if self.artifacts.is_empty() {
            self.table_state.select(None);
        } else {
            self.sync_selection();
        }
        self.total_selected_size = 0;
        self.session_freed = self.session_freed.saturating_sub(freed);
        self.session_cleaned = self.session_cleaned.saturating_sub(restored.len());
        self.last_result = None;
        self.show_toast(
            format!("↩ {} item(ns) de volta · {}", restored.len(), bytes(freed)),
            Color::Cyan,
        );
        self.input_mode = InputMode::Normal;
    }
}

/// Six-line logo; a narrow terminal gets the plain word instead.
const LOGO: [&str; 6] = [
    r" ██╗  ██╗██╗     ███████╗ █████╗ ███╗   ██╗",
    r" ██║ ██╔╝██║     ██╔════╝██╔══██╗████╗  ██║",
    r" █████╔╝ ██║     █████╗  ███████║██╔██╗ ██║",
    r" ██╔═██╗ ██║     ██╔══╝  ██╔══██║██║╚██╗██║",
    r" ██║  ██╗███████╗███████╗██║  ██║██║ ╚████║",
    r" ╚═╝  ╚═╝╚══════╝╚══════╝╚═╝  ╚═╝╚═╝  ╚═══╝",
];

const SPINNER: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

pub struct Tui;

impl Tui {
    /// Loading screen: logo, the directory being read, counters, elapsed time.
    pub fn render_scanning(f: &mut Frame, feed: &ScanFeed, root: &Path) {
        let area = f.area();
        let elapsed = feed.elapsed();
        let frame = spinner(elapsed);

        let mut lines: Vec<Line> = Vec::new();
        if area.width >= 50 && area.height >= 14 {
            lines.extend(
                LOGO.iter()
                    .map(|line| Line::from(Span::styled(*line, Style::default().fg(Color::Cyan)))),
            );
            lines.push(Line::from(""));
        } else {
            lines.push(Line::from(Span::styled(
                "klean",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )));
            lines.push(Line::from(""));
        }

        lines.push(Line::from(vec![
            Span::styled(format!("{frame} "), Style::default().fg(Color::Cyan)),
            Span::raw("lendo pastas e arquivos"),
        ]));

        let seconds = elapsed.as_secs_f32();
        let rate = if seconds >= 0.5 {
            format!(" · {:.0} pastas/s", feed.dirs as f32 / seconds)
        } else {
            String::new()
        };
        lines.push(Line::from(Span::styled(
            format!("{} pastas{rate} · {}", feed.dirs, clock(elapsed)),
            Style::default().fg(Color::DarkGray),
        )));

        lines.push(Line::from(""));
        if let Some(current) = &feed.current {
            let shown = current.strip_prefix(root).unwrap_or(current);
            lines.push(Line::from(Span::styled(
                truncate_middle(
                    &shown.display().to_string(),
                    area.width.saturating_sub(8) as usize,
                ),
                Style::default().fg(Color::Gray),
            )));
        }

        let rect = centered_rect(
            area,
            64.min(area.width.saturating_sub(2)).max(20),
            (lines.len() as u16 + 2).min(area.height),
        );
        f.render_widget(Clear, rect);
        f.render_widget(Paragraph::new(lines).alignment(Alignment::Center), rect);
    }

    pub fn render_list(f: &mut Frame, state: &mut TuiState) {
        let unsafe_selected = state.selected_unsafe_count();
        let toast = state
            .toast
            .as_ref()
            .filter(|(_, _, at)| at.elapsed() < TOAST)
            .cloned();

        // The footer gains a line per extra message (toast, session, notice).
        let extra_lines = usize::from(toast.is_some())
            + usize::from(state.session_cleaned > 0)
            + usize::from(state.notice.is_some())
            + usize::from(unsafe_selected > 0);
        let footer_height = (4 + extra_lines) as u16;

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([Constraint::Min(3), Constraint::Length(footer_height)])
            .split(f.area());
        state.table_area = chunks[0];

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

        let bold = Style::default().add_modifier(Modifier::BOLD);
        let header = Row::new(vec![
            Cell::from(" "),
            Cell::from("ARTEFATO").style(bold),
            Cell::from(Line::from("TAMANHO").alignment(Alignment::Right)).style(bold),
            Cell::from(""),
            Cell::from("PROJETO / CAMINHO").style(bold),
        ])
        .style(Style::default().fg(Color::Cyan))
        .height(1);

        let largest = state.artifacts.iter().map(|a| a.size).max().unwrap_or(0);
        let flash = state.flash.filter(|(_, at)| at.elapsed() < FLASH);

        let rows: Vec<Row> = state
            .artifacts
            .iter()
            .enumerate()
            .map(|(i, artifact)| {
                let is_selected = state.selected_artifacts.get(i).copied().unwrap_or(false);
                let flashing = flash.is_some_and(|(row, _)| row == i);

                let style = match (flashing, is_selected) {
                    (true, true) => Style::default()
                        .bg(Color::Green)
                        .fg(Color::Black)
                        .add_modifier(Modifier::BOLD),
                    (true, false) => Style::default().bg(Color::Yellow).fg(Color::Black),
                    (false, true) => Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                    (false, false) => Style::default(),
                };
                let bar_style = if flashing {
                    style
                } else if is_selected {
                    Style::default().fg(Color::Green)
                } else {
                    Style::default().fg(Color::DarkGray)
                };
                let fraction = if largest == 0 {
                    0.0
                } else {
                    artifact.size as f64 / largest as f64
                };

                Row::new(vec![
                    Cell::from(if is_selected { "●" } else { "○" }),
                    Cell::from(artifact.name.clone()),
                    Cell::from(Line::from(artifact.size_string()).alignment(Alignment::Right)),
                    Cell::from(Span::styled(size_bar(fraction, 8), bar_style)),
                    Cell::from(location(artifact, &state.root)),
                ])
                .style(style)
            })
            .collect();

        let total: u64 = state.artifacts.iter().map(|a| a.size).sum();
        let title = Line::from(vec![
            Span::styled(
                " klean ",
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(format!(
                " {} artefatos · {} projetos · {} ",
                state.artifacts.len(),
                state.project_count(),
                bytes(total)
            )),
        ]);

        let table = Table::new(
            rows,
            [
                Constraint::Length(1),
                Constraint::Min(16),
                Constraint::Length(11),
                Constraint::Length(8),
                Constraint::Min(20),
            ],
        )
        .header(header)
        .block(
            Block::default()
                .title(title)
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray)),
        )
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
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

        let mut status_text = vec![Line::from(
            [
                ("↑↓", "mover"),
                ("Espaço", "marcar"),
                ("A", "todos"),
                ("I", "inverter"),
                ("D", "nenhum"),
                ("Enter", "limpar"),
                ("Q", "sair"),
            ]
            .into_iter()
            .flat_map(|(key, label)| key_hint(key, label))
            .collect::<Vec<_>>(),
        )];

        let count = state.selected_count();
        status_text.push(if count == 0 {
            Line::from(Span::styled(
                "○ nada marcado — Espaço marca a linha, A marca tudo, clique também marca",
                Style::default().fg(Color::DarkGray),
            ))
        } else {
            let fraction = if total == 0 {
                0.0
            } else {
                state.total_selected_size as f64 / total as f64
            };
            Line::from(vec![
                Span::styled(
                    format!(
                        "● {count} marcado(s) em {} projeto(s) · {} ",
                        state.selected_project_count(),
                        bytes(state.total_selected_size)
                    ),
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("de {} ", bytes(total)),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::styled(size_bar(fraction, 12), Style::default().fg(Color::Green)),
                Span::styled(
                    format!(" {:.0}%", fraction * 100.0),
                    Style::default().fg(Color::DarkGray),
                ),
            ])
        });

        if let Some((text, color, _)) = toast {
            status_text.push(Line::from(Span::styled(text, Style::default().fg(color))));
        }

        if state.session_cleaned > 0 {
            status_text.push(Line::from(Span::styled(
                format!(
                    "✓ {} liberados nesta sessão ({} itens)",
                    bytes(state.session_freed),
                    state.session_cleaned
                ),
                Style::default().fg(Color::Green),
            )));
        }

        if unsafe_selected > 0 {
            status_text.push(Line::from(Span::styled(
                format!(
                    "⚠ {unsafe_selected} marcado(s) podem ser código, não build (bin, out, Library...) — confira"
                ),
                Style::default().fg(Color::Yellow),
            )));
        }

        if let Some(notice) = &state.notice {
            status_text.push(Line::from(Span::styled(
                notice.clone(),
                Style::default().fg(Color::Red),
            )));
        }

        let status = Paragraph::new(status_text)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::DarkGray)),
            )
            .alignment(Alignment::Left);

        f.render_widget(status, chunks[1]);
    }

    /// The question before anything is touched: what goes, how much, and three
    /// buttons whose focus changes the headline so the choice reads clearly.
    pub fn render_confirmation(f: &mut Frame, state: &TuiState, backup: bool) {
        let area = f.area();
        let width = 74.min(area.width.saturating_sub(2)).max(24);
        let inner_width = usize::from(width.saturating_sub(4));
        let choice = state.confirm_choice;
        let count = state.selected_count();
        let projects = state.selected_project_count();
        let unsafe_selected = state.selected_unsafe_count();

        let (headline, accent) = match choice {
            ConfirmChoice::Clean if backup => (
                format!("Mover {count} item(ns) de {projects} projeto(s) para o backup?"),
                Color::Yellow,
            ),
            ConfirmChoice::Clean => (
                format!("Apagar para sempre {count} item(ns) de {projects} projeto(s)?"),
                Color::Red,
            ),
            ConfirmChoice::Trash => (
                format!("Mover {count} item(ns) para a lixeira? (U desfaz)"),
                Color::Cyan,
            ),
            ConfirmChoice::Cancel => (
                "Voltar para a lista sem tocar em nada".to_string(),
                Color::Gray,
            ),
        };

        let mut lines = vec![
            Line::from(""),
            Line::from(Span::styled(
                headline,
                Style::default().add_modifier(Modifier::BOLD),
            ))
            .alignment(Alignment::Center),
            Line::from(Span::styled(
                bytes(state.total_selected_size),
                Style::default().fg(accent).add_modifier(Modifier::BOLD),
            ))
            .alignment(Alignment::Center),
            Line::from(""),
        ];

        // Biggest first, as many as fit; the rest is folded into one line.
        let mut selected = state.get_selected_artifacts();
        selected.sort_by_key(|a| std::cmp::Reverse(a.size));
        let room = usize::from(area.height.saturating_sub(16)).clamp(1, 6);
        for artifact in selected.iter().take(room) {
            lines.push(item_line(
                "●",
                Color::Green,
                artifact,
                &state.root,
                inner_width,
                None,
            ));
        }
        if selected.len() > room {
            let rest: u64 = selected.iter().skip(room).map(|a| a.size).sum();
            lines.push(Line::from(Span::styled(
                format!("  … e mais {} · {}", selected.len() - room, bytes(rest)),
                Style::default().fg(Color::DarkGray),
            )));
        }

        if unsafe_selected > 0 {
            lines.push(Line::from(""));
            lines.push(
                Line::from(Span::styled(
                    format!("⚠ {unsafe_selected} pode(m) ser código, não build — confira antes"),
                    Style::default().fg(Color::Yellow),
                ))
                .alignment(Alignment::Center),
            );
        }

        lines.push(Line::from(""));
        let buttons: Vec<Span> = ConfirmChoice::ALL
            .iter()
            .flat_map(|button| {
                let (key, label, color) = match button {
                    ConfirmChoice::Clean if backup => ("Y", "Backup", Color::Yellow),
                    ConfirmChoice::Clean => ("Y", "Apagar", Color::Red),
                    ConfirmChoice::Trash => ("T", "Lixeira", Color::Cyan),
                    ConfirmChoice::Cancel => ("N", "Cancelar", Color::Gray),
                };
                let span = if *button == choice {
                    Span::styled(
                        format!(" ▸ {label} ({key}) "),
                        Style::default()
                            .bg(color)
                            .fg(Color::Black)
                            .add_modifier(Modifier::BOLD),
                    )
                } else {
                    Span::styled(format!("   {label} ({key}) "), Style::default().fg(color))
                };
                [span, Span::raw("  ")]
            })
            .collect();
        lines.push(Line::from(buttons).alignment(Alignment::Center));
        lines.push(
            Line::from(Span::styled(
                "←→ escolhe · Enter confirma · Esc cancela",
                Style::default().fg(Color::DarkGray),
            ))
            .alignment(Alignment::Center),
        );

        fit_lines(&mut lines, area.height, 2);
        let height = (lines.len() as u16 + 3)
            .min(area.height.saturating_sub(2))
            .max(5);
        let dialog_area = centered_rect(area, width, height);
        let dialog = Paragraph::new(lines).block(
            Block::default()
                .title(Span::styled(
                    " Confirmar limpeza ",
                    Style::default().fg(accent).add_modifier(Modifier::BOLD),
                ))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(accent)),
        );

        // Clear the area behind the dialog so the table does not bleed through.
        f.render_widget(Clear, dialog_area);
        f.render_widget(dialog, dialog_area);
    }

    /// Live view of a running clean: gauge, the artifact and entry going right
    /// now, and a log of what is already gone.
    pub fn render_cleaning(f: &mut Frame, feed: &CleanFeed, root: &Path) {
        let area = f.area();
        let width = 78.min(area.width.saturating_sub(2)).max(24);
        let height = 22.min(area.height.saturating_sub(2)).max(12);
        let dialog_area = centered_rect(area, width, height);
        let inner_width = usize::from(width.saturating_sub(4));
        let elapsed = feed.elapsed();

        let (title, accent) = match feed.action {
            CleanerAction::Delete => ("Apagando", Color::Red),
            CleanerAction::Trash => ("Movendo para a lixeira", Color::Cyan),
            CleanerAction::Backup => ("Movendo para o backup", Color::Yellow),
        };
        let block = Block::default()
            .title(Span::styled(
                format!(" {} {title} ", spinner(elapsed)),
                Style::default().fg(accent).add_modifier(Modifier::BOLD),
            ))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(accent));
        let inner = block.inner(dialog_area);
        f.render_widget(Clear, dialog_area);
        f.render_widget(block, dialog_area);

        let parts = Layout::default()
            .direction(Direction::Vertical)
            .horizontal_margin(1)
            .constraints([
                Constraint::Length(2), // badge + scope
                Constraint::Length(1), // gauge
                Constraint::Length(2), // numbers
                Constraint::Length(3), // current item
                Constraint::Min(1),    // log
                Constraint::Length(1), // hint
            ])
            .split(inner);

        let mut header = Vec::new();
        if feed.started.elapsed() < CONFIRMED_BADGE {
            header.push(Span::styled(
                " ✓ CONFIRMADO ",
                Style::default()
                    .bg(Color::Green)
                    .fg(Color::Black)
                    .add_modifier(Modifier::BOLD),
            ));
            header.push(Span::raw("  "));
        }
        header.push(Span::raw(format!(
            "{} item(ns) · {}",
            feed.items.len(),
            bytes(feed.total_bytes)
        )));
        f.render_widget(Paragraph::new(Line::from(header)), parts[0]);

        let ratio = feed.ratio();
        f.render_widget(
            Gauge::default()
                .gauge_style(Style::default().fg(accent))
                .use_unicode(true)
                .ratio(ratio)
                .label(Span::styled(
                    format!("{:.0}%", ratio * 100.0),
                    Style::default().add_modifier(Modifier::BOLD),
                )),
            parts[1],
        );

        let seconds = elapsed.as_secs_f64();
        let rate = if seconds >= 0.5 && feed.freed > 0 {
            format!(" · {}/s", bytes((feed.freed as f64 / seconds) as u64))
        } else {
            String::new()
        };
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                format!(
                    "{} de {} itens · {} de {} · {}{rate}",
                    feed.finished.len(),
                    feed.items.len(),
                    bytes(feed.freed),
                    bytes(feed.total_bytes),
                    clock(elapsed)
                ),
                Style::default().fg(Color::Gray),
            ))),
            parts[2],
        );

        let mut current = Vec::new();
        if let Some(artifact) = feed.current.and_then(|index| feed.items.get(index)) {
            current.push(item_line(
                spinner(elapsed),
                accent,
                artifact,
                root,
                inner_width,
                None,
            ));
            if let Some(entry) = &feed.entry {
                let shown = entry.strip_prefix(&artifact.path).unwrap_or(entry);
                current.push(Line::from(vec![
                    Span::styled("  └ ", Style::default().fg(Color::DarkGray)),
                    Span::styled(
                        truncate_middle(
                            &shown.display().to_string(),
                            inner_width.saturating_sub(20),
                        ),
                        Style::default().fg(Color::Gray),
                    ),
                    Span::styled(
                        format!("  {}/{}", feed.entry_done + 1, feed.entry_total),
                        Style::default().fg(Color::DarkGray),
                    ),
                ]));
            }
        } else if !feed.done {
            current.push(Line::from(Span::styled(
                format!("{} preparando…", spinner(elapsed)),
                Style::default().fg(Color::DarkGray),
            )));
        }
        f.render_widget(Paragraph::new(current), parts[3]);

        // Newest at the bottom; only what fits.
        let room = usize::from(parts[4].height);
        let log: Vec<Line> = feed
            .finished
            .iter()
            .skip(feed.finished.len().saturating_sub(room))
            .filter_map(|(index, error)| {
                let artifact = feed.items.get(*index)?;
                Some(match error {
                    None => item_line("✓", Color::Green, artifact, root, inner_width, None),
                    Some(error) => {
                        item_line("✗", Color::Red, artifact, root, inner_width, Some(error))
                    }
                })
            })
            .collect();
        f.render_widget(Paragraph::new(log), parts[4]);

        let hint = if feed.cancel_requested() {
            Span::styled(
                "⏸ interrompendo depois do item atual…",
                Style::default().fg(Color::Yellow),
            )
        } else {
            Span::styled(
                "Esc interrompe depois do item atual (nenhum item fica pela metade)",
                Style::default().fg(Color::DarkGray),
            )
        };
        f.render_widget(Paragraph::new(Line::from(hint)), parts[5]);
    }

    /// What the run just did, on screen until the user presses a key.
    pub fn render_summary(f: &mut Frame, state: &TuiState) {
        let Some(result) = &state.last_result else {
            return;
        };

        let area = f.area();
        let width = 70.min(area.width.saturating_sub(2)).max(24);
        let inner_width = usize::from(width.saturating_sub(4));
        let removed = result.removed();

        let (icon, headline, accent) = if result.cancelled {
            ("⏸", "Limpeza interrompida".to_string(), Color::Yellow)
        } else if removed == 0 && result.failed > 0 {
            ("✗", "Nada foi removido".to_string(), Color::Red)
        } else if result.failed > 0 {
            (
                "✓",
                format!("Concluída com {} falha(s)", result.failed),
                Color::Yellow,
            )
        } else {
            ("✓", "Limpeza concluída".to_string(), Color::Green)
        };

        let verb = match result.action {
            CleanerAction::Delete => "liberados",
            CleanerAction::Trash => "na lixeira (U desfaz)",
            CleanerAction::Backup => "movidos para o backup",
        };

        let mut scope = format!(
            "em {} · {} item(ns) · {} projeto(s)",
            human_duration(result.elapsed),
            removed,
            result.projects
        );
        if result.cancelled {
            scope.push_str(&format!(
                " · {} de {} feitos",
                removed + result.failed,
                result.planned
            ));
        }

        let mut lines = vec![
            Line::from(""),
            Line::from(Span::styled(
                format!("{icon} {headline}"),
                Style::default().fg(accent).add_modifier(Modifier::BOLD),
            ))
            .alignment(Alignment::Center),
            Line::from(""),
            Line::from(vec![
                Span::styled(
                    format!(" {} ", bytes(result.freed)),
                    Style::default()
                        .bg(accent)
                        .fg(Color::Black)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(format!(" {verb}"), Style::default().fg(accent)),
            ])
            .alignment(Alignment::Center),
            Line::from(Span::styled(scope, Style::default().fg(Color::Gray)))
                .alignment(Alignment::Center),
        ];

        if !result.breakdown.is_empty() {
            lines.push(Line::from(""));
            let name_width = result
                .breakdown
                .iter()
                .map(|(name, _, _)| name.chars().count())
                .max()
                .unwrap_or(0)
                .min(24);
            let biggest = result.breakdown.first().map_or(0, |(_, _, size)| *size);
            // name, ×count, size, bar: one block, centered as a whole.
            let block_width = name_width + 2 + 5 + 11 + 2 + 14;
            let pad = " ".repeat(inner_width.saturating_sub(block_width) / 2);
            for (name, count, size) in result.breakdown.iter().take(6) {
                let fraction = if biggest == 0 {
                    0.0
                } else {
                    *size as f64 / biggest as f64
                };
                lines.push(Line::from(vec![
                    Span::raw(pad.clone()),
                    Span::styled(
                        format!("{:<name_width$}  ", truncate_middle(name, name_width)),
                        Style::default().add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(format!("×{count:<4}"), Style::default().fg(Color::Gray)),
                    Span::raw(format!("{:>11}  ", bytes(*size))),
                    Span::styled(size_bar(fraction, 14), Style::default().fg(accent)),
                ]));
            }
            if result.breakdown.len() > 6 {
                lines.push(
                    Line::from(Span::styled(
                        format!("… e mais {} tipo(s)", result.breakdown.len() - 6),
                        Style::default().fg(Color::DarkGray),
                    ))
                    .alignment(Alignment::Center),
                );
            }
        }

        lines.push(Line::from(""));
        if let Some((path, size)) = &result.largest {
            lines.push(
                Line::from(vec![
                    Span::styled("maior: ", Style::default().fg(Color::DarkGray)),
                    Span::raw(truncate_middle(path, inner_width.saturating_sub(22))),
                    Span::styled(
                        format!(" · {}", bytes(*size)),
                        Style::default().fg(Color::Gray),
                    ),
                ])
                .alignment(Alignment::Center),
            );
        }

        if result.failed > 0 {
            lines.push(
                Line::from(Span::styled(
                    format!(
                        "✗ {} falha(s) — continuam na lista para tentar de novo",
                        result.failed
                    ),
                    Style::default().fg(Color::Red),
                ))
                .alignment(Alignment::Center),
            );
            for error in result.errors.iter().take(2) {
                lines.push(
                    Line::from(Span::styled(
                        truncate_middle(error, inner_width),
                        Style::default().fg(Color::Red),
                    ))
                    .alignment(Alignment::Center),
                );
            }
        }

        lines.push(
            Line::from(Span::styled(
                format!("total da sessão: {}", bytes(state.session_freed)),
                Style::default().fg(Color::Gray),
            ))
            .alignment(Alignment::Center),
        );

        lines.push(Line::from(""));
        let mut keys = key_hint("Enter", "volta para a lista");
        if result.trashed > 0 {
            keys.extend(key_hint(
                "U",
                &format!("desfaz ({} voltam)", result.trashed),
            ));
        }
        keys.extend(key_hint("Q", "sai"));
        lines.push(Line::from(keys).alignment(Alignment::Center));

        fit_lines(&mut lines, area.height, 1);
        let height = (lines.len() as u16 + 3)
            .min(area.height.saturating_sub(2))
            .max(7);
        let dialog_area = centered_rect(area, width, height);
        let dialog = Paragraph::new(lines).block(
            Block::default()
                .title(Span::styled(
                    " Resumo ",
                    Style::default().fg(accent).add_modifier(Modifier::BOLD),
                ))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(accent)),
        );

        f.render_widget(Clear, dialog_area);
        f.render_widget(dialog, dialog_area);
    }
}

/// On a short terminal, drop lines from the middle of a dialog so the last
/// `keep_tail` lines (buttons, key hints) are never the ones cut off.
fn fit_lines(lines: &mut Vec<Line>, screen_height: u16, keep_tail: usize) {
    // Two rows of margin around the dialog, three for its borders and padding.
    let max = usize::from(screen_height.saturating_sub(5));
    while lines.len() > max && lines.len() > keep_tail + 1 {
        lines.remove(lines.len() - keep_tail - 1);
    }
}

fn bytes(size: u64) -> String {
    humansize::format_size(size, humansize::BINARY)
}

fn spinner(elapsed: Duration) -> &'static str {
    SPINNER[(elapsed.as_millis() / 90) as usize % SPINNER.len()]
}

/// A key in reverse video followed by what it does.
fn key_hint(key: &str, label: &str) -> Vec<Span<'static>> {
    vec![
        Span::styled(
            format!(" {key} "),
            Style::default()
                .bg(Color::DarkGray)
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(format!(" {label}  "), Style::default().fg(Color::Gray)),
    ]
}

/// Project (and the folder inside it) an artifact lives in, relative to root.
fn location(artifact: &Artifact, root: &Path) -> String {
    let project = artifact.project_relative_to(root);
    // Path of the artifact's parent inside the project (usually the project
    // root itself, in which case only the project is shown).
    let inside = artifact
        .path
        .parent()
        .and_then(|p| p.strip_prefix(&artifact.project).ok())
        .filter(|p| !p.as_os_str().is_empty());

    match (project.as_os_str().is_empty(), inside) {
        (true, Some(inside)) => inside.display().to_string(),
        (true, None) => artifact.relative_to(root).display().to_string(),
        (false, Some(inside)) => format!("{} / {}", project.display(), inside.display()),
        (false, None) => project.display().to_string(),
    }
}

/// `mark name  size  location` on one line, with an optional error tail.
fn item_line(
    mark: &str,
    color: Color,
    artifact: &Artifact,
    root: &Path,
    width: usize,
    error: Option<&str>,
) -> Line<'static> {
    let name = truncate_middle(&artifact.name, 18);
    let used = 3 + 18 + 2 + 10 + 2;
    let rest = width.saturating_sub(used);
    let tail = match error {
        Some(error) => truncate_middle(error, rest),
        None => truncate_middle(&location(artifact, root), rest),
    };
    Line::from(vec![
        Span::styled(format!(" {mark} "), Style::default().fg(color)),
        Span::styled(
            format!("{name:<18}  "),
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        ),
        Span::raw(format!("{:>10}  ", artifact.size_string())),
        Span::styled(
            tail,
            Style::default().fg(if error.is_some() {
                Color::Red
            } else {
                Color::DarkGray
            }),
        ),
    ])
}

/// Horizontal bar with eighth-block precision, `width` cells wide.
fn size_bar(fraction: f64, width: usize) -> String {
    const PARTS: [char; 8] = [' ', '▏', '▎', '▍', '▌', '▋', '▊', '▉'];
    let eighths = (fraction.clamp(0.0, 1.0) * (width * 8) as f64).round() as usize;
    let mut bar = "█".repeat(eighths / 8);
    if !eighths.is_multiple_of(8) {
        bar.push(PARTS[eighths % 8]);
    }
    // Something non-zero never renders as an empty bar.
    if bar.is_empty() && fraction > 0.0 {
        bar.push('▏');
    }
    format!("{bar:<width$}")
}

/// `0.4s`, `12.3s`, then `mm:ss`.
fn human_duration(elapsed: Duration) -> String {
    if elapsed.as_secs() < 60 {
        format!("{:.1}s", elapsed.as_secs_f64())
    } else {
        clock(elapsed)
    }
}

/// `mm:ss`, hour-aware, for the loading screen.
fn clock(elapsed: std::time::Duration) -> String {
    let total = elapsed.as_secs();
    let (hours, minutes, seconds) = (total / 3600, (total % 3600) / 60, total % 60);
    if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes:02}:{seconds:02}")
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

/// Never larger than `area`: a dialog's minimum size must not draw outside a
/// tiny terminal.
fn centered_rect(area: Rect, width: u16, height: u16) -> Rect {
    let (width, height) = (width.min(area.width), height.min(area.height));
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

    fn run(action: CleanerAction, cleaned: Vec<Artifact>) -> CleanRun {
        CleanRun {
            action,
            elapsed: Duration::from_millis(1200),
            planned: cleaned.len(),
            cancelled: false,
            cleaned,
        }
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
    fn toggling_flashes_the_row_and_says_what_changed() {
        let mut s = state(3);
        s.selected = 1;
        s.toggle_selected();

        assert!(s.selected_artifacts[1]);
        assert_eq!(s.flash.map(|(row, _)| row), Some(1));
        let (text, color, _) = s.toast.clone().expect("a toast confirms the toggle");
        assert!(text.starts_with("✓ marcado node_modules"));
        assert_eq!(color, Color::Green);
        assert!(s.animating());

        s.toggle_selected();
        assert!(!s.selected_artifacts[1]);
        assert!(s.toast.unwrap().0.starts_with("○ desmarcado"));
    }

    #[test]
    fn inverting_flips_every_mark() {
        let mut s = state(3);
        s.selected = 0;
        s.toggle_selected();
        s.invert_selection();
        assert_eq!(s.selected_artifacts, vec![false, true, true]);
        assert_eq!(s.total_selected_size, 2);
    }

    #[test]
    fn a_click_on_a_row_moves_there_and_toggles_it() {
        let mut s = state(5);
        s.table_area = Rect::new(0, 0, 80, 10);
        // Row 4 on screen = border + header + third row.
        s.click(10, 4);
        assert_eq!(s.selected, 2);
        assert!(s.selected_artifacts[2]);

        // Header and borders are not rows.
        s.click(10, 1);
        s.click(0, 3);
        assert_eq!(s.selected_count(), 1);
    }

    #[test]
    fn confirm_focus_cycles_both_ways() {
        let choice = ConfirmChoice::Clean;
        assert_eq!(choice.next(), ConfirmChoice::Trash);
        assert_eq!(choice.next().next().next(), ConfirmChoice::Clean);
        assert_eq!(choice.previous(), ConfirmChoice::Cancel);
    }

    #[test]
    fn the_feed_tracks_progress_inside_the_current_artifact() {
        let mut items = state(2).artifacts;
        items[0].size = 100;
        items[1].size = 300;
        let mut feed = CleanFeed::new(CleanerAction::Delete, items.clone());
        assert_eq!(feed.ratio(), 0.0);

        feed.record(CleanEvent::Started {
            index: 0,
            artifact: &items[0],
        });
        feed.record(CleanEvent::Finished {
            index: 0,
            error: None,
        });
        assert_eq!(feed.ratio(), 0.25);
        assert_eq!(feed.freed, 100);

        feed.record(CleanEvent::Started {
            index: 1,
            artifact: &items[1],
        });
        feed.record(CleanEvent::Entry {
            path: Path::new("/tmp/a1/node_modules/react"),
            done: 1,
            total: 2,
        });
        assert_eq!(feed.ratio(), 0.625, "half of the second artifact is gone");

        feed.record(CleanEvent::Finished {
            index: 1,
            error: Some("boom"),
        });
        assert_eq!(feed.ratio(), 1.0);
        assert_eq!(feed.freed, 100, "a failure frees nothing");

        let run = feed.run();
        assert_eq!(run.cleaned.len(), 1);
        assert!(!run.cancelled);
    }

    #[test]
    fn a_cancel_before_the_end_marks_the_run_cancelled() {
        let items = state(2).artifacts;
        let mut feed = CleanFeed::new(CleanerAction::Trash, items);
        feed.record(CleanEvent::Finished {
            index: 0,
            error: None,
        });
        feed.cancel.store(true, Ordering::Relaxed);
        let run = feed.run();
        assert!(run.cancelled);
        assert_eq!(run.planned, 2);
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
            trashed: 0,
            failed: 1,
            total_size_freed: 4096,
            errors: vec![format!("{}: boom", kept.display())],
        };
        s.apply_clean_result(
            &result,
            run(CleanerAction::Delete, vec![artifact_at(&gone)]),
        );

        assert_eq!(s.artifacts.len(), 1);
        assert_eq!(s.artifacts[0].path, kept);
        assert!(
            !s.selected_artifacts[0],
            "a lista não fica toda marcada depois de limpar"
        );
        assert_eq!(s.selected, 0);
        assert_eq!(s.total_selected_size, 0);
        assert_eq!(s.session_freed, 4096);
        assert_eq!(s.session_cleaned, 1);
        assert_eq!(s.input_mode, InputMode::Summary);
        let last = s.last_result.as_ref().unwrap();
        assert_eq!(last.failed, 1);
        assert_eq!(last.elapsed, Duration::from_millis(1200));

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn the_summary_groups_what_was_removed_by_kind() {
        let mut s = state(0);
        let mut big = artifact_at(Path::new("/tmp/web/node_modules"));
        big.size = 300;
        let mut small = artifact_at(Path::new("/tmp/api/node_modules"));
        small.size = 100;
        let mut target = artifact_at(Path::new("/tmp/cli/target"));
        target.name = "target".to_string();
        target.size = 200;

        let result = CleanResult {
            deleted: 3,
            backed_up: 0,
            trashed: 0,
            failed: 0,
            total_size_freed: 600,
            errors: Vec::new(),
        };
        s.apply_clean_result(
            &result,
            run(CleanerAction::Delete, vec![small, target, big]),
        );

        let last = s.last_result.unwrap();
        assert_eq!(
            last.breakdown,
            vec![
                ("node_modules".to_string(), 2, 400),
                ("target".to_string(), 1, 200)
            ]
        );
        assert_eq!(last.largest, Some(("web/node_modules".to_string(), 300)));
        assert_eq!(last.projects, 3);
        assert_eq!(last.removed(), 3);
    }

    #[test]
    fn size_bars_scale_and_never_hide_a_non_zero_size() {
        assert_eq!(size_bar(1.0, 4), "████");
        assert_eq!(size_bar(0.5, 4), "██  ");
        assert_eq!(size_bar(0.0, 4), "    ");
        assert_eq!(size_bar(0.001, 4), "▏   ");
    }

    #[test]
    fn an_undo_puts_the_items_back_and_gives_the_totals_back() {
        let base = std::env::temp_dir().join("klean-tui-undo");
        let restored = base.join("app/node_modules");
        std::fs::create_dir_all(&restored).unwrap();

        let mut s = state(0);
        s.session_freed = 4096;
        s.session_cleaned = 1;

        s.apply_undo(&[(restored.clone(), 4096)]);

        assert_eq!(s.artifacts.len(), 1);
        assert_eq!(s.artifacts[0].path, restored);
        assert_eq!(s.artifacts[0].name, "node_modules");
        assert_eq!(s.artifacts[0].size, 4096);
        assert_eq!(s.session_freed, 0, "o total da sessão é devolvido");
        assert_eq!(s.session_cleaned, 0);
        assert!(!s.selected_artifacts[0], "volta desmarcado");
        assert_eq!(s.input_mode, InputMode::Normal);
        assert!(s.toast.is_some());

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
