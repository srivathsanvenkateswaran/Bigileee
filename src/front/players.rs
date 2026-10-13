//! Player and ball models, and their procedural animation.
//!
//! Players are generic low-poly figures in their team's kit. Each rig is a root at the player's
//! feet, a hip pivot carrying the upper body, and four limb pivots that swing for running and kicking.

use super::font5x7::text_image;
use super::stadium::sim_to_world;
use super::textures::{self, color};
use super::view::{PlayerView, View};
use super::AppState;
use crate::sim::game::{Match, Role};
use bevy::pbr::NotShadowCaster;
use bevy::prelude::*;
use std::f32::consts::{FRAC_PI_2, PI, TAU};

pub const BALL_R: f32 = 0.13;

#[derive(Component)]
pub struct Rig {
    pub idx: usize,
    phase: f32,
    hips: Entity,
    leg_l: Entity,
    leg_r: Entity,
    arm_l: Entity,
    arm_r: Entity,
}

#[derive(Component)]
pub struct BallModel {
    spin: Quat,
}

/// Ring under, and arrow over, the player a human controls.
#[derive(Component)]
pub struct Indicator {
    pub human: usize,
    pub arrow: bool,
}

pub const HUMAN_COLOURS: [Color; 2] = [Color::srgb(1.0, 0.85, 0.15), Color::srgb(0.25, 0.85, 1.0)];

struct Parts {
    torso: Handle<Mesh>,
    shorts: Handle<Mesh>,
    head: Handle<Mesh>,
    hair: Handle<Mesh>,
    arm: Handle<Mesh>,
    sleeve: Handle<Mesh>,
    thigh: Handle<Mesh>,
    shin: Handle<Mesh>,
    boot: Handle<Mesh>,
    number: Handle<Mesh>,
    neck: Handle<Mesh>,
    collar: Handle<Mesh>,
    hand: Handle<Mesh>,
    band: Handle<Mesh>,
    beard: Handle<Mesh>,
}

fn lum(c: u32) -> f32 {
    let [r, g, b] = textures::rgb(c);
    r as f32 * 0.3 + g as f32 * 0.59 + b as f32 * 0.11
}

pub fn spawn_players(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    images: &mut Assets<Image>,
    m: &Match,
) {
    let scoped = StateScoped(AppState::Match);
    let parts = Parts {
        torso: meshes.add(Capsule3d::new(0.17, 0.3)),
        shorts: meshes.add(Cuboid::new(0.36, 0.22, 0.22)),
        head: meshes.add(Sphere::new(0.115)),
        hair: meshes.add(Sphere::new(0.12)),
        arm: meshes.add(Capsule3d::new(0.045, 0.36)),
        sleeve: meshes.add(Capsule3d::new(0.062, 0.1)),
        thigh: meshes.add(Capsule3d::new(0.072, 0.3)),
        shin: meshes.add(Capsule3d::new(0.058, 0.3)),
        boot: meshes.add(Cuboid::new(0.1, 0.08, 0.26)),
        number: meshes.add(Plane3d::new(Vec3::Z, Vec2::new(0.13, 0.11))),
        neck: meshes.add(Cylinder::new(0.055, 0.12)),
        collar: meshes.add(Torus::new(0.05, 0.085)),
        hand: meshes.add(Sphere::new(0.05)),
        band: meshes.add(Cylinder::new(0.066, 0.06)),
        beard: meshes.add(Sphere::new(0.085)),
    };
    let mat = |materials: &mut Assets<StandardMaterial>, c: Color| {
        materials.add(StandardMaterial { base_color: c, perceptual_roughness: 0.75, ..default() })
    };
    let skins: Vec<Handle<StandardMaterial>> = [0xE8B896u32, 0xC68A5E, 0x9C6640, 0x6B4429]
        .iter()
        .map(|&c| mat(materials, color(c)))
        .collect();
    let hairs: Vec<Handle<StandardMaterial>> = [0x1A1410u32, 0x3B2414, 0x6E4A26, 0xC9A45C, 0x2A2A2A]
        .iter()
        .map(|&c| mat(materials, color(c)))
        .collect();
    let boots: Vec<Handle<StandardMaterial>> = [0x141418u32, 0x141418, 0xF4F4F4, 0xFF5A1F, 0x23C4F0, 0xE8FF3A, 0xE0218A]
        .iter()
        .map(|&c| mat(materials, color(c)))
        .collect();

    for t in 0..2 {
        let team = &m.teams[t];
        let shirt_c = m.kit[t];
        let trim_c = if shirt_c == team.home { team.trim } else { team.home };
        let kit_img = images.add(textures::kit(team.crest, shirt_c, trim_c));
        let shirt = materials.add(StandardMaterial { base_color_texture: Some(kit_img), perceptual_roughness: 0.75, ..default() });
        let sleeves = mat(materials, color(shirt_c));
        let trim = mat(materials, color(trim_c));
        let gloves = mat(materials, color(0xE8FF3A));
        let keeper = mat(materials, color(team.keeper));
        let shorts_c = if (lum(shirt_c) - lum(trim_c)).abs() > 60.0 { trim_c } else if lum(shirt_c) > 128.0 { 0x15151A } else { 0xF2F2F2 };
        let shorts = mat(materials, color(shorts_c));
        let socks = mat(materials, color(shirt_c));
        let number_fg = |bg: u32| if lum(bg) > 140.0 { [20, 20, 28, 255] } else { [250, 250, 250, 255] };

        for i in t * 11..t * 11 + 11 {
            let p = &m.players[i];
            let gk = p.role == Role::Gk;
            let top = if gk { keeper.clone() } else { shirt.clone() };
            let sleeve = if gk { keeper.clone() } else { sleeves.clone() };
            let seed = p.name.bytes().fold(7usize, |a, b| a.wrapping_mul(31).wrapping_add(b as usize));
            let skin = skins[seed % skins.len()].clone();
            let hair = hairs[(seed / 7) % hairs.len()].clone();
            let num_img = images.add(text_image(&p.num.to_string(), number_fg(if gk { team.keeper } else { shirt_c }), [0, 0, 0, 0], 8, 2));
            let num_mat = materials.add(StandardMaterial {
                base_color_texture: Some(num_img),
                alpha_mode: AlphaMode::Mask(0.5),
                perceptual_roughness: 0.8,
                ..default()
            });

            // Heights vary a little; keepers and centre-backs run tall.
            let tall = matches!(p.role, Role::Gk | Role::Def) as u8 as f32 * 0.03;
            let height = 0.94 + (seed % 9) as f32 * 0.012 + tall;
            let root = commands
                .spawn((
                    Transform::from_translation(sim_to_world(p.pos, 0.0)).with_scale(Vec3::splat(height)),
                    Visibility::default(),
                    scoped.clone(),
                ))
                .id();
            let hips = commands.spawn((Transform::from_xyz(0.0, 0.95, 0.0), Visibility::default(), ChildOf(root))).id();
            let piece = |commands: &mut Commands, mesh: &Handle<Mesh>, mat: &Handle<StandardMaterial>, parent: Entity, tf: Transform| {
                commands.spawn((Mesh3d(mesh.clone()), MeshMaterial3d(mat.clone()), tf, ChildOf(parent)));
            };
            piece(commands, &parts.torso, &top, hips, Transform::from_xyz(0.0, 0.33, 0.0).with_scale(Vec3::new(1.3, 1.0, 0.8)));
            piece(commands, &parts.shorts, &shorts, hips, Transform::from_xyz(0.0, 0.02, 0.0));
            piece(commands, &parts.head, &skin, hips, Transform::from_xyz(0.0, 0.76, 0.0));
            piece(commands, &parts.neck, &skin, hips, Transform::from_xyz(0.0, 0.63, 0.0));
            piece(commands, &parts.collar, if gk { &top } else { &trim }, hips, Transform::from_xyz(0.0, 0.6, 0.0));
            // A handful of hairstyles: cropped, tall, long at the back, shaved, and a top knot.
            match (seed / 11) % 6 {
                0 | 1 => piece(commands, &parts.hair, &hair, hips, Transform::from_xyz(0.0, 0.8, 0.015).with_scale(Vec3::new(1.0, 0.7, 1.0))),
                2 => piece(commands, &parts.hair, &hair, hips, Transform::from_xyz(0.0, 0.84, 0.0).with_scale(Vec3::new(1.12, 0.95, 1.12))),
                3 => {
                    piece(commands, &parts.hair, &hair, hips, Transform::from_xyz(0.0, 0.8, 0.02).with_scale(Vec3::new(1.05, 0.75, 1.05)));
                    piece(commands, &parts.hair, &hair, hips, Transform::from_xyz(0.0, 0.7, 0.07).with_scale(Vec3::new(0.95, 1.0, 0.6)));
                }
                4 => {}
                _ => {
                    piece(commands, &parts.hair, &hair, hips, Transform::from_xyz(0.0, 0.8, 0.015).with_scale(Vec3::new(1.0, 0.65, 1.0)));
                    piece(commands, &parts.hand, &hair, hips, Transform::from_xyz(0.0, 0.92, 0.05).with_scale(Vec3::splat(1.1)));
                }
            }
            if (seed / 5) % 4 == 0 {
                piece(commands, &parts.beard, &hair, hips, Transform::from_xyz(0.0, 0.7, -0.04).with_scale(Vec3::new(1.05, 0.7, 0.8)));
            }
            piece(commands, &parts.number, &num_mat, hips, Transform::from_xyz(0.0, 0.38, 0.141));

            let limb = |commands: &mut Commands, at: Vec3| commands.spawn((Transform::from_translation(at), Visibility::default(), ChildOf(hips))).id();
            let arm_l = limb(commands, Vec3::new(-0.25, 0.55, 0.0));
            let arm_r = limb(commands, Vec3::new(0.25, 0.55, 0.0));
            for arm in [arm_l, arm_r] {
                piece(commands, &parts.sleeve, &sleeve, arm, Transform::from_xyz(0.0, -0.06, 0.0));
                piece(commands, &parts.arm, if gk { &top } else { &skin }, arm, Transform::from_xyz(0.0, -0.25, 0.0));
                piece(commands, &parts.hand, if gk { &gloves } else { &skin }, arm, Transform::from_xyz(0.0, -0.47, 0.0).with_scale(Vec3::splat(if gk { 1.35 } else { 1.0 })));
            }
            let leg_l = limb(commands, Vec3::new(-0.1, -0.06, 0.0));
            let leg_r = limb(commands, Vec3::new(0.1, -0.06, 0.0));
            for leg in [leg_l, leg_r] {
                piece(commands, &parts.thigh, &skin, leg, Transform::from_xyz(0.0, -0.22, 0.0));
                piece(commands, &parts.shin, &socks, leg, Transform::from_xyz(0.0, -0.6, 0.0));
                piece(commands, &parts.band, &trim, leg, Transform::from_xyz(0.0, -0.44, 0.0));
                piece(commands, &parts.boot, &boots[(seed / 3) % boots.len()], leg, Transform::from_xyz(0.0, -0.85, -0.05));
            }
            commands.entity(root).insert(Rig { idx: i, phase: (seed % 100) as f32 / 100.0, hips, leg_l, leg_r, arm_l, arm_r });
        }
    }

    // Ball.
    let ball_img = images.add(textures::ball());
    commands.spawn((
        Mesh3d(meshes.add(Sphere::new(BALL_R).mesh().uv(32, 18))),
        MeshMaterial3d(materials.add(StandardMaterial { base_color_texture: Some(ball_img), perceptual_roughness: 0.45, ..default() })),
        Transform::from_xyz(0.0, BALL_R, 0.0),
        BallModel { spin: Quat::IDENTITY },
        scoped.clone(),
    ));

    // Control indicators for up to two humans.
    for (h, c) in HUMAN_COLOURS.iter().enumerate() {
        let glow = materials.add(StandardMaterial { base_color: *c, emissive: LinearRgba::from(*c) * 2.0, unlit: true, ..default() });
        commands.spawn((
            Mesh3d(meshes.add(Annulus::new(0.62, 0.78).mesh().resolution(40))),
            MeshMaterial3d(glow.clone()),
            Transform::from_xyz(0.0, 0.02, 0.0).with_rotation(Quat::from_rotation_x(-FRAC_PI_2)),
            Visibility::Hidden,
            Indicator { human: h, arrow: false },
            NotShadowCaster,
            scoped.clone(),
        ));
        commands.spawn((
            Mesh3d(meshes.add(Cone::new(0.16, 0.32))),
            MeshMaterial3d(glow),
            Transform::from_xyz(0.0, 2.3, 0.0).with_rotation(Quat::from_rotation_x(PI)),
            Visibility::Hidden,
            Indicator { human: h, arrow: true },
            NotShadowCaster,
            scoped.clone(),
        ));
    }
}

fn yaw(facing: Vec2) -> Quat {
    // Models face -Z; simulation facing (x, y) maps to world (x, 0, y).
    Quat::from_rotation_y((-facing.x).atan2(-facing.y))
}

/// Joint angles for one frame of a player's animation.
struct Pose {
    hip_y: f32,
    body: Quat,
    leg_l: f32,
    leg_r: f32,
    arm_l: Quat,
    arm_r: Quat,
}

fn pose(p: &PlayerView, phase: f32, t: f32) -> Pose {
    let speed = p.vel.length();
    let amp = (speed / 8.0).min(1.0);
    let s = (phase * TAU).sin();
    let mut pose = Pose {
        hip_y: 0.95 + (phase * TAU * 2.0).cos().abs() * 0.05 * amp,
        body: Quat::from_rotation_x(-0.18 * amp),
        leg_l: s * 0.85 * amp,
        leg_r: -s * 0.85 * amp,
        arm_l: Quat::from_rotation_x(-s * 0.7 * amp) * Quat::from_rotation_z(-0.12),
        arm_r: Quat::from_rotation_x(s * 0.7 * amp) * Quat::from_rotation_z(0.12),
    };
    if speed < 0.3 {
        let breathe = (t * 2.2 + phase * 6.0).sin() * 0.03;
        pose.arm_l = Quat::from_rotation_z(-0.12 - breathe);
        pose.arm_r = Quat::from_rotation_z(0.12 + breathe);
    }
    if p.kick > 0.0 {
        let k = 1.0 - p.kick / 0.35;
        pose.leg_r = if k < 0.3 { -1.0 * k / 0.3 } else { -1.0 + 2.5 * ((k - 0.3) / 0.7).min(1.0) };
        pose.leg_l = 0.1;
        pose.arm_l = Quat::from_rotation_z(-0.7);
        pose.arm_r = Quat::from_rotation_z(0.5);
        pose.body = Quat::from_rotation_x(0.12);
    }
    if p.slide > 0.0 {
        pose.hip_y = 0.32;
        pose.body = Quat::from_rotation_x(1.15) * Quat::from_rotation_z(0.3);
        pose.leg_l = 0.4;
        pose.leg_r = 1.1;
        pose.arm_l = Quat::from_rotation_z(-1.0);
        pose.arm_r = Quat::from_rotation_z(0.6);
    }
    if p.dive > 0.0 {
        // Sideways relative to facing.
        let right = Vec2::new(-p.facing.y, p.facing.x);
        let side = p.vel.dot(right).signum();
        let air = (p.dive / 0.65).min(1.0);
        pose.hip_y = 0.45 + air * 0.5;
        pose.body = Quat::from_rotation_z(-side * 1.35);
        pose.arm_l = Quat::from_rotation_z(-2.9);
        pose.arm_r = Quat::from_rotation_z(2.9);
        pose.leg_l = 0.2;
        pose.leg_r = -0.2;
    }
    if p.holding {
        pose.arm_l = Quat::from_rotation_x(1.2);
        pose.arm_r = Quat::from_rotation_x(1.2);
    }
    if p.celebrating {
        let jump = (t * 7.0 + phase * 5.0).sin().abs();
        pose.hip_y += jump * 0.25;
        pose.arm_l = Quat::from_rotation_z(-2.6 - jump * 0.3);
        pose.arm_r = Quat::from_rotation_z(2.6 + jump * 0.3);
    }
    if p.stun > 0.0 && p.slide <= 0.0 && p.dive <= 0.0 {
        pose.body = Quat::from_rotation_z((t * 12.0).sin() * 0.12) * Quat::from_rotation_x(-0.3);
    }
    pose
}

pub fn animate_players(
    view: Res<View>,
    time: Res<Time>,
    mut rigs: Query<(&mut Rig, &mut Transform, &mut Visibility)>,
    mut parts: Query<&mut Transform, Without<Rig>>,
) {
    let dt = time.delta_secs();
    let t = time.elapsed_secs();
    for (mut rig, mut tf, mut vis) in &mut rigs {
        let Some(p) = view.players.get(rig.idx) else { continue };
        *vis = if p.sent_off { Visibility::Hidden } else { Visibility::Inherited };
        let speed = p.vel.length();
        // About one stride cycle per 1.9 m.
        rig.phase = (rig.phase + speed * dt / 1.9).fract();
        tf.translation = sim_to_world(p.pos, 0.0);
        let target = yaw(p.facing);
        tf.rotation = tf.rotation.slerp(target, (dt * 18.0).min(1.0));

        let pose = pose(p, rig.phase, t);
        if let Ok(mut h) = parts.get_mut(rig.hips) {
            h.translation.y = pose.hip_y;
            h.rotation = pose.body;
        }
        for (e, a) in [(rig.leg_l, pose.leg_l), (rig.leg_r, pose.leg_r)] {
            if let Ok(mut l) = parts.get_mut(e) {
                l.rotation = Quat::from_rotation_x(a);
            }
        }
        for (e, q) in [(rig.arm_l, pose.arm_l), (rig.arm_r, pose.arm_r)] {
            if let Ok(mut a) = parts.get_mut(e) {
                a.rotation = q;
            }
        }
    }
}

pub fn animate_ball(view: Res<View>, time: Res<Time>, mut q: Query<(&mut BallModel, &mut Transform)>) {
    let dt = time.delta_secs();
    for (mut b, mut tf) in &mut q {
        let v = view.ball.vel;
        if v.length() > 0.05 {
            // Roll about the axis perpendicular to travel.
            let axis = Vec3::new(v.y, 0.0, -v.x).normalize();
            b.spin = Quat::from_axis_angle(axis, -v.length() * dt / BALL_R) * b.spin;
        }
        tf.translation = sim_to_world(view.ball.pos, view.ball.z + BALL_R);
        tf.rotation = b.spin;
    }
}

pub fn place_indicators(view: Res<View>, time: Res<Time>, mut q: Query<(&Indicator, &mut Transform, &mut Visibility)>) {
    for (ind, mut tf, mut vis) in &mut q {
        match view.controlled.get(ind.human).copied().flatten() {
            Some(i) => {
                let p = view.players[i].pos;
                let bob = (time.elapsed_secs() * 4.0).sin() * 0.08;
                tf.translation = if ind.arrow { sim_to_world(p, 2.35 + bob) } else { sim_to_world(p, 0.02) };
                *vis = Visibility::Visible;
            }
            None => *vis = Visibility::Hidden,
        }
    }
}
