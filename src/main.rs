use std::env;
use std::process::Command;

use iced::event::Event;
use iced::keyboard::{self, Key};
use iced::widget::{column, container, row, scrollable, text, Column};
use iced::{Color, Element, Length, Task};
use iced_layershell::build_pattern::application;
use iced_layershell::reexport::{Anchor, KeyboardInteractivity, Layer};
use iced_layershell::settings::{LayerShellSettings, StartMode};
use iced_layershell::to_layer_message;
use iced_layershell::Settings;
use serde::Deserialize;

fn is_demo() -> bool {
    env::args().any(|a| a == "--demo")
}

#[derive(Debug, Deserialize)]
struct HyprWorkspace {
    id: i64,
}

#[derive(Debug, Deserialize)]
struct HyprClient {
    address: String,
    class: String,
    title: String,
    workspace: HyprWorkspace,
}

fn hyprctl_clients() -> Vec<HyprClient> {
    Command::new("hyprctl")
        .args(["clients", "-j"])
        .output()
        .ok()
        .and_then(|o| serde_json::from_slice(&o.stdout).ok())
        .unwrap_or_default()
}

fn hyprctl_active_workspace_id() -> Option<i64> {
    let output = Command::new("hyprctl")
        .args(["activeworkspace", "-j"])
        .output()
        .ok()?;
    let val: serde_json::Value = serde_json::from_slice(&output.stdout).ok()?;
    val.get("id")?.as_i64()
}

fn hyprctl_dispatch(args: &[&str]) {
    let _ = Command::new("hyprctl").arg("dispatch").args(args).output();
}

fn demo_windows() -> Vec<WindowInfo> {
    vec![
        WindowInfo { title: "Welcome to Hyprland".into(), class: "kitty".into(), address: "0x1".into(), workspace_id: 1 },
        WindowInfo { title: "main.rs - hyprmwh".into(), class: "neovim".into(), address: "0x2".into(), workspace_id: 1 },
        WindowInfo { title: "GitHub - Mozilla Firefox".into(), class: "firefox".into(), address: "0x3".into(), workspace_id: 2 },
        WindowInfo { title: "Spotify Premium".into(), class: "spotify".into(), address: "0x4".into(), workspace_id: 3 },
        WindowInfo { title: "Discord".into(), class: "discord".into(), address: "0x5".into(), workspace_id: 2 },
        WindowInfo { title: "Signal".into(), class: "signal".into(), address: "0x6".into(), workspace_id: 3 },
        WindowInfo { title: "htop".into(), class: "kitty".into(), address: "0x7".into(), workspace_id: 1 },
        WindowInfo { title: "nix develop".into(), class: "foot".into(), address: "0x8".into(), workspace_id: 4 },
    ]
}

fn main() -> Result<(), iced_layershell::Error> {
    application(App::new, namespace, update, view)
        .style(style)
        .subscription(subscription)
        .settings(Settings {
            layer_settings: LayerShellSettings {
                size: Some((600, 500)),
                exclusive_zone: -1,
                anchor: Anchor::empty(),
                layer: Layer::Overlay,
                keyboard_interactivity: KeyboardInteractivity::Exclusive,
                margin: (0, 0, 0, 0),
                start_mode: StartMode::Active,
                events_transparent: false,
            },
            ..Default::default()
        })
        .run()
}

fn namespace() -> String {
    "hyprmwh".into()
}

#[derive(Debug, Clone)]
struct WindowInfo {
    title: String,
    class: String,
    address: String,
    workspace_id: i64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Mode {
    Normal,
    Search,
    Command,
}

struct App {
    query: String,
    cmd: String,
    windows: Vec<WindowInfo>,
    filtered: Vec<usize>,
    selected: usize,
    visible: bool,
    demo: bool,
    mode: Mode,
}

impl App {
    fn new() -> (Self, Task<Message>) {
        let demo = is_demo();
        let mut app = Self {
            query: String::new(),
            cmd: String::new(),
            windows: Vec::new(),
            filtered: Vec::new(),
            selected: 0,
            visible: true,
            demo,
            mode: Mode::Normal,
        };
        app.refresh_windows();
        app.filter();
        (app, Task::none())
    }

    fn refresh_windows(&mut self) {
        if self.demo {
            self.windows = demo_windows();
            return;
        }
        self.windows = hyprctl_clients()
            .into_iter()
            .filter(|c| !c.title.is_empty() && c.class != "hyprmwh")
            .map(|c| WindowInfo {
                title: c.title,
                class: c.class,
                address: c.address,
                workspace_id: c.workspace.id,
            })
            .collect();
    }

    fn filter(&mut self) {
        let q = self.query.to_lowercase();
        self.filtered = self
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
        if self.selected >= self.filtered.len() {
            self.selected = self.filtered.len().saturating_sub(1);
        }
    }

    fn move_selected_to_current_workspace(&self) {
        if self.demo {
            return;
        }
        if let Some(&idx) = self.filtered.get(self.selected) {
            let win = &self.windows[idx];
            if let Some(ws_id) = hyprctl_active_workspace_id() {
                hyprctl_dispatch(&[
                    "movetoworkspace",
                    &format!("{},address:{}", ws_id, win.address),
                ]);
                hyprctl_dispatch(&["focuswindow", &format!("address:{}", win.address)]);
            }
        }
    }

    fn goto_selected_workspace(&self) {
        if self.demo {
            return;
        }
        if let Some(&idx) = self.filtered.get(self.selected) {
            let win = &self.windows[idx];
            hyprctl_dispatch(&["workspace", &win.workspace_id.to_string()]);
            hyprctl_dispatch(&["focuswindow", &format!("address:{}", win.address)]);
        }
    }

    fn hide(&mut self) -> Task<Message> {
        std::process::exit(0);
    }
}

#[to_layer_message]
#[derive(Debug, Clone)]
enum Message {
    Toggle,
    IcedEvent(Event),
}

fn subscription(_app: &App) -> iced::Subscription<Message> {
    iced::event::listen().map(Message::IcedEvent)
}

fn update(app: &mut App, message: Message) -> Task<Message> {
    match message {
        Message::Toggle => {
            app.visible = !app.visible;
            if app.visible {
                app.refresh_windows();
                app.query.clear();
                app.filter();
                app.mode = Mode::Normal;
                Task::done(Message::KeyboardInteractivityChange(
                    KeyboardInteractivity::Exclusive,
                ))
            } else {
                app.hide()
            }
        }
        Message::IcedEvent(Event::Keyboard(keyboard::Event::KeyPressed {
            key, modifiers, ..
        })) => {
            if !app.visible {
                return Task::none();
            }

            match app.mode {
                Mode::Search => match key {
                    Key::Named(keyboard::key::Named::Escape) => {
                        app.query.clear();
                        app.filter();
                        app.mode = Mode::Normal;
                        Task::none()
                    }
                    Key::Named(keyboard::key::Named::Enter) => {
                        app.mode = Mode::Normal;
                        Task::none()
                    }
                    Key::Named(keyboard::key::Named::Backspace) => {
                        app.query.pop();
                        app.filter();
                        Task::none()
                    }
                    Key::Character(ref c) if !modifiers.control() && !modifiers.alt() => {
                        app.query.push_str(c);
                        app.filter();
                        Task::none()
                    }
                    _ => Task::none(),
                },
                Mode::Command => match key {
                    Key::Named(keyboard::key::Named::Escape) => {
                        app.cmd.clear();
                        app.mode = Mode::Normal;
                        Task::none()
                    }
                    Key::Named(keyboard::key::Named::Enter) => {
                        let cmd = app.cmd.trim().to_string();
                        app.cmd.clear();
                        app.mode = Mode::Normal;
                        match cmd.as_str() {
                            "q" | "wq" | "q!" => app.hide(),
                            _ => Task::none(),
                        }
                    }
                    Key::Named(keyboard::key::Named::Backspace) => {
                        app.cmd.pop();
                        if app.cmd.is_empty() {
                            app.mode = Mode::Normal;
                        }
                        Task::none()
                    }
                    Key::Character(ref c) if !modifiers.control() && !modifiers.alt() => {
                        app.cmd.push_str(c);
                        Task::none()
                    }
                    _ => Task::none(),
                },
                Mode::Normal => {
                    let max = app.filtered.len().saturating_sub(1);
                    match key {
                        // Shift+Enter = go to window's workspace
                        Key::Named(keyboard::key::Named::Enter) if modifiers.shift() => {
                            if !app.filtered.is_empty() {
                                app.goto_selected_workspace();
                            }
                            app.hide()
                        }
                        // Enter = bring window here
                        Key::Named(keyboard::key::Named::Enter) => {
                            if !app.filtered.is_empty() {
                                app.move_selected_to_current_workspace();
                            }
                            app.hide()
                        }
                        // q = quit
                        Key::Character(ref c) if c == "q" && modifiers.is_empty() => {
                            app.hide()
                        }
                        // j = down
                        Key::Character(ref c) if c == "j" && modifiers.is_empty() => {
                            if app.selected < max {
                                app.selected += 1;
                            }
                            Task::none()
                        }
                        // k = up
                        Key::Character(ref c) if c == "k" && modifiers.is_empty() => {
                            if app.selected > 0 {
                                app.selected -= 1;
                            }
                            Task::none()
                        }
                        // g = top
                        Key::Character(ref c) if c == "g" && modifiers.is_empty() => {
                            app.selected = 0;
                            Task::none()
                        }
                        // G = bottom
                        Key::Character(ref c)
                            if (c == "G" || (c == "g" && modifiers.shift())) =>
                        {
                            app.selected = max;
                            Task::none()
                        }
                        // / = search
                        Key::Character(ref c) if c == "/" && modifiers.is_empty() => {
                            app.query.clear();
                            app.filter();
                            app.mode = Mode::Search;
                            Task::none()
                        }
                        // : = command
                        Key::Character(ref c)
                            if (c == ":" || (c == ";" && modifiers.shift())) =>
                        {
                            app.cmd.clear();
                            app.mode = Mode::Command;
                            Task::none()
                        }
                        _ => Task::none(),
                    }
                }
            }
        }
        _ => Task::none(),
    }
}

fn view(app: &App) -> Element<'_, Message> {
    if !app.visible {
        return container(text(""))
            .width(Length::Fill)
            .height(Length::Fill)
            .into();
    }

    // Window list
    let items: Vec<Element<Message>> = app
        .filtered
        .iter()
        .enumerate()
        .map(|(i, &win_idx)| {
            let w = &app.windows[win_idx];
            let is_selected = i == app.selected;
            let label = format!("{} — {}", w.class, w.title);

            let bg = if is_selected {
                Color::from_rgba(0.3, 0.5, 0.8, 0.8)
            } else {
                Color::TRANSPARENT
            };

            container(text(label).size(16).color(Color::WHITE))
                .width(Length::Fill)
                .padding(10)
                .style(move |_theme: &iced::Theme| container::Style {
                    background: Some(bg.into()),
                    border: iced::Border {
                        radius: 8.0.into(),
                        ..Default::default()
                    },
                    ..Default::default()
                })
                .into()
        })
        .collect();

    let list = scrollable(Column::with_children(items).spacing(2)).height(Length::Fill);

    // Status bar
    let status_left = match app.mode {
        Mode::Search => text(format!("/{}", app.query))
            .size(14)
            .color(Color::WHITE),
        Mode::Command => text(format!(":{}", app.cmd))
            .size(14)
            .color(Color::WHITE),
        Mode::Normal => {
            let demo_str = if app.demo { " DEMO" } else { "" };
            text(format!("NORMAL{}", demo_str))
                .size(14)
                .color(Color::from_rgb(0.5, 0.8, 0.5))
        }
    };

    let status_right = text(format!(
        "{}/{}",
        if app.filtered.is_empty() {
            0
        } else {
            app.selected + 1
        },
        app.filtered.len()
    ))
    .size(14)
    .color(Color::from_rgb(0.6, 0.6, 0.6));

    let status_bar = container(
        row![status_left, iced::widget::Space::new().width(Length::Fill), status_right]
            .padding([4, 8])
            .align_y(iced::Alignment::Center),
    )
    .width(Length::Fill)
    .style(|_theme: &iced::Theme| container::Style {
        background: Some(Color::from_rgba(0.08, 0.08, 0.1, 1.0).into()),
        border: iced::Border {
            radius: 8.0.into(),
            ..Default::default()
        },
        ..Default::default()
    });

    let content = column![list, status_bar].spacing(0).padding(8);

    container(content)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(|_theme: &iced::Theme| container::Style {
            background: Some(Color::from_rgba(0.12, 0.12, 0.15, 0.75).into()),
            border: iced::Border {
                color: Color::from_rgb(0.3, 0.3, 0.4),
                width: 1.0,
                radius: 12.0.into(),
            },
            ..Default::default()
        })
        .into()
}

fn style(_app: &App, _theme: &iced::Theme) -> iced::theme::Style {
    iced::theme::Style {
        background_color: Color::TRANSPARENT,
        text_color: Color::WHITE,
    }
}
