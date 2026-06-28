//! Open/close transitions and the overlay lifecycle.
//!
//! An overlay eases in when spawned and eases out when dismissed, instead of
//! popping. [`Transition`] rides on the overlay root for its whole life and
//! animates two things off a single 0→1 progress value `t`: the scrim's alpha
//! and the body's scale (built-in panel or content wrapper). `t` eases toward
//! `target` — 1 while open, flipped to 0 to close. When a closing overlay
//! reaches 0 the root despawns, which prunes the stack and releases the input
//! gate exactly as a direct despawn does.
//!
//! Two close paths, by design:
//! - the **dismiss API** ([`request_close`], used by `dismiss_overlay`, the
//!   Escape handler and the scrim tap) eases out, then despawns;
//! - a **direct `despawn()`** (e.g. a state machine on `OnExit`) closes instantly.

use bevy::prelude::*;

use crate::events::{CloseReason, CloseReasons};

/// Tags the foreground node (built-in panel or content wrapper) so the
/// transition scales it without scaling the full-screen scrim.
#[derive(Component)]
pub(crate) struct OverlayBody;

/// Drives one overlay's open/close animation. `target` is 1 while open; setting
/// it to 0 (via [`request_close`]) plays the exit and despawns the root.
#[derive(Component)]
pub(crate) struct Transition {
    /// Eased progress: 0 = fully gone, 1 = fully shown.
    t: f32,
    /// Where `t` is easing toward (1 open, 0 closing → despawn).
    target: f32,
    open_rate: f32,
    close_rate: f32,
    scrim: Entity,
    /// The scrim's fully-open colour; alpha is scaled by the eased progress.
    scrim_color: Color,
    body: Entity,
    /// Body scale at `t = 0`; eases to 1.0 at `t = 1`.
    scale_from: f32,
}

impl Transition {
    pub(crate) fn opening(
        scrim: Entity,
        scrim_color: Color,
        body: Entity,
        open_secs: f32,
        close_secs: f32,
        scale_from: f32,
    ) -> Self {
        Self {
            t: 0.0,
            target: 1.0,
            open_rate: 1.0 / open_secs.max(1.0e-4),
            close_rate: 1.0 / close_secs.max(1.0e-4),
            scrim,
            scrim_color,
            body,
            scale_from,
        }
    }
}

/// Begin closing the overlay `root` for `reason`: ease out, then despawn. The
/// reason is stashed so [`OverlayClosed`](crate::events::OverlayClosed) can report
/// it when the root finally leaves the stack. Idempotent — a later call just
/// overwrites the reason and re-targets the (already-closing) animation. An
/// overlay with no [`Transition`] is despawned outright, so callers never special-
/// case the instant path.
pub(crate) fn request_close(world: &mut World, root: Entity, reason: CloseReason) {
    if world.get_entity(root).is_err() {
        return; // already gone
    }
    world.resource_mut::<CloseReasons>().0.insert(root, reason);
    if let Some(mut transition) = world.get_mut::<Transition>(root) {
        transition.target = 0.0;
    } else {
        world.entity_mut(root).despawn();
    }
}

/// Advance every overlay's transition: ease the scrim alpha and body scale, and
/// despawn a fully-closed overlay (its `Overlay` removal reconciles the stack and
/// the input gate).
pub(crate) fn drive_transitions(
    time: Res<Time>,
    mut transitions: Query<(Entity, &mut Transition)>,
    mut backgrounds: Query<&mut BackgroundColor>,
    mut bodies: Query<&mut UiTransform>,
    mut commands: Commands,
) {
    let dt = time.delta_secs();
    for (root, mut transition) in transitions.iter_mut() {
        let rate = if transition.target >= transition.t {
            transition.open_rate
        } else {
            transition.close_rate
        };
        transition.t = move_toward(transition.t, transition.target, rate * dt);
        let eased = smoothstep(transition.t);

        if let Ok(mut bg) = backgrounds.get_mut(transition.scrim) {
            bg.0 = transition
                .scrim_color
                .with_alpha(transition.scrim_color.alpha() * eased);
        }
        if let Ok(mut xform) = bodies.get_mut(transition.body) {
            xform.scale =
                Vec2::splat(transition.scale_from + (1.0 - transition.scale_from) * eased);
        }

        if transition.target == 0.0 && transition.t <= 0.0 {
            commands.entity(root).despawn();
        }
    }
}

fn move_toward(current: f32, target: f32, step: f32) -> f32 {
    if current < target {
        (current + step).min(target)
    } else {
        (current - step).max(target)
    }
}

/// Smoothstep ease (gentle in and out) over a clamped 0→1 input.
fn smoothstep(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}
