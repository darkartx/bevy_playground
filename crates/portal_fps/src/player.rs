//! Player setup and input: camera, mouse look, respawn.

use bevy::camera::visibility::RenderLayers;
use bevy::input::mouse::AccumulatedMouseMotion;
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};

use crate::FALL_LIMIT;
use crate::physics::{Look, MoveInput, Player, spawn_player_body};
use crate::world::{PORTAL_LAYER, SPAWN_POINT, WORLD_LAYER};

/// The player's camera.
#[derive(Component)]
pub struct PlayerCamera;

const MOUSE_SENSITIVITY: f32 = 0.0022;
const PITCH_LIMIT: f32 = std::f32::consts::FRAC_PI_2 - 0.01;
/// Height of the eyes above the center of the capsule.
const EYE_HEIGHT: f32 = 0.55;

pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_player).add_systems(
            Update,
            (apply_look, respawn_when_fallen).chain(),
        );
        app.add_systems(
            FixedUpdate,
            gather_input.before(crate::physics::apply_movement),
        );
    }
}

fn setup_player(mut commands: Commands) {
    let player = spawn_player_body(&mut commands, SPAWN_POINT);

    commands.entity(player).with_children(|parent| {
        parent.spawn((
            Camera3d::default(),
            Camera {
                // Portal cameras use order -1, so this renders on top of them.
                order: 0,
                ..default()
            },
            Projection::Perspective(PerspectiveProjection {
                fov: 90.0_f32.to_radians(),
                // The camera sits right at the portal plane, a small near
                // plane avoids clipping the portal screen away.
                near: 0.05,
                ..default()
            }),
            Transform::from_xyz(0.0, EYE_HEIGHT, 0.0),
            // Sees the world and the portal screens.
            RenderLayers::layer(WORLD_LAYER).with(PORTAL_LAYER),
            PlayerCamera,
            Name::new("PlayerCamera"),
        ));
    });
}

/// Turns the keyboard state into a world-space direction.
fn gather_input(keys: Res<ButtonInput<KeyCode>>, mut query: Query<(&Look, &mut MoveInput)>) {
    for (look, mut input) in &mut query {
        let mut strafe = 0.0;
        let mut forward = 0.0;
        if keys.pressed(KeyCode::KeyW) {
            forward += 1.0;
        }
        if keys.pressed(KeyCode::KeyS) {
            forward -= 1.0;
        }
        if keys.pressed(KeyCode::KeyD) {
            strafe += 1.0;
        }
        if keys.pressed(KeyCode::KeyA) {
            strafe -= 1.0;
        }

        let (sin, cos) = look.yaw.sin_cos();
        let forward_dir = Vec3::new(-sin, 0.0, -cos);
        let right_dir = Vec3::new(cos, 0.0, -sin);

        input.direction = (forward_dir * forward + right_dir * strafe).normalize_or_zero();
        input.jump = keys.pressed(KeyCode::Space);
        input.sprint = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    }
}

/// Mouse look: yaw goes to the body, pitch to the camera.
fn apply_look(
    mouse_motion: Res<AccumulatedMouseMotion>,
    cursor: Single<&CursorOptions, With<PrimaryWindow>>,
    mut player: Query<(&mut Look, &mut Transform), (With<Player>, Without<PlayerCamera>)>,
    mut camera: Query<&mut Transform, (With<PlayerCamera>, Without<Player>)>,
) {
    let Ok((mut look, mut body)) = player.single_mut() else {
        return;
    };

    if cursor.grab_mode != CursorGrabMode::None {
        let delta = mouse_motion.delta;
        look.yaw -= delta.x * MOUSE_SENSITIVITY;
        look.pitch = (look.pitch - delta.y * MOUSE_SENSITIVITY).clamp(-PITCH_LIMIT, PITCH_LIMIT);
    }

    body.rotation = Quat::from_rotation_y(look.yaw);
    if let Ok(mut camera) = camera.single_mut() {
        camera.rotation = Quat::from_rotation_x(look.pitch);
    }
}

/// `R` respawns; falling out of the world does the same.
fn respawn_when_fallen(
    keys: Res<ButtonInput<KeyCode>>,
    mut query: Query<(&mut Transform, &mut Player)>,
) {
    let manual = keys.just_pressed(KeyCode::KeyR);
    for (mut transform, mut player) in &mut query {
        if manual || transform.translation.y < FALL_LIMIT {
            transform.translation = SPAWN_POINT;
            player.velocity = Vec3::ZERO;
        }
    }
}
