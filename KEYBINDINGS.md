# Keybindings

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
| `,`          | Open settings           |
| `?`          | Show help               |
| `q`          | Quit                    |
| `Tab`        | Switch focus panel      |

## Search

| Key          | Action                                          |
|--------------|-------------------------------------------------|
| `/`          | Open search / return to query input             |
| `Enter`      | Submit query                                    |
| `Esc`        | Exit search and return to home                  |
| `Tab`        | Switch focus between category sidebar and results |
| `j` / `k`   | Move through results (or categories in sidebar) |
| `Enter`      | Play track / drill into album, artist, playlist |
| `h` / `Esc` | Back out of drill-down to search results        |
| `q`          | Exit search and return to home                  |
| `Space` / `n` / `p` | Playback controls (work in search too) |

## Settings

| Key          | Action                                  |
|--------------|-----------------------------------------|
| `,`          | Open settings (and close it again)      |
| `h` / `l`   | Previous / next tab                     |
| `Tab`        | Next tab                                |
| `j` / `k`   | Move between settings                   |
| `Enter`      | Edit the focused setting                |
| `r`          | Reset the focused setting to its default |
| `Esc`        | Cancel an edit, or close settings       |
| `q`          | Close settings                          |

## Rebinding

Keys live in `~/.config/parrotui-spotify/keybindings.toml`, written with the
defaults on first run. Each entry maps a command to the keys that trigger it:

```toml
[normal]
move_down = ["j", "down"]
jump_top = ["g g"]
```

Keys are named (`space`, `esc`, `down`, `f5`), single characters (`j`, `G`),
modifier combinations (`ctrl+x`), or space-separated sequences (`g g`).
Uppercase and lowercase are different keys.

Sections are the context a key applies in: `normal`, `search`, `search_input`,
`settings`, `settings_editor` and `help`. Commands you leave out keep their
defaults, and an empty list unbinds one.

Two things can't be rebound: `Ctrl+C` always quits, so a broken file can't trap
you, and in `search_input` or a text setting every printable character is typed
rather than run as a command.

The Keybindings tab in the settings view lists whatever is currently loaded.
