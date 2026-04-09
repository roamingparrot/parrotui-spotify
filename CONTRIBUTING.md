# Contributing to parrotui-spotify

Thanks for your interest in contributing. This document covers the conventions
we follow so the codebase stays consistent and easy to work with.

## Getting started

```sh
nix develop          # preferred — pins toolchain + system deps
# or
nix-shell -p cargo rustc pkg-config openssl dbus libsecret alsa-lib cmake
```

Then:

```sh
cargo build
cargo run
```

You'll need a Spotify Premium account and a registered developer app
(see README.md for setup).

## Project layout

```
src/
  main.rs          Event loop, terminal setup, startup
  api/             Spotify Web API client (metadata, playlists)
  auth/            OAuth PKCE flow + token persistence
  config/          TOML config loading, first-run setup
  error/           SpotError enum, Result alias
  input/           Keyboard event → Action mapping
  playback/        librespot session, Spirc, progress tracking
  player/          Action dispatch (bridges input → playback/API)
  state/           App struct, content views, UI state
  ui/              ratatui rendering + widgets
```

The data flow is: **input** produces an `Action` → **player** dispatches it
→ side effects go to **playback** (local Spirc) or **api** (Web API) →
**state** gets mutated → **ui** reads state and renders.

## Commit conventions

We use short, lowercase commit messages. No period at the end.

```
<type>: <what changed>
```

### Types

| Type       | When                                           |
|------------|------------------------------------------------|
| `feat`     | New user-facing feature                        |
| `fix`      | Bug fix                                        |
| `refactor` | Internal restructuring, no behavior change     |
| `ui`       | TUI layout, widget, or styling change          |
| `auth`     | OAuth, token, credential changes               |
| `playback` | librespot, Spirc, audio backend changes        |
| `api`      | Spotify Web API client changes                 |
| `config`   | Configuration, first-run setup                 |
| `nix`      | flake.nix, devShell, build packaging           |
| `docs`     | README, CONTRIBUTING, comments                 |
| `deps`     | Dependency version bumps                       |
| `chore`    | CI, tooling, gitignore, formatting             |

### Examples

```
feat: liked songs pagination
fix: progress bar resets on pause
refactor: split playback engine from player dispatch
ui: dim inactive panel borders
playback: clear stale credentials on startup
auth: add streaming scope for librespot
nix: add alsa-lib to buildInputs
```

### Body

Add a body when the "why" isn't obvious from the subject. Leave a blank
line between subject and body. Wrap at ~72 chars.

```
fix: seek clamps to track duration

Previously you could seek past the end of a track, which caused the
progress bar to show negative remaining time. Now seek_relative clamps
the target to 0..duration_ms.
```

### What not to do

- Don't use `Update file.rs` or `misc changes`
- Don't capitalize the subject or end with a period
- Don't squash unrelated changes into one commit
- Don't prefix with ticket numbers unless we start using an issue tracker

## Branches

- `main` is the release branch. It should always build.
- Feature branches: `feat/liked-songs-search`, `fix/token-refresh-loop`
- Keep branches short-lived. Rebase on main before merging.

## Pull requests

### Title

Same format as commit subjects:

```
feat: queue view with drag reorder
```

### Description template

```markdown
## Summary

Brief description of what this changes and why.

## Changes

- Bullet list of what was modified
- One line per logical change

## Testing

How you verified this works. "Ran it and played a playlist" is fine
for UI changes. Mention edge cases you checked.
```

### Review norms

- PRs should be reviewable in one sitting. If it's big, split it.
- Reviewer approves or requests changes — no "looks good" without reading.
- Author resolves conversations after addressing feedback.
- Squash-merge into main with a clean commit message.

## Code style

### Rust conventions

- Follow standard `rustfmt` defaults. Don't fight the formatter.
- `cargo clippy` should pass without warnings.
- Prefer `thiserror` for error types, `tracing` for logging.
- Use `?` propagation. Avoid `.unwrap()` outside of tests.

### Naming

- Modules: lowercase, short (`api`, `auth`, `ui`, not `spotify_api_client`)
- Types: standard Rust conventions (`PascalCase`)
- Functions: `snake_case`, verb-first for actions (`load_playlists`, `toggle_shuffle`)
- Constants: `SCREAMING_SNAKE_CASE`
- No Hungarian notation, no prefixes like `m_` or `s_`

### Comments

- Don't comment obvious code. `// increment counter` above `i += 1` helps nobody.
- Do comment non-obvious decisions, workarounds, or Spotify API quirks.
- Use `// TODO:` for known gaps. Include enough context to act on it later.

### Error handling

- Propagate with `?` wherever possible.
- User-facing errors → `app.notify_error()` (shows in status bar).
- Internal errors → `tracing::warn!` or `tracing::debug!`.
- Don't silently discard errors with `let _ =` unless you leave a comment explaining why.

### Module boundaries

- `playback` owns the librespot session. Nothing else touches Spirc directly.
- `player` is the only module that calls both `playback` and `api`.
- `ui` reads from `App` state but never mutates it.
- `state` should not import from `ui`, `input`, or `player`.

### Adding dependencies

- Check if the standard library or an existing dep already covers it.
- Prefer well-maintained crates with minimal transitive dependencies.
- Pin librespot to a specific git commit (via Cargo.lock). Don't use `branch = "main"` without locking.

## Testing

We don't have a test suite yet. When we add one:

- Unit tests go in the same file (`#[cfg(test)] mod tests { ... }`).
- Integration tests go in `tests/`.
- Mock the Spotify API for tests, not librespot internals.

## Releases

Not formalized yet. When we get there:

- Bump version in `Cargo.toml`
- Update `flake.nix` version
- Tag with `v0.x.y`
- Changelog entries follow the commit type format
