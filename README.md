# hyprmwh

A terminal-styled window switcher and app launcher for [Hyprland](https://hyprland.org/), with vim keybinds.

Built with [iced](https://iced.rs/) and [iced_layershell](https://github.com/waycrate/exwlshelleventloop) as a Wayland overlay. Designed for NixOS but works on any system running Hyprland.

## Features

- Vim-style navigation (`j`/`k`/`g`/`G`/`/`/`:`)
- Window switcher: jump to a window's workspace or bring it to yours
- App launcher: launch any XDG desktop application
- Relative line numbers (configurable: relative, absolute, or hidden)
- Daemon mode for instant popup via keybind
- Transparent overlay with terminal-style UI
- Fully configurable colors and layout via TOML
- Tab to switch between APP and WIN views without closing

## Installation (NixOS Flake)

### 1. Add the flake input

In your NixOS or home-manager `flake.nix`:

```nix
{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    hyprmwh.url = "github:YOUR_USERNAME/hyprmwh";
    # ...
  };
}
```

### 2. Add to your packages

With home-manager:

```nix
{ inputs, pkgs, ... }:
{
  home.packages = [
    inputs.hyprmwh.packages.${pkgs.system}.default
  ];
}
```

Or in a NixOS system configuration:

```nix
{ inputs, pkgs, ... }:
{
  environment.systemPackages = [
    inputs.hyprmwh.packages.${pkgs.system}.default
  ];
}
```

### 3. Set up Hyprland keybinds

In your Hyprland config (`hyprland.conf` or via home-manager):

```bash
# Start the daemon on login
exec-once = hyprmwh --daemon

# Keybinds to show the switcher/launcher
bind = $mainMod, R, exec, hyprmwh --apps
bind = $mainMod, W, exec, hyprmwh --windows
```

With home-manager's Hyprland module:

```nix
wayland.windowManager.hyprland.settings = {
  exec-once = [ "hyprmwh --daemon" ];
  bind = [
    "$mainMod, R, exec, hyprmwh --apps"
    "$mainMod, W, exec, hyprmwh --windows"
  ];
};
```

## Usage

```
hyprmwh [OPTIONS]

Options:
  -h, --help             Show help message
  -c, --config PATH      Custom config file path
  -d, --daemon           Run as background daemon
  -w, --windows          Show window switcher
  -a, --apps             Show app launcher
  -r, --reload           Tell daemon to reload config and app list
```

### Daemon mode (recommended)

Start the daemon once, then signal it to show/hide:

```bash
hyprmwh --daemon        # start daemon in background
hyprmwh --apps          # signal daemon to show app launcher
hyprmwh --windows       # signal daemon to show window switcher
hyprmwh --reload        # reload config and refresh app list
```

### Standalone mode

Run without a daemon (opens and exits after use):

```bash
hyprmwh                 # opens app launcher (default)
hyprmwh --windows       # opens window switcher
```

## Keybinds

### Navigation

| Key | Action |
|-----|--------|
| `j` / `k` | Move down / up |
| `g` / `G` | Jump to top / bottom |
| `Tab` | Switch between APP and WIN views |

### Search & Commands

| Key | Action |
|-----|--------|
| `/` or `Space` | Start searching / filtering |
| `:` | Enter command mode |
| `Esc` | Cancel search or command |

### Window View

| Key | Action |
|-----|--------|
| `Enter` | Go to the window's workspace and focus it |
| `Shift+Enter` | Move the window to your current workspace |

### App View

| Key | Action |
|-----|--------|
| `Enter` | Launch the selected app |

### Quit

| Key | Action |
|-----|--------|
| `q` | Close |
| `Esc Esc` | Double-tap Escape to close |
| `:q` `:wq` `:q!` | Close via command mode |

### Help

| Key | Action |
|-----|--------|
| `?` | Show help screen |
| `:help` or `:?` | Show help screen |

## Configuration

Config file location: `~/.config/hyprmwh/config.toml`

Use `--config /path/to/config.toml` to specify a custom path.

### Full example

```toml
[window]
anchor = "center"         # center | top | bottom | left | right
width  = 600
margin = 0                # px from anchored edge (ignored for center)
line_numbers = "relative" # relative | absolute | hidden

[style]
# Window
container_background = "#26262ECC"
container_border     = "#FFFFFF1A"
container_radius     = 18.0

# Selection highlight
button_selected_background = "#405999FF"
button_selected_border     = "#6699FF99"

# Text
text_color        = "#FFFFFFFF"
placeholder_color = "#FFFFFF66"

# Status bar
statusbar_background   = "#1A1A22FF"
statusbar_text         = "#FFFFFF99"
statusbar_mode_normal  = "#80CC80FF"
statusbar_mode_search  = "#FFFFFFFF"
statusbar_mode_command = "#FFFFFFFF"
```

Colors are in `#RRGGBB` or `#RRGGBBAA` hex format. All fields are optional and fall back to defaults.

### Line numbers

- `relative` (default) -- selected line shows its absolute number, all others show their distance from the selection (like `set relativenumber` in vim)
- `absolute` -- all lines show their 1-based index
- `hidden` -- no line numbers

## Building from source

```bash
nix build            # build with flake
nix develop          # enter dev shell with rust toolchain
cargo build          # build manually (requires wayland libs)
```

## License

MIT
