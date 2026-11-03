//! Player movement: a kinematic character controller on top of Avian's
//! `move_and_slide`, plus hand-rolled gravity and jumping.

use avian3d::math::{ToF32Precision, ToRealPrecision};
use avian3d::prelude::*;
use bevy::prelude::*;

/// Radius of the player capsule.
pub const PLAYER_RADIUS: f32 = 0.35;
/// Length of the cylindrical part of the player capsule. The total height is
/// `PLAYER_LENGTH + 2 * PLAYER_RADIUS`.
pub const PLAYER_LENGTH: f32 = 0.9;

/// Player controller state.
#[derive(Component)]
pub struct Player {
    pub velocity: Vec3,
    pub grounded: bool,
    /// Blocks the portal trigger for a short time after a teleport so the
    /// player cannot bounce between the two portals.
    pub teleport_cooldown: f32,
}

impl Default for Player {
    fn default() -> Self {
        Self {
            velocity: Vec3::ZERO,
            grounded: false,
            teleport_cooldown: 0.0,
        }
    }
}

/// Yaw/pitch of the view, in radians.
#[derive(Component, Default, Debug)]
pub struct Look {
    pub yaw: f32,
    pub pitch: f32,
}

/// Movement intent for this tick, in world space.
#[derive(Component, Default)]
pub struct MoveInput {
    pub direction: Vec3,
    pub jump: bool,
    pub sprint: bool,
}

const MOVE_SPEED: f32 = 5.0;
const SPRINT_SPEED: f32 = 8.0;
const ACCELERATION: f32 = 60.0;
const GRAVITY: f32 = -22.0;
const JUMP_SPEED: f32 = 8.0;
const MAX_FALL_SPEED: f32 = -40.0;

pub struct PhysicsPlugin;

impl Plugin for PhysicsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            FixedUpdate,
            (apply_movement, update_grounded, move_player).chain(),
        );
    }
}

pub(crate) fn apply_movement(time: Res<Time>, mut query: Query<(&MoveInput, &mut Player)>) {
    let dt = time.delta_secs();
    for (input, mut player) in &mut query {
        let target_speed = if input.sprint { SPRINT_SPEED } else { MOVE_SPEED };
        let target = input.direction * target_speed;

        let horizontal = Vec3::new(player.velocity.x, 0.0, player.velocity.z);
        let delta = target - horizontal;
        let horizontal = horizontal + delta.clamp_length_max(ACCELERATION * dt);

        player.velocity.x = horizontal.x;
        player.velocity.z = horizontal.z;
        player.velocity.y = (player.velocity.y + GRAVITY * dt).max(MAX_FALL_SPEED);

        if input.jump && player.grounded {
            player.velocity.y = JUMP_SPEED;
            player.grounded = false;
        }

        player.teleport_cooldown = (player.teleport_cooldown - dt).max(0.0);
    }
}

/// A short shape cast downwards tells us whether we may jump.
fn update_grounded(spatial: SpatialQuery, mut query: Query<(Entity, &Transform, &mut Player)>) {
    let shape = Collider::capsule(PLAYER_RADIUS * 0.95, PLAYER_LENGTH * 0.9);
    for (entity, transform, mut player) in &mut query {
        let hit = spatial.cast_shape(
            &shape,
            transform.translation.real(),
            transform.rotation,
            Dir3::NEG_Y,
            &ShapeCastConfig::from_max_distance(0.15),
            &SpatialQueryFilter::from_excluded_entities([entity]),
        );
        player.grounded = hit.is_some() && player.velocity.y <= 0.5;
    }
}

/// Sweep-and-slide handles walls, stairs and sliding along surfaces.
fn move_player(
    time: Res<Time>,
    move_and_slide: MoveAndSlide,
    mut query: Query<(Entity, &mut Transform, &mut Player, &Collider)>,
) {
    let dt = time.delta();
    for (entity, mut transform, mut player, collider) in &mut query {
        let output = move_and_slide.move_and_slide(
            collider,
            transform.translation.real(),
            transform.rotation,
            player.velocity.real(),
            dt,
            &MoveAndSlideConfig::default(),
            &SpatialQueryFilter::from_excluded_entities([entity]),
            |_hit| MoveAndSlideHitResponse::Accept,
        );

        transform.translation = output.position.f32();
        player.velocity = output.projected_velocity.f32();
    }
}

/// Spawns the physics body. The camera is attached later, in `player.rs`.
pub fn spawn_player_body(commands: &mut Commands, spawn: Vec3) -> Entity {
    commands
        .spawn((
            Player::default(),
            MoveInput::default(),
            Look::default(),
            RigidBody::Kinematic,
            // We integrate the position ourselves via `move_and_slide`.
            CustomPositionIntegration,
            Collider::capsule(PLAYER_RADIUS, PLAYER_LENGTH),
            Transform::from_translation(spawn),
            // Parent of the camera: keeps the hierarchy validation happy.
            Visibility::default(),
            Name::new("Player"),
        ))
        .id()
}
