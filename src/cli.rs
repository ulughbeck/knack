use crate::Result;
use crossterm::cursor;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use crossterm::terminal::{self, ClearType};
use crossterm::ExecutableCommand;
use std::io::{self, Write};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

pub struct Progress {
    interactive: bool,
    state: Arc<Mutex<ProgressState>>,
    handle: Option<std::thread::JoinHandle<()>>,
}

struct ProgressState {
    message: String,
    running: bool,
    last_len: usize,
    spinner_idx: usize,
}

impl Progress {
    pub fn new() -> Self {
        Self {
            interactive: atty::is(atty::Stream::Stdout),
            state: Arc::new(Mutex::new(ProgressState {
                message: String::new(),
                running: false,
                last_len: 0,
                spinner_idx: 0,
            })),
            handle: None,
        }
    }

    pub fn start(&mut self, msg: &str) -> Result<()> {
        if !self.interactive {
            println!("{}", msg);
            return Ok(());
        }
        {
            let mut state = self.state.lock().unwrap();
            state.message = msg.to_string();
            state.running = true;
        }
        let state = Arc::clone(&self.state);
        self.handle = Some(thread::spawn(move || loop {
            let (message, pad, done) = {
                let mut st = state.lock().unwrap();
                if !st.running {
                    (String::new(), 0, true)
                } else {
                    let frames = ['-', '\\', '|', '/'];
                    let frame = frames[st.spinner_idx % frames.len()];
                    st.spinner_idx = st.spinner_idx.wrapping_add(1);
                    let text = format!("{} {}", frame, st.message);
                    let pad = st.last_len.saturating_sub(text.len());
                    st.last_len = text.len();
                    (text, pad, false)
                }
            };
            if done {
                break;
            }
            let mut stderr = io::stderr();
            let _ = write!(stderr, "\r{}{}", message, " ".repeat(pad));
            let _ = stderr.flush();
            thread::sleep(Duration::from_millis(120));
        }));
        Ok(())
    }

    pub fn update(&mut self, msg: &str) -> Result<()> {
        if !self.interactive {
            println!("{}", msg);
            return Ok(());
        }
        let mut state = self.state.lock().unwrap();
        state.message = msg.to_string();
        Ok(())
    }

    pub fn finish(&mut self, msg: &str) -> Result<()> {
        if self.interactive {
            {
                let mut state = self.state.lock().unwrap();
                state.running = false;
                if !msg.is_empty() {
                    let text = format!("✓ {}", msg);
                    let pad = state.last_len.saturating_sub(text.len());
                    let mut stderr = io::stderr();
                    write!(stderr, "\r{}{}", text, " ".repeat(pad))?;
                    writeln!(stderr)?;
                    stderr.flush()?;
                } else {
                    let pad = state.last_len;
                    let mut stderr = io::stderr();
                    write!(stderr, "\r{}", " ".repeat(pad))?;
                    write!(stderr, "\r")?;
                    stderr.flush()?;
                }
                state.last_len = 0;
            }
            if let Some(handle) = self.handle.take() {
                let _ = handle.join();
            }
        } else if !msg.is_empty() {
            println!("{}", msg);
        }
        Ok(())
    }
}

#[derive(Clone, Copy)]
pub struct SelectorUi<'a> {
    footer: &'a str,
    cancel_message: &'a str,
    empty_message: &'a str,
    confirm_label: Option<&'a str>,
}

impl<'a> SelectorUi<'a> {
    pub fn multi() -> Self {
        Self {
            footer: "↑/↓:move · space:toggle · a:all · n:none · Enter:install · Esc:cancel",
            cancel_message: "skill install cancelled",
            empty_message: "no items selected",
            confirm_label: Some("Install"),
        }
    }

    pub fn single() -> Self {
        Self {
            footer: "Enter:select · Esc:cancel",
            cancel_message: "skill install cancelled",
            empty_message: "no options available",
            confirm_label: None,
        }
    }

    pub fn with_cancel(mut self, message: &'a str) -> Self {
        self.cancel_message = message;
        self
    }

    pub fn with_empty(mut self, message: &'a str) -> Self {
        self.empty_message = message;
        self
    }

    pub fn with_footer(mut self, footer: &'a str) -> Self {
        self.footer = footer;
        self
    }

    pub fn with_confirm_label(mut self, label: &'a str) -> Self {
        self.confirm_label = Some(label);
        self
    }
}

pub fn select_many(
    prompt: &str,
    options: &[String],
    default_indexes: &[usize],
) -> Result<Vec<usize>> {
    select_many_with_ui(prompt, options, default_indexes, SelectorUi::multi())
}

pub fn select_many_with_ui(
    prompt: &str,
    options: &[String],
    default_indexes: &[usize],
    ui: SelectorUi<'_>,
) -> Result<Vec<usize>> {
    run_selector(SelectorConfig::multi(prompt, options, default_indexes, ui))
}

pub fn select_one(prompt: &str, options: &[String], default_index: Option<usize>) -> Result<usize> {
    select_one_with_ui(prompt, options, default_index, SelectorUi::single())
}

pub fn select_one_with_ui(
    prompt: &str,
    options: &[String],
    default_index: Option<usize>,
    ui: SelectorUi<'_>,
) -> Result<usize> {
    let default_indexes = default_index.map(|idx| vec![idx]).unwrap_or_default();
    let indexes = run_selector(SelectorConfig::single(
        prompt,
        options,
        &default_indexes,
        ui,
    ))?;
    if indexes.len() != 1 {
        return Err(crate::error::KnackError::msg(
            "please select a single option",
        ));
    }
    Ok(indexes[0])
}

#[derive(Clone)]
enum SelectorMode {
    Single,
    Multi { confirm_label: String },
}

struct SelectorConfig<'a> {
    prompt: &'a str,
    options: &'a [String],
    default_indexes: Vec<usize>,
    cancel_message: &'a str,
    empty_message: &'a str,
    footer: &'a str,
    mode: SelectorMode,
}

impl<'a> SelectorConfig<'a> {
    fn single(
        prompt: &'a str,
        options: &'a [String],
        default_indexes: &[usize],
        ui: SelectorUi<'a>,
    ) -> Self {
        Self {
            prompt,
            options,
            default_indexes: default_indexes.to_vec(),
            cancel_message: ui.cancel_message,
            empty_message: ui.empty_message,
            footer: ui.footer,
            mode: SelectorMode::Single,
        }
    }

    fn multi(
        prompt: &'a str,
        options: &'a [String],
        default_indexes: &[usize],
        ui: SelectorUi<'a>,
    ) -> Self {
        let confirm_label = ui.confirm_label.unwrap_or("Install").to_string();
        Self {
            prompt,
            options,
            default_indexes: default_indexes.to_vec(),
            cancel_message: ui.cancel_message,
            empty_message: ui.empty_message,
            footer: ui.footer,
            mode: SelectorMode::Multi { confirm_label },
        }
    }
}

fn run_selector(config: SelectorConfig<'_>) -> Result<Vec<usize>> {
    if config.options.is_empty() {
        return Err(crate::error::KnackError::msg(config.empty_message));
    }

    if !atty::is(atty::Stream::Stdin) || !atty::is(atty::Stream::Stdout) {
        return run_selector_fallback(&config);
    }

    let mut selected = vec![false; config.options.len()];
    for idx in &config.default_indexes {
        if *idx < selected.len() {
            selected[*idx] = true;
        }
    }

    let mut cursor_idx = match config.mode {
        SelectorMode::Single => config.default_indexes.first().copied().unwrap_or(0),
        SelectorMode::Multi { .. } => selected.iter().position(|on| *on).unwrap_or(0),
    };
    if cursor_idx >= config.options.len() {
        cursor_idx = 0;
    }

    let install_index = config.options.len();
    let mut scroll_offset = 0usize;
    let mut last_lines = 0usize;

    terminal::enable_raw_mode()?;
    let mut stdout = io::stdout();
    stdout.execute(cursor::Hide)?;

    let result = loop {
        clear_rendered_lines(&mut stdout, last_lines)?;
        let (width, height) = terminal::size().unwrap_or((80, 24));
        last_lines = match &config.mode {
            SelectorMode::Single => {
                let list_height = max_list_height(height, 3);
                scroll_offset = compute_scroll_offset(
                    scroll_offset,
                    cursor_idx,
                    config.options.len(),
                    list_height,
                );
                render_single_prompt(
                    &mut stdout,
                    config.prompt,
                    config.options,
                    cursor_idx,
                    scroll_offset,
                    list_height,
                    width,
                    config.footer,
                )?
            }
            SelectorMode::Multi { confirm_label } => {
                let list_height = max_list_height(height, 4);
                scroll_offset = compute_scroll_offset(
                    scroll_offset,
                    cursor_idx,
                    config.options.len(),
                    list_height,
                );
                render_multiline_prompt(
                    &mut stdout,
                    config.prompt,
                    config.options,
                    &selected,
                    cursor_idx,
                    scroll_offset,
                    list_height,
                    width,
                    confirm_label,
                    config.footer,
                )?
            }
        };

        match event::read()? {
            Event::Key(KeyEvent {
                code, modifiers, ..
            }) => match code {
                KeyCode::Left | KeyCode::Up => match config.mode {
                    SelectorMode::Multi { .. } => {
                        if cursor_idx == 0 {
                            cursor_idx = install_index;
                        } else {
                            cursor_idx -= 1;
                        }
                    }
                    SelectorMode::Single => {
                        if cursor_idx == 0 {
                            cursor_idx = config.options.len() - 1;
                        } else {
                            cursor_idx -= 1;
                        }
                    }
                },
                KeyCode::Right | KeyCode::Down => match config.mode {
                    SelectorMode::Multi { .. } => {
                        if cursor_idx >= install_index {
                            cursor_idx = 0;
                        } else {
                            cursor_idx += 1;
                        }
                    }
                    SelectorMode::Single => {
                        if cursor_idx + 1 >= config.options.len() {
                            cursor_idx = 0;
                        } else {
                            cursor_idx += 1;
                        }
                    }
                },
                KeyCode::Char('a') | KeyCode::Char('A') => {
                    if matches!(config.mode, SelectorMode::Multi { .. }) {
                        for item in &mut selected {
                            *item = true;
                        }
                    }
                }
                KeyCode::Char('n') | KeyCode::Char('N') => {
                    if matches!(config.mode, SelectorMode::Multi { .. }) {
                        for item in &mut selected {
                            *item = false;
                        }
                    }
                }
                KeyCode::Char(' ') => {
                    if matches!(config.mode, SelectorMode::Multi { .. })
                        && cursor_idx < selected.len()
                    {
                        selected[cursor_idx] = !selected[cursor_idx];
                    }
                }
                KeyCode::Enter => match config.mode {
                    SelectorMode::Single => break Ok(vec![cursor_idx]),
                    SelectorMode::Multi { .. } => {
                        if cursor_idx == install_index {
                            break Ok(selected
                                .iter()
                                .enumerate()
                                .filter_map(|(idx, on)| if *on { Some(idx) } else { None })
                                .collect());
                        }
                        if cursor_idx < selected.len() {
                            selected[cursor_idx] = !selected[cursor_idx];
                        }
                    }
                },
                KeyCode::Esc => break Err(crate::error::KnackError::msg(config.cancel_message)),
                KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => {
                    break Err(crate::error::KnackError::msg(config.cancel_message))
                }
                _ => {}
            },
            _ => {}
        }
    };

    clear_rendered_lines(&mut stdout, last_lines)?;
    stdout.execute(cursor::Show)?;
    terminal::disable_raw_mode()?;

    let indexes = result?;
    if indexes.is_empty() {
        return Err(crate::error::KnackError::msg(config.empty_message));
    }
    Ok(indexes)
}

fn run_selector_fallback(config: &SelectorConfig<'_>) -> Result<Vec<usize>> {
    let mut stdout = io::stdout();
    writeln!(stdout, "{}", config.prompt)?;
    for (i, option) in config.options.iter().enumerate() {
        writeln!(stdout, "  {}) {}", i + 1, option)?;
    }
    if !config.default_indexes.is_empty() {
        write!(
            stdout,
            "Enter numbers separated by comma (or press Enter for default): "
        )?;
    } else {
        write!(stdout, "Enter numbers separated by comma: ")?;
    }
    stdout.flush()?;

    let mut input = String::new();
    let bytes = io::stdin().read_line(&mut input)?;
    if bytes == 0 && input.is_empty() {
        return Err(crate::error::KnackError::msg("no input received"));
    }
    let line = input.trim();
    let indexes = if line.is_empty() {
        config.default_indexes.clone()
    } else if line.eq_ignore_ascii_case("all") {
        (0..config.options.len()).collect()
    } else {
        let mut indexes = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for part in line.split(',') {
            let part = part.trim();
            if part.is_empty() {
                continue;
            }
            let idx: usize = part.parse().map_err(|_| {
                crate::error::KnackError::msg(format!("invalid selection: {}", part))
            })?;
            if idx == 0 || idx > config.options.len() {
                return Err(crate::error::KnackError::msg(format!(
                    "invalid selection: {}",
                    part
                )));
            }
            let zero = idx - 1;
            if seen.insert(zero) {
                indexes.push(zero);
            }
        }
        indexes
    };

    if indexes.is_empty() {
        return Err(crate::error::KnackError::msg(config.empty_message));
    }
    if matches!(config.mode, SelectorMode::Single) && indexes.len() > 1 {
        return Err(crate::error::KnackError::msg(
            "please select a single option",
        ));
    }
    Ok(indexes)
}

pub fn is_interactive() -> bool {
    atty::is(atty::Stream::Stdin)
}

fn render_multiline_prompt(
    stdout: &mut io::Stdout,
    prompt: &str,
    options: &[String],
    selected: &[bool],
    cursor_idx: usize,
    scroll_offset: usize,
    list_height: usize,
    width: u16,
    confirm_label: &str,
    footer: &str,
) -> Result<usize> {
    let selected_count = selected.iter().filter(|on| **on).count();
    let total = options.len();
    let mut lines = Vec::new();

    lines.push(prompt.to_string());
    lines.push(format!("Selected {}/{}", selected_count, total));

    for line_idx in 0..list_height {
        let idx = scroll_offset + line_idx;
        if idx >= options.len() {
            lines.push(String::new());
            continue;
        }
        let mark = if selected.get(idx).copied().unwrap_or(false) {
            "x"
        } else {
            " "
        };
        let pointer = if idx == cursor_idx { ">" } else { " " };
        lines.push(format!("{} [{}] {}", pointer, mark, options[idx]));
    }

    let footer_line = if cursor_idx == options.len() {
        format!(">[ {} ]<", confirm_label)
    } else {
        format!("[ {} ]", confirm_label)
    };
    lines.push(footer_line);
    lines.push(footer.to_string());

    for (i, line) in lines.iter().enumerate() {
        write_line(stdout, line, width)?;
        if i + 1 < lines.len() {
            writeln!(stdout)?;
        }
    }
    stdout.flush()?;
    Ok(lines.len())
}

fn render_single_prompt(
    stdout: &mut io::Stdout,
    prompt: &str,
    options: &[String],
    cursor_idx: usize,
    scroll_offset: usize,
    list_height: usize,
    width: u16,
    footer: &str,
) -> Result<usize> {
    let total = options.len();
    let mut lines = Vec::new();

    lines.push(prompt.to_string());
    lines.push(format!("Option {}/{}", cursor_idx + 1, total));

    for line_idx in 0..list_height {
        let idx = scroll_offset + line_idx;
        if idx >= options.len() {
            lines.push(String::new());
            continue;
        }
        let pointer = if idx == cursor_idx { ">" } else { " " };
        lines.push(format!("{} {}", pointer, options[idx]));
    }

    lines.push(footer.to_string());

    for (i, line) in lines.iter().enumerate() {
        write_line(stdout, line, width)?;
        if i + 1 < lines.len() {
            writeln!(stdout)?;
        }
    }
    stdout.flush()?;
    Ok(lines.len())
}

fn write_line(stdout: &mut io::Stdout, line: &str, width: u16) -> Result<()> {
    stdout.execute(cursor::MoveToColumn(0))?;
    stdout.execute(terminal::Clear(ClearType::CurrentLine))?;
    let max = width.saturating_sub(1) as usize;
    if max == 0 {
        return Ok(());
    }
    if line.len() > max {
        let take = max.saturating_sub(3);
        if take > 0 && take < line.len() {
            write!(stdout, "{}...", &line[..take])?;
        } else {
            write!(stdout, "{}", &line[..max.min(line.len())])?;
        }
    } else {
        write!(stdout, "{}", line)?;
    }
    Ok(())
}

fn clear_rendered_lines(stdout: &mut io::Stdout, line_count: usize) -> Result<()> {
    if line_count == 0 {
        return Ok(());
    }
    stdout.execute(cursor::MoveUp((line_count - 1) as u16))?;
    for i in 0..line_count {
        stdout.execute(cursor::MoveToColumn(0))?;
        stdout.execute(terminal::Clear(ClearType::CurrentLine))?;
        if i + 1 < line_count {
            stdout.execute(cursor::MoveDown(1))?;
        }
    }
    stdout.execute(cursor::MoveUp((line_count - 1) as u16))?;
    Ok(())
}

fn max_list_height(term_height: u16, reserved: usize) -> usize {
    let height = term_height as usize;
    if height > reserved {
        height - reserved
    } else {
        1
    }
}

fn compute_scroll_offset(
    current: usize,
    cursor_idx: usize,
    total: usize,
    list_height: usize,
) -> usize {
    if total <= list_height {
        return 0;
    }
    if cursor_idx >= total {
        return total.saturating_sub(list_height);
    }
    let mut offset = current;
    if cursor_idx < offset {
        offset = cursor_idx;
    } else if cursor_idx >= offset + list_height {
        offset = cursor_idx + 1 - list_height;
    }
    offset
}
