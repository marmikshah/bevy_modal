//! A button-per-feature showcase of `bevy_modal`. A plain `bevy_ui` menu (the
//! "game screen") with a row per capability — each opens an overlay, a confirm
//! dialog, or fires toasts. Overlays animate in/out, are keyboard-navigable
//! (Tab / arrows / Enter), and the menu shows lifecycle messages as toasts.
//!
//! Run: `cargo run --example showcase`

use std::time::Duration;

use bevy::prelude::*;
use bevy_modal::prelude::*;

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, ModalPlugin))
        .add_systems(Startup, setup)
        .add_systems(Update, (on_press, restyle, announce_lifecycle))
        .run();
}

/// What each menu button does.
#[derive(Component, Clone, Copy)]
enum Action {
    PauseMenu,
    Confirm,
    CustomContent,
    ToastInfo,
    ToastSuccess,
    ToastWarning,
    ToastError,
    ToastUndo,
}

const ROWS: &[(&str, Action)] = &[
    ("Pause menu (stacks Settings)", Action::PauseMenu),
    ("Confirm dialog", Action::Confirm),
    ("Custom content overlay", Action::CustomContent),
    ("Toast: info", Action::ToastInfo),
    ("Toast: success", Action::ToastSuccess),
    ("Toast: warning", Action::ToastWarning),
    ("Toast: error", Action::ToastError),
    ("Toast with action", Action::ToastUndo),
];

fn setup(mut commands: Commands) {
    commands.spawn(Camera2d);
    commands
        .spawn(Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            row_gap: Val::Px(8.0),
            ..default()
        })
        .with_children(|root| {
            root.spawn((
                Text::new("bevy_modal showcase — tab/arrows + Enter navigate overlays"),
                TextFont {
                    font_size: FontSize::Px(16.0),
                    ..default()
                },
                TextColor(Color::srgb(0.7, 0.8, 1.0)),
                Node {
                    margin: UiRect::bottom(Val::Px(14.0)),
                    ..default()
                },
            ));
            for (label, action) in ROWS {
                root.spawn((
                    *action,
                    Button,
                    Node {
                        width: Val::Px(340.0),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        padding: UiRect::axes(Val::Px(16.0), Val::Px(10.0)),
                        ..default()
                    },
                    BackgroundColor(REST),
                ))
                .with_children(|b| {
                    b.spawn((
                        Text::new(*label),
                        TextFont {
                            font_size: FontSize::Px(18.0),
                            ..default()
                        },
                        TextColor(Color::WHITE),
                    ));
                });
            }
        });
}

const REST: Color = Color::srgb(0.16, 0.20, 0.32);
const HOVER: Color = Color::srgb(0.24, 0.30, 0.46);
const PRESS: Color = Color::srgb(0.36, 0.46, 0.70);

#[allow(clippy::type_complexity)]
fn restyle(
    mut buttons: Query<(&Interaction, &mut BackgroundColor), (Changed<Interaction>, With<Action>)>,
) {
    for (interaction, mut bg) in buttons.iter_mut() {
        bg.0 = match interaction {
            Interaction::Pressed => PRESS,
            Interaction::Hovered => HOVER,
            Interaction::None => REST,
        };
    }
}

fn on_press(buttons: Query<(&Interaction, &Action), Changed<Interaction>>, mut commands: Commands) {
    for (interaction, action) in buttons.iter() {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match action {
            // A pause menu whose "Settings" button stacks a second overlay on top
            // — the bug the crate kills: the scrim occludes the lower buttons.
            Action::PauseMenu => {
                overlay(&mut commands, "pause")
                    .title("PAUSED")
                    .body("The game is paused.")
                    .button("Settings", |c| {
                        overlay(c, "settings")
                            .title("Settings")
                            .body("(stacked over the pause menu)")
                            .button("Back", |c| c.dismiss_overlay("settings"))
                            .dismissable(true)
                            .escape(true)
                            .push();
                    })
                    .button("Resume", |c| c.dismiss_overlay("pause"))
                    .dismissable(true)
                    .escape(true)
                    .push_unique();
            }
            Action::Confirm => {
                confirm(&mut commands, "delete", "Delete save?")
                    .message("This can't be undone.")
                    .confirm_label("Delete")
                    .cancel_label("Keep")
                    .on_confirm(|c| {
                        toast(c, "Save deleted").level(ToastLevel::Success).push();
                    })
                    .push();
            }
            // The `.content()` escape hatch hosting a *reusable* builder
            // (`bespoke_panel`, typed `&mut ChildSpawnerCommands`) under the still
            // scrimmed/stacked/gated/animated root — the standard idiom drops in.
            Action::CustomContent => {
                overlay(&mut commands, "custom")
                    .dismissable(true)
                    .content(bespoke_panel)
                    .push_unique();
            }
            Action::ToastInfo => {
                toast(&mut commands, "Autosaved").push();
            }
            Action::ToastSuccess => {
                toast(&mut commands, "Level complete!")
                    .level(ToastLevel::Success)
                    .push();
            }
            Action::ToastWarning => {
                toast(&mut commands, "Low on health")
                    .level(ToastLevel::Warning)
                    .push();
            }
            Action::ToastError => {
                toast(&mut commands, "Connection lost")
                    .level(ToastLevel::Error)
                    .duration(Duration::from_secs(6))
                    .push();
            }
            Action::ToastUndo => {
                toast(&mut commands, "Item discarded")
                    .action("Undo", |c| {
                        toast(c, "Restored").level(ToastLevel::Success).push();
                    })
                    .push();
            }
        }
    }
}

/// A reusable `bevy_ui` builder typed for `&mut ChildSpawnerCommands` — the kind
/// of helper a real app already has. It drops straight into `.content()` (the
/// point of #13): the standard `spawn().with_children` idiom, no re-authoring.
fn bespoke_panel(parent: &mut ChildSpawnerCommands) {
    parent
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: Val::Px(10.0),
                padding: UiRect::all(Val::Px(28.0)),
                ..default()
            },
            BackgroundColor(Color::srgb(0.05, 0.07, 0.12)),
        ))
        .with_children(|card| {
            card.spawn((
                Text::new("Bespoke panel"),
                TextFont {
                    font_size: FontSize::Px(24.0),
                    ..default()
                },
                TextColor(Color::srgb(0.6, 0.9, 0.7)),
            ));
            card.spawn((
                Text::new("A reused ChildSpawnerCommands helper. Tap the scrim to close."),
                TextFont {
                    font_size: FontSize::Px(16.0),
                    ..default()
                },
                TextColor(Color::srgb(0.7, 0.7, 0.8)),
            ));
        });
}

/// Lifecycle messages surfaced as toasts, so opening/closing is visible.
fn announce_lifecycle(
    mut opened: MessageReader<OverlayOpened>,
    mut closed: MessageReader<OverlayClosed>,
    mut commands: Commands,
) {
    for OverlayOpened(id) in opened.read() {
        toast(&mut commands, format!("opened: {id}")).push();
    }
    for event in closed.read() {
        toast(
            &mut commands,
            format!("closed: {} ({:?})", event.id, event.reason),
        )
        .push();
    }
}
