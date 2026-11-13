// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/11 15:13:51.932350
//+ Revised: 	2026/11/12 09:16:41.288332
// -----------------------------------------------------------------------------

use std::sync::{Arc, Mutex, MutexGuard};

/// One contiguous terminal style run; spaces retain their backgrounds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScreenSpan {
    pub text: String,
    pub foreground: Option<[u8; 3]>,
    pub background: Option<[u8; 3]>,
    pub bold: bool,
    pub underline: bool,
    pub italic: bool,
    pub inverse: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ScreenRow {
    pub spans: Vec<ScreenSpan>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TerminalSnapshot {
    pub rows: Vec<ScreenRow>,
    pub cursor: (u16, u16),
    pub cursor_visible: bool,
    pub application_cursor: bool,
    pub bracketed_paste: bool,
    pub scrollback: usize,
    pub running: bool,
    pub exit_code: Option<u32>,
    pub text: String,
    pub error: Option<String>,
}

/// Bounded VT state shared by the PTY reader and its UI snapshot cache.
pub struct TerminalScreen {
    parser: vt100::Parser,
    snapshot: TerminalSnapshot,
}

pub(super) type SharedScreen = Arc<Mutex<TerminalScreen>>;

impl TerminalScreen {
    pub fn new(rows: u16, cols: u16) -> anyhow::Result<Self> {
        validate_size(rows, cols)?;
        let mut screen = Self {
            parser: vt100::Parser::new(rows, cols, 1_000),
            snapshot: TerminalSnapshot::default(),
        };
        screen.refresh();
        Ok(screen)
    }

    pub fn process(&mut self, bytes: &[u8]) {
        self.parser.process(bytes);
        self.refresh();
    }

    /// Parse `bytes` without rebuilding the snapshot; call [`Self::process`] (or
    /// `process(&[])`) once a whole read is in.
    pub(super) fn feed(&mut self, bytes: &[u8]) {
        self.parser.process(bytes);
    }

    /// Where the cursor is right now (row, column; zero-based), without a snapshot.
    pub(super) fn cursor(&self) -> (u16, u16) {
        self.parser.screen().cursor_position()
    }

    pub fn resize(&mut self, rows: u16, cols: u16) -> anyhow::Result<()> {
        validate_size(rows, cols)?;
        self.parser.set_size(rows, cols);
        self.refresh();
        Ok(())
    }

    pub fn snapshot(&self) -> TerminalSnapshot {
        self.snapshot.clone()
    }

    pub fn scroll(&mut self, lines: i32) {
        let offset = self
            .parser
            .screen()
            .scrollback()
            .saturating_add_signed(lines as isize);
        self.parser.set_scrollback(offset);
        self.refresh();
    }

    pub(super) fn mark_running(&mut self) {
        self.snapshot.running = true;
    }

    pub(super) fn mark_exited(&mut self, code: u32) {
        self.snapshot.running = false;
        self.snapshot.exit_code = Some(code);
    }

    pub(super) fn set_error(&mut self, error: String) {
        self.snapshot.error = Some(error);
    }

    fn refresh(&mut self) {
        let screen = self.parser.screen();
        let (rows, cols) = screen.size();
        self.snapshot.rows = (0..rows).map(|row| screen_row(screen, row, cols)).collect();
        self.snapshot.cursor = screen.cursor_position();
        self.snapshot.cursor_visible = !screen.hide_cursor();
        self.snapshot.application_cursor = screen.application_cursor();
        self.snapshot.bracketed_paste = screen.bracketed_paste();
        self.snapshot.scrollback = screen.scrollback();
        self.snapshot.text = screen.contents();
    }
}

const MAX_ROWS: u16 = 512;
const MAX_COLUMNS: u16 = 1_024;
const MAX_CELLS: u32 = 131_072;

pub(super) fn validate_size(rows: u16, cols: u16) -> anyhow::Result<()> {
    anyhow::ensure!(
        rows > 0
            && cols > 0
            && rows <= MAX_ROWS
            && cols <= MAX_COLUMNS
            && u32::from(rows) * u32::from(cols) <= MAX_CELLS,
        "Terminal dimensions must be positive and contain at most {MAX_CELLS} cells ({MAX_ROWS} rows, {MAX_COLUMNS} columns)."
    );
    Ok(())
}

pub(super) fn lock(screen: &SharedScreen) -> MutexGuard<'_, TerminalScreen> {
    screen.lock().unwrap_or_else(|poisoned| {
        tracing::error!("terminal screen lock was poisoned");
        let mut state = poisoned.into_inner();
        state.set_error("Terminal screen state was interrupted.".into());
        state
    })
}

fn screen_row(screen: &vt100::Screen, row: u16, cols: u16) -> ScreenRow {
    let mut result = ScreenRow::default();
    for col in 0..cols {
        let cell = screen.cell(row, col).expect("validated terminal cell");
        if cell.is_wide_continuation() {
            continue;
        }
        let span = cell_span(cell);
        if let Some(previous) = result
            .spans
            .last_mut()
            .filter(|previous| same_style(previous, &span))
        {
            previous.text.push_str(&span.text);
        } else {
            result.spans.push(span);
        }
    }
    result
}

fn cell_span(cell: &vt100::Cell) -> ScreenSpan {
    let text = cell.contents();
    ScreenSpan {
        text: if text.is_empty() { " ".into() } else { text },
        foreground: color(cell.fgcolor()),
        background: color(cell.bgcolor()),
        bold: cell.bold(),
        underline: cell.underline(),
        italic: cell.italic(),
        inverse: cell.inverse(),
    }
}

fn same_style(left: &ScreenSpan, right: &ScreenSpan) -> bool {
    left.foreground == right.foreground
        && left.background == right.background
        && left.bold == right.bold
        && left.underline == right.underline
        && left.italic == right.italic
        && left.inverse == right.inverse
}

fn color(color: vt100::Color) -> Option<[u8; 3]> {
    match color {
        vt100::Color::Default => None,
        vt100::Color::Rgb(red, green, blue) => Some([red, green, blue]),
        vt100::Color::Idx(index) => Some(indexed_color(index)),
    }
}

fn indexed_color(index: u8) -> [u8; 3] {
    const ANSI: [[u8; 3]; 16] = [
        [0, 0, 0],
        [205, 0, 0],
        [0, 205, 0],
        [205, 205, 0],
        [0, 0, 238],
        [205, 0, 205],
        [0, 205, 205],
        [229, 229, 229],
        [127, 127, 127],
        [255, 0, 0],
        [0, 255, 0],
        [255, 255, 0],
        [92, 92, 255],
        [255, 0, 255],
        [0, 255, 255],
        [255, 255, 255],
    ];
    match index {
        0..=15 => ANSI[usize::from(index)],
        16..=231 => {
            const LEVELS: [u8; 6] = [0, 95, 135, 175, 215, 255];
            let cube = usize::from(index - 16);
            [LEVELS[cube / 36], LEVELS[cube / 6 % 6], LEVELS[cube % 6]]
        }
        _ => [8 + (index - 232) * 10; 3],
    }
}
