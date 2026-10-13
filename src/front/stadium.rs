//! The static 3D scene: pitch, markings, goals, stands, boards, floodlights and lighting.
//!
//! Simulation space is (x along the pitch, y across it, z up). World space is Bevy's
//! y-up, so a simulation point (x, y, z) sits at world (x, z, y).

use super::font5x7::text_image;
use super::textures;
use super::AppState;
use crate::sim::consts::*;
use bevy::asset::RenderAssetUsages;
use bevy::pbr::{CascadeShadowConfigBuilder, NotShadowCaster};
use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};
use std::f32::consts::{FRAC_PI_2, PI, TAU};

pub fn sim_to_world(p: Vec2, z: f32) -> Vec3 {
    Vec3::new(p.x, z, p.y)
}

/// Accumulates flat quads into one mesh (used for all pitch markings).
#[derive(Default)]
struct QuadMesh {
    pos: Vec<[f32; 3]>,
    nor: Vec<[f32; 3]>,
    uv: Vec<[f32; 2]>,
    idx: Vec<u32>,
}

impl QuadMesh {
    fn quad(&mut self, corners: [Vec3; 4], normal: Vec3) {
        let base = self.pos.len() as u32;
        for (k, c) in corners.iter().enumerate() {
            self.pos.push(c.to_array());
            self.nor.push(normal.to_array());
            self.uv.push([(k == 1 || k == 2) as u8 as f32, (k >= 2) as u8 as f32]);
        }
        self.idx.extend_from_slice(&[base, base + 2, base + 1, base, base + 3, base + 2]);
    }

    /// A painted line of `width` metres between two pitch points.
    fn line(&mut self, a: Vec2, b: Vec2, width: f32) {
        let d = (b - a).normalize_or_zero();
        let n = d.perp() * width / 2.0;
        let y = 0.012;
        let w = |p: Vec2| Vec3::new(p.x, y, p.y);
        self.quad([w(a - n), w(b - n), w(b + n), w(a + n)], Vec3::Y);
    }

    fn arc(&mut self, c: Vec2, r: f32, from: f32, to: f32, width: f32) {
        let n = ((to - from).abs() * r * 1.5).ceil().max(6.0) as usize;
        for k in 0..n {
            let a0 = from + (to - from) * k as f32 / n as f32;
            let a1 = from + (to - from) * (k + 1) as f32 / n as f32;
            self.line(c + Vec2::from_angle(a0) * r, c + Vec2::from_angle(a1) * r, width);
        }
    }

    fn disc(&mut self, c: Vec2, r: f32) {
        self.arc(c, r / 2.0, 0.0, TAU, r);
    }

    fn build(self) -> Mesh {
        Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
            .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, self.pos)
            .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, self.nor)
            .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, self.uv)
            .with_inserted_indices(Indices::U32(self.idx))
    }
}

fn markings() -> Mesh {
    let mut m = QuadMesh::default();
    let w = 0.12;
    let r = |x0: f32, y0: f32, x1: f32, y1: f32, m: &mut QuadMesh| {
        m.line(Vec2::new(x0, y0), Vec2::new(x1, y0), w);
        m.line(Vec2::new(x1, y0), Vec2::new(x1, y1), w);
        m.line(Vec2::new(x1, y1), Vec2::new(x0, y1), w);
        m.line(Vec2::new(x0, y1), Vec2::new(x0, y0), w);
    };
    r(-HALF_L, -HALF_W, HALF_L, HALF_W, &mut m);
    m.line(Vec2::new(0.0, -HALF_W), Vec2::new(0.0, HALF_W), w);
    m.arc(Vec2::ZERO, CIRCLE_R, 0.0, TAU, w);
    m.disc(Vec2::ZERO, 0.3);
    for side in [-1.0f32, 1.0] {
        let gx = side * HALF_L;
        r(gx, -BOX_HW, gx - side * BOX_D, BOX_HW, &mut m);
        r(gx, -SIX_HW, gx - side * SIX_D, SIX_HW, &mut m);
        let spot = Vec2::new(gx - side * PEN_SPOT, 0.0);
        m.disc(spot, 0.3);
        let half = ((BOX_D - PEN_SPOT) / CIRCLE_R).acos();
        let facing = if side > 0.0 { PI } else { 0.0 };
        m.arc(spot, CIRCLE_R, facing - half, facing + half, w);
        for ys in [-1.0f32, 1.0] {
            let start = if side > 0.0 { PI } else { 0.0 };
            m.arc(Vec2::new(gx, ys * HALF_W), 1.0, start, start - side * ys * FRAC_PI_2, w);
        }
    }
    m.build()
}

/// A sloped bank of seats facing the pitch: `length` along the stand, rising from `near` to `far`
/// metres from the pitch edge. Built facing +z, then rotated into place.
fn stand_mesh(length: f32, near: f32, far: f32, height: f32, tiles: f32) -> Mesh {
    let mut m = QuadMesh::default();
    let h = length / 2.0;
    let slope = Vec3::new(0.0, height, -(far - near)).normalize();
    let normal = Vec3::X.cross(slope).normalize();
    let a = Vec3::new(-h, 0.8, -near);
    let b = Vec3::new(h, 0.8, -near);
    let c = Vec3::new(h, height, -far);
    let d = Vec3::new(-h, height, -far);
    m.quad([a, b, c, d], normal);
    // Stretch the crowd texture across the stand.
    for (k, uv) in m.uv.iter_mut().enumerate() {
        uv[0] = if k == 1 || k == 2 { tiles } else { 0.0 };
        uv[1] = if k >= 2 { tiles * 0.4 * height / 18.0 } else { 0.0 };
    }
    m.build()
}

#[derive(Component)]
pub struct Stadium;

pub fn spawn_stadium(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    images: &mut Assets<Image>,
    home_colour: u32,
    away_colour: u32,
    league: &str,
) {
    let scoped = StateScoped(AppState::Match);

    // Sky, sun and fill light.
    commands.insert_resource(ClearColor(Color::srgb(0.05, 0.07, 0.13)));
    commands.insert_resource(AmbientLight { color: Color::srgb(0.75, 0.8, 1.0), brightness: 650.0, ..default() });
    commands.spawn((
        DirectionalLight { illuminance: 9_000.0, shadows_enabled: true, ..default() },
        Transform::from_xyz(-30.0, 60.0, 40.0).looking_at(Vec3::ZERO, Vec3::Y),
        CascadeShadowConfigBuilder { maximum_distance: 140.0, first_cascade_far_bound: 30.0, ..default() }.build(),
        scoped.clone(),
    ));

    // Surround and pitch.
    let runoff = materials.add(StandardMaterial { base_color: Color::srgb(0.15, 0.36, 0.15), perceptual_roughness: 0.95, ..default() });
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(HALF_L * 2.0 + 16.0, HALF_W * 2.0 + 14.0))),
        MeshMaterial3d(runoff),
        Transform::from_xyz(0.0, -0.01, 0.0),
        scoped.clone(),
    ));
    let grass = images.add(textures::grass(18));
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(HALF_L * 2.0, HALF_W * 2.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color_texture: Some(grass),
            perceptual_roughness: 0.9,
            reflectance: 0.2,
            ..default()
        })),
        scoped.clone(),
    ));
    commands.spawn((
        Mesh3d(meshes.add(markings())),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.95, 0.95, 0.95),
            perceptual_roughness: 0.8,
            ..default()
        })),
        NotShadowCaster,
        scoped.clone(),
    ));

    spawn_goals(commands, meshes, materials, images);

    // Stands on all four sides, plus a roof over the long sides.
    let crowd = images.add(textures::crowd(home_colour, away_colour));
    let crowd_mat = materials.add(StandardMaterial {
        base_color_texture: Some(crowd),
        perceptual_roughness: 1.0,
        cull_mode: None,
        double_sided: true,
        ..default()
    });
    let concrete = materials.add(StandardMaterial { base_color: Color::srgb(0.32, 0.33, 0.37), perceptual_roughness: 0.9, ..default() });
    let roof = materials.add(StandardMaterial { base_color: Color::srgb(0.12, 0.13, 0.16), perceptual_roughness: 0.7, ..default() });
    let long = meshes.add(stand_mesh(HALF_L * 2.0 + 20.0, 0.0, 30.0, 20.0, 34.0));
    let short = meshes.add(stand_mesh(HALF_W * 2.0 + 16.0, 0.0, 26.0, 17.0, 22.0));
    let sides: [(Handle<Mesh>, Vec3, f32, f32); 4] = [
        (long.clone(), Vec3::new(0.0, 0.0, -(HALF_W + 9.0)), 0.0, HALF_L * 2.0 + 20.0),
        (long.clone(), Vec3::new(0.0, 0.0, HALF_W + 9.0), PI, HALF_L * 2.0 + 20.0),
        (short.clone(), Vec3::new(-(HALF_L + 9.0), 0.0, 0.0), FRAC_PI_2, HALF_W * 2.0 + 16.0),
        (short.clone(), Vec3::new(HALF_L + 9.0, 0.0, 0.0), -FRAC_PI_2, HALF_W * 2.0 + 16.0),
    ];
    for (k, (mesh, at, yaw, len)) in sides.into_iter().enumerate() {
        let rot = Quat::from_rotation_y(yaw);
        commands.spawn((Mesh3d(mesh), MeshMaterial3d(crowd_mat.clone()), Transform::from_translation(at).with_rotation(rot), scoped.clone()));
        // Front wall below the first row.
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(len, 1.2, 0.4))),
            MeshMaterial3d(concrete.clone()),
            Transform::from_translation(at + rot * Vec3::new(0.0, 0.6, 0.0)).with_rotation(rot),
            scoped.clone(),
        ));
        // Back wall.
        let depth = if k < 2 { 30.0 } else { 26.0 };
        let height = if k < 2 { 20.0 } else { 17.0 };
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(len, height + 6.0, 1.0))),
            MeshMaterial3d(concrete.clone()),
            Transform::from_translation(at + rot * Vec3::new(0.0, (height + 6.0) / 2.0, -depth - 0.5)).with_rotation(rot),
            scoped.clone(),
        ));
        if k < 2 {
            commands.spawn((
                Mesh3d(meshes.add(Cuboid::new(len, 0.6, 24.0))),
                MeshMaterial3d(roof.clone()),
                Transform::from_translation(at + rot * Vec3::new(0.0, height + 6.0, -depth + 11.0)).with_rotation(rot),
                NotShadowCaster,
                scoped.clone(),
            ));
        }
    }

    // LED advertising boards around the pitch, with parody sponsors.
    let ads = [
        "BIGILEEE 22",
        "EAT MORE DOSA",
        "SAMBAR ENERGY",
        "VADA PAV AIRWAYS",
        league,
        "FILTER KAAPI",
        "KAALPANTHU",
        "PAROTTA BANK",
    ];
    let ad_mats: Vec<Handle<StandardMaterial>> = ads
        .iter()
        .enumerate()
        .map(|(k, text)| {
            let bgs = [[16, 30, 90, 255], [150, 20, 30, 255], [10, 10, 14, 255], [0, 110, 70, 255]];
            let fgs = [[255, 255, 255, 255], [255, 220, 60, 255]];
            let img = images.add(text_image(text, fgs[k % 2], bgs[k % 4], 6, 10));
            materials.add(StandardMaterial {
                base_color_texture: Some(img.clone()),
                emissive_texture: Some(img),
                emissive: LinearRgba::rgb(1.6, 1.6, 1.6),
                unlit: false,
                ..default()
            })
        })
        .collect();
    let board_w = 12.0;
    let mut k = 0;
    for (along, offset, yaw, len) in [
        (Vec3::X, Vec3::new(0.0, 0.0, -(HALF_W + 4.0)), 0.0, HALF_L * 2.0 + 6.0),
        (Vec3::X, Vec3::new(0.0, 0.0, HALF_W + 4.0), PI, HALF_L * 2.0 + 6.0),
        (Vec3::Z, Vec3::new(-(HALF_L + 4.0), 0.0, 0.0), FRAC_PI_2, HALF_W * 2.0),
        (Vec3::Z, Vec3::new(HALF_L + 4.0, 0.0, 0.0), -FRAC_PI_2, HALF_W * 2.0),
    ] {
        let n = (len / board_w).floor() as i32;
        for i in 0..n {
            let t = (i as f32 + 0.5) * board_w - n as f32 * board_w / 2.0;
            let at = offset + along * t + Vec3::Y * 0.5;
            // Leave a gap behind each goal for the net.
            if along == Vec3::Z && t.abs() < 6.0 {
                continue;
            }
            commands.spawn((
                Mesh3d(meshes.add(Plane3d::new(Vec3::Z, Vec2::new(board_w / 2.0 - 0.05, 0.45)))),
                MeshMaterial3d(ad_mats[k % ad_mats.len()].clone()),
                Transform::from_translation(at).with_rotation(Quat::from_rotation_y(yaw)),
                NotShadowCaster,
                scoped.clone(),
            ));
            k += 1;
        }
    }

    // Floodlight towers in the corners.
    let steel = materials.add(StandardMaterial { base_color: Color::srgb(0.5, 0.52, 0.56), metallic: 0.6, ..default() });
    let lamp = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        emissive: LinearRgba::rgb(40.0, 38.0, 32.0),
        ..default()
    });
    for (sx, sz) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
        let base = Vec3::new(sx * (HALF_L + 22.0), 0.0, sz * (HALF_W + 24.0));
        commands.spawn((
            Mesh3d(meshes.add(Cylinder::new(0.6, 46.0))),
            MeshMaterial3d(steel.clone()),
            Transform::from_translation(base + Vec3::Y * 23.0),
            scoped.clone(),
        ));
        let head = base + Vec3::Y * 47.0;
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(8.0, 4.0, 0.6))),
            MeshMaterial3d(lamp.clone()),
            Transform::from_translation(head).looking_at(Vec3::ZERO, Vec3::Y),
            NotShadowCaster,
            scoped.clone(),
        ));
        commands.spawn((
            PointLight { intensity: 6.0e7, range: 180.0, shadows_enabled: false, ..default() },
            Transform::from_translation(head - head.normalize() * 2.0),
            scoped.clone(),
        ));
    }
}

fn spawn_goals(commands: &mut Commands, meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>, images: &mut Assets<Image>) {
    let scoped = StateScoped(AppState::Match);
    let white = materials.add(StandardMaterial { base_color: Color::WHITE, perceptual_roughness: 0.3, ..default() });
    let net_img = images.add(textures::net());
    let net = materials.add(StandardMaterial {
        base_color_texture: Some(net_img),
        alpha_mode: AlphaMode::Mask(0.5),
        cull_mode: None,
        double_sided: true,
        perceptual_roughness: 1.0,
        ..default()
    });
    let r = 0.06;
    let post = meshes.add(Cylinder::new(r, GOAL_H + r));
    let bar = meshes.add(Cylinder::new(r, GOAL_HW * 2.0 + 2.0 * r));
    let back_post = meshes.add(Cylinder::new(0.025, GOAL_H));
    for side in [-1.0f32, 1.0] {
        let x = side * (HALF_L + r);
        let bx = side * (HALF_L + GOAL_DEPTH);
        for ys in [-1.0f32, 1.0] {
            commands.spawn((Mesh3d(post.clone()), MeshMaterial3d(white.clone()), Transform::from_xyz(x, (GOAL_H + r) / 2.0, ys * GOAL_HW), scoped.clone()));
            commands.spawn((Mesh3d(back_post.clone()), MeshMaterial3d(white.clone()), Transform::from_xyz(bx, GOAL_H / 2.0, ys * GOAL_HW), scoped.clone()));
        }
        commands.spawn((
            Mesh3d(bar.clone()),
            MeshMaterial3d(white.clone()),
            Transform::from_xyz(x, GOAL_H + r, 0.0).with_rotation(Quat::from_rotation_x(FRAC_PI_2)),
            scoped.clone(),
        ));
        // Net: back, roof and two sides, with the mesh texture tiled per metre.
        let tile = |w: f32, h: f32| {
            let mut mesh = Mesh::from(Plane3d::new(Vec3::Y, Vec2::new(w / 2.0, h / 2.0)));
            if let Some(bevy::render::mesh::VertexAttributeValues::Float32x2(uvs)) = mesh.attribute_mut(Mesh::ATTRIBUTE_UV_0) {
                for uv in uvs.iter_mut() {
                    uv[0] *= w * 4.0;
                    uv[1] *= h * 4.0;
                }
            }
            mesh
        };
        let back = meshes.add(tile(GOAL_H, GOAL_HW * 2.0));
        commands.spawn((
            Mesh3d(back),
            MeshMaterial3d(net.clone()),
            Transform::from_xyz(bx, GOAL_H / 2.0, 0.0).with_rotation(Quat::from_rotation_z(FRAC_PI_2)),
            NotShadowCaster,
            scoped.clone(),
        ));
        let roof = meshes.add(tile(GOAL_DEPTH, GOAL_HW * 2.0));
        commands.spawn((
            Mesh3d(roof),
            MeshMaterial3d(net.clone()),
            Transform::from_xyz(side * (HALF_L + GOAL_DEPTH / 2.0), GOAL_H, 0.0),
            NotShadowCaster,
            scoped.clone(),
        ));
        let wall = meshes.add(tile(GOAL_DEPTH, GOAL_H));
        for ys in [-1.0f32, 1.0] {
            commands.spawn((
                Mesh3d(wall.clone()),
                MeshMaterial3d(net.clone()),
                Transform::from_xyz(side * (HALF_L + GOAL_DEPTH / 2.0), GOAL_H / 2.0, ys * GOAL_HW)
                    .with_rotation(Quat::from_rotation_x(FRAC_PI_2)),
                NotShadowCaster,
                scoped.clone(),
            ));
        }
    }
}
