//! Headless integration tests for the overlay stack, the input gate, escape
//! popping, open/close transitions and deterministic layering. Everything runs
//! under `MinimalPlugins` with no renderer — spawning `Node` components is fine
//! headless, and the stack/gate bookkeeping is what we assert on.
//!
//! Dismissing now *animates* the close, so a fixed manual time step is inserted
//! and a [`settle`] helper advances enough frames to finish the exit before we
//! assert the overlay is gone.

use std::time::Duration;

use bevy::ecs::system::RunSystemOnce;
use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;

use bevy::ui::FocusPolicy;

use crate::events::{CloseReason, OverlayClosed, OverlayOpened};
use crate::focus::{Focusable, Focused};
use crate::scrim::Scrim;
use crate::stack::{Z_BASE, Z_STEP};
use crate::toast::{Toast, ToastLayer};
use crate::transition::OverlayBody;
use crate::{
    ModalPlugin, OverlayCommandsExt, OverlayStack, SafeAreaInsets, Theme, UiCapturing, overlay,
    toast,
};

/// A headless app with the plugin wired, a keyboard resource (which
/// `MinimalPlugins` omits but `escape_pops_top` reads), and a fixed 200ms manual
/// time step so transitions advance deterministically (200ms > the default open
/// and close durations, so one settled frame completes either).
fn test_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins).add_plugins(ModalPlugin);
    app.init_resource::<ButtonInput<KeyCode>>();
    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
        200,
    )));
    app
}

/// Advance a handful of frames so any in-flight open/close transition completes
/// and the resulting despawn is pruned.
fn settle(app: &mut App) {
    for _ in 0..4 {
        app.update();
    }
}

fn depth(app: &App) -> usize {
    app.world().resource::<OverlayStack>().depth()
}

fn capturing(app: &App) -> bool {
    app.world().resource::<UiCapturing>().0
}

/// The focusable buttons of `overlay`, in navigation order.
fn focusables_of(app: &mut App, overlay: Entity) -> Vec<Entity> {
    let mut query = app.world_mut().query::<(Entity, &Focusable)>();
    let mut found: Vec<(usize, Entity)> = query
        .iter(app.world())
        .filter(|(_, f)| f.overlay == overlay)
        .map(|(e, f)| (f.order, e))
        .collect();
    found.sort_by_key(|(order, _)| *order);
    found.into_iter().map(|(_, e)| e).collect()
}

fn is_focused(app: &App, entity: Entity) -> bool {
    app.world().get::<Focused>(entity).is_some()
}

/// Accumulates lifecycle messages so assertions don't race the message buffer.
#[derive(Resource, Default, Clone)]
struct EventLog {
    opened: Vec<String>,
    closed: Vec<(String, CloseReason)>,
}

fn record_events(
    mut log: ResMut<EventLog>,
    mut opened: MessageReader<OverlayOpened>,
    mut closed: MessageReader<OverlayClosed>,
) {
    for OverlayOpened(id) in opened.read() {
        log.opened.push(id.clone());
    }
    for closed in closed.read() {
        log.closed.push((closed.id.clone(), closed.reason));
    }
}

/// A [`test_app`] that records lifecycle messages into [`EventLog`].
fn event_app() -> App {
    let mut app = test_app();
    app.init_resource::<EventLog>();
    app.add_systems(Update, record_events);
    app
}

fn event_log(app: &App) -> EventLog {
    app.world().resource::<EventLog>().clone()
}

/// Press a key, run the frame that reacts to it, then fully release it. Without
/// `InputPlugin` nothing manages the input, so we release *and* clear — otherwise
/// the key stays in `pressed` and a later `press` of the same key never registers
/// as `just_pressed` again.
fn tap_key(app: &mut App, key: KeyCode) {
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(key);
    app.update();
    let mut input = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
    input.release(key);
    input.clear();
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

    // A direct despawn (e.g. a state machine on OnExit) closes instantly — no
    // transition involved.
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
    settle(&mut app);

    let stack = app.world().resource::<OverlayStack>();
    assert_eq!(stack.depth(), 1, "exactly one overlay dismissed");
    assert!(
        stack.is_open("b"),
        "the surviving overlay is the untouched one"
    );
    assert!(!stack.is_open("a"));
}

#[test]
fn dismiss_eases_out_before_despawning() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins).add_plugins(ModalPlugin);
    app.init_resource::<ButtonInput<KeyCode>>();
    // Small step so the default 0.12s close spans several frames and we can
    // observe it mid-flight.
    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
        20,
    )));

    app.world_mut()
        .run_system_once(|mut commands: Commands| {
            overlay(&mut commands, "x").title("T").push();
        })
        .unwrap();
    for _ in 0..12 {
        app.update(); // let it open
    }
    assert_eq!(depth(&app), 1);

    app.world_mut()
        .run_system_once(|mut commands: Commands| {
            commands.dismiss_overlay("x");
        })
        .unwrap();
    app.update(); // one 20ms tick into a 120ms close

    assert_eq!(
        depth(&app),
        1,
        "still easing out one frame in, not despawned"
    );
    assert!(capturing(&app), "the gate stays armed while it eases out");

    for _ in 0..12 {
        app.update(); // past the close
    }
    assert_eq!(depth(&app), 0, "despawns once the close completes");
    assert!(!capturing(&app), "the gate releases once it's gone");
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

#[test]
fn opening_focuses_the_first_button() {
    let mut app = test_app();
    app.world_mut()
        .run_system_once(|mut commands: Commands| {
            overlay(&mut commands, "m")
                .title("M")
                .button("A", |_| {})
                .button("B", |_| {})
                .push();
        })
        .unwrap();
    app.update(); // maintain_focus sets the initial focus

    let root = app.world().resource::<OverlayStack>().roots[0];
    let buttons = focusables_of(&mut app, root);
    assert_eq!(buttons.len(), 2);
    assert!(
        is_focused(&app, buttons[0]),
        "the first button is focused on open"
    );
    assert!(!is_focused(&app, buttons[1]));
}

#[test]
fn arrows_move_and_wrap_focus() {
    let mut app = test_app();
    app.world_mut()
        .run_system_once(|mut commands: Commands| {
            overlay(&mut commands, "m")
                .button("A", |_| {})
                .button("B", |_| {})
                .push();
        })
        .unwrap();
    app.update();
    let root = app.world().resource::<OverlayStack>().roots[0];
    let buttons = focusables_of(&mut app, root);

    tap_key(&mut app, KeyCode::ArrowDown);
    assert!(
        is_focused(&app, buttons[1]),
        "down moves to the next button"
    );

    tap_key(&mut app, KeyCode::ArrowDown);
    assert!(is_focused(&app, buttons[0]), "down wraps to the first");

    tap_key(&mut app, KeyCode::ArrowUp);
    assert!(is_focused(&app, buttons[1]), "up wraps to the last");
}

#[test]
fn enter_activates_the_focused_button() {
    #[derive(Resource, Default)]
    struct Hit(bool);

    let mut app = test_app();
    app.init_resource::<Hit>();
    app.world_mut()
        .run_system_once(|mut commands: Commands| {
            overlay(&mut commands, "m")
                .button("Go", |commands| {
                    commands.queue(|world: &mut World| world.resource_mut::<Hit>().0 = true);
                })
                .push();
        })
        .unwrap();
    app.update(); // focus the button

    tap_key(&mut app, KeyCode::Enter);
    assert!(
        app.world().resource::<Hit>().0,
        "Enter runs the focused button's action"
    );
}

#[test]
fn focus_follows_the_top_overlay() {
    let mut app = test_app();
    app.world_mut()
        .run_system_once(|mut commands: Commands| {
            overlay(&mut commands, "a").button("a0", |_| {}).push();
        })
        .unwrap();
    app.update();
    let a = app.world().resource::<OverlayStack>().entity("a").unwrap();
    let a0 = focusables_of(&mut app, a)[0];
    assert!(is_focused(&app, a0));

    app.world_mut()
        .run_system_once(|mut commands: Commands| {
            overlay(&mut commands, "b").button("b0", |_| {}).push();
        })
        .unwrap();
    app.update();
    let b = app.world().resource::<OverlayStack>().entity("b").unwrap();
    let b0 = focusables_of(&mut app, b)[0];
    assert!(is_focused(&app, b0), "focus moves to the new top overlay");
    assert!(
        !is_focused(&app, a0),
        "the lower overlay's button drops focus"
    );

    app.world_mut()
        .run_system_once(|mut commands: Commands| {
            commands.dismiss_overlay("b");
        })
        .unwrap();
    settle(&mut app);
    assert!(
        is_focused(&app, a0),
        "focus returns to the revealed overlay"
    );
}

#[test]
fn opening_emits_overlay_opened() {
    let mut app = event_app();
    app.world_mut()
        .run_system_once(|mut commands: Commands| {
            overlay(&mut commands, "pause").push();
        })
        .unwrap();
    app.update();
    app.update();
    assert_eq!(event_log(&app).opened, vec!["pause".to_string()]);
}

#[test]
fn dismiss_emits_closed_with_dismissed_reason() {
    let mut app = event_app();
    app.world_mut()
        .run_system_once(|mut commands: Commands| {
            overlay(&mut commands, "a").push();
        })
        .unwrap();
    app.update();

    app.world_mut()
        .run_system_once(|mut commands: Commands| {
            commands.dismiss_overlay("a");
        })
        .unwrap();
    settle(&mut app);

    assert_eq!(
        event_log(&app).closed,
        vec![("a".to_string(), CloseReason::Dismissed)]
    );
}

#[test]
fn escape_emits_closed_with_escape_reason() {
    let mut app = event_app();
    app.world_mut()
        .run_system_once(|mut commands: Commands| {
            overlay(&mut commands, "a").escape(true).push();
        })
        .unwrap();
    app.update();

    press_escape_once(&mut app);

    assert_eq!(
        event_log(&app).closed,
        vec![("a".to_string(), CloseReason::Escape)]
    );
}

#[test]
fn direct_despawn_emits_closed_with_despawned_reason() {
    let mut app = event_app();
    app.world_mut()
        .run_system_once(|mut commands: Commands| {
            overlay(&mut commands, "a").push();
        })
        .unwrap();
    app.update();

    let root = app.world().resource::<OverlayStack>().roots[0];
    app.world_mut().entity_mut(root).despawn();
    app.update();
    app.update();

    assert_eq!(
        event_log(&app).closed,
        vec![("a".to_string(), CloseReason::Despawned)]
    );
}

#[test]
fn action_button_is_added_only_with_an_action() {
    let mut app = test_app();

    app.world_mut()
        .run_system_once(|mut commands: Commands| {
            toast(&mut commands, "hi").push();
        })
        .unwrap();
    app.update();
    let plain = app
        .world_mut()
        .query_filtered::<Entity, With<Button>>()
        .iter(app.world())
        .count();
    assert_eq!(plain, 0, "a plain toast has no button");

    app.world_mut()
        .run_system_once(|mut commands: Commands| {
            toast(&mut commands, "Saved").action("Undo", |_| {}).push();
        })
        .unwrap();
    app.update();
    let with_action = app
        .world_mut()
        .query_filtered::<Entity, With<Button>>()
        .iter(app.world())
        .count();
    assert_eq!(with_action, 1, "an action toast adds one button");
}

#[test]
fn visible_toasts_are_capped() {
    let mut app = test_app(); // Theme::max_toasts defaults to 4
    app.world_mut()
        .run_system_once(|mut commands: Commands| {
            for i in 0..6 {
                toast(&mut commands, format!("t{i}")).push();
            }
        })
        .unwrap();
    app.update(); // cap_toasts dismisses the two oldest

    let count = app
        .world_mut()
        .query_filtered::<Entity, With<Toast>>()
        .iter(app.world())
        .count();
    assert_eq!(count, 4, "visible toasts are capped at max_toasts");
}

#[test]
fn scrim_blocks_the_interaction_path() {
    // `ui_focus_system` (which drives `Interaction` for ordinary `Button`s) only
    // stops at a `FocusPolicy::Block`. `Node` auto-adds `FocusPolicy::Pass`, so
    // the scrim must override it to Block — otherwise clicks fall through to the
    // buttons underneath. (The picking path is covered by `Pickable::default()`.)
    let mut app = test_app();
    app.world_mut()
        .run_system_once(|mut commands: Commands| {
            overlay(&mut commands, "m").push();
        })
        .unwrap();
    app.update();

    let mut query = app
        .world_mut()
        .query_filtered::<&FocusPolicy, With<Scrim>>();
    let policy = query.iter(app.world()).next().expect("a scrim");
    assert!(
        matches!(policy, FocusPolicy::Block),
        "scrim must block the Interaction path, not pass through"
    );
}

#[test]
fn content_spawns_caller_children_via_command_spawner() {
    let mut app = test_app();
    app.world_mut()
        .run_system_once(|mut commands: Commands| {
            overlay(&mut commands, "custom")
                .content(|parent: &mut ChildSpawnerCommands| {
                    parent.spawn(Node::default());
                    parent.spawn(Node::default());
                })
                .push();
        })
        .unwrap();
    // Content spawns via `Commands` (deferred); a couple of frames flush it.
    app.update();
    app.update();

    let mut query = app
        .world_mut()
        .query_filtered::<&Children, With<OverlayBody>>();
    let children = query.iter(app.world()).next().expect("a content body");
    assert_eq!(
        children.len(),
        2,
        "the caller's content children were spawned"
    );
}

#[test]
fn panel_caps_at_max_width() {
    let mut app = test_app();
    app.world_mut()
        .run_system_once(|mut commands: Commands| {
            overlay(&mut commands, "m")
                .title("M")
                .button("Ok", |_| {})
                .push();
        })
        .unwrap();
    app.update();

    let max = app.world().resource::<Theme>().panel_max_width;
    let mut query = app.world_mut().query_filtered::<&Node, With<OverlayBody>>();
    let panel = query.iter(app.world()).next().expect("a built-in panel");
    assert_eq!(panel.width, Val::Percent(82.0));
    assert_eq!(panel.max_width, Val::Px(max), "panel width is capped");
}

#[test]
fn safe_area_pads_the_overlay_root() {
    let mut app = test_app();
    app.insert_resource(SafeAreaInsets {
        top: 50.0,
        bottom: 30.0,
        left: 10.0,
        right: 10.0,
    });
    app.world_mut()
        .run_system_once(|mut commands: Commands| {
            overlay(&mut commands, "m").push();
        })
        .unwrap();
    app.update();

    let root = app.world().resource::<OverlayStack>().roots[0];
    let node = app.world().get::<Node>(root).expect("root node");
    assert_eq!(node.padding.top, Val::Px(50.0));
    assert_eq!(node.padding.bottom, Val::Px(30.0));
    assert_eq!(node.padding.left, Val::Px(10.0));
}

#[test]
fn safe_area_offsets_the_toast_layer() {
    let mut app = test_app();
    app.insert_resource(SafeAreaInsets {
        top: 50.0,
        ..Default::default()
    });
    app.world_mut()
        .run_system_once(|mut commands: Commands| {
            toast(&mut commands, "hi").push();
        })
        .unwrap();
    app.update();

    let mut query = app.world_mut().query_filtered::<&Node, With<ToastLayer>>();
    let node = query.iter(app.world()).next().expect("a toast layer");
    // Default top position: 16px margin + 50px inset.
    assert_eq!(node.top, Val::Px(66.0));
}

/// Presses Escape, runs the frame that requests the pop, clears the (un-managed)
/// keyboard state so it doesn't re-fire, then settles the close. Without
/// `InputPlugin` nothing clears `just_pressed`, so we clear it ourselves.
fn press_escape_once(app: &mut App) {
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::Escape);
    app.update();
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .clear();
    settle(app);
}
