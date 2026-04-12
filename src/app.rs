use std::{
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    sync::mpsc::{self, Receiver, Sender},
    thread,
    time::Duration,
};

use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend, layout::Rect};

use crate::{
    ansi::TerminalBuffer,
    runner::{BuildJob, BuildPlan},
    ui::BuildScreen,
};

pub(crate) const LOG_READ_MAX: usize = 64 * 1024;
pub(crate) const LOG_TICK_MAX: usize = 512 * 1024;
const IDLE_MS: u64 = 16;

pub struct XrunApp {
    plan: BuildPlan,
    states: Vec<JobState>,
}

impl XrunApp {
    pub fn new(plan: BuildPlan) -> Self {
        Self {
            states: plan.jobs().iter().map(JobState::from_job).collect(),
            plan,
        }
    }

    pub fn run(&mut self) -> Result<i32, String> {
        TerminalGuard::enter().and_then(|mut terminal| {
            JobSupervisor::new(&self.plan).spawn().and_then(|events| {
                InputReader::spawn().and_then(|keys| {
                    AppLoop::new(&mut self.states, events, keys, terminal.terminal_mut()).run()
                })
            })
        })
    }
}

struct AppLoop<'a> {
    states: &'a mut [JobState],
    events: Receiver<JobEvent>,
    keys: Receiver<KeyPress>,
    terminal: &'a mut Terminal<CrosstermBackend<std::io::Stdout>>,
    active_pane: usize,
    scrollbacks: Vec<usize>,
    popup: Option<PopupState>,
    popup_dismissed: bool,
}

impl<'a> AppLoop<'a> {
    fn new(
        states: &'a mut [JobState],
        events: Receiver<JobEvent>,
        keys: Receiver<KeyPress>,
        terminal: &'a mut Terminal<CrosstermBackend<std::io::Stdout>>,
    ) -> Self {
        let pane_count = states.len();

        Self {
            states,
            events,
            keys,
            terminal,
            active_pane: 0,
            scrollbacks: vec![0; pane_count],
            popup: None,
            popup_dismissed: false,
        }
    }

    fn run(&mut self) -> Result<i32, String> {
        loop {
            let busy = self.drain_events();
            let busy = self.refresh_logs() || busy;
            self.refresh_popup();
            self.render()?;

            if let Some(key) = self.key_pressed()
                && self.handle_key(key)
            {
                return Ok(self.exit_code_or_abort(key));
            }
            if !busy {
                thread::sleep(Duration::from_millis(IDLE_MS));
            }
        }
    }

    fn drain_events(&mut self) -> bool {
        let mut busy = false;

        while let Ok(event) = self.events.try_recv() {
            self.states[event.index].apply(event);
            busy = true;
        }

        busy
    }

    fn refresh_logs(&mut self) -> bool {
        self.states
            .iter_mut()
            .fold(false, |busy, st| st.refresh_log() || busy)
    }

    fn render(&mut self) -> Result<(), String> {
        self.terminal
            .draw(|frame| {
                BuildScreen::from_states(
                    self.states,
                    self.active_pane,
                    &self.scrollbacks,
                    self.popup,
                )
                .render(frame)
            })
            .map_err(|err| format!("xrun: failed to render TUI: {err}"))
            .map(|_| ())
    }

    fn refresh_popup(&mut self) {
        if self.all_finished() && !self.popup_dismissed {
            self.popup = Some(PopupState::Finished);
        }
    }

    fn all_finished(&self) -> bool {
        self.states.iter().all(JobState::is_finished)
    }

    fn exit_code(&self) -> i32 {
        self.states
            .iter()
            .find(|state| !state.is_success())
            .map(|state| state.status_code())
            .unwrap_or(0)
    }

    fn exit_code_or_abort(&self, key: KeyPress) -> i32 {
        if !self.all_finished() && key.should_abort_confirmed() {
            130
        } else {
            self.exit_code()
        }
    }

    fn key_pressed(&self) -> Option<KeyPress> {
        self.keys.try_recv().ok()
    }

    fn handle_key(&mut self, key: KeyPress) -> bool {
        if !self.all_finished() {
            return self.handle_live_key(key);
        }
        if self.popup.is_some() {
            if key.should_quit_finished() {
                self.cleanup_logs_for(key);
                return true;
            }
            self.popup = None;
            self.popup_dismissed = true;
            return false;
        }
        self.handle_finished_key(key)
    }

    fn handle_live_key(&mut self, key: KeyPress) -> bool {
        if self.popup == Some(PopupState::AbortConfirm) {
            if key.should_abort_confirmed() {
                return true;
            }
            self.popup = None;
            return false;
        }
        if key.should_abort_requested() {
            self.popup = Some(PopupState::AbortConfirm);
            return false;
        }
        if key.should_quit_live() {
            return true;
        }
        key.navigation()
            .into_iter()
            .for_each(|direction| self.move_active_pane(direction));
        key.scroll()
            .into_iter()
            .for_each(|scroll| self.scroll_active_pane(scroll));
        false
    }

    fn handle_finished_key(&mut self, key: KeyPress) -> bool {
        if key.should_quit_finished() {
            self.cleanup_logs_for(key);
            return true;
        }
        key.navigation()
            .into_iter()
            .for_each(|direction| self.move_active_pane(direction));
        key.scroll()
            .into_iter()
            .for_each(|scroll| self.scroll_active_pane(scroll));
        false
    }

    fn move_active_pane(&mut self, direction: PaneDirection) {
        if self.states.is_empty() {
            return;
        }
        self.active_pane = direction.next_index(self.active_pane, self.states.len());
    }

    fn scroll_active_pane(&mut self, scroll: PaneScroll) {
        let page_height = self.active_viewport_height();

        self.scrollbacks
            .get_mut(self.active_pane)
            .into_iter()
            .for_each(|scrollback| *scrollback = scroll.next_offset(*scrollback, page_height));
    }

    fn active_viewport_height(&self) -> usize {
        self.terminal
            .size()
            .ok()
            .map(|size| {
                BuildScreen::viewport_height(
                    self.states.len(),
                    self.active_pane,
                    Rect::new(0, 0, size.width, size.height),
                )
            })
            .unwrap_or(10)
    }

    fn cleanup_logs_for(&self, key: KeyPress) {
        key.should_cleanup_logs()
            .then_some(())
            .into_iter()
            .for_each(|_| self.remove_xrun_root());
    }

    fn remove_xrun_root(&self) {
        XrunRoot::path().into_iter().for_each(|path| {
            std::fs::remove_dir_all(path)
                .ok()
                .into_iter()
                .for_each(drop)
        });
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum PopupState {
    Finished,
    AbortConfirm,
}

#[derive(Clone, Copy)]
struct KeyPress {
    code: KeyCode,
    modifiers: KeyModifiers,
}

impl KeyPress {
    fn from_event(evt: Event) -> Option<Self> {
        match evt {
            Event::Key(key) => Some(Self::from_key(key)),
            _ => None,
        }
    }

    fn from_key(key: KeyEvent) -> Self {
        Self {
            code: key.code,
            modifiers: key.modifiers,
        }
    }

    fn should_quit(&self) -> bool {
        self.is_escape() || self.is_any_quit_char() || self.is_ctrl_c()
    }

    fn should_quit_live(&self) -> bool {
        self.is_escape() || self.is_any_quit_char()
    }

    fn should_abort_requested(&self) -> bool {
        self.is_ctrl_c()
    }

    fn should_abort_confirmed(&self) -> bool {
        self.is_ctrl_c() || self.is_abort_yes()
    }

    fn should_quit_finished(&self) -> bool {
        self.is_preserve_quit() || self.is_ctrl_c()
    }

    fn should_cleanup_logs(&self) -> bool {
        self.is_ctrl_c()
    }

    fn navigation(&self) -> Option<PaneDirection> {
        match self.code {
            KeyCode::BackTab => Some(PaneDirection::Prev),
            KeyCode::Tab => Some(PaneDirection::Next),
            _ => None,
        }
    }

    fn scroll(&self) -> Option<PaneScroll> {
        match self.code {
            KeyCode::Up => Some(PaneScroll::LineUp),
            KeyCode::Down => Some(PaneScroll::LineDown),
            KeyCode::PageUp if self.modifiers.contains(KeyModifiers::SHIFT) => {
                Some(PaneScroll::Top)
            }
            KeyCode::PageDown if self.modifiers.contains(KeyModifiers::SHIFT) => {
                Some(PaneScroll::Bottom)
            }
            KeyCode::Home => Some(PaneScroll::Top),
            KeyCode::End => Some(PaneScroll::Bottom),
            KeyCode::PageUp => Some(PaneScroll::PageUp),
            KeyCode::PageDown => Some(PaneScroll::PageDown),
            _ => None,
        }
    }

    fn is_ctrl_c(&self) -> bool {
        self.code == KeyCode::Char('c') && self.modifiers.contains(KeyModifiers::CONTROL)
    }

    fn is_escape(&self) -> bool {
        self.code == KeyCode::Esc
    }

    fn is_any_quit_char(&self) -> bool {
        match self.code {
            KeyCode::Char(ch) => ch.eq_ignore_ascii_case(&'q'),
            _ => false,
        }
    }

    fn is_preserve_quit(&self) -> bool {
        self.code == KeyCode::Char('p')
    }

    fn is_abort_yes(&self) -> bool {
        matches!(self.code, KeyCode::Char('y') | KeyCode::Char('Y'))
    }
}

#[derive(Clone, Copy)]
enum PaneDirection {
    Prev,
    Next,
}

impl PaneDirection {
    fn next_index(&self, active: usize, count: usize) -> usize {
        if count == 0 {
            return 0;
        }
        match self {
            Self::Prev => active.checked_sub(1).unwrap_or(count - 1),
            Self::Next => (active + 1) % count,
        }
    }
}

#[derive(Clone, Copy)]
enum PaneScroll {
    LineUp,
    LineDown,
    PageUp,
    PageDown,
    Top,
    Bottom,
}

impl PaneScroll {
    fn next_offset(&self, current: usize, page_height: usize) -> usize {
        match self {
            Self::LineUp => current.saturating_add(1),
            Self::LineDown => current.saturating_sub(1),
            Self::PageUp => current.saturating_add(page_height.max(1)),
            Self::PageDown => current.saturating_sub(page_height.max(1)),
            Self::Top => usize::MAX,
            Self::Bottom => 0,
        }
    }
}

struct JobSupervisor<'a> {
    plan: &'a BuildPlan,
}

struct InputReader;

impl InputReader {
    fn spawn() -> Result<Receiver<KeyPress>, String> {
        let (tx, rx) = mpsc::channel();

        thread::Builder::new()
            .name("xrun-input".to_string())
            .spawn(move || Self::run(tx))
            .map_err(|err| format!("xrun: failed to start input thread: {err}"))?;

        Ok(rx)
    }

    fn run(tx: Sender<KeyPress>) {
        loop {
            let event = event::read();

            if let Ok(evt) = event
                && let Some(key) = KeyPress::from_event(evt)
                && tx.send(key).is_err()
            {
                return;
            }
        }
    }
}

impl<'a> JobSupervisor<'a> {
    fn new(plan: &'a BuildPlan) -> Self {
        Self { plan }
    }

    fn spawn(&self) -> Result<Receiver<JobEvent>, String> {
        let (tx, rx) = mpsc::channel();

        self.plan
            .jobs()
            .iter()
            .cloned()
            .enumerate()
            .for_each(|(index, job)| JobWorker::new(index, job, tx.clone()).spawn());

        Ok(rx)
    }
}

struct JobWorker {
    index: usize,
    job: BuildJob,
    tx: Sender<JobEvent>,
}

impl JobWorker {
    fn new(index: usize, job: BuildJob, tx: Sender<JobEvent>) -> Self {
        Self { index, job, tx }
    }

    fn spawn(self) {
        thread::spawn(move || {
            let _ = self.tx.send(JobEvent::building(self.index));
            let _ = self.tx.send(
                self.job
                    .prepare()
                    .and_then(|_| self.job.run_build())
                    .and_then(|status| {
                        if status == 0 && self.job.should_mirror_results() {
                            self.tx
                                .send(JobEvent::mirroring(self.index))
                                .map_err(|err| err.to_string())
                                .map(|_| status)
                        } else {
                            Ok(status)
                        }
                    })
                    .and_then(|status| {
                        if status == 0 {
                            self.job.run_mirror().map(|_| status)
                        } else {
                            Ok(status)
                        }
                    })
                    .map(|status| JobEvent::finished(self.index, status))
                    .unwrap_or_else(|err| JobEvent::failed(self.index, err)),
            );
        });
    }
}

#[derive(Clone)]
pub struct JobState {
    title: String,
    log_path: PathBuf,
    log_buffer: TerminalBuffer,
    rendered_lines: Vec<ratatui::text::Line<'static>>,
    log_offset: u64,
    stage: JobStage,
    done: Option<JobStage>,
    status_code: i32,
}

impl JobState {
    pub(crate) fn from_job(job: &BuildJob) -> Self {
        Self {
            title: job.target().title(),
            log_path: job.log_path().to_path_buf(),
            log_buffer: TerminalBuffer::new(),
            rendered_lines: Vec::new(),
            log_offset: 0,
            stage: JobStage::Pending,
            done: None,
            status_code: 0,
        }
    }

    pub(crate) fn apply(&mut self, event: JobEvent) {
        self.status_code = event.status_code;
        if let Some(message) = event.error {
            let dirty = self.log_buffer.push_text(&format!("\n{message}\n"));
            self.rendered_lines.truncate(dirty);
            self.rendered_lines
                .extend(self.log_buffer.lines_from(dirty));
        }
        if event.stage.is_finished() {
            self.done = Some(event.stage);
            self.finish_if_drained();
            return;
        }
        self.done = None;
        self.stage = event.stage;
    }

    pub(crate) fn refresh_log(&mut self) -> bool {
        if !self.log_path.exists() {
            self.finish_if_drained();
            return false;
        }

        let mut busy = false;
        let mut total = 0;
        let mut dirty = self.rendered_lines.len();

        while total < LOG_TICK_MAX {
            let Some(bytes) = self
                .read_new_log_bytes((LOG_TICK_MAX - total).min(LOG_READ_MAX))
                .ok()
                .filter(|bytes| !bytes.is_empty())
            else {
                break;
            };

            total += bytes.len();
            dirty = dirty.min(
                self.log_buffer
                    .push_text(String::from_utf8_lossy(&bytes).as_ref()),
            );
            busy = true;

            if bytes.len() < LOG_READ_MAX {
                break;
            }
        }

        if busy {
            self.rendered_lines.truncate(dirty);
            self.rendered_lines
                .extend(self.log_buffer.lines_from(dirty));
        }

        self.finish_if_drained();

        busy
    }

    fn read_new_log_bytes(&mut self, max: usize) -> Result<Vec<u8>, String> {
        File::open(&self.log_path)
            .map_err(|err| format!("xrun: failed to open log file: {err}"))
            .and_then(|mut file| {
                file.seek(SeekFrom::Start(self.log_offset))
                    .map_err(|err| format!("xrun: failed to seek log file: {err}"))?;
                let mut bytes = vec![0_u8; max];
                let read_len = file
                    .read(&mut bytes)
                    .map_err(|err| format!("xrun: failed to read log file: {err}"))?;
                bytes.truncate(read_len);
                self.log_offset = self
                    .log_offset
                    .saturating_add(u64::try_from(bytes.len()).unwrap_or(u64::MAX));
                Ok(bytes)
            })
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn log_lines(&self) -> &[ratatui::text::Line<'static>] {
        &self.rendered_lines
    }

    pub fn summary(&self) -> &str {
        self.stage.label()
    }

    pub fn stage(&self) -> JobStage {
        self.stage
    }

    pub fn is_finished(&self) -> bool {
        self.stage.is_finished()
    }

    pub fn is_success(&self) -> bool {
        self.stage.is_success()
    }

    pub fn status_code(&self) -> i32 {
        self.status_code
    }

    fn finish_if_drained(&mut self) {
        if self.done.is_some() && self.log_drained() {
            self.stage = self.done.take().unwrap_or(self.stage);
        }
    }

    fn log_drained(&self) -> bool {
        std::fs::metadata(&self.log_path)
            .map(|meta| meta.len() <= self.log_offset)
            .unwrap_or(true)
    }
}

struct XrunRoot;

impl XrunRoot {
    fn path() -> Option<&'static Path> {
        Some(Path::new(".xrun"))
    }
}

#[derive(Clone, Copy)]
pub enum JobStage {
    Pending,
    Building,
    Mirroring,
    Success,
    Failed,
}

impl JobStage {
    pub(crate) fn label(&self) -> &str {
        match self {
            Self::Pending => "pending",
            Self::Building => "building",
            Self::Mirroring => "mirroring",
            Self::Success => "finished",
            Self::Failed => "failed",
        }
    }

    fn is_finished(&self) -> bool {
        matches!(self, Self::Success | Self::Failed)
    }

    fn is_success(&self) -> bool {
        matches!(self, Self::Success)
    }
}

pub(crate) struct JobEvent {
    index: usize,
    stage: JobStage,
    status_code: i32,
    error: Option<String>,
}

impl JobEvent {
    pub(crate) fn building(index: usize) -> Self {
        Self {
            index,
            stage: JobStage::Building,
            status_code: 0,
            error: None,
        }
    }

    pub(crate) fn mirroring(index: usize) -> Self {
        Self {
            index,
            stage: JobStage::Mirroring,
            status_code: 0,
            error: None,
        }
    }

    pub(crate) fn finished(index: usize, status_code: i32) -> Self {
        Self {
            index,
            stage: if status_code == 0 {
                JobStage::Success
            } else {
                JobStage::Failed
            },
            status_code,
            error: None,
        }
    }

    pub(crate) fn failed(index: usize, error: String) -> Self {
        Self {
            index,
            stage: JobStage::Failed,
            status_code: 1,
            error: Some(error),
        }
    }
}

struct TerminalGuard {
    terminal: Terminal<CrosstermBackend<std::io::Stdout>>,
}

impl TerminalGuard {
    fn enter() -> Result<Self, String> {
        enable_raw_mode().map_err(|err| format!("xrun: failed to enable raw mode: {err}"))?;
        execute!(std::io::stdout(), EnterAlternateScreen)
            .map_err(|err| format!("xrun: failed to enter alternate screen: {err}"))?;
        Terminal::new(CrosstermBackend::new(std::io::stdout()))
            .map_err(|err| format!("xrun: failed to create terminal: {err}"))
            .map(|terminal| Self { terminal })
    }

    fn terminal_mut(&mut self) -> &mut Terminal<CrosstermBackend<std::io::Stdout>> {
        &mut self.terminal
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(self.terminal.backend_mut(), LeaveAlternateScreen);
        let _ = self.terminal.show_cursor();
    }
}
