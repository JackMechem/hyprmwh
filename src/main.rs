mod app;
mod config;
mod daemon;
mod data;
mod styles;

use app::{RunMode, namespace, new, subscription, update, view};
use iced_layershell::build_pattern::daemon;
use iced_layershell::reexport::{KeyboardInteractivity, Layer};
use iced_layershell::settings::{LayerShellSettings, Settings, StartMode};
use styles::window_style;

struct Args {
    help: bool,
    daemon_mode: bool,
    show_windows: bool,
    show_apps: bool,
    reload: bool,
    config: Option<String>,
}

fn parse_args() -> Args {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let mut args = Args {
        help: false,
        daemon_mode: false,
        show_windows: false,
        show_apps: false,
        reload: false,
        config: None,
    };
    let mut i = 0;
    while i < argv.len() {
        match argv[i].as_str() {
            "--help" | "-h" => args.help = true,
            "--daemon" | "-d" => args.daemon_mode = true,
            "--windows" | "-w" => args.show_windows = true,
            "--apps" | "-a" => args.show_apps = true,
            "--reload" | "-r" => args.reload = true,
            "--config" | "-c" => {
                i += 1;
                match argv.get(i) {
                    Some(p) => args.config = Some(p.clone()),
                    None => {
                        eprintln!("Error: --config requires a path argument");
                        std::process::exit(1);
                    }
                }
            }
            other => {
                eprintln!("Unknown argument: {other}");
                eprintln!("Run with --help for usage.");
                std::process::exit(1);
            }
        }
        i += 1;
    }
    args
}

fn print_help() {
    println!(
        "Usage: hyprmwh [OPTIONS]

Options:
  -h, --help             Show this help message
  -c, --config PATH      Custom config file
  -d, --daemon           Run as background daemon
  -w, --windows          Show window switcher (signals daemon if running)
  -a, --apps             Show app launcher (signals daemon if running)
  -r, --reload           Tell daemon to reload config and app list

Daemon mode:
  hyprmwh --daemon       Start daemon in background
  hyprmwh --windows      Signal daemon to show window switcher
  hyprmwh --apps         Signal daemon to show app launcher

Standalone (no daemon):
  hyprmwh                Opens window switcher, exits after use

Hyprland keybind examples:
  bind = $mainMod, W, exec, hyprmwh --windows
  bind = $mainMod, R, exec, hyprmwh --apps"
    );
}

fn main() -> Result<(), iced_layershell::Error> {
    let args = parse_args();

    if args.help {
        print_help();
        return Ok(());
    }

    if let Some(path) = args.config {
        config::set_path(path);
    }

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();

    if args.reload {
        if !rt.block_on(daemon::try_send("reload")) {
            eprintln!("Error: no daemon running. Start one with --daemon.");
            std::process::exit(1);
        }
        return Ok(());
    }

    if args.show_windows {
        if rt.block_on(daemon::is_running()) {
            rt.block_on(daemon::try_send("windows"));
            return Ok(());
        }
        // No daemon: run one-shot with windows view
        app::DEFAULT_VIEW.set(app::ViewMode::Windows).ok();
        app::RUN_MODE.set(RunMode::Normal).ok();
    } else if args.show_apps {
        if rt.block_on(daemon::is_running()) {
            rt.block_on(daemon::try_send("apps"));
            return Ok(());
        }
        // No daemon: run one-shot with apps view
        app::DEFAULT_VIEW.set(app::ViewMode::Apps).ok();
        app::RUN_MODE.set(RunMode::Normal).ok();
    } else if args.daemon_mode {
        if rt.block_on(daemon::is_running()) {
            eprintln!("Error: daemon is already running.");
            std::process::exit(1);
        }
        app::RUN_MODE.set(RunMode::Daemon).ok();
    } else {
        // No flags: one-shot app launcher (default)
        app::RUN_MODE.set(RunMode::Normal).ok();
    }

    let cfg = config::get();

    let (initial_size, initial_keyboard) = match app::run_mode() {
        RunMode::Normal => ((cfg.window.width, 500), KeyboardInteractivity::Exclusive),
        RunMode::Daemon => ((1, 1), KeyboardInteractivity::None),
    };

    daemon(new, namespace, update, view)
        .style(window_style)
        .subscription(subscription)
        .settings(Settings {
            layer_settings: LayerShellSettings {
                size: Some(initial_size),
                anchor: cfg.window.anchor.to_anchor(),
                margin: cfg.window.anchor.to_margin(cfg.window.margin),
                exclusive_zone: 0,
                layer: Layer::Overlay,
                keyboard_interactivity: initial_keyboard,
                start_mode: StartMode::AllScreens,
                ..Default::default()
            },
            ..Default::default()
        })
        .run()
}
