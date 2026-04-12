use ratatui::{
    Frame,
    layout::{Constraint, Flex, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
};

use crate::app::{JobStage, JobState};

#[cfg(test)]
use crate::runner::BuildPlan;

enum TileLines<'a> {
    Borrowed(&'a [Line<'static>]),
    #[cfg(test)]
    Owned(Vec<Line<'static>>),
}

impl<'a> TileLines<'a> {
    fn as_slice(&self) -> &[Line<'static>] {
        match self {
            Self::Borrowed(lines) => lines,
            #[cfg(test)]
            Self::Owned(lines) => lines,
        }
    }
}

pub struct BuildScreen<'a> {
    tiles: Vec<BuildTile<'a>>,
    popup: Option<FinishPopup>,
}

impl<'a> BuildScreen<'a> {
    #[cfg(test)]
    pub fn from_plan(plan: &BuildPlan) -> Self {
        Self {
            tiles: plan.jobs().iter().map(BuildTile::from_job).collect(),
            popup: None,
        }
    }

    pub fn from_states(
        states: &'a [JobState],
        active_pane: usize,
        scrollbacks: &[usize],
        popup_open: bool,
    ) -> Self {
        Self {
            tiles: states
                .iter()
                .enumerate()
                .map(|(index, state)| {
                    BuildTile::from_state(
                        state,
                        index == active_pane,
                        *scrollbacks.get(index).unwrap_or(&0),
                    )
                })
                .collect(),
            popup: popup_open.then_some(FinishPopup::done()),
        }
    }

    pub fn render(&self, frame: &mut Frame<'_>) {
        TileGrid::new(self.tiles.len())
            .split(frame.area())
            .iter()
            .zip(self.tiles.iter())
            .for_each(|(area, tile)| tile.render(frame, *area));
        self.popup.iter().for_each(|popup| popup.render(frame));
    }

    pub fn viewport_height(count: usize, active_pane: usize, area: Rect) -> usize {
        TileGrid::new(count)
            .split(area)
            .get(active_pane)
            .map(|tile_area| TileLayout::new(*tile_area).split().viewport())
            .map(|viewport| viewport.height as usize)
            .unwrap_or(1)
    }

    #[cfg(test)]
    pub fn tiles(&self) -> &[BuildTile<'a>] {
        &self.tiles
    }
}

pub struct BuildTile<'a> {
    active: bool,
    status: TileStatus,
    viewport: TileViewport<'a>,
}

impl<'a> BuildTile<'a> {
    #[cfg(test)]
    pub fn from_job(job: &crate::runner::BuildJob) -> Self {
        Self {
            active: false,
            status: TileStatus::from_job(job),
            viewport: TileViewport::empty(),
        }
    }

    pub fn from_state(state: &'a JobState, active: bool, scrollback: usize) -> Self {
        Self {
            active,
            status: TileStatus::from_state(state),
            viewport: TileViewport::from_lines(state.log_lines(), scrollback),
        }
    }

    pub fn render(&self, frame: &mut Frame<'_>, area: Rect) {
        let layout = TileLayout::new(area).split();

        frame.render_widget(Clear, area);
        self.viewport.render(frame, layout.viewport(), self.active);
        self.status.render(frame, layout.status());
    }

    #[cfg(test)]
    pub fn status(&self) -> &TileStatus {
        &self.status
    }
}

pub struct TileStatus {
    title: String,
    summary: String,
    stage: JobStage,
}

impl TileStatus {
    #[cfg(test)]
    pub fn from_job(job: &crate::runner::BuildJob) -> Self {
        Self {
            title: job.target().title(),
            summary: "pending".to_string(),
            stage: JobStage::Pending,
        }
    }

    pub fn from_state(state: &JobState) -> Self {
        Self {
            title: state.title().to_string(),
            summary: state.summary().to_string(),
            stage: state.stage(),
        }
    }

    #[cfg(test)]
    pub fn from_fixture(title: &str, stage: JobStage) -> Self {
        Self {
            title: title.to_string(),
            summary: stage.label().to_string(),
            stage,
        }
    }

    pub fn render(&self, frame: &mut Frame<'_>, area: Rect) {
        frame.render_widget(Clear, area);
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(format!(" {} ", self.title), self.style()),
                Span::styled(format!(" {} ", self.summary), self.style()),
            ]))
            .style(self.style())
            .wrap(Wrap { trim: false }),
            area,
        );
    }

    #[cfg(test)]
    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn style(&self) -> Style {
        match self.stage {
            JobStage::Pending | JobStage::Building | JobStage::Mirroring => {
                Style::default().bg(Color::Black).fg(Color::Yellow)
            }
            JobStage::Success => Style::default()
                .bg(Color::Green)
                .fg(Color::White)
                .add_modifier(ratatui::style::Modifier::BOLD),
            JobStage::Failed => Style::default()
                .bg(Color::Red)
                .fg(Color::White)
                .add_modifier(ratatui::style::Modifier::BOLD),
        }
    }
}

pub struct TileViewport<'a> {
    lines: TileLines<'a>,
    scrollback: usize,
}

#[cfg(test)]
impl TileViewport<'static> {
    pub fn empty() -> Self {
        Self {
            lines: TileLines::Owned(Vec::new()),
            scrollback: 0,
        }
    }

    pub fn from_ansi(source: &str, scrollback: usize) -> Self {
        Self::from_owned_lines(crate::ansi::AnsiDocument::parse(source).lines(), scrollback)
    }

    fn from_owned_lines(lines: Vec<Line<'static>>, scrollback: usize) -> Self {
        Self {
            lines: TileLines::Owned(lines),
            scrollback,
        }
    }
}

impl<'a> TileViewport<'a> {
    pub fn from_lines(lines: &'a [Line<'static>], scrollback: usize) -> Self {
        Self {
            lines: TileLines::Borrowed(lines),
            scrollback,
        }
    }

    pub fn render(&self, frame: &mut Frame<'_>, area: Rect, active: bool) {
        let block = Block::default()
            .borders(Borders::TOP | Borders::LEFT | Borders::RIGHT)
            .border_style(self.border_style(active));
        let inner = block.inner(area);
        let visible_lines = self.visible_lines(inner);

        frame.render_widget(block, area);
        frame.render_widget(
            Paragraph::new(visible_lines).wrap(Wrap { trim: false }),
            inner,
        );
    }

    #[cfg(test)]
    pub fn scroll_y(&self, line_count: usize, area: Rect) -> u16 {
        line_count
            .saturating_sub(self.visible_tail_start(area))
            .try_into()
            .unwrap_or(u16::MAX)
    }

    #[cfg(test)]
    fn visible_tail_start(&self, area: Rect) -> usize {
        self.inner_height(area).saturating_add(self.scrollback)
    }

    fn visible_lines(&self, area: Rect) -> Vec<Line<'static>> {
        let lines = self.lines.as_slice();
        let end = lines.len().saturating_sub(self.scrollback);
        let start = end.saturating_sub(self.inner_height(area));

        lines[start..end].to_vec()
    }

    fn inner_height(&self, area: Rect) -> usize {
        area.height as usize
    }

    fn border_style(&self, active: bool) -> Style {
        active
            .then_some(Style::default().fg(Color::LightGreen))
            .unwrap_or_default()
    }
}

pub struct TileLayout {
    area: Rect,
}

impl TileLayout {
    pub fn new(area: Rect) -> Self {
        Self { area }
    }

    pub fn split(&self) -> SplitTileLayout {
        Layout::vertical([Constraint::Min(1), Constraint::Length(1)])
            .split(self.area)
            .to_vec()
            .pipe_ref(|chunks| SplitTileLayout::new(chunks[0], chunks[1]))
    }
}

pub struct SplitTileLayout {
    viewport: Rect,
    status: Rect,
}

impl SplitTileLayout {
    pub fn new(viewport: Rect, status: Rect) -> Self {
        Self { viewport, status }
    }

    pub fn viewport(&self) -> Rect {
        self.viewport
    }

    pub fn status(&self) -> Rect {
        self.status
    }
}

pub struct TileGrid {
    count: usize,
}

impl TileGrid {
    pub fn new(count: usize) -> Self {
        Self { count }
    }

    pub fn split(&self, area: Rect) -> Vec<Rect> {
        GridShape::from_count(self.count)
            .rows(area)
            .iter()
            .flat_map(|row| GridShape::from_count(self.count).cols(*row))
            .take(self.count)
            .collect()
    }
}

pub struct GridShape {
    rows: usize,
    cols: usize,
}

impl GridShape {
    pub fn from_count(count: usize) -> Self {
        ((count.max(1) as f64).sqrt().ceil() as usize).pipe_ref(|cols| Self {
            cols: *cols,
            rows: count.max(1).div_ceil(*cols),
        })
    }

    pub fn rows(&self, area: Rect) -> Vec<Rect> {
        Layout::vertical(
            (0..self.rows)
                .map(|_| Constraint::Ratio(1, self.rows as u32))
                .collect::<Vec<_>>(),
        )
        .split(area)
        .to_vec()
    }

    pub fn cols(&self, area: Rect) -> Vec<Rect> {
        Layout::horizontal(
            (0..self.cols)
                .map(|_| Constraint::Ratio(1, self.cols as u32))
                .collect::<Vec<_>>(),
        )
        .split(area)
        .to_vec()
    }
}

pub struct FinishPopup {
    text: &'static str,
}

impl FinishPopup {
    pub fn done() -> Self {
        Self {
            text: "Press ^C to quit, \"p\" to quit and preserve logs, any key to continue",
        }
    }

    pub fn render(&self, frame: &mut Frame<'_>) {
        let area = PopupLayout::new(frame.area(), self.width()).area();
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::White))
            .style(Style::default().bg(Color::Cyan));
        let inner = block.inner(area);

        frame.render_widget(Clear, area);
        frame.render_widget(block, area);
        frame.render_widget(
            Paragraph::new(Line::styled(
                self.text,
                Style::default().bg(Color::Cyan).fg(Color::White),
            ))
            .wrap(Wrap { trim: false }),
            inner,
        );
    }

    fn width(&self) -> u16 {
        self.text
            .chars()
            .count()
            .saturating_add(4)
            .try_into()
            .unwrap_or(u16::MAX)
    }
}

struct PopupLayout {
    area: Rect,
    width: u16,
}

impl PopupLayout {
    fn new(area: Rect, width: u16) -> Self {
        Self { area, width }
    }

    fn area(&self) -> Rect {
        Layout::vertical([Constraint::Length(3)])
            .flex(Flex::Center)
            .split(self.area)
            .to_vec()
            .pipe_ref(|rows| {
                Layout::horizontal([Constraint::Length(self.width(rows[0]))])
                    .flex(Flex::Center)
                    .split(rows[0])
                    .to_vec()[0]
            })
    }

    fn width(&self, row: Rect) -> u16 {
        row.width.min(self.width)
    }
}

trait PipeRef: Sized {
    fn pipe_ref<T>(self, f: impl FnOnce(&Self) -> T) -> T {
        f(&self)
    }
}

impl<T> PipeRef for T {}
