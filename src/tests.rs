//! Headless integration tests for the overlay stack, the input gate, escape
//! popping and deterministic layering. Everything runs under `MinimalPlugins`
//! with no renderer — spawning `Node` components is fine headless, and the
//! stack/gate bookkeeping is what we assert on.

use bevy::ecs::system::RunSystemOnce;
use bevy::prelude::*;

use crate::stack::{Z_BASE, Z_STEP};
use crate::{ModalPlugin, OverlayCommandsExt, OverlayStack, UiCapturing, overlay};

/// A headless app with the plugin wired and a keyboard resource (which
/// `MinimalPlugins` omits but `escape_pops_top` reads).
fn test_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins).add_plugins(ModalPlugin);
    app.init_resource::<ButtonInput<KeyCode>>();
    app
}

fn depth(app: &App) -> usize {
    app.world().resource::<OverlayStack>().depth()
}

fn capturing(app: &App) -> bool {
    app.world().resource::<UiCapturing>().0
}

#[test]
fn push_registers_root_and_arms_gate_then_prune_releases_it() {
    let mut app = test_app();

    app.world_mut()
        .run_system_once(|mut commands: Commands| {
            overlay(&mut commands, "pause").title("PAUSED").push();
        })
        .unwrap();

    assert_eq!(depth(&app), 1, "push should register one root");
    assert!(capturing(&app), "an open overlay arms the input gate");

    let root = app.world().resource::<OverlayStack>().roots[0];
    app.world_mut().entity_mut(root).despawn();

    // Prune is removal-driven: one update to read the removal and reconcile.
    app.update();

    assert_eq!(depth(&app), 0, "despawning the root prunes the stack");
    assert!(!capturing(&app), "the last close releases the gate");
}

#[test]
fn push_unique_dedups_by_id() {
    let mut app = test_app();

    app.world_mut()
        .run_system_once(|mut commands: Commands| {
            overlay(&mut commands, "settings").push_unique();
            overlay(&mut commands, "settings").push_unique();
        })
        .unwrap();

    assert_eq!(depth(&app), 1, "same id should spawn only once");
}

#[test]
fn dismiss_overlay_despawns_the_right_one_and_reconciles() {
    let mut app = test_app();

    app.world_mut()
        .run_system_once(|mut commands: Commands| {
            overlay(&mut commands, "a").push();
            overlay(&mut commands, "b").push();
        })
        .unwrap();
    assert_eq!(depth(&app), 2);

    app.world_mut()
        .run_system_once(|mut commands: Commands| {
            commands.dismiss_overlay("a");
        })
        .unwrap();
    app.update();

    let stack = app.world().resource::<OverlayStack>();
    assert_eq!(stack.depth(), 1, "exactly one overlay dismissed");
    assert!(
        stack.is_open("b"),
        "the surviving overlay is the untouched one"
    );
    assert!(!stack.is_open("a"));
}

#[test]
fn escape_pops_only_the_top_when_it_opted_in() {
    let mut app = test_app();

    app.world_mut()
        .run_system_once(|mut commands: Commands| {
            overlay(&mut commands, "bottom").escape(false).push();
            overlay(&mut commands, "top").escape(true).push();
        })
        .unwrap();
    assert_eq!(depth(&app), 2);

    press_escape_once(&mut app);

    let stack = app.world().resource::<OverlayStack>();
    assert_eq!(stack.depth(), 1, "escape pops exactly one");
    assert!(stack.is_open("bottom"), "only the top was popped");
    assert!(!stack.is_open("top"));
}

#[test]
fn escape_does_nothing_when_the_top_did_not_opt_in() {
    let mut app = test_app();

    app.world_mut()
        .run_system_once(|mut commands: Commands| {
            overlay(&mut commands, "bottom").escape(true).push();
            overlay(&mut commands, "top").escape(false).push();
        })
        .unwrap();
    assert_eq!(depth(&app), 2);

    press_escape_once(&mut app);

    assert_eq!(
        depth(&app),
        2,
        "a non-opted-in top blocks escape even if a lower one opted in"
    );
}

#[test]
fn z_index_is_deterministic_per_depth() {
    let mut app = test_app();

    app.world_mut()
        .run_system_once(|mut commands: Commands| {
            overlay(&mut commands, "a").push();
            overlay(&mut commands, "b").push();
            overlay(&mut commands, "c").push();
        })
        .unwrap();

    let roots = app.world().resource::<OverlayStack>().roots.clone();
    for (i, root) in roots.iter().enumerate() {
        let z = app.world().get::<GlobalZIndex>(*root).expect("root has z");
        assert_eq!(
            z.0,
            Z_BASE + i as i32 * Z_STEP,
            "depth {i} gets the deterministic z floor",
        );
    }
}

/// Presses Escape, runs the frame that pops, then clears the (un-managed)
/// keyboard state and runs the frame that prunes. Without `InputPlugin` nothing
/// clears `just_pressed`, so we clear it ourselves to avoid re-firing.
fn press_escape_once(app: &mut App) {
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::Escape);
    app.update();
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .clear();
    app.update();
}
