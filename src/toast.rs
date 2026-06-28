//! Toasts — transient, non-blocking notifications. Unlike an overlay, a toast
//! draws no scrim, never touches [`OverlayStack`](crate::OverlayStack) or
//! [`UiCapturing`](crate::UiCapturing), and auto-dismisses when its timer
//! elapses. They stack in a single top-anchored column so multiple toasts pile
//! deterministically rather than overdrawing one another.
//!
//! ```no_run
//! use bevy::prelude::*;
//! use bevy_modal::prelude::*;
//! use std::time::Duration;
//!
//! fn notify(mut commands: Commands) {
//!     toast(&mut commands, "Saved").duration(Duration::from_secs(2)).push();
//! }
//! ```

use std::time::Duration;

use bevy::prelude::*;

use crate::theme::Theme;

/// Toasts float above overlays; they're notifications, not modals.
pub(crate) const TOAST_Z: i32 = 100_000;

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
    duration: Duration,
}

impl Command for SpawnToast {
    type Out = ();
    fn apply(self, world: &mut World) {
        let theme = world.resource::<Theme>().clone();
        let accent = self.accent.unwrap_or_else(|| self.level.color(&theme));

        let layer = find_or_spawn_layer(world);

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
    }
}

/// Reuse the existing toast column if one is up, else spawn it. Anchored to the
/// top-centre and laid out as a column so toasts stack top-to-bottom.
fn find_or_spawn_layer(world: &mut World) -> Entity {
    let mut existing = world.query_filtered::<Entity, With<ToastLayer>>();
    if let Some(layer) = existing.iter(world).next() {
        return layer;
    }
    world
        .spawn((
            ToastLayer,
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(16.0),
                left: Val::Px(0.0),
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: Val::Px(10.0),
                ..default()
            },
            GlobalZIndex(TOAST_Z),
        ))
        .id()
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
            commands.entity(entity).despawn();
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
}
