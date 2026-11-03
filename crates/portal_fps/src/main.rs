//! Portal FPS demo: a first-person sandbox in which the only way between two
//! sealed rooms is a pair of portals.
//!
//! Controls:
//! - `WASD` — walk
//! - mouse — look around
//! - `Space` — jump
//! - `Shift` — sprint
//! - `R` — respawn
//! - `Esc` — release the mouse cursor, click to grab it again

// Bevy systems must take their parameters by value, which `needless_pass_by_value`
// cannot see through, and a few query tuples are inherently long.
#![allow(clippy::needless_pass_by_value, clippy::type_complexity)]

mod player;
mod physics;
mod portal;
mod ui;
mod world;

use avian3d::prelude::PhysicsPlugins;
use bevy::diagnostic::FrameTimeDiagnosticsPlugin;
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow, WindowResolution};

/// Distance (in world units) below the level at which the player is considered
/// to have fallen out of the world.
pub(crate) const FALL_LIMIT: f32 = -30.0;

fn main() {
    App::new()
        .add_plugins(
            DefaultPlugins
                .set(AssetPlugin {
                    // The demo is a workspace member, so its assets live next to
                    // the crate rather than next to the workspace root. An
                    // absolute path works both for `cargo run` and for the
                    // compiled binary.
                    file_path: concat!(env!("CARGO_MANIFEST_DIR"), "/assets").into(),
                    ..default()
                })
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "Portal FPS".into(),
                        resolution: WindowResolution::new(1280, 720),
                        ..default()
                    }),
                    ..default()
                }),
        )
        .add_plugins(PhysicsPlugins::default())
        // Keep exactly as many samples as the HUD graph draws, so its history
        // and the graph always agree.
        .add_plugins(FrameTimeDiagnosticsPlugin::new(ui::GRAPH_SAMPLES))
        .add_plugins((
            world::WorldPlugin,
            physics::PhysicsPlugin,
            player::PlayerPlugin,
            portal::PortalPlugin,
            ui::UiPlugin,
        ))
        .add_systems(Update, cursor_controls)
        .run();
}

/// Grabs the cursor on click and releases it on `Escape`, so the mouse can be
/// used for looking around without the pointer escaping the window.
fn cursor_controls(
    mut cursor: Single<&mut CursorOptions, With<PrimaryWindow>>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
) {
    if mouse_buttons.just_pressed(MouseButton::Left) && cursor.grab_mode == CursorGrabMode::None {
        cursor.grab_mode = CursorGrabMode::Locked;
        cursor.visible = false;
    }
    if keys.just_pressed(KeyCode::Escape) {
        cursor.grab_mode = CursorGrabMode::None;
        cursor.visible = true;
    }
}
