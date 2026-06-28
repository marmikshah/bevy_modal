//! Overlay lifecycle messages.
//!
//! [`OverlayOpened`] fires when an overlay registers, [`OverlayClosed`] when it
//! leaves the stack — for *every* close path, tagged with a [`CloseReason`] so
//! consumers can tell an Escape from a scrim tap from a code-driven dismiss. Use
//! them to play a sound/haptic, pause/resume the game, or record analytics.
//!
//! ```no_run
//! use bevy::prelude::*;
//! use bevy_modal::prelude::*;
//!
//! fn react(mut opened: MessageReader<OverlayOpened>, mut closed: MessageReader<OverlayClosed>) {
//!     for OverlayOpened(id) in opened.read() {
//!         println!("opened {id}");
//!     }
//!     for closed in closed.read() {
//!         println!("closed {} via {:?}", closed.id, closed.reason);
//!     }
//! }
//! ```

use std::collections::HashMap;

use bevy::prelude::*;

use crate::stack::Overlay;

/// Why an overlay closed.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CloseReason {
    /// Closed via `dismiss_overlay` — includes the built-in `confirm` buttons.
    Dismissed,
    /// Closed by the Escape key.
    Escape,
    /// Closed by a tap on the scrim.
    Scrim,
    /// Despawned directly (e.g. a state machine on `OnExit`), not via the API.
    Despawned,
}

/// Emitted when an overlay registers on the stack. The payload is its id.
#[derive(Message, Clone, Debug)]
pub struct OverlayOpened(pub String);

/// Emitted when an overlay leaves the stack, however it closed.
#[derive(Message, Clone, Debug)]
pub struct OverlayClosed {
    pub id: String,
    pub reason: CloseReason,
}

/// Records why each overlay is closing so [`OverlayClosed`] can report it once
/// the root despawns (the entity is gone by the time the prune sees the removal,
/// so the reason is stashed here when the close is requested). Entities that
/// despawn without a recorded reason default to [`CloseReason::Despawned`].
#[derive(Resource, Default)]
pub(crate) struct CloseReasons(pub(crate) HashMap<Entity, CloseReason>);

/// Announce each newly-registered overlay exactly once.
pub(crate) fn announce_opened(
    added: Query<&Overlay, Added<Overlay>>,
    mut opened: MessageWriter<OverlayOpened>,
) {
    for overlay in &added {
        opened.write(OverlayOpened(overlay.id.clone()));
    }
}
