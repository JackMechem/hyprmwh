use std::cell::RefCell;
use std::sync::{OnceLock, mpsc};
use std::time::Instant;

use iced::futures::SinkExt;
use iced::keyboard::key::Named;
use iced::widget::{button, column, container, row, scrollable, text};
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
use crate::styles::{button_style, button_style_selected, container_style, statusbar_style};

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

    // double-escape tracking
    pub last_esc: Option<Instant>,
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
        self.selected = 0;
        self.vim_mode = VimMode::Normal;
        self.show_help = false;
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
        last_esc: None,
    };

    // Pre-filter both
    state.view_mode = ViewMode::Apps;
    state.filter();
    state.view_mode = dv;

    (state, cmd)
}

// ── layer shell helpers ──────────────────────────────────────────────────────

const WINDOW_HEIGHT: u32 = 500;

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
                return Command::none();
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
                        Command::none()
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

        VimMode::Normal => handle_normal(app, key, modifiers),
    }
}

fn handle_normal(
    app: &mut App,
    key: keyboard::Key,
    modifiers: keyboard::Modifiers,
) -> Command<Message> {
    let max = app.items_len().saturating_sub(1);

    match key {
        // Double Escape = close/hide
        keyboard::Key::Named(Named::Escape) => {
            let now = Instant::now();
            if let Some(prev) = app.last_esc {
                if now.duration_since(prev).as_millis() < 500 {
                    app.last_esc = None;
                    return do_close(app);
                }
            }
            app.last_esc = Some(now);
            Command::none()
        }

        // ? = help
        keyboard::Key::Character(ref c) if (c == "?" || (c == "/" && modifiers.shift())) => {
            app.show_help = true;
            Command::none()
        }

        // Tab = switch view
        keyboard::Key::Named(Named::Tab) => {
            match app.view_mode {
                ViewMode::Windows => {
                    app.view_mode = ViewMode::Apps;
                }
                ViewMode::Apps => {
                    app.view_mode = ViewMode::Windows;
                    app.windows = load_windows();
                }
            }
            app.reset_state();
            app.filter();
            Command::none()
        }

        // Shift+Enter (windows only) = move window to current workspace
        keyboard::Key::Named(Named::Enter) if modifiers.shift() => {
            if app.view_mode == ViewMode::Windows {
                if let Some(&idx) = app.win_filtered.get(app.selected) {
                    let win = &app.windows[idx];
                    if let Some(ws_id) = hyprctl_active_workspace_id() {
                        hyprctl_dispatch(&[
                            "movetoworkspace",
                            &format!("{},address:{}", ws_id, win.address),
                        ]);
                        hyprctl_dispatch(&[
                            "focuswindow",
                            &format!("address:{}", win.address),
                        ]);
                    }
                }
            }
            do_close(app)
        }

        // Enter = select
        keyboard::Key::Named(Named::Enter) => {
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
            do_close(app)
        }

        // q = quit
        keyboard::Key::Character(ref c) if c == "q" && modifiers.is_empty() => do_close(app),

        // j = down
        keyboard::Key::Character(ref c) if c == "j" && modifiers.is_empty() => {
            if app.selected < max {
                app.selected += 1;
            }
            Command::none()
        }
        // k = up
        keyboard::Key::Character(ref c) if c == "k" && modifiers.is_empty() => {
            if app.selected > 0 {
                app.selected -= 1;
            }
            Command::none()
        }
        // g = top
        keyboard::Key::Character(ref c) if c == "g" && modifiers.is_empty() => {
            app.selected = 0;
            Command::none()
        }
        // G = bottom
        keyboard::Key::Character(ref c) if (c == "G" || (c == "g" && modifiers.shift())) => {
            app.selected = max;
            Command::none()
        }
        // / = search
        keyboard::Key::Character(ref c) if c == "/" && modifiers.is_empty() => {
            app.query.clear();
            app.filter();
            app.vim_mode = VimMode::Search;
            Command::none()
        }
        // : = command
        keyboard::Key::Character(ref c) if (c == ":" || (c == ";" && modifiers.shift())) => {
            app.cmd.clear();
            app.vim_mode = VimMode::Command;
            Command::none()
        }
        _ => Command::none(),
    }
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

    // Help screen replaces the list
    let list_content: Element<Message> = if app.show_help {
        let help = column![
            text("Keybinds").size(18).color(parse_color(&s.text_color)),
            text("").size(8),
            text("j / k          move down / up").size(13).color(parse_color(&s.statusbar_text)),
            text("g / G          jump to top / bottom").size(13).color(parse_color(&s.statusbar_text)),
            text("Enter          go to window's workspace / launch app").size(13).color(parse_color(&s.statusbar_text)),
            text("Shift+Enter    move window here (WIN)").size(13).color(parse_color(&s.statusbar_text)),
            text("/              search / filter").size(13).color(parse_color(&s.statusbar_text)),
            text("?              show this help").size(13).color(parse_color(&s.statusbar_text)),
            text("Tab            switch APP / WIN view").size(13).color(parse_color(&s.statusbar_text)),
            text("q              close").size(13).color(parse_color(&s.statusbar_text)),
            text(":q :wq :q!     close (command mode)").size(13).color(parse_color(&s.statusbar_text)),
            text(":help :?       show this help").size(13).color(parse_color(&s.statusbar_text)),
            text("Esc            cancel search/command").size(13).color(parse_color(&s.statusbar_text)),
            text("").size(12),
            text("Press any key to dismiss").size(12).color(parse_color(&s.placeholder_color)),
        ]
        .spacing(3)
        .padding(16);

        scrollable(help).height(Length::Fill).into()
    } else {
        // Approximate max chars that fit in the window width
        let max_chars = (get().window.width as usize).saturating_sub(80) / 8;

        let items: Vec<Element<Message>> = match app.view_mode {
            ViewMode::Windows => app
                .win_filtered
                .iter()
                .enumerate()
                .map(|(i, &idx)| {
                    let w = &app.windows[idx];
                    let label = truncate(&format!("{} — {}", w.class, w.title), max_chars);
                    let style = if i == app.selected {
                        button_style_selected
                            as fn(&iced::Theme, button::Status) -> button::Style
                    } else {
                        button_style
                    };
                    button(text(label))
                        .style(style)
                        .width(Length::Fill)
                        .padding([10, 20])
                        .into()
                })
                .collect(),
            ViewMode::Apps => app
                .app_filtered
                .iter()
                .enumerate()
                .map(|(i, &idx)| {
                    let a = &app.all_apps[idx];
                    let label = truncate(&a.name, max_chars);
                    let style = if i == app.selected {
                        button_style_selected
                            as fn(&iced::Theme, button::Status) -> button::Style
                    } else {
                        button_style
                    };
                    button(text(label))
                        .style(style)
                        .width(Length::Fill)
                        .padding([10, 20])
                        .into()
                })
                .collect(),
        };

        scrollable(
            iced::widget::Column::with_children(items)
                .spacing(4)
                .width(Length::Fill),
        )
        .height(Length::Fill)
        .into()
    };

    // Status bar
    let mode_label = match app.view_mode {
        ViewMode::Windows => "WIN",
        ViewMode::Apps => "APP",
    };

    let status_left = if app.show_help {
        text("HELP").size(14).color(parse_color(&s.statusbar_mode_normal))
    } else {
        match app.vim_mode {
            VimMode::Search => text(format!("/{}", app.query))
                .size(14)
                .color(parse_color(&s.statusbar_mode_search)),
            VimMode::Command => text(format!(":{}", app.cmd))
                .size(14)
                .color(parse_color(&s.statusbar_mode_command)),
            VimMode::Normal => text(format!("[{}] NORMAL", mode_label))
                .size(14)
                .color(parse_color(&s.statusbar_mode_normal)),
        }
    };

    let count = app.items_len();
    let status_right = if app.show_help {
        text("? to toggle").size(14).color(parse_color(&s.statusbar_text))
    } else {
        text(format!(
            "{}/{}",
            if count == 0 { 0 } else { app.selected + 1 },
            count
        ))
        .size(14)
        .color(parse_color(&s.statusbar_text))
    };

    let status_bar = container(
        row![
            status_left,
            iced::widget::Space::new().width(Length::Fill),
            status_right
        ]
        .padding([4, 12])
        .align_y(Alignment::Center),
    )
    .width(Length::Fill)
    .style(statusbar_style);

    let content = column![list_content, status_bar]
        .spacing(4)
        .padding(12)
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
