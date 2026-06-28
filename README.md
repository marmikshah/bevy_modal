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
  `UiCapturing`, sit in a top-anchored column, and auto-despawn when their timer
  elapses. They float above overlays.
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

Transient, non-blocking — they never scrim or capture input, and auto-dismiss:

```rust
use std::time::Duration;

fn notify(mut commands: Commands) {
    toast(&mut commands, "Saved")
        .duration(Duration::from_secs(2))  // defaults to 4s
        .accent(Color::srgb(0.4, 0.9, 0.5))
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
| `ModalPlugin` | Wires the stack, gate, toast expiry and button feedback; registers a default `Theme`. |
| `overlay(c, id)` | Builder for a modal overlay (`title`/`body`/`button`/`content`/`dismissable`/`escape`/`accent`). `push()` / `push_unique()`. |
| `confirm(c, id, title)` | Two-button dialog (`message`/`confirm_label`/`cancel_label`/`on_confirm`/`on_cancel`/`accent`). `push()`. |
| `toast(c, msg)` | Transient notification (`duration`/`accent`). `push()`. |
| `OverlayStack` | Live stack of open overlays; `is_open`/`entity`/`top`/`depth`. |
| `OverlayCommandsExt::dismiss_overlay(id)` | Despawn the open overlay with this id. |
| `UiCapturing` / `ui_not_capturing` | The input gate resource + run condition. |
| `Theme` | Injected colours, fonts, borders, button-state alphas. |

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
widths, button-state alphas). A neutral dark default is registered by
`ModalPlugin` so examples run with zero setup; insert your own to match your
game's chrome. Toasts reuse the same palette (`ink` fill, `accent` border).

## Compatibility

| `bevy_modal` | `bevy` |
|--------------|--------|
| 0.1          | 0.18   |

Pre-release: consume via a path dependency (`bevy_modal = { path = "../bevy_modal" }`)
and dogfood before this hits crates.io.

## Building overlays

Two tiers, pick per screen:

- **Built-in panel** — `overlay(c, id).title(..).body(..).button(label, on_click)`.
  The crate builds the panel; good for simple dialogs.
- **Bespoke content** — `overlay(c, id).content(|parent| { /* your bevy_ui */ })`.
  The crate owns the (scrimmed, stacked, gated) root; you fill it. For settings
  grids, icon rows, anything it shouldn't try to model.

Lifecycle is yours to choose:

- **builder-driven** — `.dismissable(true)` (scrim tap) and/or `.escape(true)`
  (Esc) pop the overlay.
- **state-driven** — spawn on `OnEnter`, despawn the `Overlay` on `OnExit`;
  leave dismiss/escape **off** so the state machine stays authoritative.

## Example

```
cargo run --example stacked
```

Two stacked overlays, a confirm dialog and a toast; the spinning sprite freezes
while any overlay is open (but not for a toast).

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

- **No focus trap or directional (keyboard/gamepad) nav yet.** Touch/mouse
  only. This is the main gap for full accessibility and is the next planned
  addition.
- Built-in panel is intentionally minimal (title / body / buttons). Richer
  layouts go through `.content()` — by design, not omission.

## License

Licensed under either of [Apache-2.0](LICENSE-APACHE) or [MIT](LICENSE-MIT) at
your option.
