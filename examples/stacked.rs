//! Two stacked overlays — the bug `bevy_modal` exists to kill — plus a confirm
//! dialog and a toast.
//!
//! The first (large) overlay opens a second, smaller one on top. In raw
//! `bevy_ui` the first overlay's button would still be clickable around the
//! edges of the second; here the scrim occludes it. Meanwhile the spinning
//! sprite (driven by raw input via `ui_not_capturing`) freezes whenever any
//! overlay is open — the UI→gameplay gate. The first overlay also offers a
//! `confirm` dialog (modal) and a `toast` (transient, non-blocking).
//!
//! Run: `cargo run --example stacked`

use std::time::Duration;

use bevy::prelude::*;
use bevy_modal::prelude::*;

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, ModalPlugin))
        .add_systems(Startup, (setup, open_first))
        .add_systems(Update, spin_sprite.run_if(ui_not_capturing))
        .run();
}

#[derive(Component)]
struct Spinner;

fn setup(mut commands: Commands) {
    commands.spawn(Camera2d);
    commands.spawn((
        Spinner,
        Sprite {
            color: Color::srgb(0.45, 0.70, 1.0),
            custom_size: Some(Vec2::new(80.0, 80.0)),
            ..default()
        },
        Transform::from_xyz(0.0, 200.0, 0.0),
    ));
}

/// The base overlay. Its buttons stack a second overlay, raise a confirm dialog,
/// and fire a toast.
fn open_first(mut commands: Commands) {
    overlay(&mut commands, "first")
        .title("FIRST")
        .body("press a button")
        .button("Open dialog", |c| {
            overlay(c, "second")
                .title("SECOND")
                .body("scrim below blocks the first's button")
                .button("Close", |c| {
                    c.queue(|world: &mut World| {
                        if let Some(top) = world.resource::<OverlayStack>().top() {
                            world.entity_mut(top).despawn();
                        }
                    });
                })
                .dismissable(true)
                .escape(true)
                .push();
        })
        .button("Delete save", |c| {
            confirm(c, "delete-save", "Delete save?")
                .message("This can't be undone.")
                .confirm_label("Delete")
                .cancel_label("Keep")
                .on_confirm(|c| {
                    toast(c, "Save deleted").push();
                })
                .push();
        })
        .button("Notify", |c| {
            toast(c, "Hello from a toast")
                .duration(Duration::from_secs(3))
                .push();
        })
        .escape(true)
        .push();
}

/// Raw-input-style gameplay system: spins only while no overlay captures input.
fn spin_sprite(time: Res<Time>, mut spinner: Query<&mut Transform, With<Spinner>>) {
    for mut t in spinner.iter_mut() {
        t.rotate_z(time.delta_secs());
    }
}
