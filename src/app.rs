use std::cell::RefCell;
use std::sync::{OnceLock, mpsc};

use iced::futures::SinkExt;
use iced::keyboard::key::Named;
use iced::widget::{column, container, row, scrollable, text};
use iced::window::Id as WindowId;
use iced::{Alignment, Element, Event, Length, Task as Command, event, keyboard};
use iced_layershell::reexport::KeyboardInteractivity;
use iced_layershell::to_layer_message;

use crate::config::{get, parse_color};
use crate::daemon::{DAEMON_RX, DaemonCommand, listen_for_commands};
use crate::data::{
    DesktopApp, WindowInfo, hyprctl_active_workspace_id, hyprctl_dispatch, launch_app, load_apps,
    load_windows,
};
use crate::styles::{container_style, scrollbar_style};

const SCROLLABLE_ID: &str = "main-list";

// ── run mode ─────────────────────────────────────────────────────────────────

#[derive(Debug, PartialEq)]
pub enum RunMode {
    Normal,
    Daemon,
}

pub static RUN_MODE: OnceLock<RunMode> = OnceLock::new();
pub static DEFAULT_VIEW: OnceLock<ViewMode> = OnceLock::new();

pub fn run_mode() -> &'static RunMode {
    RUN_MODE.get().unwrap_or(&RunMode::Normal)
}

pub fn default_view() -> ViewMode {
    DEFAULT_VIEW.get().copied().unwrap_or(ViewMode::Apps)
}

// ── view mode ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ViewMode {
    Windows,
    Apps,
}

// ── vim mode ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum VimMode {
    Normal,
    Search,
    Command,
    MoveWindow,
}

// ── state ────────────────────────────────────────────────────────────────────

pub struct App {
    pub visible: bool,
    pub view_mode: ViewMode,
    pub show_help: bool,
    pub known_ids: RefCell<Vec<WindowId>>,

    // shared vim state
    pub query: String,
    pub cmd: String,
    pub selected: usize,
    pub vim_mode: VimMode,

    // app launcher data
    pub all_apps: Vec<DesktopApp>,
    pub app_filtered: Vec<usize>,

    // window switcher data
    pub windows: Vec<WindowInfo>,
    pub win_filtered: Vec<usize>,

    // vim count prefix (e.g. "4j" moves down 4)
    pub count_buf: String,

    // pending key sequence for multi-key bindings (e.g. "g" waiting for "gg")
    pub key_seq: Vec<String>,

    // workspace number being typed for move-window mode
    pub move_buf: String,

}

impl App {
    fn items_len(&self) -> usize {
        match self.view_mode {
            ViewMode::Apps => self.app_filtered.len(),
            ViewMode::Windows => self.win_filtered.len(),
        }
    }

    fn filter(&mut self) {
        let q = self.query.to_lowercase();
        match self.view_mode {
            ViewMode::Apps => {
                self.app_filtered = self
                    .all_apps
                    .iter()
                    .enumerate()
                    .filter(|(_, a)| q.is_empty() || a.name.to_lowercase().contains(&q))
                    .map(|(i, _)| i)
                    .collect();
            }
            ViewMode::Windows => {
                self.win_filtered = self
                    .windows
                    .iter()
                    .enumerate()
                    .filter(|(_, w)| {
                        q.is_empty()
                            || w.title.to_lowercase().contains(&q)
                            || w.class.to_lowercase().contains(&q)
                    })
                    .map(|(i, _)| i)
                    .collect();
            }
        }
        let len = self.items_len();
        if self.selected >= len {
            self.selected = len.saturating_sub(1);
        }
    }

    fn reset_state(&mut self) {
        self.query.clear();
        self.cmd.clear();
        self.move_buf.clear();
        self.key_seq.clear();
        self.selected = 0;
        self.vim_mode = VimMode::Normal;
        self.show_help = false;
        self.count_buf.clear();
    }
}

#[to_layer_message(multi)]
#[derive(Debug, Clone)]
pub enum Message {
    IcedEvent(Event),
    Close,
    Reload,
    ShowApps,
    ShowWindows,
}

pub fn namespace() -> String {
    "hyprmwh".into()
}

pub fn new() -> (App, Command<Message>) {
    let dv = default_view();

    let cmd = match run_mode() {
        RunMode::Daemon => {
            let (tx, rx) = mpsc::channel();
            DAEMON_RX.set(std::sync::Mutex::new(rx)).ok();
            std::thread::spawn(move || {
                tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .unwrap()
                    .block_on(listen_for_commands(tx));
            });
            Command::none()
        }
        RunMode::Normal => match dv {
            ViewMode::Apps => Command::done(Message::ShowApps),
            ViewMode::Windows => Command::done(Message::ShowWindows),
        },
    };

    let mut state = App {
        visible: false,
        view_mode: dv,
        show_help: false,
        known_ids: RefCell::new(Vec::new()),

        query: String::new(),
        cmd: String::new(),
        selected: 0,
        vim_mode: VimMode::Normal,

        all_apps: load_apps(),
        app_filtered: Vec::new(),

        windows: Vec::new(),
        win_filtered: Vec::new(),
        count_buf: String::new(),
        key_seq: Vec::new(),
        move_buf: String::new(),
    };

    // Pre-filter both
    state.view_mode = ViewMode::Apps;
    state.filter();
    state.view_mode = dv;

    (state, cmd)
}

// ── layer shell helpers ──────────────────────────────────────────────────────

const WINDOW_HEIGHT: u32 = 500;
const HELP_HEIGHT: u32 = 620;

fn resize_all(app: &App, height: u32) -> Command<Message> {
    let cfg = get();
    let width = cfg.window.width;
    let anchor = cfg.window.anchor.to_anchor();
    let cmds: Vec<Command<Message>> = app
        .known_ids
        .borrow()
        .iter()
        .map(|&id| {
            Command::done(Message::AnchorSizeChange {
                id,
                anchor,
                size: (width, height),
            })
        })
        .collect();
    Command::batch(cmds)
}

fn all_ids_hide(app: &App) -> Command<Message> {
    let anchor = get().window.anchor.to_anchor();
    let cmds: Vec<Command<Message>> = app
        .known_ids
        .borrow()
        .iter()
        .flat_map(|&id| {
            [
                Command::done(Message::AnchorSizeChange {
                    id,
                    anchor,
                    size: (1, 1),
                }),
                Command::done(Message::KeyboardInteractivityChange {
                    id,
                    keyboard_interactivity: KeyboardInteractivity::None,
                }),
            ]
        })
        .collect();
    Command::batch(cmds)
}

fn all_ids_show(app: &App) -> Command<Message> {
    let cfg = get();
    let anchor = cfg.window.anchor.to_anchor();
    let margin = cfg.window.anchor.to_margin(cfg.window.margin);
    let width = cfg.window.width;
    let cmds: Vec<Command<Message>> = app
        .known_ids
        .borrow()
        .iter()
        .flat_map(|&id| {
            [
                Command::done(Message::AnchorSizeChange {
                    id,
                    anchor,
                    size: (width, WINDOW_HEIGHT),
                }),
                Command::done(Message::MarginChange { id, margin }),
                Command::done(Message::KeyboardInteractivityChange {
                    id,
                    keyboard_interactivity: KeyboardInteractivity::Exclusive,
                }),
            ]
        })
        .collect();
    Command::batch(cmds)
}

fn do_close(app: &mut App) -> Command<Message> {
    app.visible = false;
    app.reset_state();
    match run_mode() {
        RunMode::Normal => std::process::exit(0),
        RunMode::Daemon => all_ids_hide(app),
    }
}

// ── update ───────────────────────────────────────────────────────────────────

pub fn update(app: &mut App, message: Message) -> Command<Message> {
    match message {
        Message::ShowWindows => {
            app.view_mode = ViewMode::Windows;
            app.reset_state();
            app.windows = load_windows();
            app.filter();
            app.visible = true;
            all_ids_show(app)
        }

        Message::ShowApps => {
            app.view_mode = ViewMode::Apps;
            app.reset_state();
            app.vim_mode = VimMode::Search;
            app.filter();
            app.visible = true;
            all_ids_show(app)
        }

        Message::Reload => {
            crate::config::reload();
            app.all_apps = load_apps();
            Command::none()
        }

        Message::Close => do_close(app),

        Message::IcedEvent(Event::Keyboard(keyboard::Event::KeyPressed {
            key,
            modifiers,
            ..
        })) => {
            if !app.visible {
                return Command::none();
            }

            // Help screen: any key dismisses it
            if app.show_help {
                app.show_help = false;
                return resize_all(app, WINDOW_HEIGHT);
            }

            handle_key(app, key, modifiers)
        }

        Message::IcedEvent(_) => Command::none(),
        _ => Command::none(),
    }
}

// ── unified key handler ──────────────────────────────────────────────────────

fn handle_key(
    app: &mut App,
    key: keyboard::Key,
    modifiers: keyboard::Modifiers,
) -> Command<Message> {
    match app.vim_mode {
        VimMode::Search => match key {
            keyboard::Key::Named(Named::Escape) => {
                app.query.clear();
                app.filter();
                app.vim_mode = VimMode::Normal;
                Command::none()
            }
            keyboard::Key::Named(Named::Enter) => {
                app.vim_mode = VimMode::Normal;
                Command::none()
            }
            keyboard::Key::Named(Named::Backspace) => {
                app.query.pop();
                app.filter();
                Command::none()
            }
            keyboard::Key::Character(ref c) if !modifiers.control() && !modifiers.alt() => {
                app.query.push_str(c);
                app.filter();
                Command::none()
            }
            _ => Command::none(),
        },

        VimMode::Command => match key {
            keyboard::Key::Named(Named::Escape) => {
                app.cmd.clear();
                app.vim_mode = VimMode::Normal;
                Command::none()
            }
            keyboard::Key::Named(Named::Enter) => {
                let cmd = app.cmd.trim().to_string();
                app.cmd.clear();
                app.vim_mode = VimMode::Normal;
                match cmd.as_str() {
                    "q" | "wq" | "q!" => do_close(app),
                    "help" | "?" => {
                        app.show_help = true;
                        resize_all(app, HELP_HEIGHT)
                    }
                    _ => Command::none(),
                }
            }
            keyboard::Key::Named(Named::Backspace) => {
                app.cmd.pop();
                if app.cmd.is_empty() {
                    app.vim_mode = VimMode::Normal;
                }
                Command::none()
            }
            keyboard::Key::Character(ref c) if !modifiers.control() && !modifiers.alt() => {
                app.cmd.push_str(c);
                Command::none()
            }
            _ => Command::none(),
        },

        VimMode::MoveWindow => match key {
            keyboard::Key::Named(Named::Escape) => {
                app.move_buf.clear();
                app.vim_mode = VimMode::Normal;
                Command::none()
            }
            keyboard::Key::Named(Named::Backspace) => {
                app.move_buf.pop();
                if app.move_buf.is_empty() {
                    app.vim_mode = VimMode::Normal;
                }
                Command::none()
            }
            keyboard::Key::Named(Named::Enter) => {
                if let Ok(ws_id) = app.move_buf.trim().parse::<i64>() {
                    if let Some(&idx) = app.win_filtered.get(app.selected) {
                        let addr = app.windows[idx].address.clone();
                        hyprctl_dispatch(&[
                            "movetoworkspace",
                            &format!("{},address:{}", ws_id, addr),
                        ]);
                        app.windows = load_windows();
                        app.filter();
                    }
                }
                app.move_buf.clear();
                app.vim_mode = VimMode::Normal;
                Command::none()
            }
            keyboard::Key::Character(ref c) if !modifiers.control() && !modifiers.alt() => {
                if c.chars().all(|ch| ch.is_ascii_digit()) {
                    app.move_buf.push_str(c);
                }
                Command::none()
            }
            _ => Command::none(),
        },

        VimMode::Normal => handle_normal(app, key, modifiers),
    }
}

fn scroll_to_selected(app: &App) -> Command<Message> {
    let count = app.items_len();
    if count <= 1 {
        return Command::none();
    }
    let ratio = app.selected as f32 / (count - 1) as f32;
    iced::widget::operation::snap_to(
        iced::widget::Id::new(SCROLLABLE_ID),
        iced::widget::scrollable::RelativeOffset { y: ratio, x: 0.0 },
    )
}

fn take_count(app: &mut App) -> usize {
    let n = app.count_buf.parse::<usize>().unwrap_or(1);
    app.count_buf.clear();
    n
}

// ── keybind sequence engine ──────────────────────────────────────────────────

/// Parse a binding string like "gg", "G", "Space", "Tab" into key tokens.
/// Special names (Space, Tab) are consumed as a unit; all other chars are
/// individual tokens so "gg" → ["g", "g"].
fn parse_seq(s: &str) -> Vec<String> {
    const SPECIALS: &[&str] = &["Space", "Tab"];
    let mut tokens = Vec::new();
    let mut pos = 0;
    while pos < s.len() {
        let rest = &s[pos..];
        let mut matched = false;
        for &sp in SPECIALS {
            if rest.starts_with(sp) {
                tokens.push(sp.to_string());
                pos += sp.len();
                matched = true;
                break;
            }
        }
        if !matched {
            let ch = rest.chars().next().unwrap();
            tokens.push(ch.to_string());
            pos += ch.len_utf8();
        }
    }
    tokens
}

/// Normalise an iced key to a token string for sequence matching.
/// Returns None for keys we don't route through the seq engine (modifiers, etc.).
fn key_token(key: &keyboard::Key, modifiers: keyboard::Modifiers) -> Option<String> {
    if modifiers.control() || modifiers.alt() {
        return None;
    }
    match key {
        keyboard::Key::Named(Named::Space) => Some("Space".into()),
        keyboard::Key::Named(Named::Tab) => Some("Tab".into()),
        keyboard::Key::Character(c) => Some(c.to_string()),
        _ => None,
    }
}

#[derive(Debug, Clone, Copy)]
enum Action {
    MoveDown,
    MoveUp,
    GoTop,
    GoBottom,
    Search,
    Quit,
    SwitchView,
    MoveToWorkspace,
    CloseWindow,
}

fn all_bindings(kb: &crate::config::KeybindsConfig) -> Vec<(&Vec<String>, Action)> {
    vec![
        (&kb.move_down,         Action::MoveDown),
        (&kb.move_up,           Action::MoveUp),
        (&kb.go_top,            Action::GoTop),
        (&kb.go_bottom,         Action::GoBottom),
        (&kb.search,            Action::Search),
        (&kb.quit,              Action::Quit),
        (&kb.switch_view,       Action::SwitchView),
        (&kb.move_to_workspace, Action::MoveToWorkspace),
        (&kb.close_window,      Action::CloseWindow),
    ]
}

/// Check if `seq` exactly matches any configured binding. Returns the action if so.
fn match_exact(seq: &[String], kb: &crate::config::KeybindsConfig) -> Option<Action> {
    for (bindings, action) in all_bindings(kb) {
        for s in bindings {
            if parse_seq(s) == seq {
                return Some(action);
            }
        }
    }
    None
}

/// Returns true if `seq` is a strict prefix of at least one configured binding.
fn is_prefix(seq: &[String], kb: &crate::config::KeybindsConfig) -> bool {
    for (bindings, _) in all_bindings(kb) {
        for s in bindings {
            let tokens = parse_seq(s);
            if tokens.len() > seq.len() && tokens.starts_with(seq) {
                return true;
            }
        }
    }
    false
}

fn execute_action(app: &mut App, action: Action) -> Command<Message> {
    let max = app.items_len().saturating_sub(1);
    match action {
        Action::MoveDown => {
            let n = take_count(app);
            app.selected = (app.selected + n).min(max);
            scroll_to_selected(app)
        }
        Action::MoveUp => {
            let n = take_count(app);
            app.selected = app.selected.saturating_sub(n);
            scroll_to_selected(app)
        }
        Action::GoTop => {
            app.count_buf.clear();
            app.selected = 0;
            scroll_to_selected(app)
        }
        Action::GoBottom => {
            app.count_buf.clear();
            app.selected = max;
            scroll_to_selected(app)
        }
        Action::Search => {
            app.count_buf.clear();
            app.query.clear();
            app.filter();
            app.vim_mode = VimMode::Search;
            Command::none()
        }
        Action::Quit => {
            app.count_buf.clear();
            do_close(app)
        }
        Action::SwitchView => {
            app.count_buf.clear();
            match app.view_mode {
                ViewMode::Windows => {
                    app.view_mode = ViewMode::Apps;
                    app.reset_state();
                    app.vim_mode = VimMode::Search;
                }
                ViewMode::Apps => {
                    app.view_mode = ViewMode::Windows;
                    app.windows = load_windows();
                    app.reset_state();
                }
            }
            app.filter();
            Command::none()
        }
        Action::MoveToWorkspace => {
            app.count_buf.clear();
            if app.view_mode == ViewMode::Windows {
                app.move_buf.clear();
                app.vim_mode = VimMode::MoveWindow;
            }
            Command::none()
        }
        Action::CloseWindow => {
            app.count_buf.clear();
            if app.view_mode == ViewMode::Windows {
                if let Some(&idx) = app.win_filtered.get(app.selected) {
                    let addr = app.windows[idx].address.clone();
                    hyprctl_dispatch(&["closewindow", &format!("address:{}", addr)]);
                    app.windows.remove(idx);
                    app.filter();
                }
            }
            Command::none()
        }
    }
}

// ── normal mode handler ──────────────────────────────────────────────────────

fn handle_normal(
    app: &mut App,
    key: keyboard::Key,
    modifiers: keyboard::Modifiers,
) -> Command<Message> {
    // ── hardcoded keys (not configurable) ────────────────────────────────────

    // Escape: cancel pending sequence, or close
    if matches!(key, keyboard::Key::Named(Named::Escape)) {
        if !app.key_seq.is_empty() {
            app.key_seq.clear();
            app.count_buf.clear();
            return Command::none();
        }
        return do_close(app);
    }

    // ? / shift+/ = help
    if let keyboard::Key::Character(ref c) = key {
        if c == "?" || (c == "/" && modifiers.shift()) {
            app.key_seq.clear();
            app.count_buf.clear();
            app.show_help = true;
            return resize_all(app, HELP_HEIGHT);
        }
        // : / shift+; = command mode
        if c == ":" || (c == ";" && modifiers.shift()) {
            app.key_seq.clear();
            app.count_buf.clear();
            app.cmd.clear();
            app.vim_mode = VimMode::Command;
            return Command::none();
        }
    }

    // Shift+Enter (windows only) = move window to current workspace
    if matches!(key, keyboard::Key::Named(Named::Enter)) && modifiers.shift() {
        app.key_seq.clear();
        app.count_buf.clear();
        if app.view_mode == ViewMode::Windows {
            if let Some(&idx) = app.win_filtered.get(app.selected) {
                let win = &app.windows[idx];
                if let Some(ws_id) = hyprctl_active_workspace_id() {
                    hyprctl_dispatch(&[
                        "movetoworkspace",
                        &format!("{},address:{}", ws_id, win.address),
                    ]);
                    hyprctl_dispatch(&["focuswindow", &format!("address:{}", win.address)]);
                }
            }
        }
        return do_close(app);
    }

    // Enter = select / activate
    if matches!(key, keyboard::Key::Named(Named::Enter)) {
        app.key_seq.clear();
        app.count_buf.clear();
        match app.view_mode {
            ViewMode::Windows => {
                if let Some(&idx) = app.win_filtered.get(app.selected) {
                    let win = &app.windows[idx];
                    hyprctl_dispatch(&["workspace", &win.workspace_id.to_string()]);
                    hyprctl_dispatch(&["focuswindow", &format!("address:{}", win.address)]);
                }
            }
            ViewMode::Apps => {
                if let Some(&idx) = app.app_filtered.get(app.selected) {
                    launch_app(&app.all_apps[idx].exec.clone());
                }
            }
        }
        return do_close(app);
    }

    // ── configurable key sequences ────────────────────────────────────────────

    // Digits accumulate into count_buf (only when no sequence is in progress,
    // so "3j" still works but "g3" doesn't start a count)
    if app.key_seq.is_empty() {
        if let keyboard::Key::Character(ref c) = key {
            if modifiers.is_empty() && c.len() == 1 {
                let ch = c.chars().next().unwrap();
                if ch.is_ascii_digit() && (!app.count_buf.is_empty() || ch != '0') {
                    app.count_buf.push(ch);
                    return Command::none();
                }
            }
        }
    }

    let Some(token) = key_token(&key, modifiers) else {
        app.key_seq.clear();
        app.count_buf.clear();
        return Command::none();
    };

    app.key_seq.push(token);

    let cfg = get();
    let kb = &cfg.keybinds;

    if let Some(action) = match_exact(&app.key_seq, kb) {
        app.key_seq.clear();
        return execute_action(app, action);
    }

    if is_prefix(&app.key_seq, kb) {
        // Partial match — show pending in status bar and wait for next key
        return Command::none();
    }

    // No match — discard
    app.key_seq.clear();
    app.count_buf.clear();
    Command::none()
}

// ── view ─────────────────────────────────────────────────────────────────────

pub fn view(app: &App, id: WindowId) -> Element<'_, Message> {
    {
        let mut ids = app.known_ids.borrow_mut();
        if !ids.contains(&id) {
            ids.push(id);
        }
    }

    if !app.visible {
        return container(column![])
            .width(Length::Fixed(1.0))
            .height(Length::Fixed(1.0))
            .into();
    }

    let s = &get().style;
    let fg = parse_color(&s.text_color);
    let dim = parse_color(&s.statusbar_text);
    let sel_bg = parse_color(&s.button_selected_background);
    let sel_border = parse_color(&s.button_selected_border);
    let mono = iced::Font::MONOSPACE;
    let sz: f32 = 14.0;

    // Help screen replaces the list
    let list_content: Element<Message> = if app.show_help {
        let l = |t: &str| text(t.to_string()).size(sz).font(mono).color(dim);
        let h = |t: &str| text(t.to_string()).size(sz).font(mono).color(fg);
        let help = column![
            h("~ navigation ~"),
            l("  j / k ............. move down / up"),
            l("  gg / G ............ jump to top / bottom"),
            l("  Tab ............... switch APP / WIN view"),
            text(String::new()).size(4),
            h("~ search & commands ~"),
            l("  / Space ........... search / filter"),
            l("  : ................. command mode"),
            l("  Esc ............... cancel search/command"),
            text(String::new()).size(4),
            h("~ window view ~"),
            l("  Enter ............. go to window's workspace"),
            l("  Shift+Enter ....... move window here"),
            l("  t ................. move window to workspace"),
            text(String::new()).size(4),
            h("~ app view ~"),
            l("  Enter ............. launch app"),
            text(String::new()).size(4),
            h("~ quit ~"),
            l("  q ................. close"),
            l("  Esc ............... close"),
            l("  :q :wq :q! ........ close (command mode)"),
            text(String::new()).size(4),
            h("~ help ~"),
            l("  ? ................. show this help"),
            l("  :help :? .......... show this help"),
            text(String::new()).size(8),
            text("  press any key to dismiss".to_string()).size(sz).font(mono).color(parse_color(&s.placeholder_color)),
        ]
        .spacing(2)
        .padding([8, 16]);

        scrollable(help)
            .direction(scrollable::Direction::Vertical(
                scrollable::Scrollbar::new()
                    .width(4)
                    .scroller_width(4)
                    .margin(2),
            ))
            .style(scrollbar_style)
            .height(Length::Fill)
            .into()
    } else {
        use crate::config::LineNumbers;
        let line_nums = get().window.line_numbers;
        let max_chars = (get().window.width as usize).saturating_sub(100) / 8;
        let total = app.items_len();
        let num_width = if total > 0 { format!("{}", total).len() } else { 1 };

        let make_line_num = |i: usize, selected: bool| -> String {
            match line_nums {
                LineNumbers::Hidden => String::new(),
                LineNumbers::Absolute => format!("{:>w$} ", i + 1, w = num_width),
                LineNumbers::Relative => {
                    if selected {
                        format!("{:>w$} ", i + 1, w = num_width)
                    } else {
                        let rel = (i as isize - app.selected as isize).unsigned_abs();
                        format!("{:>w$} ", rel, w = num_width)
                    }
                }
            }
        };

        let make_row = |i: usize, label: String| -> Element<Message> {
            let selected = i == app.selected;
            let cursor = if selected { ">" } else { " " };
            let ln = make_line_num(i, selected);
            let line_num_color = if selected { fg } else { dim };

            let content = row![
                text(format!(" {}", ln)).size(sz).font(mono).color(
                    if selected { parse_color(&s.statusbar_mode_normal) } else { line_num_color }
                ),
                text(format!("{} {}", cursor, label)).size(sz).font(mono).color(
                    if selected { fg } else { dim }
                ),
            ];

            if selected {
                container(content)
                    .width(Length::Fill)
                    .padding([3, 0])
                    .style(move |_theme: &iced::Theme| container::Style {
                        background: Some(iced::Background::Color(sel_bg)),
                        border: iced::Border {
                            color: sel_border,
                            width: 0.0,
                            radius: 0.0.into(),
                        },
                        ..Default::default()
                    })
                    .into()
            } else {
                container(content)
                    .width(Length::Fill)
                    .padding([3, 0])
                    .into()
            }
        };

        let items: Vec<Element<Message>> = match app.view_mode {
            ViewMode::Windows => app
                .win_filtered
                .iter()
                .enumerate()
                .map(|(i, &idx)| {
                    let w = &app.windows[idx];
                    let ws = if w.workspace_id < 0 { "sp".to_string() } else { w.workspace_id.to_string() };
                    let label = truncate(&format!("[{}] {} | {}", ws, w.class, w.title), max_chars);
                    make_row(i, label)
                })
                .collect(),
            ViewMode::Apps => app
                .app_filtered
                .iter()
                .enumerate()
                .map(|(i, &idx)| {
                    let a = &app.all_apps[idx];
                    let label = truncate(&a.name, max_chars);
                    make_row(i, label)
                })
                .collect(),
        };

        scrollable(
            iced::widget::Column::with_children(items)
                .spacing(0)
                .width(Length::Fill),
        )
        .id(iced::widget::Id::new(SCROLLABLE_ID))
        .direction(scrollable::Direction::Vertical(
            scrollable::Scrollbar::new()
                .width(4)
                .scroller_width(4)
                .margin(2),
        ))
        .style(scrollbar_style)
        .height(Length::Fill)
        .into()
    };

    // Title bar — like a terminal title
    let mode_label = match app.view_mode {
        ViewMode::Windows => "WIN",
        ViewMode::Apps => "APP",
    };
    let count = app.items_len();

    let title_line = text(format!(
        "─── {} ({}) ───",
        mode_label, count
    ))
    .size(sz)
    .font(mono)
    .color(dim);

    // Status line — vim-style at bottom
    let status_left = if app.show_help {
        text("HELP").size(sz).font(mono).color(parse_color(&s.statusbar_mode_normal))
    } else {
        match app.vim_mode {
            VimMode::Search => text(format!("/{}", app.query))
                .size(sz)
                .font(mono)
                .color(parse_color(&s.statusbar_mode_search)),
            VimMode::Command => text(format!(":{}", app.cmd))
                .size(sz)
                .font(mono)
                .color(parse_color(&s.statusbar_mode_command)),
            VimMode::MoveWindow => text(format!("move to ws: {}", app.move_buf))
                .size(sz)
                .font(mono)
                .color(parse_color(&s.statusbar_mode_command)),
            VimMode::Normal => text("-- NORMAL --")
                .size(sz)
                .font(mono)
                .color(parse_color(&s.statusbar_mode_normal)),
        }
    };

    let status_right = if app.show_help {
        text("? to toggle").size(sz).font(mono).color(dim)
    } else if !app.key_seq.is_empty() {
        text(app.key_seq.join("")).size(sz).font(mono).color(parse_color(&s.statusbar_mode_normal))
    } else {
        text(format!(
            "{}/{}",
            if count == 0 { 0 } else { app.selected + 1 },
            count
        ))
        .size(sz)
        .font(mono)
        .color(dim)
    };

    let status_bar = row![
        text(" ").size(sz),
        status_left,
        iced::widget::Space::new().width(Length::Fill),
        status_right,
        text(" ").size(sz),
    ]
    .align_y(Alignment::Center);

    let content = column![title_line, list_content, status_bar]
        .spacing(2)
        .padding([8, 12])
        .width(Length::Fill)
        .height(Length::Fill);

    container(content)
        .style(container_style)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

// ── subscription ─────────────────────────────────────────────────────────────

pub fn subscription(_app: &App) -> iced::Subscription<Message> {
    let event_sub = event::listen().map(Message::IcedEvent);

    if matches!(run_mode(), RunMode::Normal) {
        return event_sub;
    }

    let daemon_sub = iced::Subscription::run(|| {
        iced::stream::channel(
            1,
            |mut output: iced::futures::channel::mpsc::Sender<Message>| async move {
                loop {
                    let (tx, rx) = iced::futures::channel::oneshot::channel();
                    std::thread::spawn(move || {
                        let cmd = DAEMON_RX
                            .get()
                            .and_then(|rx| rx.lock().unwrap().recv().ok());
                        let _ = tx.send(cmd);
                    });
                    if let Ok(Some(cmd)) = rx.await {
                        let msg = match cmd {
                            DaemonCommand::ShowWindows => Message::ShowWindows,
                            DaemonCommand::ShowApps => Message::ShowApps,
                            DaemonCommand::Reload => Message::Reload,
                        };
                        let _ = output.send(msg).await;
                    }
                }
            },
        )
    });

    iced::Subscription::batch([event_sub, daemon_sub])
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let truncated: String = s.chars().take(max.saturating_sub(1)).collect();
        format!("{}…", truncated)
    }
}
