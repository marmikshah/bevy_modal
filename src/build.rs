//! The ergonomic layer. `overlay(&mut commands, id)` collects a spec through a
//! chained builder, then `.push()` queues a [`Command`] that — with `&mut
//! World` in hand — reads the [`Theme`], spawns the root + scrim + panel,
//! registers the overlay on the stack and stamps a deterministic
//! `GlobalZIndex`. Everything it emits is plain `bevy_ui`; this is boilerplate
//! reduction, not a widget framework.

use bevy::picking::prelude::*;
use bevy::prelude::*;

use crate::events::CloseReason;
use crate::focus::{Focusable, Focused};
use crate::scrim::scrim_bundle;
use crate::stack::{Overlay, OverlayStack, Z_BASE, Z_STEP, push_root};
use crate::theme::Theme;
use crate::transition::{OverlayBody, Transition, request_close};

/// A button's click handler. Boxed `FnMut` so the builder stays non-generic;
/// `&mut Commands` is the full escape hatch — `commands.queue(|w: &mut World|
/// ..)` reaches any resource or state from inside it.
type ButtonCb = Box<dyn FnMut(&mut Commands) + Send + Sync>;

/// A button's action, held on the button entity so every input source — a
/// pointer click or keyboard Enter/Space — runs the one callback through
/// [`run`](ButtonAction::run).
#[derive(Component)]
pub(crate) struct ButtonAction(ButtonCb);

impl ButtonAction {
    pub(crate) fn run(&mut self, commands: &mut Commands) {
        (self.0)(commands);
    }
}

/// Fills the overlay root with caller-owned children (a bespoke panel + its
/// content) instead of the built-in title/body/button panel. The escape hatch
/// for screens the crate shouldn't try to model — settings grids, icon rows.
type ContentFn = Box<dyn FnOnce(&mut ChildSpawner) + Send + Sync>;

#[derive(Component, Clone, Copy)]
pub(crate) struct ModalButtonStyle {
    accent: Color,
}

/// Chained overlay builder. Borrows `Commands` for the duration of one
/// statement; `.push()` enqueues the spawn.
pub struct OverlayBuilder<'a, 'w, 's> {
    commands: &'a mut Commands<'w, 's>,
    spec: SpawnOverlay,
}

/// Start building an overlay with the given id. The id is opaque to the crate —
/// use it to find or dismiss the overlay later via [`OverlayStack`](crate::OverlayStack).
pub fn overlay<'a, 'w, 's>(
    commands: &'a mut Commands<'w, 's>,
    id: impl Into<String>,
) -> OverlayBuilder<'a, 'w, 's> {
    OverlayBuilder {
        commands,
        spec: SpawnOverlay {
            id: id.into(),
            accent: None,
            title: None,
            body: Vec::new(),
            buttons: Vec::new(),
            dismissable: false,
            pop_on_escape: false,
            content: None,
            unique: false,
        },
    }
}

impl<'a, 'w, 's> OverlayBuilder<'a, 'w, 's> {
    /// Override the accent (title + button tint). Defaults to `Theme::accent`.
    pub fn accent(mut self, color: Color) -> Self {
        self.spec.accent = Some(color);
        self
    }

    /// Display-face heading at the top of the panel.
    pub fn title(mut self, text: impl Into<String>) -> Self {
        self.spec.title = Some(text.into());
        self
    }

    /// A line of body text. Call repeatedly for multiple lines.
    pub fn body(mut self, text: impl Into<String>) -> Self {
        self.spec.body.push(text.into());
        self
    }

    /// A button. `on_click` runs on press with `&mut Commands` — defer to a
    /// world closure for state changes: `c.queue(|w: &mut World| ..)`.
    pub fn button(
        mut self,
        label: impl Into<String>,
        on_click: impl FnMut(&mut Commands) + Send + Sync + 'static,
    ) -> Self {
        self.spec.buttons.push((label.into(), Box::new(on_click)));
        self
    }

    /// When true, a tap on the scrim dismisses this overlay (top-only).
    pub fn dismissable(mut self, yes: bool) -> Self {
        self.spec.dismissable = yes;
        self
    }

    /// When true, the Escape key pops this overlay while it's on top. Default
    /// off: state-driven overlays (spawned on `OnEnter`, despawned on `OnExit`)
    /// must leave this off, or Escape despawns the root behind the state
    /// machine's back and desyncs it.
    pub fn escape(mut self, yes: bool) -> Self {
        self.spec.pop_on_escape = yes;
        self
    }

    /// Host caller-owned children instead of the built-in panel. When set,
    /// `title`/`body`/`button` are ignored — the closure owns everything under
    /// the (still scrimmed, stacked, gated) root. Use this for bespoke screens.
    pub fn content(mut self, fill: impl FnOnce(&mut ChildSpawner) + Send + Sync + 'static) -> Self {
        self.spec.content = Some(Box::new(fill));
        self
    }

    /// Queue the spawn. Nothing happens until command application.
    pub fn push(self) {
        self.commands.queue(self.spec);
    }

    /// Queue the spawn, but no-op if an overlay with this id is already open.
    /// The duplicate check runs when the command applies (sequentially, against
    /// `&mut World`), so it is race-free even when two systems both request the
    /// same overlay in one frame — the first spawns, the second sees it open and
    /// skips. Use this for opener buttons (a fast double-tap can't double-spawn).
    pub fn push_unique(mut self) {
        self.spec.unique = true;
        self.commands.queue(self.spec);
    }
}

/// The deferred spawn. Holds the spec; built against `&mut World` so it can read
/// the theme and stack synchronously.
struct SpawnOverlay {
    id: String,
    accent: Option<Color>,
    title: Option<String>,
    body: Vec<String>,
    buttons: Vec<(String, ButtonCb)>,
    dismissable: bool,
    pop_on_escape: bool,
    content: Option<ContentFn>,
    /// When set (via `push_unique`), skip the spawn if this id is already open.
    unique: bool,
}

impl Command for SpawnOverlay {
    type Out = ();
    fn apply(self, world: &mut World) {
        // Race-free dedup: by the time this command applies, any earlier
        // same-frame spawn of the same id has already registered on the stack.
        if self.unique && world.resource::<OverlayStack>().is_open(&self.id) {
            return;
        }
        let theme = world.resource::<Theme>().clone();
        let accent = self.accent.unwrap_or(theme.accent);

        let id = self.id.clone();
        let root = world
            .spawn((
                Overlay {
                    id: self.id,
                    pop_on_escape: self.pop_on_escape,
                },
                Node {
                    position_type: PositionType::Absolute,
                    top: Val::Px(0.0),
                    left: Val::Px(0.0),
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                },
            ))
            .id();
        let depth = push_root(world, root, &id);
        world
            .entity_mut(root)
            .insert(GlobalZIndex(Z_BASE + depth as i32 * Z_STEP));

        // Scrim starts transparent; the transition eases its alpha up to
        // `theme.scrim`. Tapping it (when dismissable) eases the overlay out.
        let scrim = world.spawn(scrim_bundle(theme.scrim.with_alpha(0.0))).id();
        world.entity_mut(root).add_child(scrim);
        if self.dismissable {
            world.entity_mut(scrim).observe(
                move |_: On<Pointer<Click>>, mut commands: Commands| {
                    commands.queue(move |world: &mut World| {
                        request_close(world, root, CloseReason::Scrim)
                    });
                },
            );
        }

        let scale_from = theme.panel_scale_from;

        // The foreground node — built-in panel or content wrapper. The transition
        // scales *this* (never the root), so the full-screen scrim stays put.
        let body = if let Some(content) = self.content {
            let body = world
                .spawn((
                    OverlayBody,
                    UiTransform::from_scale(Vec2::splat(scale_from)),
                    Node::default(),
                ))
                .id();
            world.entity_mut(root).add_child(body);
            world.entity_mut(body).with_children(content);
            body
        } else {
            let panel = world
                .spawn((
                    OverlayBody,
                    UiTransform::from_scale(Vec2::splat(scale_from)),
                    Node {
                        width: Val::Percent(82.0),
                        max_width: Val::Px(theme.panel_max_width),
                        flex_direction: FlexDirection::Column,
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        row_gap: Val::Px(16.0),
                        padding: UiRect::axes(Val::Px(16.0), Val::Px(24.0)),
                        border: UiRect::all(Val::Px(theme.panel_border)),
                        ..default()
                    },
                    BackgroundColor(theme.ink),
                    BorderColor::all(theme.line),
                ))
                .id();
            world.entity_mut(root).add_child(panel);

            if let Some(title) = self.title {
                let label = world
                    .spawn((
                        Text::new(title),
                        TextFont {
                            font: theme.display.clone().into(),
                            font_size: FontSize::Px(28.0),
                            ..default()
                        },
                        TextColor(accent),
                    ))
                    .id();
                world.entity_mut(panel).add_child(label);
            }

            for line in self.body {
                let label = world
                    .spawn((
                        Text::new(line),
                        TextFont {
                            font: theme.body.clone().into(),
                            font_size: FontSize::Px(22.0),
                            ..default()
                        },
                        TextColor(theme.text_dim),
                    ))
                    .id();
                world.entity_mut(panel).add_child(label);
            }

            for (order, (text, on_click)) in self.buttons.into_iter().enumerate() {
                let button = world
                    .spawn((
                        Node {
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            padding: UiRect::axes(Val::Px(18.0), Val::Px(8.0)),
                            border: UiRect::all(Val::Px(theme.button_border)),
                            ..default()
                        },
                        Button,
                        BackgroundColor(accent.with_alpha(theme.btn_fill_rest)),
                        BorderColor::all(accent.with_alpha(theme.btn_border_rest)),
                        ModalButtonStyle { accent },
                        ButtonAction(on_click),
                        Focusable {
                            overlay: root,
                            order,
                        },
                    ))
                    .id();
                let label = world
                    .spawn((
                        Text::new(text),
                        TextFont {
                            font: theme.body.clone().into(),
                            font_size: FontSize::Px(24.0),
                            ..default()
                        },
                        TextColor(theme.text),
                    ))
                    .id();
                world.entity_mut(button).add_child(label);
                world.entity_mut(panel).add_child(button);
                // A pointer click runs the same stored action as keyboard/gamepad
                // activation; capture the button so a click on its label still hits it.
                world.entity_mut(button).observe(
                    move |_: On<Pointer<Click>>,
                          mut actions: Query<&mut ButtonAction>,
                          mut commands: Commands| {
                        if let Ok(mut action) = actions.get_mut(button) {
                            action.run(&mut commands);
                        }
                    },
                );
            }
            panel
        };

        world.entity_mut(root).insert(Transition::opening(
            scrim,
            theme.scrim,
            body,
            theme.open_secs,
            theme.close_secs,
            scale_from,
        ));
    }
}

/// Hover/press/focus feedback, theme-driven. A keyboard- or gamepad-focused
/// button gets the hover look so the selection is visible without a pointer.
/// Runs every frame (focus is a separate component from `Interaction`) but only
/// writes when the target colour actually changes, so it stays cheap.
pub(crate) fn react_buttons(
    theme: Res<Theme>,
    mut buttons: Query<(
        &Interaction,
        Has<Focused>,
        &ModalButtonStyle,
        &mut BackgroundColor,
        &mut BorderColor,
    )>,
) {
    for (interaction, focused, style, mut bg, mut border) in buttons.iter_mut() {
        let (fill, line) = match interaction {
            Interaction::Pressed => (theme.btn_fill_press, theme.btn_border_hover),
            Interaction::Hovered => (theme.btn_fill_hover, theme.btn_border_hover),
            Interaction::None if focused => (theme.btn_fill_hover, theme.btn_border_hover),
            Interaction::None => (theme.btn_fill_rest, theme.btn_border_rest),
        };
        let new_bg = style.accent.with_alpha(fill);
        if bg.0 != new_bg {
            bg.0 = new_bg;
            *border = BorderColor::all(style.accent.with_alpha(line));
        }
    }
}
