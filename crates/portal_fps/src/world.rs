//! The level: two sealed rooms separated by a solid wall, each with a portal.

use avian3d::prelude::*;
use bevy::camera::visibility::RenderLayers;
use bevy::color::palettes::tailwind;
use bevy::prelude::*;

pub const ROOM_HALF_DEPTH: f32 = 8.0;
pub const ROOM_WIDTH: f32 = 16.0;
pub const ROOM_HEIGHT: f32 = 4.0;
pub const WALL_THICKNESS: f32 = 0.2;

pub const PORTAL_WIDTH: f32 = 1.2;
pub const PORTAL_HEIGHT: f32 = 2.0;

/// Center of portal A, flush with the left face of the dividing wall. Its
/// local `+Z` points into room A.
pub const PORTAL_A_CENTER: Vec3 = Vec3::new(
    -WALL_THICKNESS * 0.5 - 0.02,
    PORTAL_HEIGHT * 0.5,
    -3.0,
);
/// Center of portal B, flush with the right face of the dividing wall. Its
/// local `+Z` points into room B.
pub const PORTAL_B_CENTER: Vec3 = Vec3::new(
    WALL_THICKNESS * 0.5 + 0.02,
    PORTAL_HEIGHT * 0.5,
    3.0,
);

/// Where the player starts (room A).
pub const SPAWN_POINT: Vec3 = Vec3::new(-9.0, 1.2, 0.0);

/// Everything both the player and the portal cameras can see.
pub const WORLD_LAYER: usize = 0;
/// Only the player's camera sees this layer: the portal screens.
pub const PORTAL_LAYER: usize = 1;

pub struct WorldPlugin;

impl Plugin for WorldPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_world);
    }
}

fn setup_world(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.insert_resource(GlobalAmbientLight {
        color: Color::srgb(0.6, 0.65, 0.8),
        brightness: 900.0,
        ..default()
    });

    let mut level = LevelBuilder {
        commands: &mut commands,
        meshes: &mut meshes,
        materials: &mut materials,
    };

    let half_width = ROOM_WIDTH * 2.0 + 0.8;
    let half_depth = ROOM_HALF_DEPTH * 2.0 + 0.8;
    let mid_height = ROOM_HEIGHT * 0.5;

    // Floor and ceiling.
    level.block(
        Vec3::new(half_width, 0.4, half_depth),
        Vec3::new(0.0, -0.2, 0.0),
        tailwind::SLATE_700,
    );
    level.block(
        Vec3::new(half_width, 0.4, half_depth),
        Vec3::new(0.0, ROOM_HEIGHT + 0.2, 0.0),
        tailwind::SLATE_800,
    );
    // Outer side walls.
    for x in [-ROOM_WIDTH - 0.2, ROOM_WIDTH + 0.2] {
        level.block(
            Vec3::new(0.4, ROOM_HEIGHT, half_depth),
            Vec3::new(x, mid_height, 0.0),
            tailwind::SLATE_600,
        );
    }
    // End walls.
    for z in [-ROOM_HALF_DEPTH - 0.2, ROOM_HALF_DEPTH + 0.2] {
        level.block(
            Vec3::new(half_width, ROOM_HEIGHT, 0.4),
            Vec3::new(0.0, mid_height, z),
            tailwind::SLATE_600,
        );
    }

    // The dividing wall. Completely solid: the only way into the other room is
    // through a portal, and the physics will stop you from walking through it.
    level.block(
        Vec3::new(WALL_THICKNESS, ROOM_HEIGHT, half_depth),
        Vec3::new(0.0, mid_height, 0.0),
        tailwind::SLATE_500,
    );

    // Room A: warm. Room B: cold.
    level.room(Vec3::new(-ROOM_WIDTH * 0.5, 0.0, 0.0), -1.0, tailwind::ORANGE_800, tailwind::AMBER_200);
    level.room(Vec3::new(ROOM_WIDTH * 0.5, 0.0, 0.0), 1.0, tailwind::BLUE_800, tailwind::SKY_200);

    // A few loose crates to show that the physics is alive.
    for (x, z) in [(-5.5, 2.0), (5.5, -2.0)] {
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(0.7, 0.7, 0.7))),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: Color::from(tailwind::LIME_300),
                ..default()
            })),
            Transform::from_xyz(x, 2.6, z),
            RigidBody::Dynamic,
            Collider::cuboid(0.7, 0.7, 0.7),
            RenderLayers::layer(WORLD_LAYER),
        ));
    }

    // Lights. They are on both layers so the portal cameras see a lit room as
    // well; without that the portal image would be black.
    let light_layers = RenderLayers::layer(WORLD_LAYER).with(PORTAL_LAYER);
    for (x, color) in [(-8.0, tailwind::ORANGE_200), (8.0, tailwind::SKY_200)] {
        commands.spawn((
            PointLight {
                intensity: 900_000.0,
                range: 30.0,
                color: Color::from(color),
                shadow_maps_enabled: true,
                ..default()
            },
            Transform::from_xyz(x, 3.6, 0.0),
            light_layers.clone(),
        ));
    }
}

struct LevelBuilder<'a, 'w, 's> {
    commands: &'a mut Commands<'w, 's>,
    meshes: &'a mut Assets<Mesh>,
    materials: &'a mut Assets<StandardMaterial>,
}

impl LevelBuilder<'_, '_, '_> {
    fn block(&mut self, size: Vec3, position: Vec3, color: Srgba) {
        self.commands.spawn((
            Mesh3d(self.meshes.add(Cuboid::new(size.x, size.y, size.z))),
            MeshMaterial3d(self.materials.add(StandardMaterial {
                base_color: Color::from(color),
                perceptual_roughness: 0.85,
                ..default()
            })),
            Transform::from_translation(position),
            RigidBody::Static,
            Collider::cuboid(size.x, size.y, size.z),
            RenderLayers::layer(WORLD_LAYER),
        ));
    }

    /// Decorates one room. `side` is `-1` for the room on the left and `1` for
    /// the room on the right.
    fn room(&mut self, center: Vec3, side: f32, back: Srgba, accent: Srgba) {
        // Bright panel on the far wall. This is what you mostly see through a
        // portal, so it doubles as the "which room am I looking at" cue.
        self.commands.spawn((
            Mesh3d(
                self.meshes
                    .add(Cuboid::new(0.08, ROOM_HEIGHT - 0.4, ROOM_HALF_DEPTH * 2.0 - 1.0)),
            ),
            MeshMaterial3d(self.materials.add(StandardMaterial {
                base_color: Color::from(back),
                emissive: LinearRgba::from(Color::from(back)) * 0.6,
                ..default()
            })),
            Transform::from_xyz(
                center.x + side * (ROOM_WIDTH * 0.5 - 0.06),
                ROOM_HEIGHT * 0.5,
                0.0,
            ),
            RenderLayers::layer(WORLD_LAYER),
        ));

        // A pillar in the middle of the room: looking through the portal and
        // strafing makes it slide across the image, which is the clearest
        // possible proof that the portal is a real window into the world.
        self.block(
            Vec3::new(0.8, ROOM_HEIGHT, 0.8),
            Vec3::new(center.x - side * 4.0, ROOM_HEIGHT * 0.5, 4.5),
            accent,
        );

        // A short staircase made of static blocks to climb on.
        let base = center.x + side * 3.0;
        for i in 0..3_u8 {
            let step = f32::from(i);
            let height = 0.6 * (step + 1.0);
            self.block(
                Vec3::new(1.0, height, 1.0),
                Vec3::new(base, height * 0.5, -5.0 + step),
                accent,
            );
        }
    }
}
