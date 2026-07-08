//! Standalone themed widgets — buttons, toggles, sliders and scrollable lists
//! that spawn anywhere a [`ChildSpawnerCommands`] does, styled from the same
//! [`Theme`] the overlay panel uses.
//!
//! The modal layer (overlays, confirm, toasts) covers popups, but full-screen
//! destinations — title screens, settings pages, HUD chrome — still need leaf
//! controls. Rather than hand-roll press-state styling and dispatch on raw
//! `Node`s, spawn these: they carry the crate's rest/hover/press skin, take part
//! in keyboard focus navigation (inside an overlay or under a standalone
//! [`FocusScope`]), and drive a callback.
//!
//! These are leaf widgets, not a retained layout DSL — you own the layout and
//! drop widgets into it.
//!
//! ```no_run
//! use bevy::prelude::*;
//! use bevy_modal::prelude::*;
//!
//! fn settings_screen(mut commands: Commands, theme: Res<Theme>) {
//!     let scope = commands.spawn((FocusScope, Node::default())).id();
//!     commands.entity(scope).with_children(|p| {
//!         p.button(&theme, scope, 0, "Play", |c| { c.queue(|_w: &mut World| {}); });
//!         p.toggle(&theme, scope, 1, "Sound", true, |_c, on| { let _ = on; });
//!         p.slider(&theme, scope, 2, 0.0..=1.0, 0.8, |_c, v| { let _ = v; });
//!     });
//! }
//! ```

use std::ops::RangeInclusive;

use bevy::input::mouse::MouseWheel;
use bevy::picking::prelude::*;
use bevy::prelude::*;
use bevy::ui::RelativeCursorPosition;

use crate::build::{ButtonAction, button_visual};
use crate::focus::{Focusable, Focused};
use crate::theme::Theme;

// ---- Button ----

type ClickCb = Box<dyn FnMut(&mut Commands) + Send + Sync>;
type ToggleCb = Box<dyn FnMut(&mut Commands, bool) + Send + Sync>;
type SliderCb = Box<dyn FnMut(&mut Commands, f32) + Send + Sync>;

// ---- Toggle ----

/// A two-state switch. Read `on` to observe its state; flipped by a pointer
/// click or by Enter/Space while focused, each running the widget's callback.
#[derive(Component)]
pub struct Toggle {
    /// Whether the switch is currently on.
    pub on: bool,
    accent: Color,
}

#[derive(Component)]
pub(crate) struct ToggleAction(ToggleCb);

/// Tags the sliding knob inside a [`Toggle`] so its skin system can move it.
#[derive(Component)]
pub(crate) struct ToggleKnob;

// ---- Slider ----

/// A draggable value in `[min, max]`. Read `value`; set by dragging the track or
/// by Left/Right arrows while focused, each running the widget's callback.
#[derive(Component)]
pub struct Slider {
    /// The current value, always within `[min, max]`.
    pub value: f32,
    min: f32,
    max: f32,
}

impl Slider {
    /// The value as a `0..=1` fraction across the range.
    pub fn fraction(&self) -> f32 {
        if self.max > self.min {
            (self.value - self.min) / (self.max - self.min)
        } else {
            0.0
        }
    }

    fn set_fraction(&mut self, f: f32) {
        self.value = self.min + f.clamp(0.0, 1.0) * (self.max - self.min);
    }
}

#[derive(Component)]
pub(crate) struct SliderAction(SliderCb);

/// Tags the filled portion of a [`Slider`] so its skin system can size it.
#[derive(Component)]
pub(crate) struct SliderFill;

// ---- Scrollable list ----

/// Marks a scrollable container: the wheel scrolls it while the pointer is over
/// it. Fill it with your own row children.
#[derive(Component)]
pub struct Scrollable;

/// Spawn standalone widgets as children — the extension the crate adds to the
/// `bevy_ui` child spawner. Each returns the widget's [`EntityCommands`] so you
/// can tweak it further (add a `Node` width, a margin, more children).
pub trait WidgetSpawnerExt {
    /// A themed button. `scope` is the widget's focus scope (an overlay root or a
    /// [`FocusScope`] container); `order` is its tab position. `on_click` runs on
    /// press or Enter/Space with `&mut Commands`.
    fn button(
        &mut self,
        theme: &Theme,
        scope: Entity,
        order: usize,
        label: impl Into<String>,
        on_click: impl FnMut(&mut Commands) + Send + Sync + 'static,
    ) -> EntityCommands<'_>;

    /// A themed on/off toggle starting at `initial`. `on_change` runs with the
    /// new state whenever it flips.
    fn toggle(
        &mut self,
        theme: &Theme,
        scope: Entity,
        order: usize,
        label: impl Into<String>,
        initial: bool,
        on_change: impl FnMut(&mut Commands, bool) + Send + Sync + 'static,
    ) -> EntityCommands<'_>;

    /// A themed slider over `range`, starting at `initial`. `on_change` runs with
    /// the new value while dragging or on an arrow step.
    fn slider(
        &mut self,
        theme: &Theme,
        scope: Entity,
        order: usize,
        range: RangeInclusive<f32>,
        initial: f32,
        on_change: impl FnMut(&mut Commands, f32) + Send + Sync + 'static,
    ) -> EntityCommands<'_>;

    /// A scrollable, themed container. Fill the returned entity with your own row
    /// children; the wheel scrolls it while hovered.
    fn list(&mut self, theme: &Theme) -> EntityCommands<'_>;
}

impl WidgetSpawnerExt for ChildSpawnerCommands<'_> {
    fn button(
        &mut self,
        theme: &Theme,
        scope: Entity,
        order: usize,
        label: impl Into<String>,
        on_click: impl FnMut(&mut Commands) + Send + Sync + 'static,
    ) -> EntityCommands<'_> {
        let accent = theme.accent;
        let text = label.into();
        let mut ec = self.spawn((
            button_visual(theme, accent, UiRect::axes(Val::Px(16.0), Val::Px(8.0))),
            ButtonAction::new(Box::new(on_click) as ClickCb),
            Focusable { scope, order },
        ));
        let font = theme.body.clone();
        let color = theme.text;
        ec.with_children(|b| {
            b.spawn((
                Text::new(text),
                TextFont {
                    font: font.into(),
                    font_size: FontSize::Px(22.0),
                    ..default()
                },
                TextColor(color),
            ));
        });
        let id = ec.id();
        ec.observe(
            move |_: On<Pointer<Click>>,
                  mut actions: Query<&mut ButtonAction>,
                  mut commands: Commands| {
                if let Ok(mut action) = actions.get_mut(id) {
                    action.run(&mut commands);
                }
            },
        );
        ec
    }

    fn toggle(
        &mut self,
        theme: &Theme,
        scope: Entity,
        order: usize,
        label: impl Into<String>,
        initial: bool,
        on_change: impl FnMut(&mut Commands, bool) + Send + Sync + 'static,
    ) -> EntityCommands<'_> {
        let accent = theme.accent;
        let text = label.into();
        let font = theme.body.clone();
        let color = theme.text;
        let track_border = theme.button_border;

        let mut ec = self.spawn((
            Node {
                width: Val::Px(200.0),
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                column_gap: Val::Px(12.0),
                padding: UiRect::axes(Val::Px(10.0), Val::Px(6.0)),
                ..default()
            },
            Button,
            Toggle {
                on: initial,
                accent,
            },
            ToggleAction(Box::new(on_change) as ToggleCb),
            Focusable { scope, order },
        ));
        ec.with_children(|row| {
            row.spawn((
                Text::new(text),
                TextFont {
                    font: font.into(),
                    font_size: FontSize::Px(20.0),
                    ..default()
                },
                TextColor(color),
            ));
            // The track; its knob is pushed left (off) or right (on) by the skin.
            row.spawn((
                Node {
                    width: Val::Px(46.0),
                    height: Val::Px(26.0),
                    align_items: AlignItems::Center,
                    justify_content: if initial {
                        JustifyContent::FlexEnd
                    } else {
                        JustifyContent::FlexStart
                    },
                    padding: UiRect::all(Val::Px(3.0)),
                    border: UiRect::all(Val::Px(track_border)),
                    ..default()
                },
                BackgroundColor(accent.with_alpha(if initial { 0.34 } else { 0.10 })),
                BorderColor::all(accent.with_alpha(0.65)),
                children![(
                    Node {
                        width: Val::Px(16.0),
                        height: Val::Px(16.0),
                        ..default()
                    },
                    BackgroundColor(color),
                    ToggleKnob,
                )],
            ));
        });
        let id = ec.id();
        ec.observe(
            move |_: On<Pointer<Click>>,
                  mut toggles: Query<(&mut Toggle, &mut ToggleAction)>,
                  mut commands: Commands| {
                if let Ok((mut t, mut action)) = toggles.get_mut(id) {
                    t.on = !t.on;
                    (action.0)(&mut commands, t.on);
                }
            },
        );
        ec
    }

    fn slider(
        &mut self,
        theme: &Theme,
        scope: Entity,
        order: usize,
        range: RangeInclusive<f32>,
        initial: f32,
        on_change: impl FnMut(&mut Commands, f32) + Send + Sync + 'static,
    ) -> EntityCommands<'_> {
        let accent = theme.accent;
        let (min, max) = (*range.start(), *range.end());
        let slider = Slider {
            value: initial.clamp(min, max),
            min,
            max,
        };
        let frac = slider.fraction();
        let border = theme.button_border;

        let mut ec = self.spawn((
            Node {
                width: Val::Px(200.0),
                height: Val::Px(24.0),
                align_items: AlignItems::Center,
                padding: UiRect::horizontal(Val::Px(2.0)),
                border: UiRect::all(Val::Px(border)),
                ..default()
            },
            Button,
            BackgroundColor(accent.with_alpha(0.10)),
            BorderColor::all(accent.with_alpha(0.65)),
            RelativeCursorPosition::default(),
            slider,
            SliderAction(Box::new(on_change) as SliderCb),
            Focusable { scope, order },
            children![(
                Node {
                    width: Val::Percent(frac * 100.0),
                    height: Val::Percent(100.0),
                    ..default()
                },
                BackgroundColor(accent.with_alpha(0.55)),
                SliderFill,
            )],
        ));
        ec.insert(Pickable::default());
        ec
    }

    fn list(&mut self, theme: &Theme) -> EntityCommands<'_> {
        self.spawn((
            Node {
                flex_direction: FlexDirection::Column,
                width: Val::Percent(100.0),
                max_height: Val::Px(240.0),
                overflow: Overflow::scroll_y(),
                row_gap: Val::Px(4.0),
                padding: UiRect::all(Val::Px(6.0)),
                border: UiRect::all(Val::Px(theme.panel_border)),
                ..default()
            },
            BackgroundColor(theme.ink),
            BorderColor::all(theme.line),
            Scrollable,
        ))
    }
}

// ---- Reactive systems ----

/// Move a toggle's knob and tint its track to match its state (and focus/press).
pub(crate) fn react_toggles(
    toggles: Query<(&Toggle, &Children, Has<Focused>, &Interaction), Changed<Toggle>>,
    mut tracks: Query<(&mut Node, &mut BackgroundColor), Without<Toggle>>,
    knobs: Query<(), With<ToggleKnob>>,
    children_of: Query<&Children>,
) {
    for (toggle, kids, _focused, _interaction) in &toggles {
        // The track is the toggle's child that itself owns a ToggleKnob child.
        for &child in kids {
            let has_knob = children_of
                .get(child)
                .map(|gc| gc.iter().any(|k| knobs.get(k).is_ok()))
                .unwrap_or(false);
            if !has_knob {
                continue;
            }
            if let Ok((mut node, mut bg)) = tracks.get_mut(child) {
                node.justify_content = if toggle.on {
                    JustifyContent::FlexEnd
                } else {
                    JustifyContent::FlexStart
                };
                bg.0 = toggle
                    .accent
                    .with_alpha(if toggle.on { 0.34 } else { 0.10 });
            }
        }
    }
}

/// Enter/Space flips the focused toggle (keyboard parity with a click).
pub(crate) fn toggle_keyboard(
    keys: Res<ButtonInput<KeyCode>>,
    mut focused: Query<(&mut Toggle, &mut ToggleAction), With<Focused>>,
    mut commands: Commands,
) {
    if !(keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::Space)) {
        return;
    }
    for (mut t, mut action) in &mut focused {
        t.on = !t.on;
        (action.0)(&mut commands, t.on);
    }
}

/// Drag the track (pointer held over it) to set a slider's value.
pub(crate) fn slider_drag(
    mut sliders: Query<(
        &Interaction,
        &RelativeCursorPosition,
        &mut Slider,
        &mut SliderAction,
    )>,
    mut commands: Commands,
) {
    for (interaction, cursor, mut slider, mut action) in &mut sliders {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let Some(pos) = cursor.normalized else {
            continue;
        };
        let before = slider.value;
        slider.set_fraction(pos.x);
        if slider.value != before {
            (action.0)(&mut commands, slider.value);
        }
    }
}

/// Left/Right arrows nudge the focused slider by 5% of its range.
pub(crate) fn slider_keyboard(
    keys: Res<ButtonInput<KeyCode>>,
    mut focused: Query<(&mut Slider, &mut SliderAction), With<Focused>>,
    mut commands: Commands,
) {
    let step = if keys.just_pressed(KeyCode::ArrowRight) {
        0.05
    } else if keys.just_pressed(KeyCode::ArrowLeft) {
        -0.05
    } else {
        return;
    };
    for (mut slider, mut action) in &mut focused {
        let before = slider.value;
        let f = slider.fraction() + step;
        slider.set_fraction(f);
        if slider.value != before {
            (action.0)(&mut commands, slider.value);
        }
    }
}

/// Resize a slider's fill to match its value.
pub(crate) fn react_sliders(
    sliders: Query<(&Slider, &Children), Changed<Slider>>,
    mut fills: Query<&mut Node, With<SliderFill>>,
) {
    for (slider, kids) in &sliders {
        for &child in kids {
            if let Ok(mut node) = fills.get_mut(child) {
                node.width = Val::Percent(slider.fraction() * 100.0);
            }
        }
    }
}

/// Scroll a hovered [`Scrollable`] with the mouse wheel.
pub(crate) fn scroll_lists(
    mut wheel: MessageReader<MouseWheel>,
    mut lists: Query<(&Interaction, &mut ScrollPosition), With<Scrollable>>,
) {
    let dy: f32 = wheel.read().map(|e| e.y).sum();
    if dy == 0.0 {
        return;
    }
    for (interaction, mut scroll) in &mut lists {
        if *interaction != Interaction::None {
            scroll.0.y -= dy * 24.0;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicI32, Ordering};

    use bevy::ecs::system::RunSystemOnce;

    use super::*;
    use crate::ModalPlugin;
    use crate::focus::FocusScope;

    fn app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins).add_plugins(ModalPlugin);
        app.init_resource::<ButtonInput<KeyCode>>();
        app
    }

    /// Spawn a standalone `FocusScope` screen with a button, toggle and slider.
    /// Returns (scope, toggle, slider) entities.
    fn spawn_screen(app: &mut App, hits: Arc<AtomicI32>) -> (Entity, Entity, Entity) {
        app.world_mut()
            .run_system_once(move |mut commands: Commands, theme: Res<Theme>| {
                let scope = commands.spawn((FocusScope, Node::default())).id();
                let (h1, h2, h3) = (hits.clone(), hits.clone(), hits.clone());
                let mut toggle = Entity::PLACEHOLDER;
                let mut slider = Entity::PLACEHOLDER;
                commands.entity(scope).with_children(|p| {
                    p.button(&theme, scope, 0, "Play", move |_c| {
                        h1.fetch_add(1, Ordering::SeqCst);
                    });
                    toggle = p
                        .toggle(&theme, scope, 1, "Sound", false, move |_c, on| {
                            h2.fetch_add(if on { 1 } else { -1 }, Ordering::SeqCst);
                        })
                        .id();
                    slider = p
                        .slider(&theme, scope, 2, 0.0..=1.0, 0.5, move |_c, v| {
                            h3.store((v * 1000.0) as i32, Ordering::SeqCst);
                        })
                        .id();
                });
                commands.insert_resource(Handles {
                    scope,
                    toggle,
                    slider,
                });
            })
            .unwrap();
        app.update();
        let h = app.world().resource::<Handles>();
        (h.scope, h.toggle, h.slider)
    }

    #[derive(Resource, Clone, Copy)]
    struct Handles {
        scope: Entity,
        toggle: Entity,
        slider: Entity,
    }

    fn press(app: &mut App, key: KeyCode) {
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(key);
    }
    fn release_all(app: &mut App) {
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .clear();
    }

    #[test]
    fn slider_fraction_maps_the_range() {
        let s = Slider {
            value: 5.0,
            min: 0.0,
            max: 10.0,
        };
        assert_eq!(s.fraction(), 0.5);
        let mut s = s;
        s.set_fraction(0.25);
        assert_eq!(s.value, 2.5);
        s.set_fraction(2.0); // clamps
        assert_eq!(s.value, 10.0);
    }

    #[test]
    fn standalone_scope_focuses_its_first_widget() {
        let mut app = app();
        let (scope, _t, _s) = spawn_screen(&mut app, Arc::new(AtomicI32::new(0)));
        app.update(); // maintain_focus runs

        // The lowest-order focusable (the button) under the scope is focused,
        // even though no overlay is open.
        let focused = app
            .world_mut()
            .query_filtered::<&Focusable, With<Focused>>()
            .iter(app.world())
            .next()
            .map(|f| f.scope);
        assert_eq!(
            focused,
            Some(scope),
            "first widget in the scope takes focus"
        );
    }

    #[test]
    fn enter_flips_the_focused_toggle_and_runs_the_callback() {
        let hits = Arc::new(AtomicI32::new(0));
        let mut app = app();
        let (_scope, toggle, _s) = spawn_screen(&mut app, hits.clone());

        // Focus the toggle explicitly, then Enter should flip it on.
        app.world_mut().entity_mut(toggle).insert(Focused);
        // Clear the button's default focus so only the toggle is focused.
        let button = app
            .world_mut()
            .query_filtered::<Entity, (With<Focusable>, With<Focused>)>()
            .iter(app.world())
            .filter(|e| *e != toggle)
            .collect::<Vec<_>>();
        for e in button {
            app.world_mut().entity_mut(e).remove::<Focused>();
        }

        press(&mut app, KeyCode::Enter);
        app.update();
        release_all(&mut app);

        assert!(
            app.world().entity(toggle).get::<Toggle>().unwrap().on,
            "Enter flipped the toggle on"
        );
        assert_eq!(hits.load(Ordering::SeqCst), 1, "callback ran with on=true");
    }

    #[test]
    fn arrow_right_raises_the_focused_slider() {
        let hits = Arc::new(AtomicI32::new(0));
        let mut app = app();
        let (_scope, _t, slider) = spawn_screen(&mut app, hits.clone());

        // Focus only the slider.
        let focused = app
            .world_mut()
            .query_filtered::<Entity, With<Focused>>()
            .iter(app.world())
            .collect::<Vec<_>>();
        for e in focused {
            app.world_mut().entity_mut(e).remove::<Focused>();
        }
        app.world_mut().entity_mut(slider).insert(Focused);

        let before = app.world().entity(slider).get::<Slider>().unwrap().value;
        press(&mut app, KeyCode::ArrowRight);
        app.update();
        release_all(&mut app);

        let after = app.world().entity(slider).get::<Slider>().unwrap().value;
        assert!(after > before, "ArrowRight nudged the slider up");
        assert!(hits.load(Ordering::SeqCst) > 0, "slider callback ran");
    }

    #[test]
    fn react_sliders_sizes_the_fill_to_the_value() {
        let mut app = app();
        let (_scope, _t, slider) = spawn_screen(&mut app, Arc::new(AtomicI32::new(0)));

        // Drive the value directly and let react_sliders resize the fill.
        app.world_mut()
            .entity_mut(slider)
            .get_mut::<Slider>()
            .unwrap()
            .value = 0.25;
        app.update();

        let fill_width = app
            .world_mut()
            .query_filtered::<(&Node, &ChildOf), With<SliderFill>>()
            .iter(app.world())
            .find(|(_, parent)| parent.parent() == slider)
            .map(|(node, _)| node.width);
        assert_eq!(
            fill_width,
            Some(Val::Percent(25.0)),
            "fill tracks the value"
        );
    }

    #[test]
    fn list_is_a_scrollable_container() {
        let mut app = app();
        app.world_mut()
            .run_system_once(|mut commands: Commands, theme: Res<Theme>| {
                commands.spawn(Node::default()).with_children(|p| {
                    p.list(&theme);
                });
            })
            .unwrap();
        app.update();

        let (node, _) = app
            .world_mut()
            .query_filtered::<(&Node, &Scrollable), ()>()
            .iter(app.world())
            .next()
            .expect("a list spawned");
        assert_eq!(
            node.overflow,
            Overflow::scroll_y(),
            "the list scrolls vertically"
        );
    }
}
