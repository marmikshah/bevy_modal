# bevy_modal

A modal/overlay stack for native `bevy_ui`. It fixes one specific, recurring
bug — **stacked popups leak input** — and folds away the `children!` /
`SpawnWith` boilerplate while it's there. No retained widget framework, no
layout engine: it emits ordinary `bevy_ui` nodes. On top of the stack it ships
two conveniences: **toasts** (transient, non-blocking notifications) and a
**confirm dialog** (a titled two-button modal).

> **Status: experimental (0.1, pre-release).** Built to be dogfooded across
> several games before it hits crates.io; the API will move. Consume via a path
> or git dependency for now.

## The bug it kills

Spawn a small popup over a larger one in raw `bevy_ui` and the larger popup's
buttons stay clickable around the edges of the smaller one — picking only
occludes where nodes overlap, and the small popup doesn't cover them. Worse, a
game that reads `Touches` / mouse **directly** for its own controls bypasses UI
picking entirely, so gameplay keeps responding *under* the popup.

`bevy_modal` closes both with two occlusion planes:

| Plane | Mechanism |
|-------|-----------|
| **UI → UI** | Every overlay owns a full-screen `Pickable` **scrim** that blocks all lower picks — regardless of the top panel's size. |
| **UI → gameplay** | A `UiCapturing` resource flips true while any overlay is open. Raw-input systems gate on it via the `ui_not_capturing` run condition. |

Layering is by spawn order (deterministic `GlobalZIndex` per depth), not entity
id or sibling position.

## How it works

- **`overlay()`** queues a command that, with `&mut World` in hand, reads the
  `Theme`, spawns the root + scrim + panel, registers the overlay on
  `OverlayStack` and stamps `GlobalZIndex = Z_BASE + depth * Z_STEP`.
- **`OverlayStack`** is the single source of truth for who's open and in what
  order. It tracks ids parallel to roots so you can query/dismiss by id.
- **Pruning is removal-driven**: despawn an overlay root (recursive — scrim and
  content go with it) and a system reconciles the stack and the input gate.
- **Toasts** are *not* modal: they draw no scrim, never touch `OverlayStack` or
  `UiCapturing`, sit in a column pinned to either screen edge, and auto-despawn
  when their timer elapses. They float above overlays.
- **Overlays animate** in and out, support keyboard focus navigation, and emit
  `OverlayOpened` / `OverlayClosed` lifecycle messages — see the sections below.
- **`confirm()`** is a thin wrapper over `overlay()` — same scrim/stack/gate, two
  buttons that run a callback and then dismiss the dialog.

## Quick start

```rust
use bevy::prelude::*;
use bevy_modal::prelude::*;

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, ModalPlugin))
        // .insert_resource(Theme { /* your chrome + fonts */ })  // optional
        .run();
}

fn open_pause(mut commands: Commands) {
    overlay(&mut commands, "pause")
        .title("PAUSED")
        .button("Resume", |c| { c.queue(|w: &mut World| { /* resume */ }); })
        .button("Quit", |c| { c.queue(|w: &mut World| { /* to menu */ }); })
        .dismissable(true)   // tap scrim to close; Esc pops the top
        .push();
}
```

### Toasts

Transient, non-blocking — they never scrim or capture input, and auto-dismiss
(or on tap). Pick a severity with `.level(..)` (it selects the accent from the
theme's `success`/`warning`/`danger`/`accent`), add an optional `.action(..)`
button, and stack them at the top or bottom edge via `Theme::toast_position`. At
most `Theme::max_toasts` (4) show at once — the oldest are dismissed past that.

```rust
use std::time::Duration;

fn notify(mut commands: Commands) {
    toast(&mut commands, "Saved").push();                 // Info, 4s, top
    toast(&mut commands, "Upload failed")
        .level(ToastLevel::Error)
        .duration(Duration::from_secs(6))
        .action("Retry", |c| { c.queue(|_w: &mut World| { /* retry */ }); })
        .push();
}
```

### Confirm dialog

A titled two-button modal over the overlay machinery; either press dismisses it:

```rust
fn ask(mut commands: Commands) {
    confirm(&mut commands, "delete-save", "Delete save?")
        .message("This can't be undone.")
        .confirm_label("Delete")
        .cancel_label("Keep")
        .on_confirm(|c| { c.queue(|w: &mut World| { /* delete */ }); })
        .on_cancel(|_c| { /* optional */ })
        .push();
}
```

## API

| Item | What it does |
|------|--------------|
| `ModalPlugin` | Wires the stack, transitions, focus/keyboard nav, lifecycle messages, the input gate, safe-area, toasts and button feedback; registers a default `Theme`. |
| `overlay(c, id)` | Builder for a modal overlay (`title`/`body`/`button`/`content`/`dismissable`/`escape`/`accent`). `push()` / `push_unique()`. |
| `confirm(c, id, title)` | Two-button dialog (`message`/`confirm_label`/`cancel_label`/`on_confirm`/`on_cancel`/`accent`). `push()`. |
| `toast(c, msg)` | Transient notification (`level`/`duration`/`accent`/`action`). `push()`. |
| `OverlayOpened` / `OverlayClosed` | Lifecycle messages; `OverlayClosed` carries a `CloseReason`. |
| `SafeAreaInsets` | Resource for notch/home-indicator insets — pads overlays, offsets toasts. |
| `OverlayStack` | Live stack of open overlays; `is_open`/`entity`/`top`/`depth`. |
| `OverlayCommandsExt::dismiss_overlay(id)` | Despawn the open overlay with this id. |
| `UiCapturing` / `ui_not_capturing` | The input gate resource + run condition. |
| `Theme` | Injected colours (incl. semantic + scrim), fonts, borders, transition timing, toast position, sizing. |

## The input-gate contract (read this)

The library cannot reach into your game's bespoke input reads. **You** must gate
every system that consumes raw input for gameplay:

```rust
app.add_systems(Update, rotate_player.run_if(ui_not_capturing));
```

Skip this and the UI→gameplay plane does nothing — your popups will look modal
but the game keeps playing underneath. `ui_not_capturing` defaults to *passing*
(capturing is false), so headless tests and fairness sims are never gated.
Toasts are deliberately outside this contract: they never set `UiCapturing`.

## Theming

Everything is driven by an injected `Theme` resource (colours, fonts, border
widths, button-state alphas, and the transition timing below). A neutral dark
default is registered by `ModalPlugin` so examples run with zero setup; insert
your own to match your game's chrome. Toasts reuse the same palette (`ink` fill,
`accent` border).

## Transitions

Overlays ease **in** when opened and **out** when dismissed, instead of popping —
the scrim fades and the panel scales (`Theme::open_secs` / `close_secs` /
`panel_scale_from`; set `panel_scale_from = 1.0` for fade-only, or the durations
to `0.0` for instant). Dismissal via the API — `dismiss_overlay`, Escape, or a
scrim tap — plays the exit, then despawns. A **direct `despawn()`** (e.g. a state
machine on `OnExit`) still closes instantly. The input gate stays armed until the
overlay is fully gone, so input never leaks under a still-visible modal.

## Focus & keyboard navigation

Built-in panel buttons are navigable without a pointer: opening an overlay focuses
its first button, `Tab`/`Shift+Tab` and the arrow keys move focus (wrapping),
`Enter`/`Space` activate it, and hovering focuses so pointer and keyboard agree.
Only the top overlay participates. (`.content()` overlays own their own input.)

## Lifecycle events

React to overlays opening and closing with two messages — useful for sound/haptic
cues, pausing the game, or analytics:

```rust
fn react(mut opened: MessageReader<OverlayOpened>, mut closed: MessageReader<OverlayClosed>) {
    for OverlayOpened(id) in opened.read() { /* play a swoosh */ }
    for c in closed.read() {
        // c.reason: Dismissed | Escape | Scrim | Despawned
    }
}
```

`OverlayClosed` fires for *every* close path, tagged with a `CloseReason`.

## Safe area & sizing

For mobile, set the `SafeAreaInsets` resource (it's generic — populate it from
your platform's notch / home-indicator insets; the crate has no platform
dependency). Overlay roots get the insets as padding (centred panels and
full-bleed `.content()` both stay inside the safe area) and the toast column
offsets its anchored edge to match — updated every frame, so rotation is handled:

```rust
fn sync_insets(mut insets: ResMut<SafeAreaInsets>) {
    insets.top = /* your platform's top inset */ 47.0;
    insets.bottom = 34.0;
}
```

The built-in panel is `82%` of the screen, clamped to `Theme::panel_max_width`
(420px default), so it doesn't stretch absurdly wide on tablets/desktop.

## Compatibility

| `bevy_modal` | `bevy` |
|--------------|--------|
| 0.1          | 0.19   |

Pre-release: consume via a path dependency (`bevy_modal = { path = "../bevy_modal" }`)
and dogfood before this hits crates.io.

## Building overlays

Two tiers, pick per screen:

- **Built-in panel** — `overlay(c, id).title(..).body(..).button(label, on_click)`.
  The crate builds the panel; good for simple dialogs.
- **Bespoke content** — `overlay(c, id).content(|parent| { /* your bevy_ui */ })`.
  The crate owns the (scrimmed, stacked, gated) root; you fill it. The closure
  gets a `&mut ChildSpawnerCommands` — the spawner `Commands::spawn().with_children`
  hands you — so your existing `bevy_ui` builder helpers drop straight in. For
  settings grids, icon rows, anything it shouldn't try to model.

Lifecycle is yours to choose:

- **builder-driven** — `.dismissable(true)` (scrim tap) and/or `.escape(true)`
  (Esc) pop the overlay.
- **state-driven** — spawn on `OnEnter`, despawn the `Overlay` on `OnExit`;
  leave dismiss/escape **off** so the state machine stays authoritative.

## Examples

```
cargo run --example showcase   # a button per feature: overlays, stacking,
                               # confirm, toast levels + action, content, events
cargo run --example stacked    # the core: two stacked overlays + the input gate
```

- **`showcase`** — a menu that opens each kind of overlay/dialog, fires toasts of
  every level (and one with an action), and surfaces `OverlayOpened`/`Closed` as
  toasts. Overlays animate in/out and are keyboard-navigable (Tab/arrows/Enter).
- **`stacked`** — the bug `bevy_modal` kills: a smaller overlay over a larger one,
  with a spinning sprite that freezes while any overlay is up (the UI→gameplay gate).

## Testing

The crate ships a headless `#[cfg(test)]` suite (`MinimalPlugins`, no renderer)
covering: `push()` registering a root and arming the gate, removal-driven prune
releasing it, `push_unique()` dedup, `dismiss_overlay(id)`, escape popping only
an opted-in top, deterministic per-depth z-index, and toast auto-expiry (driven
by a manual `Time` step so it's deterministic, not wall-clock).

```
cargo test
```

## Limitations / roadmap

- Keyboard + pointer navigation only — **no gamepad** (by choice).
- Built-in panel is intentionally minimal (title / body / buttons). Richer
  layouts go through `.content()` — by design, not omission.

## Authorship

Much of this project — the Rust crate, its tests, and these docs — was written by
**Claude Opus 4.8** (Anthropic) under human direction and review. It ships with a
passing test suite and a runnable example, but it's young (0.x): read the code,
run your own tests, and validate behavior in your app before relying on it in
production. Bug reports and PRs welcome.

## License

Licensed under either of [Apache-2.0](LICENSE-APACHE) or [MIT](LICENSE-MIT) at
your option.
