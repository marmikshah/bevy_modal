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
//! Overlays also animate in and out, support keyboard focus navigation, emit
//! [`OverlayOpened`] / [`OverlayClosed`] lifecycle messages, and respect
//! [`SafeAreaInsets`] — see the module docs and the README.
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
mod events;
mod focus;
mod gate;
mod safe_area;
mod scrim;
mod stack;
mod theme;
mod toast;
mod transition;
mod widgets;

#[cfg(test)]
mod tests;

pub use build::{OverlayBuilder, overlay};
pub use confirm::{ConfirmBuilder, confirm};
pub use events::{CloseReason, OverlayClosed, OverlayOpened};
pub use focus::FocusScope;
pub use gate::{UiCapturing, ui_not_capturing};
pub use safe_area::SafeAreaInsets;
pub use stack::{Overlay, OverlayCommandsExt, OverlayStack};
pub use theme::Theme;
pub use toast::{ToastBuilder, ToastLevel, ToastPosition, toast};
pub use widgets::{Scrollable, Slider, Toggle, WidgetSpawnerExt};

pub mod prelude {
    pub use crate::{
        CloseReason, ConfirmBuilder, FocusScope, ModalPlugin, Overlay, OverlayBuilder,
        OverlayClosed, OverlayCommandsExt, OverlayOpened, OverlayStack, SafeAreaInsets, Scrollable,
        Slider, Theme, ToastBuilder, ToastLevel, ToastPosition, Toggle, UiCapturing,
        WidgetSpawnerExt, confirm, overlay, toast, ui_not_capturing,
    };
}

/// Wires the overlay stack, the open/close transitions, focus + keyboard
/// navigation, lifecycle messages ([`OverlayOpened`] / [`OverlayClosed`]), the
/// input-capture gate, [`SafeAreaInsets`] application, the toast expiry sweep and
/// the button feedback systems. Insert your own [`Theme`] before or after — a
/// neutral default is registered here so examples run with zero setup.
pub struct ModalPlugin;

impl Plugin for ModalPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<OverlayStack>()
            .init_resource::<UiCapturing>()
            .init_resource::<Theme>()
            .init_resource::<events::CloseReasons>()
            .init_resource::<SafeAreaInsets>()
            .add_message::<OverlayOpened>()
            .add_message::<OverlayClosed>()
            .add_systems(
                Update,
                (
                    transition::drive_transitions,
                    events::announce_opened,
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
                    (
                        widgets::react_toggles,
                        widgets::toggle_keyboard,
                        widgets::slider_drag,
                        widgets::slider_keyboard,
                        widgets::react_sliders,
                        // Only when an input backend supplies wheel messages
                        // (absent under `MinimalPlugins` in headless tests).
                        widgets::scroll_lists.run_if(
                            resource_exists::<
                                bevy::ecs::message::Messages<bevy::input::mouse::MouseWheel>,
                            >,
                        ),
                    ),
                    toast::expire_toasts,
                    toast::cap_toasts,
                    safe_area::apply_safe_area,
                ),
            );
    }
}
