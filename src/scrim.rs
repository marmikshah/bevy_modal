//! The scrim — the UI→UI occlusion plane and the crate's namesake. A
//! full-screen node parented under the overlay root, drawn first (so the panel
//! sits above it). Because it covers the whole viewport, lower buttons are
//! occluded no matter how small the top panel is — the exact case raw `bevy_ui`
//! misses.
//!
//! It must block **both** input paths `bevy_ui` runs:
//! - `FocusPolicy::Block` stops `ui_focus_system`, which drives the `Interaction`
//!   ordinary `Button`s read. `Node` auto-adds a `FocusPolicy` that defaults to
//!   `Pass`, so without this the scrim is see-through to lower buttons.
//! - `Pickable::default()` (`should_block_lower`) stops the `bevy_picking` path
//!   used by `On<Pointer<..>>` observers (including the overlays' own buttons).

use bevy::prelude::*;
use bevy::ui::FocusPolicy;

/// Tags the scrim. Dismissal is wired by the spawn command via an observer that
/// captures the overlay root directly, so no back-reference is stored here.
#[derive(Component)]
pub(crate) struct Scrim;

/// The viewport-filling scrim bundle: blocks the `Interaction` path
/// (`FocusPolicy::Block`) and the picking path (`Pickable`) so nothing below it
/// can be hovered, pressed or clicked.
pub(crate) fn scrim_bundle(color: Color) -> impl Bundle {
    (
        Scrim,
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(0.0),
            left: Val::Px(0.0),
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            ..default()
        },
        BackgroundColor(color),
        FocusPolicy::Block,
        Pickable::default(),
    )
}
