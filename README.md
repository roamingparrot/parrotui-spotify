# parrotui-spotify

A keyboard-driven Spotify client for the terminal with built-in playback, built with [ratatui](https://github.com/ratatui/ratatui) and [librespot](https://github.com/librespot-org/librespot).

Part of the **parrotui** family of TUI apps. More TUI clients are on the way — stay tuned.

![Rust](https://img.shields.io/badge/rust-stable-orange)
![License](https://img.shields.io/badge/license-MIT-blue)

## Features

- **Built-in playback** — registers as a Spotify Connect device, plays audio directly
- Play/pause, skip, seek, volume, shuffle, repeat
- Browse and play playlists and saved albums
- Liked songs library
- Customize what shows in the sidebar (playlists, albums, liked songs), and switch between separated sections or one combined list
- Theme system with multiple presets (cyan, spotify, dracula)
- In-app settings view with fully remappable keybindings
- Vim-style and arrow key navigation
- Now-playing bar with smooth local progress tracking
- Appears in Spotify's device list (can be controlled from phone/desktop too)
- Non-blocking async architecture — the UI never freezes during API calls

## Requirements

- A Spotify Premium account (required for streaming)
- Working audio output:
  - **Linux** — ALSA, PipeWire, or PulseAudio (PipeWire's ALSA compat layer works fine)
  - **macOS** — CoreAudio (built-in, nothing to install)
  - **Windows** — WASAPI (built-in, nothing to install)

## Setup

On first run, the app opens a browser window for Spotify OAuth (twice — once for Web API access, once for streaming credentials). Both are cached locally after the first login.

## Installation

### Quick install (macOS / Linux)

```sh
curl -fsSL https://raw.githubusercontent.com/roamingparrot/parrotui-spotify/main/install.sh | sh
```

This downloads the latest release binary for your platform and puts it in `/usr/local/bin`. To install elsewhere:

```sh
INSTALL_DIR=~/.local/bin curl -fsSL https://raw.githubusercontent.com/roamingparrot/parrotui-spotify/main/install.sh | sh
```

### From source

```sh
cargo install --git https://github.com/roamingparrot/parrotui-spotify
```

Or clone and build locally:

```sh
cargo install --path .
```

#### Build prerequisites

- **Linux** — `libasound2-dev libdbus-1-dev libsecret-1-dev pkg-config cmake`
- **macOS** — Xcode command line tools (`xcode-select --install`)
- **Windows** — Visual Studio Build Tools with C++ workload

### NixOS / Nix

```sh
nix run github:roamingparrot/parrotui-spotify
```

Or add to your flake inputs:

```nix
{
  inputs.parrotui-spotify.url = "github:roamingparrot/parrotui-spotify";
  # ...
  environment.systemPackages = [ inputs.parrotui-spotify.packages.${system}.default ];
}
```

A dev shell is provided:

```sh
nix develop
```

## Usage

```
parrotui-spotify [--config PATH]
```

### Keybindings

See [KEYBINDINGS.md](KEYBINDINGS.md) for the full list.

## Configuration

Press `,` for the settings view. Changes are written straight to
`~/.config/parrotui-spotify/config.toml`, which you can also edit by hand:

```toml
device_name = "parrotui-spotify"     # how it shows in Spotify Connect
initial_volume = 100                 # 0-100
theme = "default"                    # default, spotify, or dracula
volume_step = 5                      # how far + and - move the volume
seek_step_secs = 5                   # how far > and < seek
bitrate = 160                        # 96, 160 or 320 kbps
normalisation = false                # even out volume between tracks
audio_cache_mb = 50                  # on-disk audio cache limit
tick_rate_ms = 50
refresh_interval_secs = 5
```

Note that the settings view rewrites this file when you change something, so
comments you add won't survive.

Device name, bitrate, normalisation and the cache size are read when the
playback engine starts, so they apply on the next launch.

Keys are configured separately in `keybindings.toml` — see
[KEYBINDINGS.md](KEYBINDINGS.md).

## How it works

parrotui-spotify uses [librespot](https://github.com/librespot-org/librespot) to register as a Spotify Connect device. When you start the app, it appears as a device in your Spotify account. Audio is decoded and played locally through your system's audio stack.

You can also control parrotui-spotify from any other Spotify client (phone, desktop, web) by selecting it as the active device.

## License

MIT
