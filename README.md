# spotatui

A keyboard-driven Spotify client for the terminal with built-in playback, built with [ratatui](https://github.com/ratatui/ratatui) and [librespot](https://github.com/librespot-org/librespot).

![Rust](https://img.shields.io/badge/rust-stable-orange)
![License](https://img.shields.io/badge/license-MIT-blue)

## Features

- **Built-in playback** — registers as a Spotify Connect device, plays audio directly
- Play/pause, skip, seek, volume, shuffle, repeat
- Browse and play playlists
- Liked songs library
- Vim-style and arrow key navigation
- Now-playing bar with smooth local progress tracking
- Appears in Spotify's device list (can be controlled from phone/desktop too)
- Works great on NixOS (and other Linux distros)

## Requirements

- A Spotify Premium account (required for streaming)
- Working audio output (ALSA/PipeWire/PulseAudio — PipeWire's ALSA compat layer works fine)
- A registered [Spotify Developer Application](https://developer.spotify.com/dashboard)

## Setup

1. Go to https://developer.spotify.com/dashboard
2. Set the redirect URI to exactly `http://127.0.0.1:8888/callback`
   - Spotify requires `127.0.0.1`, not `localhost`
3. Create an app — select **Web API** and **Web Playback SDK**
3. Note your Client ID
4. Run `spotatui` — it will prompt you for the Client ID on first launch and store it in `~/.config/spotatui/config.toml`

The first run opens a browser for Spotify OAuth. Tokens are cached locally.

## Installation

### From source

```sh
cargo install --path .
```

### NixOS / Nix

```sh
nix run github:spotatui/spotatui
```

Or add to your flake inputs:

```nix
{
  inputs.spotatui.url = "github:spotatui/spotatui";
  # ...
  environment.systemPackages = [ inputs.spotatui.packages.${system}.default ];
}
```

A dev shell is provided:

```sh
nix develop
```

## Usage

```
spotatui [--config PATH]
```

### Keybindings

| Key          | Action                  |
|--------------|-------------------------|
| `j` / `↓`   | Move down               |
| `k` / `↑`   | Move up                 |
| `h` / `←`   | Collapse / go back      |
| `l` / `→` / `Enter` | Expand / select |
| `gg`         | Jump to top             |
| `G`          | Jump to bottom          |
| `Space`      | Play/pause              |
| `n`          | Next track              |
| `p`          | Previous track          |
| `+` / `-`    | Volume up/down          |
| `s`          | Toggle shuffle          |
| `r`          | Cycle repeat mode       |
| `>` / `<`    | Seek forward/back 5s    |
| `/`          | Search (planned)        |
| `?`          | Show help               |
| `q`          | Quit                    |
| `Tab`        | Switch focus panel      |

## Configuration

Config lives at `~/.config/spotatui/config.toml`:

```toml
client_id = "your_spotify_client_id"
# redirect_uri = "http://127.0.0.1:8888/callback"  # default
# device_name = "spotatui"                          # how it shows in Spotify Connect
# initial_volume = 50                               # 0-100
# tick_rate_ms = 250
# refresh_interval_secs = 5
```

## How it works

spotatui uses [librespot](https://github.com/librespot-org/librespot) to register as a
Spotify Connect device. When you start the app, it appears as a device called "spotatui" in
your Spotify account. Audio is decoded and played locally through your system's audio stack.

You can also control spotatui from any other Spotify client (phone, desktop, web) by selecting
it as the active device.

## License

MIT
