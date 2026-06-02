use ratatui::{
    Frame,
    layout::{Constraint, Flex, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph, Wrap},
};

use crate::app::{JobStage, JobState, PopupState};

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
        popup: Option<PopupState>,
        wrap_lines: bool,
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
                        wrap_lines,
                    )
                })
                .collect(),
            popup: popup.map(FinishPopup::from_state),
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

    pub fn from_state(state: &'a JobState, active: bool, scrollback: usize, wrap_lines: bool) -> Self {
        Self {
            active,
            status: TileStatus::from_state(state),
            viewport: TileViewport::from_lines(state.log_lines(), scrollback, wrap_lines),
        }
    }

    pub fn render(&self, frame: &mut Frame<'_>, area: Rect) {
        let layout = TileLayout::new(area).split();

        frame.render_widget(Clear, area);
        self.viewport.render(frame, layout.viewport(), self.active, self.status.stage());
        self.status.render(frame, layout.status(), self.active);
    }

    #[cfg(test)]
    pub fn status(&self) -> &TileStatus {
        &self.status
    }
}

pub struct TileStatus {
    os: String,
    platform: String,
    hostname: String,
    summary: String,
    load_info: String,
    stage: JobStage,
}

fn run_m() -> Color { Color::Rgb(175, 0, 175) }
fn run_a() -> Color { Color::Rgb(175, 0, 255) }
fn run_b() -> Color { Color::Rgb(175, 95, 255) }
fn ok_m() -> Color { Color::Rgb(95, 175, 0) }
fn ok_a() -> Color { Color::Rgb(95, 215, 0) }
fn ok_b() -> Color { Color::Rgb(95, 255, 0) }
fn err_m() -> Color { Color::Rgb(255, 0, 0) }
fn err_a() -> Color { Color::Rgb(175, 0, 0) }
fn err_b() -> Color { Color::Rgb(215, 95, 0) }

fn m_color(stage: JobStage) -> Color {
    match stage {
        JobStage::Failed => err_m(),
        JobStage::Success => ok_m(),
        _ => run_m(),
    }
}

fn a_color(stage: JobStage) -> Color {
    match stage {
        JobStage::Failed => err_a(),
        JobStage::Success => ok_a(),
        _ => run_a(),
    }
}

fn b_color(stage: JobStage) -> Color {
    match stage {
        JobStage::Failed => err_b(),
        JobStage::Success => ok_b(),
        _ => run_b(),
    }
}

impl TileStatus {
    #[cfg(test)]
    pub fn from_job(job: &crate::runner::BuildJob) -> Self {
        let target = job.target();
        Self {
            os: target.os().to_string(),
            platform: target.arch().to_string(),
            hostname: target.host().to_string(),
            summary: "pending".to_string(),
            load_info: String::new(),
            stage: JobStage::Pending,
        }
    }

    pub fn from_state(state: &JobState) -> Self {
        let mut parts = state.title().splitn(3, ' ');
        let os = parts.next().unwrap_or("").to_string();
        let platform = parts.next().unwrap_or("").to_string();
        let destination = parts.next().unwrap_or("");
        let hostname = destination.split(':').next().unwrap_or(destination).to_string();
        Self {
            os,
            platform,
            hostname,
            summary: state.summary().to_string(),
            load_info: state.load_info().to_string(),
            stage: state.stage(),
        }
    }

    #[cfg(test)]
    pub fn from_fixture(os: &str, platform: &str, hostname: &str, stage: JobStage) -> Self {
        Self {
            os: os.to_string(),
            platform: platform.to_string(),
            hostname: hostname.to_string(),
            summary: stage.label().to_string(),
            load_info: String::new(),
            stage,
        }
    }

    fn border_color(&self, active: bool) -> Color {
        if matches!(self.stage, JobStage::Failed) {
            err_m()
        } else if matches!(self.stage, JobStage::Pending | JobStage::Building | JobStage::Mirroring) {
            run_m()
        } else if active {
            ok_m()
        } else {
            Color::Reset
        }
    }

    pub fn render(&self, frame: &mut Frame<'_>, area: Rect, active: bool) {
        frame.render_widget(Clear, area);

        let g = m_color(self.stage);
        let a = a_color(self.stage);
        let b = b_color(self.stage);
        let border = self.border_color(active);
        let border_style = Style::default().fg(border);
        let black = Style::default().fg(Color::Black).add_modifier(ratatui::style::Modifier::BOLD);
        let white = Style::default().fg(Color::White).add_modifier(ratatui::style::Modifier::BOLD);

        let os_text = format!(" {} ", self.os);
        let platform_text = format!(" {} ", self.platform);
        let host_text = format!(" {} ", self.hostname);
        let stage_text = format!(" {} ", self.summary);

        let load_width = if self.load_info.is_empty() {
            0
        } else {
            3 + self.load_info.len() + 2
        };

        let fixed = 9
            + os_text.len()
            + platform_text.len()
            + host_text.len()
            + stage_text.len()
            + load_width;
        let pad = (area.width as usize).saturating_sub(fixed);

        let mut status_spans: Vec<Span> = vec![
            Span::styled("╰", border_style),
            Span::styled("─", border_style),
            Span::styled("\u{E0B2}", Style::default().fg(g)),
            Span::styled(os_text, black.bg(g)),
            Span::styled("\u{E0B2}", Style::default().fg(a).bg(g)),
            Span::styled(platform_text, black.bg(a)),
            Span::styled("\u{E0B2}", Style::default().fg(b).bg(a)),
            Span::styled(host_text, black.bg(b)),
            Span::styled("\u{E0B2}", Style::default().fg(g).bg(b)),
            Span::styled(stage_text, black.bg(g)),
        ];

        if !self.load_info.is_empty() {
            let dot_style = Style::default().fg(b).bg(g).add_modifier(ratatui::style::Modifier::BOLD);
            status_spans.push(Span::styled(" \u{B7} ", dot_style));
            status_spans.push(Span::styled(format!(" {} ", self.load_info), white.bg(g)));
        }

        status_spans.push(Span::styled(" ".repeat(pad), Style::default().bg(g)));
        status_spans.push(Span::styled("\u{E0B0}", border_style));
        status_spans.push(Span::styled("─", border_style));
        status_spans.push(Span::styled("╯", border_style));

        frame.render_widget(
            Paragraph::new(Line::from(status_spans)),
            area,
        );
    }

    #[cfg(test)]
    pub fn title(&self) -> String {
        format!("{} {} {}", self.os, self.platform, self.hostname)
    }

    pub fn stage(&self) -> JobStage {
        self.stage
    }
}

pub struct TileViewport<'a> {
    lines: TileLines<'a>,
    scrollback: usize,
    wrap_lines: bool,
}

#[cfg(test)]
impl TileViewport<'static> {
    pub fn empty() -> Self {
        Self {
            lines: TileLines::Owned(Vec::new()),
            scrollback: 0,
            wrap_lines: false,
        }
    }

    pub fn from_ansi(source: &str, scrollback: usize) -> Self {
        Self::from_owned_lines(crate::ansi::AnsiDocument::parse(source).lines(), scrollback, false)
    }

    fn from_owned_lines(lines: Vec<Line<'static>>, scrollback: usize, wrap_lines: bool) -> Self {
        Self {
            lines: TileLines::Owned(lines),
            scrollback,
            wrap_lines,
        }
    }
}

impl<'a> TileViewport<'a> {
    pub fn from_lines(lines: &'a [Line<'static>], scrollback: usize, wrap_lines: bool) -> Self {
        Self {
            lines: TileLines::Borrowed(lines),
            scrollback,
            wrap_lines,
        }
    }

    pub fn render(&self, frame: &mut Frame<'_>, area: Rect, active: bool, stage: JobStage) {
        let block = Block::default()
            .borders(Borders::TOP | Borders::LEFT | Borders::RIGHT)
            .border_type(BorderType::Rounded)
            .border_style(self.border_style(active, stage));
        let inner = block.inner(area);
        let visible_lines = self.visible_lines(inner);

        frame.render_widget(block, area);
        if self.wrap_lines {
            frame.render_widget(
                Paragraph::new(visible_lines).wrap(Wrap { trim: false }),
                inner,
            );
        } else {
            frame.render_widget(Paragraph::new(visible_lines), inner);
        }
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
        let width = area.width as usize;

        lines[start..end]
            .iter()
            .map(|line| {
                if self.wrap_lines {
                    line.clone()
                } else {
                    Self::truncate_line(line, width)
                }
            })
            .collect()
    }

    fn inner_height(&self, area: Rect) -> usize {
        area.height as usize
    }

    fn truncate_line(line: &Line<'static>, width: usize) -> Line<'static> {
        if width == 0 {
            return Line::default();
        }

        let visible_width = Self::line_width(line);
        if visible_width <= width {
            return line.clone();
        }

        if width <= 3 {
            return Line::from(vec![Span::raw(".".repeat(width))]);
        }

        let keep = width - 3;
        let mut visible = 0usize;
        let mut spans = Vec::new();
        let mut ellipsis_style = Style::default();

        for span in &line.spans {
            let span_width = span.content.chars().count();

            if visible + span_width <= keep {
                if span_width > 0 {
                    ellipsis_style = span.style;
                }
                visible += span_width;
                spans.push(span.clone());
                continue;
            }

            let remaining = keep.saturating_sub(visible);
            if remaining > 0 {
                let text = span.content.chars().take(remaining).collect::<String>();
                ellipsis_style = span.style;
                spans.push(Span::styled(text, span.style));
            }
            break;
        }

        spans.push(Span::styled("...", ellipsis_style));
        Line::from(spans)
    }

    fn line_width(line: &Line<'static>) -> usize {
        line.spans
            .iter()
            .map(|span| span.content.chars().count())
            .sum()
    }

    fn border_style(&self, active: bool, stage: JobStage) -> Style {
        if matches!(stage, JobStage::Failed) {
            return Style::default().fg(err_m());
        }
        if matches!(stage, JobStage::Pending | JobStage::Building | JobStage::Mirroring) {
            return Style::default().fg(run_m());
        }
        if active {
            Style::default().fg(ok_m())
        } else {
            Style::default()
        }
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
    pub fn from_state(state: PopupState) -> Self {
        match state {
            PopupState::Finished => Self {
                text: "Press ^C to quit, \"p\" to quit and preserve logs, any key to continue",
            },
            PopupState::AbortConfirm => Self {
                text: "^C again or \"y\" to abort the running farm, any key to continue",
            },
        }
    }

    pub fn render(&self, frame: &mut Frame<'_>) {
        let area = PopupLayout::new(frame.area(), self.width()).area();
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::White))
            .style(self.block_style());
        let inner = block.inner(area);

        frame.render_widget(Clear, area);
        frame.render_widget(block, area);
        frame.render_widget(
            Paragraph::new(Line::styled(
                self.text,
                self.text_style(),
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

    fn block_style(&self) -> Style {
        if self.is_abort() {
            Style::default().bg(Color::Red)
        } else {
            Style::default().bg(Color::Cyan)
        }
    }

    fn text_style(&self) -> Style {
        if self.is_abort() {
            Style::default().bg(Color::Red).fg(Color::White)
        } else {
            Style::default().bg(Color::Cyan).fg(Color::White)
        }
    }

    fn is_abort(&self) -> bool {
        self.text.contains("abort the running farm")
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
