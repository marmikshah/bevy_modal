//! The UI→gameplay occlusion plane.
//!
//! `bevy_ui` picking only occludes input that flows *through* picking. A game
//! that reads `Touches` / `ButtonInput<MouseButton>` directly for its own
//! controls bypasses that entirely, so a scrim — however opaque to clicks —
//! never stops the gameplay underneath.
//!
//! [`UiCapturing`] is the bridge. The overlay stack flips it true while any
//! overlay is open; raw-input game systems gate themselves with
//! [`ui_not_capturing`]:
//!
//! ```ignore
//! app.add_systems(Update, rotate_player.run_if(ui_not_capturing));
//! ```
//!
//! This is a *contract*, not magic: the library can't reach into a downstream
//! game's bespoke input reads. Gate them, or input leaks under your popups.

use bevy::prelude::*;

/// True while one or more overlays are open. Maintained by the overlay stack;
/// read it (directly or via [`ui_not_capturing`]) from any system that consumes
/// raw input for gameplay.
#[derive(Resource, Default, Debug, Clone, Copy)]
pub struct UiCapturing(pub bool);

/// Run condition: passes only when no overlay is capturing input. Defaults to
/// passing (capturing is `false`) so headless tests and sims aren't gated.
pub fn ui_not_capturing(capturing: Res<UiCapturing>) -> bool {
    !capturing.0
}
