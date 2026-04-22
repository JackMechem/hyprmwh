use std::path::PathBuf;
use std::process::Command;

use serde::Deserialize;

// ── desktop apps ─────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct DesktopApp {
    pub name: String,
    pub exec: String,
}

pub fn load_apps() -> Vec<DesktopApp> {
    let data_dirs = std::env::var("XDG_DATA_DIRS")
        .unwrap_or_else(|_| "/usr/share:/run/current-system/sw/share".to_string());

    let mut apps = Vec::new();

    for dir in data_dirs.split(':') {
        let path = PathBuf::from(dir).join("applications");
        let entries = match std::fs::read_dir(&path) {
            Ok(e) => e,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("desktop") {
                continue;
            }
            if let Some(app) = parse_desktop_file(&path) {
                apps.push(app);
            }
        }
    }

    apps.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    apps.dedup_by(|a, b| a.name == b.name);
    apps
}

fn parse_desktop_file(path: &PathBuf) -> Option<DesktopApp> {
    let content = std::fs::read_to_string(path).ok()?;
    let mut name = None;
    let mut exec = None;
    let mut no_display = false;
    let mut in_desktop_entry = false;

    for line in content.lines() {
        if line == "[Desktop Entry]" {
            in_desktop_entry = true;
            continue;
        }
        if line.starts_with('[') {
            in_desktop_entry = false;
            continue;
        }
        if !in_desktop_entry {
            continue;
        }
        if let Some(val) = line.strip_prefix("Name=") {
            name.get_or_insert_with(|| val.to_string());
        } else if let Some(val) = line.strip_prefix("Exec=") {
            exec.get_or_insert_with(|| clean_exec(val));
        } else if line == "NoDisplay=true" || line == "Type=Directory" {
            no_display = true;
        }
    }

    if no_display {
        return None;
    }

    Some(DesktopApp {
        name: name?,
        exec: exec?,
    })
}

fn clean_exec(exec: &str) -> String {
    exec.split_whitespace()
        .filter(|s| !s.starts_with('%'))
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn launch_app(exec: &str) {
    let _ = Command::new("sh").arg("-c").arg(exec).spawn();
}

// ── hyprland windows ─────────────────────────────────────────────────────────

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

#[derive(Debug, Clone)]
pub struct WindowInfo {
    pub title: String,
    pub class: String,
    pub address: String,
    pub workspace_id: i64,
}

pub fn load_windows() -> Vec<WindowInfo> {
    let mut windows: Vec<WindowInfo> = Command::new("hyprctl")
        .args(["clients", "-j"])
        .output()
        .ok()
        .and_then(|o| serde_json::from_slice::<Vec<HyprClient>>(&o.stdout).ok())
        .unwrap_or_default()
        .into_iter()
        .filter(|c| !c.title.is_empty() && c.class != "hyprmwh")
        .map(|c| WindowInfo {
            title: c.title,
            class: c.class,
            address: c.address,
            workspace_id: c.workspace.id,
        })
        .collect();

    windows.sort_by_key(|w| w.workspace_id);
    windows
}

pub fn hyprctl_active_workspace_id() -> Option<i64> {
    let output = Command::new("hyprctl")
        .args(["activeworkspace", "-j"])
        .output()
        .ok()?;
    let val: serde_json::Value = serde_json::from_slice(&output.stdout).ok()?;
    val.get("id")?.as_i64()
}

pub fn hyprctl_dispatch(args: &[&str]) {
    let _ = Command::new("hyprctl").arg("dispatch").args(args).output();
}
