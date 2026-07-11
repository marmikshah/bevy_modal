//! Focus and keyboard navigation for focusable widgets.
//!
//! A navigable widget gets a [`Focusable`] tagged with its **scope** and tab
//! order. A scope is either an overlay root or a standalone [`FocusScope`]
//! container (a title screen, a settings page) — so widgets navigate the same
//! way inside or outside an overlay. Exactly one widget is [`Focused`] at a
//! time, and only the **active** scope's focusables take part: the top overlay
//! if one is open, otherwise a standalone `FocusScope`. Tab / Shift+Tab and the
//! arrow keys move focus; Enter / Space activate the focused widget via its
//! [`ButtonAction`](crate::build::ButtonAction) — the very action a pointer
//! click runs, so pointer and keyboard share one path.

use bevy::prelude::*;

use crate::build::ButtonAction;
use crate::stack::OverlayStack;

/// Marks a container as a standalone keyboard-focus scope — a full-screen
/// destination (title, settings, HUD) whose [`Focusable`] widgets should
/// tab-navigate even though no overlay is open. Give each child widget this
/// entity as its `scope`. When an overlay is open it takes focus priority; focus
/// returns to the scope once the overlay closes.
#[derive(Component)]
pub struct FocusScope;

/// A navigable widget. `scope` is its owning overlay root or [`FocusScope`]
/// container; `order` is its tab position, for stable next/prev navigation.
#[derive(Component)]
pub(crate) struct Focusable {
    pub(crate) scope: Entity,
    pub(crate) order: usize,
}

/// The single currently-focused widget, if any.
#[derive(Component)]
pub(crate) struct Focused;

/// The scope that currently owns keyboard focus: the top overlay if one is open,
/// otherwise a standalone [`FocusScope`]. `None` when neither exists.
pub(crate) fn active_scope(
    stack: &OverlayStack,
    scopes: &Query<Entity, With<FocusScope>>,
) -> Option<Entity> {
    // An open overlay always wins. Among standalone scopes (an ambiguous case —
    // one active scope is the intended design) pick the lowest entity so the
    // choice is at least stable frame-to-frame rather than query-order-random.
    stack.top().or_else(|| scopes.iter().min())
}

/// Keep focus on the top overlay: when the top changes (open/close) or the
/// current focus is stale, move focus to the top overlay's first focusable; clear
/// it when no overlay (or no focusable) applies.
pub(crate) fn maintain_focus(
    stack: Res<OverlayStack>,
    scopes: Query<Entity, With<FocusScope>>,
    focusables: Query<(Entity, &Focusable)>,
    focused: Query<Entity, With<Focused>>,
    mut commands: Commands,
) {
    let active = active_scope(&stack, &scopes);
    let valid = focused.iter().next().is_some_and(|f| {
        focusables
            .get(f)
            .map(|(_, x)| Some(x.scope) == active)
            .unwrap_or(false)
    });
    if valid {
        return;
    }
    for entity in &focused {
        commands.entity(entity).remove::<Focused>();
    }
    if let Some(active) = active
        && let Some(first) = lowest_order(&focusables, active)
    {
        commands.entity(first).insert(Focused);
    }
}

/// Tab / Shift+Tab and the up/down arrows move focus within the top overlay,
/// wrapping at the ends.
pub(crate) fn navigate_focus(
    keys: Res<ButtonInput<KeyCode>>,
    stack: Res<OverlayStack>,
    scopes: Query<Entity, With<FocusScope>>,
    focusables: Query<(Entity, &Focusable)>,
    focused: Query<Entity, With<Focused>>,
    mut commands: Commands,
) {
    let Some(active) = active_scope(&stack, &scopes) else {
        return;
    };
    let shift = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    let next = keys.just_pressed(KeyCode::ArrowDown) || (keys.just_pressed(KeyCode::Tab) && !shift);
    let prev = keys.just_pressed(KeyCode::ArrowUp) || (keys.just_pressed(KeyCode::Tab) && shift);
    let dir: i32 = if next {
        1
    } else if prev {
        -1
    } else {
        return;
    };

    let mut list: Vec<(usize, Entity)> = focusables
        .iter()
        .filter(|(_, f)| f.scope == active)
        .map(|(e, f)| (f.order, e))
        .collect();
    if list.is_empty() {
        return;
    }
    list.sort_by_key(|(order, _)| *order);

    let current = focused.iter().find(|e| list.iter().any(|(_, le)| le == e));
    let idx = current
        .and_then(|c| list.iter().position(|(_, e)| *e == c))
        .unwrap_or(0);
    let target = (idx as i32 + dir).rem_euclid(list.len() as i32) as usize;

    for entity in &focused {
        commands.entity(entity).remove::<Focused>();
    }
    commands.entity(list[target].1).insert(Focused);
}

/// Enter / Space activate the focused button's action.
pub(crate) fn activate_focused(
    keys: Res<ButtonInput<KeyCode>>,
    focused: Query<Entity, With<Focused>>,
    mut actions: Query<&mut ButtonAction>,
    mut commands: Commands,
) {
    if !(keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::Space)) {
        return;
    }
    for entity in &focused {
        if let Ok(mut action) = actions.get_mut(entity) {
            action.run(&mut commands);
        }
    }
}

/// Hovering a button focuses it, so pointer and keyboard focus stay in agreement.
/// Only within the active scope: hovering a widget in an inactive scope must not
/// steal focus (and let an Enter/Space fire it before `maintain_focus` corrects).
#[allow(clippy::type_complexity)]
pub(crate) fn hover_focuses(
    stack: Res<OverlayStack>,
    scopes: Query<Entity, With<FocusScope>>,
    changed: Query<(Entity, &Interaction, &Focusable), Changed<Interaction>>,
    focused: Query<Entity, With<Focused>>,
    mut commands: Commands,
) {
    let active = active_scope(&stack, &scopes);
    for (entity, interaction, focusable) in &changed {
        if *interaction == Interaction::Hovered && Some(focusable.scope) == active {
            for other in &focused {
                if other != entity {
                    commands.entity(other).remove::<Focused>();
                }
            }
            commands.entity(entity).insert(Focused);
        }
    }
}

fn lowest_order(focusables: &Query<(Entity, &Focusable)>, scope: Entity) -> Option<Entity> {
    focusables
        .iter()
        .filter(|(_, f)| f.scope == scope)
        .min_by_key(|(_, f)| f.order)
        .map(|(entity, _)| entity)
}
