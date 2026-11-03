//! Portals: render-to-texture cameras, screens and teleporting.
//!
//! ## How "seeing through the portal" works
//!
//! Each portal has a virtual camera that renders the world into an image. Its
//! transform is
//!
//! ```text
//! V = T_dest * RotY(180°) * T_src⁻¹ * C
//! ```
//!
//! where `C` is the player's camera and `T_src` / `T_dest` are the portal
//! transforms (local `+Z` is the outward normal). The virtual camera therefore
//! stands behind the destination portal at the same distance the player stands
//! in front of the source portal, and it looks back through it. A ray from the
//! virtual camera through screen pixel `p` hits the destination plane exactly
//! where the teleported player ray would hit it, which is why the image can be
//! sampled in **screen space** (see `assets/shaders/portal.wesl`).
//!
//! An oblique near clip plane at the destination portal hides the wall the
//! portal is mounted on.

use bevy::camera::RenderTarget;
use bevy::camera::visibility::RenderLayers;
use bevy::color::palettes::tailwind;
use bevy::prelude::*;
use bevy::reflect::TypePath;
use bevy::render::render_resource::{AsBindGroup, TextureFormat};
use bevy::render::view::{DebandDither, Tonemapping};
use bevy::shader::ShaderRef;
use bevy::window::{PrimaryWindow, WindowResized};

use std::f32::consts::PI;

use crate::physics::{Look, Player};
use crate::player::PlayerCamera;
use crate::world::{
    PORTAL_A_CENTER, PORTAL_B_CENTER, PORTAL_HEIGHT, PORTAL_LAYER, PORTAL_WIDTH, WORLD_LAYER,
};

/// How far from the portal plane the player can be and still be teleported.
/// Must be larger than the player radius plus the wall half thickness,
/// otherwise the wall stops the player first.
const TELEPORT_DISTANCE: f32 = 0.55;
/// Distance in front of the destination portal at which the player is placed.
/// Roughly the distance at which the player's capsule stops against the wall,
/// so the transition is seamless.
const EXIT_CLEARANCE: f32 = 0.45;
const TELEPORT_COOLDOWN: f32 = 0.3;

/// A portal.
#[derive(Component)]
pub struct Portal {
    pub partner: Entity,
}

/// The virtual camera that renders a portal's view.
#[derive(Component)]
pub struct PortalCamera {
    pub portal: Entity,
}

/// The screen mesh of a portal, and the material it uses.
#[derive(Component)]
pub struct PortalScreen {
    pub portal: Entity,
}

/// Number of portal transitions, for the UI.
#[derive(Resource, Default)]
pub struct TeleportCount(pub u32);

/// Draws the portal image. The shader samples the image in screen space, so no
/// UVs are involved.
#[derive(Asset, TypePath, AsBindGroup, Clone)]
pub struct PortalMaterial {
    #[texture(0)]
    #[sampler(1)]
    pub image: Handle<Image>,
}

impl Material for PortalMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/portal.wesl".into()
    }
}

pub struct PortalPlugin;

impl Plugin for PortalPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MaterialPlugin::<PortalMaterial>::default())
            .init_resource::<TeleportCount>()
            .add_systems(Startup, setup_portals)
            .add_systems(
                PostUpdate,
                (update_portal_cameras, resize_portal_targets)
                    .chain()
                    .after(bevy::transform::TransformSystems::Propagate),
            )
            .add_systems(Update, teleport_player);
    }
}

fn portal_image(images: &mut Assets<Image>, size: UVec2) -> Handle<Image> {
    images.add(Image::new_target_texture(
        size.x,
        size.y,
        TextureFormat::Rgba16Float,
        None,
    ))
}

fn setup_portals(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut portal_materials: ResMut<Assets<PortalMaterial>>,
    mut images: ResMut<Assets<Image>>,
    window: Single<&Window, With<PrimaryWindow>>,
) {
    let size = uvec2(window.physical_width(), window.physical_height()).max(UVec2::ONE);

    let a = spawn_portal(
        &mut commands,
        &mut meshes,
        &mut materials,
        &mut portal_materials,
        &mut images,
        PortalSpec {
            center: PORTAL_A_CENTER,
            yaw: -PI / 2.0,
            color: tailwind::ORANGE_400,
        },
        size,
    );
    let b = spawn_portal(
        &mut commands,
        &mut meshes,
        &mut materials,
        &mut portal_materials,
        &mut images,
        PortalSpec {
            center: PORTAL_B_CENTER,
            yaw: PI / 2.0,
            color: tailwind::SKY_400,
        },
        size,
    );

    commands.entity(a).insert(Portal { partner: b });
    commands.entity(b).insert(Portal { partner: a });
}

struct PortalSpec {
    center: Vec3,
    yaw: f32,
    color: Srgba,
}

fn spawn_portal(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    portal_materials: &mut Assets<PortalMaterial>,
    images: &mut Assets<Image>,
    spec: PortalSpec,
    image_size: UVec2,
) -> Entity {
    let image = portal_image(images, image_size);

    let root = commands
        .spawn((
            Transform::from_translation(spec.center).with_rotation(Quat::from_rotation_y(spec.yaw)),
            // The parent of a rendered child needs `Visibility` too, otherwise
            // Bevy warns that the hierarchy is inconsistent.
            Visibility::default(),
            Name::new("Portal"),
        ))
        .id();

    // The screen. It is the only object on `PORTAL_LAYER`, so portal cameras do
    // not render portals into portals (no recursion, no infinite mirror).
    commands.spawn((
        Mesh3d(meshes.add(Rectangle::new(PORTAL_WIDTH, PORTAL_HEIGHT))),
        MeshMaterial3d(portal_materials.add(PortalMaterial { image: image.clone() })),
        RenderLayers::layer(PORTAL_LAYER),
        PortalScreen { portal: root },
        ChildOf(root),
        Name::new("PortalScreen"),
    ));

    // A deliberately chunky frame so it reads as a portal, not a hole.
    let frame_color = Color::from(spec.color);
    let frame_material = materials.add(StandardMaterial {
        base_color: frame_color,
        emissive: LinearRgba::from(frame_color) * 2.0,
        perceptual_roughness: 0.5,
        ..default()
    });

    let thickness = 0.16;
    let depth = 0.22;
    let half_w = PORTAL_WIDTH * 0.5 + thickness * 0.5;
    let pieces = [
        (
            Vec3::new(0.0, PORTAL_HEIGHT * 0.5 + thickness * 0.5, 0.0),
            Vec3::new(PORTAL_WIDTH + thickness * 2.0, thickness, depth),
        ),
        (
            Vec3::new(0.0, -PORTAL_HEIGHT * 0.5 - thickness * 0.5, 0.0),
            Vec3::new(PORTAL_WIDTH + thickness * 2.0, thickness, depth),
        ),
        (
            Vec3::new(half_w, 0.0, 0.0),
            Vec3::new(thickness, PORTAL_HEIGHT, depth),
        ),
        (
            Vec3::new(-half_w, 0.0, 0.0),
            Vec3::new(thickness, PORTAL_HEIGHT, depth),
        ),
    ];
    for (offset, size) in pieces {
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(size.x, size.y, size.z))),
            MeshMaterial3d(frame_material.clone()),
            Transform::from_translation(offset),
            RenderLayers::layer(WORLD_LAYER),
            ChildOf(root),
        ));
    }

    // The virtual camera.
    commands.spawn((
        Camera3d::default(),
        Camera {
            order: -1,
            clear_color: ClearColorConfig::Custom(Color::BLACK),
            ..default()
        },
        // Render linear HDR values: the *player's* camera tonemaps the final
        // frame once, exactly like it does for the rest of the scene. If this
        // camera tonemapped too, the image would be tonemapped twice.
        Tonemapping::None,
        DebandDither::Disabled,
        Projection::Perspective(PerspectiveProjection::default()),
        RenderTarget::Image(image.into()),
        Transform::default(),
        GlobalTransform::default(),
        RenderLayers::layer(WORLD_LAYER),
        PortalCamera { portal: root },
        Name::new("PortalCamera"),
    ));

    root
}

/// The affine transform that maps a point from the source portal's frame into
/// the partner portal's frame, including the 180° yaw that makes the two
/// portals face away from each other.
///
/// Used for both the portal camera and the player teleport, so the picture and
/// the actual transition can never disagree.
fn portal_map(src: &Transform, dst: &Transform) -> Mat4 {
    dst.compute_affine() * Mat4::from_rotation_y(PI) * src.compute_affine().inverse()
}

/// Places every portal camera where the player's camera would be if they had
/// already walked through the portal.
#[allow(clippy::type_complexity)]
fn update_portal_cameras(
    player_camera: Query<(&GlobalTransform, &Projection), (With<PlayerCamera>, Without<PortalCamera>, Without<Portal>)>,
    portals: Query<&GlobalTransform, With<Portal>>,
    mut cameras: Query<
        (&PortalCamera, &mut Transform, &mut GlobalTransform, &mut Projection),
        (Without<PlayerCamera>, Without<Portal>),
    >,
    portal_data: Query<&Portal>,
) {
    let Ok((camera_global, Projection::Perspective(camera_projection))) = player_camera.single()
    else {
        return;
    };

    for (portal_camera, mut transform, mut global, mut projection) in &mut cameras {
        let Ok(portal_global) = portals.get(portal_camera.portal) else {
            continue;
        };
        let Ok(portal) = portal_data.get(portal_camera.portal) else {
            continue;
        };
        let Ok(partner_global) = portals.get(portal.partner) else {
            continue;
        };

        let affine = portal_map(
            &portal_global.compute_transform(),
            &partner_global.compute_transform(),
        ) * camera_global.affine();

        *transform = Transform::from_matrix(affine);
        *global = GlobalTransform::from(affine);

        // Oblique near plane at the destination portal: hides the wall the
        // portal is mounted on, so the image is a clean view into the room.
        let plane_point = partner_global.translation();
        let plane_normal = partner_global.rotation() * Vec3::Z;

        let view = affine.inverse();
        let point_in_view = view.transform_point3(plane_point);
        let normal_in_view = view.transform_vector3(plane_normal).normalize();

        *projection = Projection::Perspective(PerspectiveProjection {
            fov: camera_projection.fov,
            aspect_ratio: camera_projection.aspect_ratio,
            near: camera_projection.near,
            far: camera_projection.far,
            near_clip_plane: normal_in_view.extend(-normal_in_view.dot(point_in_view)),
        });
    }
}

/// Teleports the player when they step into a portal plane.
fn teleport_player(
    portals: Query<(Entity, &Transform, &Portal), Without<Player>>,
    mut players: Query<(&mut Transform, &mut Player, &mut Look), Without<Portal>>,
    mut count: ResMut<TeleportCount>,
) {
    for (mut transform, mut player, mut look) in &mut players {
        if player.teleport_cooldown > 0.0 {
            continue;
        }

        for (_, portal_transform, portal) in &portals {
            let local = portal_transform.compute_affine()
                .inverse()
                .transform_point3(transform.translation);

            // In front of the plane, inside the rectangle, and close enough.
            let inside = local.x.abs() <= PORTAL_WIDTH * 0.5
                && local.y.abs() <= PORTAL_HEIGHT * 0.5
                && local.z > 0.0
                && local.z < TELEPORT_DISTANCE;
            if !inside {
                continue;
            }

            let normal = portal_transform.rotation * Vec3::Z;
            if player.velocity.dot(normal) > -0.05 {
                continue; // Not walking into the portal.
            }

            let Ok((_, partner_transform, _)) = portals.get(portal.partner) else {
                continue;
            };

            // Same mapping as the camera, applied to the player body.
            let map = portal_map(portal_transform, partner_transform);

            let mut position = map.transform_point3(transform.translation);
            let (_, rotation, _) = map.to_scale_rotation_translation();

            // The mapped point lands behind the destination portal; nudge it
            // out in front so the player does not spawn inside the wall.
            let partner_normal = partner_transform.rotation * Vec3::Z;
            let local_out = partner_transform.compute_affine()
                .inverse()
                .transform_point3(position);
            if local_out.z < EXIT_CLEARANCE {
                position += partner_normal * (EXIT_CLEARANCE - local_out.z);
            }

            transform.translation = position;

            // Keep the view continuous: rotate the body by the portal rotation
            // and leave the pitch untouched.
            let new_rotation = rotation * transform.rotation;
            look.yaw = new_rotation.to_euler(EulerRot::YXZ).0;
            transform.rotation = Quat::from_rotation_y(look.yaw);

            player.velocity = rotation * player.velocity;
            player.teleport_cooldown = TELEPORT_COOLDOWN;
            count.0 += 1;
            break;
        }
    }
}

/// Rebuilds the render targets when the window changes size. The effect relies
/// on the portal image having the same pixel size as the player's viewport.
fn resize_portal_targets(
    mut resized: MessageReader<WindowResized>,
    window: Single<&Window, With<PrimaryWindow>>,
    mut images: ResMut<Assets<Image>>,
    mut cameras: Query<(&PortalCamera, &mut RenderTarget)>,
    screens: Query<(&PortalScreen, &MeshMaterial3d<PortalMaterial>)>,
    mut portal_materials: ResMut<Assets<PortalMaterial>>,
) {
    if resized.read().next().is_none() {
        return;
    }
    let size = uvec2(window.physical_width(), window.physical_height()).max(UVec2::ONE);

    for (portal_camera, mut target) in &mut cameras {
        let image = portal_image(&mut images, size);
        *target = RenderTarget::Image(image.clone().into());

        // Point the matching screen at the new image.
        for (screen, material_handle) in &screens {
            if screen.portal == portal_camera.portal
                && let Some(mut material) = portal_materials.get_mut(&material_handle.0)
            {
                material.image = image.clone();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn portal(translation: Vec3, yaw: f32) -> Transform {
        Transform::from_translation(translation).with_rotation(Quat::from_rotation_y(yaw))
    }

    /// Portals A and B exactly as the level positions them.
    fn pair() -> (Transform, Transform) {
        (
            portal(Vec3::new(-0.12, 1.0, -3.0), -PI / 2.0),
            portal(Vec3::new(0.12, 1.0, 3.0), PI / 2.0),
        )
    }

    /// A player 2 m in front of portal A must end up 2 m behind portal B.
    #[test]
    fn teleport_lands_behind_the_partner_portal() {
        let (a, b) = pair();
        let start = a.translation + a.rotation * Vec3::Z * 2.0;
        let mapped = portal_map(&a, &b).transform_point3(start);

        let expected = b.translation - b.rotation * Vec3::Z * 2.0;
        assert!(
            mapped.distance(expected) < 1e-4,
            "mapped {mapped} != expected {expected}"
        );
    }

    /// Walking through and back must return roughly to the start.
    #[test]
    fn teleport_is_reversible() {
        let (a, b) = pair();
        let start = a.translation + a.rotation * Vec3::Z * 1.5;
        let there = portal_map(&a, &b).transform_point3(start);
        let back = portal_map(&b, &a).transform_point3(there);
        assert!(back.distance(start) < 1e-4, "round trip drifted to {back}");
    }

    /// The portal camera stands behind the destination portal and looks out
    /// into the destination room.
    #[test]
    fn camera_looks_into_the_destination_room() {
        let (a, b) = pair();
        let player_camera = Transform::from_translation(a.translation + a.rotation * Vec3::Z * 3.0)
            .looking_at(a.translation, Vec3::Y);

        let virtual_camera = portal_map(&a, &b) * player_camera.compute_affine();
        let virtual_transform = Transform::from_matrix(virtual_camera);

        let expected_position = b.translation - b.rotation * Vec3::Z * 3.0;
        assert!(
            virtual_transform.translation.distance(expected_position) < 1e-4,
            "camera at {} instead of {expected_position}",
            virtual_transform.translation
        );

        // Forward should point out of portal B into its room.
        let outward = b.rotation * Vec3::Z;
        assert!(
            virtual_transform.forward().dot(outward) > 0.99,
            "camera looks {:?}, expected roughly {outward:?}",
            virtual_transform.forward()
        );
    }

    /// The mapped point must land on the far side of the destination portal,
    /// which is why `teleport_player` pushes it back out.
    #[test]
    fn mapped_point_is_behind_the_destination_plane() {
        let (a, b) = pair();
        let point = a.translation + a.rotation * Vec3::Z * 0.5;
        let mapped = portal_map(&a, &b).transform_point3(point);
        let local = b.compute_affine().inverse().transform_point3(mapped);
        assert!(local.z < 0.0, "expected local z < 0, got {}", local.z);
    }
}
