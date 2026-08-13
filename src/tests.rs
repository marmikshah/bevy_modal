//! Headless integration tests for the overlay stack, the input gate, escape
//! popping, open/close transitions and deterministic layering. Everything runs
//! under `MinimalPlugins` with no renderer — spawning `Node` components is fine
//! headless, and the stack/gate bookkeeping is what we assert on.
//!
//! Dismissing now *animates* the close, so a fixed manual time step is inserted
//! and a [`settle`] helper advances enough frames to finish the exit before we
//! assert the overlay is gone.

use std::time::Duration;

use bevy::asset::uuid_handle;
use bevy::ecs::system::RunSystemOnce;
use bevy::prelude::*;
use bevy::text::{LetterSpacing, LineHeight};
use bevy::time::TimeUpdateStrategy;

use bevy::ui::FocusPolicy;

use crate::events::{CloseReason, OverlayClosed, OverlayOpened};
use crate::focus::{Focusable, Focused};
use crate::stack::{Z_BASE, Z_STEP};
use crate::toast::{Toast, ToastLayer};
use crate::transition::OverlayBody;
use crate::{
    FocusScope, ModalPlugin, OverlayCommandsExt, OverlayStack, SafeAreaInsets, TextFace,
    TextMetrics, TextRole, TextTone, Theme, ThemedButton, ThemedPanel, ThemedScrim, ThemedText,
    Typography, UiCapturing, WidgetSpawnerExt, overlay, toast,
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

/// The focusable buttons of `scope`, in navigation order.
fn focusables_of(app: &mut App, scope: Entity) -> Vec<Entity> {
    let mut query = app.world_mut().query::<(Entity, &Focusable)>();
    let mut found: Vec<(usize, Entity)> = query
        .iter(app.world())
        .filter(|(_, f)| f.scope == scope)
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
        .query_filtered::<&FocusPolicy, With<ThemedScrim>>();
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

#[derive(Component)]
struct DynamicBodyField;

#[test]
fn body_tagged_attaches_caller_identity_at_spawn() {
    let mut app = test_app();
    app.world_mut()
        .run_system_once(|mut commands: Commands| {
            overlay(&mut commands, "settings")
                .body_tagged("PALETTE  DAY", DynamicBodyField)
                .push();
        })
        .unwrap();
    app.update();

    let mut fields = app
        .world_mut()
        .query_filtered::<&Text, With<DynamicBodyField>>();
    assert_eq!(fields.single(app.world()).unwrap().as_str(), "PALETTE  DAY");
}

#[test]
fn active_overlay_rethemes_in_place() {
    let mut app = test_app();
    app.world_mut()
        .run_system_once(|mut commands: Commands| {
            overlay(&mut commands, "settings")
                .title("SETTINGS")
                .body("Live presentation")
                .button("Apply", |_| {})
                .push();
        })
        .unwrap();
    settle(&mut app);
    let root = app.world().resource::<OverlayStack>().roots[0];

    {
        let mut theme = app.world_mut().resource_mut::<Theme>();
        theme.ink = Color::srgb(0.11, 0.22, 0.33);
        theme.line = Color::srgb(0.44, 0.55, 0.66);
        theme.text = Color::srgb(0.77, 0.78, 0.79);
        theme.text_dim = Color::srgb(0.61, 0.62, 0.63);
        theme.scrim = Color::srgba(0.21, 0.22, 0.23, 0.72);
        theme.accent = Color::srgb(0.82, 0.52, 0.12);
    }
    app.update();

    assert_eq!(app.world().resource::<OverlayStack>().roots, [root]);

    let mut panel = app
        .world_mut()
        .query_filtered::<(&BackgroundColor, &BorderColor), With<ThemedPanel>>();
    let (background, border) = panel.single(app.world()).unwrap();
    let theme = app.world().resource::<Theme>().clone();
    assert_eq!(background.0, theme.ink);
    assert_eq!(*border, BorderColor::all(theme.line));

    let mut text = app
        .world_mut()
        .query_filtered::<(&ThemedText, &TextColor), With<Text>>();
    for (role, color) in text.iter(app.world()) {
        let expected = match role.tone {
            TextTone::Primary => theme.text,
            TextTone::Secondary => theme.text_dim,
            TextTone::Accent => theme.accent,
            TextTone::Fixed(color) => color,
        };
        assert_eq!(color.0, expected);
    }

    let mut scrim = app
        .world_mut()
        .query_filtered::<&BackgroundColor, With<ThemedScrim>>();
    assert_eq!(scrim.single(app.world()).unwrap().0, theme.scrim);

    let mut button = app
        .world_mut()
        .query_filtered::<&BackgroundColor, With<ThemedButton>>();
    let button = button.single(app.world()).unwrap();
    assert_eq!(button.0, theme.accent.with_alpha(theme.btn_fill_hover));
}

#[test]
fn unchanged_theme_preserves_runtime_text_colour() {
    let mut app = test_app();
    let text = app
        .world_mut()
        .run_system_once(|mut commands: Commands, theme: Res<Theme>| {
            commands
                .spawn((
                    Text::new("Fading"),
                    theme.text_style(TextRole::OverlayBody, TextTone::Primary),
                ))
                .id()
        })
        .unwrap();
    app.update();

    app.world_mut()
        .get_mut::<TextColor>(text)
        .unwrap()
        .0
        .set_alpha(0.25);
    app.update();

    assert_eq!(app.world().get::<TextColor>(text).unwrap().0.alpha(), 0.25);
}

#[test]
fn every_emitted_text_surface_uses_configured_semantic_metrics() {
    let display: Handle<Font> = uuid_handle!("a262354e-39e3-4d5a-8685-688f24e18416");
    let body: Handle<Font> = uuid_handle!("6a542e62-4d35-459a-aa74-4d884fda7b4d");
    let typography = Typography {
        overlay_title: TextMetrics::new(TextFace::Display, 31.0, 1.01, 0.1),
        overlay_body: TextMetrics::new(TextFace::Body, 23.0, 1.02, 0.2),
        overlay_button: TextMetrics::new(TextFace::Body, 25.0, 1.03, 0.3),
        toast: TextMetrics::new(TextFace::Body, 21.0, 1.04, 0.4),
        toast_action: TextMetrics::new(TextFace::Body, 17.0, 1.05, 0.5),
        widget_button: TextMetrics::new(TextFace::Body, 19.0, 1.06, 0.6),
        widget_label: TextMetrics::new(TextFace::Body, 15.0, 1.07, 0.7),
    };

    let mut app = test_app();
    app.insert_resource(Theme {
        display: display.clone(),
        body: body.clone(),
        typography,
        ..Default::default()
    });
    app.world_mut()
        .run_system_once(|mut commands: Commands, theme: Res<Theme>| {
            overlay(&mut commands, "semantic")
                .title("Overlay title")
                .body("Overlay body")
                .button("Overlay button", |_| {})
                .push();
            toast(&mut commands, "Toast message")
                .action("Toast action", |_| {})
                .push();

            let scope = commands.spawn((FocusScope, Node::default())).id();
            commands.entity(scope).with_children(|parent| {
                parent.button(&theme, scope, 0, "Widget button", |_| {});
                parent.toggle(&theme, scope, 1, "Widget label", false, |_, _| {});
            });
        })
        .unwrap();
    app.update();

    let mut query = app
        .world_mut()
        .query::<(&Text, &ThemedText, &TextFont, &LineHeight, &LetterSpacing)>();
    let emitted: Vec<_> = query
        .iter(app.world())
        .map(|(text, role, font, line_height, letter_spacing)| {
            (
                text.as_str().to_string(),
                *role,
                font.clone(),
                *line_height,
                *letter_spacing,
            )
        })
        .collect();
    assert_eq!(
        emitted.len(),
        7,
        "every built-in text surface is role-owned"
    );

    for (text, style, font, line_height, letter_spacing) in emitted {
        let (expected_role, expected_tone) = match text.as_str() {
            "Overlay title" => (TextRole::OverlayTitle, TextTone::Accent),
            "Overlay body" => (TextRole::OverlayBody, TextTone::Secondary),
            "Overlay button" => (TextRole::OverlayButton, TextTone::Primary),
            "Toast message" => (TextRole::Toast, TextTone::Primary),
            "Toast action" => (
                TextRole::ToastAction,
                TextTone::Fixed(Theme::default().accent),
            ),
            "Widget button" => (TextRole::WidgetButton, TextTone::Primary),
            "Widget label" => (TextRole::WidgetLabel, TextTone::Primary),
            other => panic!("unexpected emitted text: {other}"),
        };
        assert_eq!(style.role, expected_role, "wrong role for {text}");
        assert_eq!(style.tone, expected_tone, "wrong tone for {text}");

        let metrics = typography.metrics(style.role);
        let expected_font: FontSource = match metrics.face {
            TextFace::Display => display.clone(),
            TextFace::Body => body.clone(),
        }
        .into();
        assert_eq!(font.font, expected_font, "wrong face for {text}");
        assert_eq!(
            font.font_size,
            FontSize::Px(metrics.font_size),
            "wrong size for {text}"
        );
        assert_eq!(
            line_height,
            LineHeight::RelativeToFont(metrics.line_height),
            "wrong line height for {text}"
        );
        assert_eq!(
            letter_spacing,
            LetterSpacing::Px(metrics.letter_spacing),
            "wrong letter spacing for {text}"
        );
    }

    // The semantic component also keeps an already-open entity live when the
    // caller swaps profiles or changes which family owns a role.
    {
        let mut theme = app.world_mut().resource_mut::<Theme>();
        theme.typography.overlay_body = TextMetrics::new(TextFace::Display, 37.0, 1.4, 1.25);
    }
    app.update();
    let (_, font, line_height, letter_spacing) = app
        .world_mut()
        .query::<(&Text, &TextFont, &LineHeight, &LetterSpacing)>()
        .iter(app.world())
        .find(|(text, _, _, _)| text.as_str() == "Overlay body")
        .expect("overlay body remains open");
    assert_eq!(font.font, FontSource::from(display));
    assert_eq!(font.font_size, FontSize::Px(37.0));
    assert_eq!(*line_height, LineHeight::RelativeToFont(1.4));
    assert_eq!(*letter_spacing, LetterSpacing::Px(1.25));
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
