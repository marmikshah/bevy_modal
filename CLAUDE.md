# CLAUDE.md — bevy_modal

Agent onboarding. `make` is the entry point; keep this short and current.

## What this is

A modal/overlay stack over native `bevy_ui`: a blocking scrim, deterministic
layering, an input-capture gate, and an ergonomic `overlay()` builder — plus
open/close transitions, keyboard focus navigation, lifecycle messages, safe-area
awareness, transient toasts and a confirm dialog. No retained widget framework,
no layout engine; it emits plain `bevy_ui` nodes.

## Entry point

**Everything is a `make` target — never run ad-hoc scripts.** `make help` lists them.

| target | use |
|--------|-----|
| `make run` | run the stacked-overlays example |
| `make test` | test suite |
| `make pre-commit-checks` | `cargo fmt --check` + clippy `-D warnings` (what the hooks run) |
| `make release` | tag a clean `master` → CI publishes to crates.io |
| `make clean` | wipe build artifacts |

## Architecture

- `stack.rs` — the `OverlayStack` (spawn-order layering), prune-on-despawn, escape-pop, `dismiss_overlay`.
- `scrim.rs` — the full-screen pickable scrim (the UI→UI occlusion plane).
- `gate.rs` — `UiCapturing` + the `ui_not_capturing` run condition (the UI→gameplay plane).
- `build.rs` — the `overlay()` builder, the `ButtonAction` component, and themed button feedback.
- `transition.rs` — open/close animation + the overlay lifecycle (`Opening → Open → Closing → despawn`); `request_close` is the animated close path, a direct `despawn()` is instant.
- `focus.rs` — focus + keyboard navigation (Tab/arrows/Enter) over overlay buttons; top-overlay only.
- `events.rs` — `OverlayOpened` / `OverlayClosed` lifecycle messages (with `CloseReason`).
- `safe_area.rs` — `SafeAreaInsets` applied as overlay padding + toast edge offset.
- `toast.rs` / `confirm.rs` — transient toasts (levels/position/action/cap) and the confirm-dialog convenience.
- `theme.rs` — the injected `Theme`.

## Hard constraints

- Layering is by spawn order (stack position), not entity id or sibling index — keep it deterministic.
- Toasts are non-modal: they must never touch `OverlayStack` or `UiCapturing`.
- The input gate is a *contract*: raw-input game systems opt in with `ui_not_capturing`; the crate can't reach a downstream game's bespoke input reads.
- Open source: keep examples/docs free of any personal or company identifiers.

## Dev notes

- Tests run headless (`MinimalPlugins`, no renderer); spawning `Node` components works without a window.
- Toast-expiry tests drive time deterministically (manual `Time` step); escape tests clear `ButtonInput` manually (no `InputPlugin` under `MinimalPlugins`).
