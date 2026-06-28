//! The overlay stack: who's open, in what order, and therefore who draws on top
//! and who owns input. Spawn order — not entity id or sibling position —
//! decides layering, so it's deterministic across frames and respawns.

use bevy::prelude::*;

use crate::gate::UiCapturing;

/// The `GlobalZIndex` floor for overlays; each depth adds [`Z_STEP`]. Pick a
/// base well above ordinary HUD z so overlays always win.
pub(crate) const Z_BASE: i32 = 1000;
pub(crate) const Z_STEP: i32 = 10;

/// Marks an overlay root. The string is the caller's id (for find/dismiss).
#[derive(Component, Debug, Clone)]
pub struct Overlay {
    pub id: String,
    /// Whether Escape pops this overlay while it's on top (see the builder's
    /// `escape`). Off for state-driven overlays.
    pub pop_on_escape: bool,
}

/// The live stack of overlay root entities, bottom-to-top. The builder pushes;
/// despawning an overlay root prunes it (see [`prune_despawned_overlays`]).
///
/// `ids` runs parallel to `roots` (same length, same order) so the stack can
/// answer id queries — [`is_open`](Self::is_open), [`entity`](Self::entity) —
/// without a `Query<&Overlay>`. That, plus [`OverlayBuilder::push_unique`] and
/// [`OverlayCommandsExt::dismiss_overlay`], lets a consumer manage overlays by
/// id with no hand-rolled bookkeeping.
#[derive(Resource, Default, Debug)]
pub struct OverlayStack {
    pub roots: Vec<Entity>,
    /// Id of each root, parallel to `roots`.
    ids: Vec<String>,
}

impl OverlayStack {
    pub fn is_empty(&self) -> bool {
        self.roots.is_empty()
    }

    pub fn depth(&self) -> usize {
        self.roots.len()
    }

    /// The top-most (input-owning) overlay, if any.
    pub fn top(&self) -> Option<Entity> {
        self.roots.last().copied()
    }

    /// Is an overlay with this id currently open? Cheap id check — use it for
    /// "already open?" guards instead of scanning a `Query<&Overlay>`. Because
    /// `push_unique` checks this at command-apply time (when commands run
    /// sequentially against `&mut World`), it is race-free even for two opens
    /// queued in the same frame.
    pub fn is_open(&self, id: &str) -> bool {
        self.ids.iter().any(|i| i == id)
    }

    /// Alias for [`is_open`](Self::is_open).
    pub fn contains(&self, id: &str) -> bool {
        self.is_open(id)
    }

    /// The root entity of the open overlay with this id, if any.
    pub fn entity(&self, id: &str) -> Option<Entity> {
        self.ids.iter().position(|i| i == id).map(|i| self.roots[i])
    }
}

/// Called from the spawn command (which holds `&mut World`): registers a new
/// root + its id on top, hands back its depth so the caller can stamp
/// `GlobalZIndex`, and arms the capture gate.
pub(crate) fn push_root(world: &mut World, root: Entity, id: &str) -> usize {
    let mut stack = world.resource_mut::<OverlayStack>();
    let depth = stack.roots.len();
    stack.roots.push(root);
    stack.ids.push(id.to_string());
    world.resource_mut::<UiCapturing>().0 = true;
    depth
}

/// Despawning an overlay root removes its `Overlay`; reconcile the stack and
/// release the gate when the last one closes. Removal-driven so callers just
/// `despawn()` the root (recursive — scrim and content go with it) and the
/// bookkeeping follows. Removes `roots[i]` and `ids[i]` together so the two
/// stay in lockstep.
pub(crate) fn prune_despawned_overlays(
    mut removed: RemovedComponents<Overlay>,
    mut stack: ResMut<OverlayStack>,
    mut capturing: ResMut<UiCapturing>,
) {
    let mut changed = false;
    for entity in removed.read() {
        if let Some(i) = stack.roots.iter().position(|&r| r == entity) {
            stack.roots.remove(i);
            stack.ids.remove(i);
            changed = true;
        }
    }
    if changed {
        capturing.0 = !stack.roots.is_empty();
    }
}

/// Desktop affordance: Escape pops only the top overlay (a stack, not a
/// blanket close), and only if that overlay opted in via `escape(true)`. Touch
/// builds dismiss via the scrim tap instead.
pub(crate) fn escape_pops_top(
    keys: Res<ButtonInput<KeyCode>>,
    stack: Res<OverlayStack>,
    overlays: Query<&Overlay>,
    mut commands: Commands,
) {
    if keys.just_pressed(KeyCode::Escape)
        && let Some(top) = stack.top()
        && let Ok(overlay) = overlays.get(top)
        && overlay.pop_on_escape
    {
        commands.entity(top).despawn();
    }
}

/// Despawn the open overlay with the given id, if any. Queued as a command so it
/// composes with the rest of `Commands`; the despawn triggers
/// [`prune_despawned_overlays`] which reconciles the stack and the input gate.
/// Replaces the hand-rolled `Query<(Entity, &Overlay)>` + filter + despawn loop
/// a consumer would otherwise write.
pub trait OverlayCommandsExt {
    fn dismiss_overlay(&mut self, id: impl Into<String>);
}

impl OverlayCommandsExt for Commands<'_, '_> {
    fn dismiss_overlay(&mut self, id: impl Into<String>) {
        let id = id.into();
        self.queue(move |world: &mut World| {
            if let Some(e) = world.resource::<OverlayStack>().entity(&id) {
                world.entity_mut(e).despawn();
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_open_and_entity_track_roots() {
        // Two distinct real entities (Entity has no public constructor in 0.18).
        let mut world = World::new();
        let a = world.spawn_empty().id();
        let b = world.spawn_empty().id();

        let mut stack = OverlayStack::default();
        stack.roots.push(a);
        stack.ids.push("pause".into());
        stack.roots.push(b);
        stack.ids.push("settings".into());

        assert!(stack.is_open("pause"));
        assert!(stack.contains("settings"));
        assert!(!stack.is_open("credits"));
        assert_eq!(stack.entity("settings"), Some(b));
        assert_eq!(stack.entity("missing"), None);
        assert_eq!(stack.top(), Some(b));
        assert_eq!(stack.depth(), 2);
    }
}
