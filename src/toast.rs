//! Toasts — transient, non-blocking notifications. Unlike an overlay, a toast
//! draws no scrim, never touches [`OverlayStack`](crate::OverlayStack) or
//! [`UiCapturing`](crate::UiCapturing), and auto-dismisses when its timer elapses
//! (or on tap). They stack in a single column — pinned to the top or bottom edge
//! per [`Theme::toast_position`](crate::Theme) — so multiple toasts pile
//! deterministically rather than overdrawing one another. A [`ToastLevel`] picks
//! the accent border from the theme's semantic colours, `.action(..)` adds an
//! optional button, and at most `Theme::max_toasts` show at once (oldest first
//! out).
//!
//! ```no_run
//! use bevy::prelude::*;
//! use bevy_modal::prelude::*;
//! use std::time::Duration;
//!
//! fn notify(mut commands: Commands) {
//!     toast(&mut commands, "Saved").push();
//!     toast(&mut commands, "Upload failed")
//!         .level(ToastLevel::Error)
//!         .duration(Duration::from_secs(6))
//!         .push();
//! }
//! ```

use std::time::Duration;

use bevy::picking::prelude::*;
use bevy::prelude::*;

use crate::theme::Theme;

/// Toasts float above overlays; they're notifications, not modals.
pub(crate) const TOAST_Z: i32 = 100_000;

/// Gap between the toast column and its screen edge, before any safe-area inset.
pub(crate) const TOAST_EDGE_MARGIN: f32 = 16.0;

/// A toast action: a label and the callback its button runs (the toast dismisses
/// after). Boxed `FnMut` so the builder stays non-generic, like the overlay one.
type ToastAction = (String, Box<dyn FnMut(&mut Commands) + Send + Sync>);

/// Default lifetime when a builder doesn't set one.
const DEFAULT_DURATION: Duration = Duration::from_secs(4);

/// A toast's severity, which selects its accent border from the [`Theme`].
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum ToastLevel {
    #[default]
    Info,
    Success,
    Warning,
    Error,
}

impl ToastLevel {
    fn color(self, theme: &Theme) -> Color {
        match self {
            ToastLevel::Info => theme.accent,
            ToastLevel::Success => theme.success,
            ToastLevel::Warning => theme.warning,
            ToastLevel::Error => theme.danger,
        }
    }
}

/// Which screen edge toasts stack against. Set on the [`Theme`]; fixed by the
/// first toast that spawns the shared layer.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum ToastPosition {
    #[default]
    Top,
    Bottom,
}

/// Tags a toast node so [`expire_toasts`] can find it.
#[derive(Component)]
pub(crate) struct Toast;

/// Counts down a toast's life; despawned by [`expire_toasts`] when it finishes.
#[derive(Component)]
pub(crate) struct ToastTimer(Timer);

/// The shared top-anchored column that toasts are parented under. Spawned lazily
/// on the first toast and reused thereafter.
#[derive(Component)]
pub(crate) struct ToastLayer;

/// Start a toast with the given message. `.push()` queues the spawn.
pub fn toast<'a, 'w, 's>(
    commands: &'a mut Commands<'w, 's>,
    message: impl Into<String>,
) -> ToastBuilder<'a, 'w, 's> {
    ToastBuilder {
        commands,
        spec: SpawnToast {
            message: message.into(),
            accent: None,
            level: ToastLevel::Info,
            action: None,
            duration: DEFAULT_DURATION,
        },
    }
}

/// Chained toast builder. Mirrors the overlay builder's shape; `.push()`
/// enqueues the spawn.
pub struct ToastBuilder<'a, 'w, 's> {
    commands: &'a mut Commands<'w, 's>,
    spec: SpawnToast,
}

impl ToastBuilder<'_, '_, '_> {
    /// How long the toast stays up before auto-dismissing. Defaults to 4s.
    pub fn duration(mut self, duration: Duration) -> Self {
        self.spec.duration = duration;
        self
    }

    /// Severity, which picks the accent border from the theme. Defaults to
    /// `Info`. Ignored if [`accent`](Self::accent) is set explicitly.
    pub fn level(mut self, level: ToastLevel) -> Self {
        self.spec.level = level;
        self
    }

    /// Override the accent border tint outright. Defaults to the `level` colour.
    pub fn accent(mut self, color: Color) -> Self {
        self.spec.accent = Some(color);
        self
    }

    /// Add an action button. `on_click` runs on press with `&mut Commands`; the
    /// toast dismisses afterwards (e.g. an "Undo").
    pub fn action(
        mut self,
        label: impl Into<String>,
        on_click: impl FnMut(&mut Commands) + Send + Sync + 'static,
    ) -> Self {
        self.spec.action = Some((label.into(), Box::new(on_click)));
        self
    }

    /// Queue the spawn. Nothing happens until command application.
    pub fn push(self) {
        self.commands.queue(self.spec);
    }
}

/// The deferred toast spawn; built against `&mut World` so it can read the theme
/// and find-or-spawn the shared layer synchronously.
struct SpawnToast {
    message: String,
    accent: Option<Color>,
    level: ToastLevel,
    action: Option<ToastAction>,
    duration: Duration,
}

impl Command for SpawnToast {
    type Out = ();
    fn apply(self, world: &mut World) {
        let theme = world.resource::<Theme>().clone();
        let accent = self.accent.unwrap_or_else(|| self.level.color(&theme));

        let layer = find_or_spawn_layer(world, theme.toast_position);

        let toast = world
            .spawn((
                Toast,
                ToastTimer(Timer::new(self.duration, TimerMode::Once)),
                Node {
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::axes(Val::Px(16.0), Val::Px(10.0)),
                    border: UiRect::all(Val::Px(theme.button_border)),
                    ..default()
                },
                BackgroundColor(theme.ink),
                BorderColor::all(accent),
                Pickable::default(),
            ))
            .id();
        let label = world
            .spawn((
                Text::new(self.message),
                TextFont {
                    font: theme.body.clone().into(),
                    font_size: FontSize::Px(20.0),
                    ..default()
                },
                TextColor(theme.text),
            ))
            .id();
        world.entity_mut(toast).add_child(label);
        world.entity_mut(layer).add_child(toast);

        // Optional action button. Its click runs the callback; the click then
        // bubbles to the toast's dismiss observer below, so the toast closes once
        // (no separate despawn here).
        if let Some((text, mut on_click)) = self.action {
            let button = world
                .spawn((
                    Node {
                        margin: UiRect::top(Val::Px(8.0)),
                        align_self: AlignSelf::Start,
                        padding: UiRect::axes(Val::Px(10.0), Val::Px(4.0)),
                        border: UiRect::all(Val::Px(theme.button_border)),
                        ..default()
                    },
                    Button,
                    BackgroundColor(accent.with_alpha(0.16)),
                    BorderColor::all(accent),
                ))
                .id();
            let button_label = world
                .spawn((
                    Text::new(text),
                    TextFont {
                        font: theme.body.clone().into(),
                        font_size: FontSize::Px(16.0),
                        ..default()
                    },
                    TextColor(accent),
                ))
                .id();
            world.entity_mut(button).add_child(button_label);
            world.entity_mut(toast).add_child(button);
            world.entity_mut(button).observe(
                move |_: On<Pointer<Click>>, mut commands: Commands| {
                    on_click(&mut commands);
                },
            );
        }

        // Tap to dismiss early (also fires for a bubbled action-button click).
        // `try_despawn`: a tap can race the timer expiry or the cap on the same
        // frame, so this despawn may find the toast already gone — no warning.
        world
            .entity_mut(toast)
            .observe(move |_: On<Pointer<Click>>, mut commands: Commands| {
                commands.entity(toast).try_despawn();
            });
    }
}

/// Reuse the existing toast column if one is up, else spawn it, anchored to the
/// configured edge and laid out as a centred column with newest toasts last.
fn find_or_spawn_layer(world: &mut World, position: ToastPosition) -> Entity {
    let mut existing = world.query_filtered::<Entity, With<ToastLayer>>();
    if let Some(layer) = existing.iter(world).next() {
        return layer;
    }
    let mut node = Node {
        position_type: PositionType::Absolute,
        left: Val::Px(0.0),
        width: Val::Percent(100.0),
        flex_direction: FlexDirection::Column,
        align_items: AlignItems::Center,
        row_gap: Val::Px(10.0),
        ..default()
    };
    match position {
        ToastPosition::Top => node.top = Val::Px(TOAST_EDGE_MARGIN),
        ToastPosition::Bottom => node.bottom = Val::Px(TOAST_EDGE_MARGIN),
    }
    world.spawn((ToastLayer, node, GlobalZIndex(TOAST_Z))).id()
}

/// Ticks every toast's timer; despawns it (recursively, taking its label) when
/// the timer finishes. Driven by `Time`, so it advances with whatever clock the
/// app is running — including a manual one in tests.
pub(crate) fn expire_toasts(
    time: Res<Time>,
    mut toasts: Query<(Entity, &mut ToastTimer)>,
    mut commands: Commands,
) {
    for (entity, mut timer) in toasts.iter_mut() {
        if timer.0.tick(time.delta()).just_finished() {
            // May race a tap-dismiss or the cap on this frame; tolerate a gone
            // entity rather than log `entity does not exist`.
            commands.entity(entity).try_despawn();
        }
    }
}

/// Cap the visible toast count, dismissing the oldest first. The layer's children
/// are in spawn order, so the front of the list is the oldest.
pub(crate) fn cap_toasts(
    theme: Res<Theme>,
    layers: Query<&Children, With<ToastLayer>>,
    mut commands: Commands,
) {
    if theme.max_toasts == 0 {
        return;
    }
    for children in &layers {
        if children.len() > theme.max_toasts {
            for &old in &children[..children.len() - theme.max_toasts] {
                // May race a tap-dismiss or the timer expiry on this frame;
                // tolerate a gone entity rather than warn.
                commands.entity(old).try_despawn();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ModalPlugin;
    use bevy::ecs::system::RunSystemOnce;
    use bevy::time::TimeUpdateStrategy;

    fn count_toasts(app: &mut App) -> usize {
        app.world_mut()
            .query_filtered::<Entity, With<Toast>>()
            .iter(app.world())
            .count()
    }

    #[test]
    fn toast_auto_expires_after_its_duration() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins).add_plugins(ModalPlugin);
        // `escape_pops_top` reads keyboard input, absent under `MinimalPlugins`.
        app.init_resource::<ButtonInput<KeyCode>>();
        // Each `update()` advances virtual time by exactly 100ms — deterministic.
        // (Kept under `Time<Virtual>`'s 250ms `max_delta` clamp so the manual
        // step isn't truncated.)
        app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
            100,
        )));

        app.world_mut()
            .run_system_once(|mut commands: Commands| {
                toast(&mut commands, "saved")
                    .duration(Duration::from_millis(300))
                    .push();
            })
            .unwrap();

        // First tick: spawn command applied, timer at 100ms of 300ms — still up.
        app.update();
        assert_eq!(count_toasts(&mut app), 1, "toast should be up");

        // Advance comfortably past 300ms; the timer finishes and it despawns.
        for _ in 0..5 {
            app.update();
        }
        assert_eq!(count_toasts(&mut app), 0, "toast should have expired");
    }

    #[test]
    fn double_dismiss_is_a_noop() {
        // A tap racing the timer expiry / cap can queue two despawns of the same
        // toast in one frame. The second must see a gone entity quietly (no
        // `entity does not exist`), leaving the toast count at zero.
        let mut app = app_with_modal();
        app.world_mut()
            .run_system_once(|mut commands: Commands| {
                toast(&mut commands, "saved").push();
            })
            .unwrap();
        app.update(); // apply the spawn

        let toast = app
            .world_mut()
            .query_filtered::<Entity, With<Toast>>()
            .iter(app.world())
            .next()
            .expect("a toast spawned");

        app.world_mut()
            .run_system_once(move |mut commands: Commands| {
                commands.entity(toast).try_despawn();
                commands.entity(toast).try_despawn();
            })
            .unwrap();

        assert_eq!(count_toasts(&mut app), 0, "the toast is gone, once");
    }

    #[test]
    fn toasts_never_capture_input() {
        use crate::UiCapturing;

        let mut app = App::new();
        app.add_plugins(MinimalPlugins).add_plugins(ModalPlugin);
        app.init_resource::<ButtonInput<KeyCode>>();
        app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs(1)));

        app.world_mut()
            .run_system_once(|mut commands: Commands| {
                toast(&mut commands, "fyi").push();
            })
            .unwrap();
        app.update();

        assert_eq!(count_toasts(&mut app), 1);
        assert!(
            !app.world().resource::<UiCapturing>().0,
            "a toast must not arm the input gate"
        );
    }

    fn app_with_modal() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins).add_plugins(ModalPlugin);
        app.init_resource::<ButtonInput<KeyCode>>();
        app
    }

    fn toast_border(app: &mut App) -> BorderColor {
        let mut query = app
            .world_mut()
            .query_filtered::<&BorderColor, With<Toast>>();
        *query.iter(app.world()).next().expect("a toast spawned")
    }

    #[test]
    fn level_picks_the_theme_colour() {
        let mut app = app_with_modal();
        app.world_mut()
            .run_system_once(|mut commands: Commands| {
                toast(&mut commands, "boom").level(ToastLevel::Error).push();
            })
            .unwrap();

        let danger = app.world().resource::<Theme>().danger;
        assert_eq!(toast_border(&mut app), BorderColor::all(danger));
    }

    #[test]
    fn explicit_accent_overrides_level() {
        let mut app = app_with_modal();
        app.world_mut()
            .run_system_once(|mut commands: Commands| {
                toast(&mut commands, "boom")
                    .level(ToastLevel::Error)
                    .accent(Color::WHITE)
                    .push();
            })
            .unwrap();

        assert_eq!(toast_border(&mut app), BorderColor::all(Color::WHITE));
    }

    #[test]
    fn bottom_position_anchors_the_layer() {
        let mut app = app_with_modal();
        app.insert_resource(Theme {
            toast_position: ToastPosition::Bottom,
            ..Default::default()
        });

        app.world_mut()
            .run_system_once(|mut commands: Commands| {
                toast(&mut commands, "hi").push();
            })
            .unwrap();

        let mut query = app.world_mut().query_filtered::<&Node, With<ToastLayer>>();
        let node = query.iter(app.world()).next().expect("a toast layer");
        assert_eq!(node.bottom, Val::Px(16.0), "anchored to the bottom edge");
        assert_eq!(node.top, Val::Auto, "not anchored to the top");
    }
}
