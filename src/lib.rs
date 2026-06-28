//! `bevy_modal` — a modal/overlay stack over native `bevy_ui`, plus transient
//! toasts and a confirm dialog.
//!
//! The problem it solves: stacked popups in raw `bevy_ui` leak input. A smaller
//! popup spawned over a larger one doesn't cover the larger one's buttons, so
//! picking never occludes them and clicks fall through. And anything that reads
//! input *outside* the UI picking path — a game polling `Touches`/mouse for its
//! own controls — keeps firing under the popup entirely.
//!
//! `bevy_modal` closes both gaps with two occlusion planes:
//!
//! 1. **UI → UI.** Every overlay owns a full-screen [`scrim`](crate::scrim)
//!    node that blocks lower picks. Lower buttons sit behind it, so they can't
//!    be hit regardless of the top popup's size.
//! 2. **UI → gameplay.** A [`UiCapturing`] resource flips true while any
//!    overlay is up. Raw-input game systems opt in with the
//!    [`ui_not_capturing`] run condition — the one line of integration this
//!    crate cannot do for you (see the README "input gate contract").
//!
//! On top of that sits an ergonomic [`overlay`] builder that emits ordinary
//! `bevy_ui` nodes — no retained widget framework, no layout engine, just the
//! verbose `children!` / `SpawnWith` boilerplate folded away. Two higher-level
//! conveniences ride on the same machinery: [`confirm`] (a titled two-button
//! dialog) and [`toast`] (transient, non-blocking notifications that never
//! scrim or capture input).
//!
//! ```no_run
//! use bevy::prelude::*;
//! use bevy_modal::prelude::*;
//!
//! fn open_pause(mut commands: Commands) {
//!     overlay(&mut commands, "pause")
//!         .title("PAUSED")
//!         .button("Resume", |c| { c.queue(|_w: &mut World| { /* resume */ }); })
//!         .dismissable(true)
//!         .push();
//! }
//! ```

use bevy::prelude::*;

mod build;
mod confirm;
mod focus;
mod gate;
mod scrim;
mod stack;
mod theme;
mod toast;
mod transition;

#[cfg(test)]
mod tests;

pub use build::{OverlayBuilder, overlay};
pub use confirm::{ConfirmBuilder, confirm};
pub use gate::{UiCapturing, ui_not_capturing};
pub use stack::{Overlay, OverlayCommandsExt, OverlayStack};
pub use theme::Theme;
pub use toast::{ToastBuilder, toast};

pub mod prelude {
    pub use crate::{
        ConfirmBuilder, ModalPlugin, Overlay, OverlayBuilder, OverlayCommandsExt, OverlayStack,
        Theme, ToastBuilder, UiCapturing, confirm, overlay, toast, ui_not_capturing,
    };
}

/// Wires the overlay stack, the open/close transitions, focus + keyboard
/// navigation, the input-capture gate, the toast expiry sweep and the button
/// feedback systems. Insert your own [`Theme`] before or after — a neutral
/// default is registered here so examples run with zero setup.
pub struct ModalPlugin;

impl Plugin for ModalPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<OverlayStack>()
            .init_resource::<UiCapturing>()
            .init_resource::<Theme>()
            .add_systems(
                Update,
                (
                    transition::drive_transitions,
                    stack::prune_despawned_overlays,
                    stack::escape_pops_top,
                    // Focus systems run in order so the per-frame focus decision
                    // is deterministic (maintain default → hover → navigate → activate).
                    (
                        focus::maintain_focus,
                        focus::hover_focuses,
                        focus::navigate_focus,
                        focus::activate_focused,
                    )
                        .chain(),
                    build::react_buttons,
                    toast::expire_toasts,
                ),
            );
    }
}
