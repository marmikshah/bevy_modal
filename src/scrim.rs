//! The scrim — the UI→UI occlusion plane and the crate's namesake. A
//! full-screen node parented under the overlay root, drawn first (so the panel
//! sits above it) and `Pickable` so it blocks every lower pick. Because it
//! covers the whole viewport, lower overlays' buttons are occluded no matter
//! how small the top panel is — which is the exact case raw `bevy_ui` misses.

use bevy::prelude::*;

/// Tags the scrim. Dismissal is wired by the spawn command via an observer that
/// captures the overlay root directly, so no back-reference is stored here.
#[derive(Component)]
pub(crate) struct Scrim;

/// The visual + interactive bundle for a scrim. `Pickable::default()` blocks
/// lower picks; the node is absolutely positioned to fill the viewport.
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
        Pickable::default(),
    )
}
