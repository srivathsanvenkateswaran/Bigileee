//! Player, official and ball models, and their animation.
//!
//! Players and officials are procedural skinned figures (see `humanoid`), dressed per person
//! from a small texture atlas, with name and number printed on the back.

use super::humanoid::{self, Build, FigureParts, Look, Motion, JOINTS};
use super::stadium::sim_to_world;
use super::textures::{self, color};
use super::view::{PlayerView, View};
use super::{AppState, MatchRes};
use crate::data::types::Crest;
use crate::sim::consts::*;
use crate::sim::game::{Match, Role};
use bevy::pbr::NotShadowCaster;
use bevy::prelude::*;
use std::f32::consts::{FRAC_PI_2, PI};

pub const BALL_R: f32 = 0.13;

#[derive(Component)]
pub struct Rig {
    pub idx: usize,
    /// Squad card the figure was dressed for; a substitution changes it and re-dresses.
    card: usize,
    left_foot: bool,
    phase: f32,
    joints: [Entity; JOINTS],
    /// Smoothed yaw rate, for leaning into turns.
    turn: f32,
    last_yaw: f32,
    had_ball: bool,
    /// Seconds since the last first touch / header, if one is playing.
    touch: Option<f32>,
    header: Option<f32>,
}

/// Referee (0) and the two assistants (1 near touchline / right half, 2 far touchline / left half).
#[derive(Component)]
pub struct Official {
    kind: usize,
    pos: Vec2,
    vel: Vec2,
    facing: Vec2,
    phase: f32,
    joints: [Entity; JOINTS],
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

fn lum(c: u32) -> f32 {
    let [r, g, b] = textures::rgb(c);
    r as f32 * 0.3 + g as f32 * 0.59 + b as f32 * 0.11
}

const SKINS: [u32; 6] = [0xF1C7A6, 0xE0AC85, 0xC68A5E, 0x9C6640, 0x7A4B2C, 0x5A3620];
const HAIRS: [u32; 6] = [0x17110D, 0x2E1E12, 0x5A3A1E, 0x8C6239, 0xC9A45C, 0x2A2A2A];
const BOOTS: [u32; 8] = [0x141418, 0x141418, 0xF4F4F4, 0xFF5A1F, 0x23C4F0, 0xE8FF3A, 0xE0218A, 0x7B2CFF];

/// Shared meshes for all figures in a match.
/// Invented shirt sponsors; each club gets one, chosen from its name.
const SPONSORS: [&str; 14] = [
    "SAMBAR ENERGY",
    "IDLI TELECOM",
    "PANEER PAY",
    "VADA PAV AIR",
    "PAROTTA BANK",
    "FILTER KAAPI",
    "MASALA MOTORS",
    "CHUTNEY CLOUD",
    "GHEE INSURANCE",
    "RASAM RAIL",
    "PAPPAD PHONES",
    "LASSI LABS",
    "JALEBI JETS",
    "KULFI COOLING",
];

#[derive(Resource)]
pub struct Kit {
    sponsors: [Handle<StandardMaterial>; 2],
    sponsor: Handle<Mesh>,
    bodies: Vec<(Build, Handle<Mesh>)>,
    bindposes: Handle<bevy::render::mesh::skinning::SkinnedMeshInverseBindposes>,
    back: Handle<Mesh>,
    beard: Handle<Mesh>,
    hair: Vec<Vec<(Handle<Mesh>, Transform)>>,
}

impl Kit {
    fn body(&self, bulk: usize, keeper: bool) -> Handle<Mesh> {
        self.bodies[bulk * 2 + keeper as usize].1.clone()
    }
}

#[allow(clippy::too_many_arguments)]
fn dress(
    commands: &mut Commands,
    materials: &mut Assets<StandardMaterial>,
    images: &mut Assets<Image>,
    kit: &Kit,
    look: Look,
    seed: usize,
    keeper: bool,
    back_print: Option<(&str, u8, [u8; 4])>,
    sponsor: Option<Handle<StandardMaterial>>,
    bio: Option<crate::data::bio::Bio>,
    at: Vec2,
) -> humanoid::Figure {
    let atlas = images.add(humanoid::atlas(&look));
    let material = materials.add(StandardMaterial { base_color_texture: Some(atlas), perceptual_roughness: 0.8, reflectance: 0.3, ..default() });
    let hair_material = materials.add(StandardMaterial { base_color: color(look.hair), perceptual_roughness: 0.9, ..default() });
    let back = back_print.map(|(name, num, fg)| {
        let img = humanoid::back_print(name, num, fg);
        let mat = materials.add(StandardMaterial {
            base_color_texture: Some(images.add(img)),
            alpha_mode: AlphaMode::Mask(0.5),
            perceptual_roughness: 0.8,
            ..default()
        });
        (kit.back.clone(), mat)
    });
    let beard = ((seed / 5) % 4 == 0).then(|| (kit.beard.clone(), materials.add(StandardMaterial { base_color: color(look.hair), perceptual_roughness: 0.95, ..default() })));
    // Real heights where known; the model's default stance is about 1.8 m.
    let height = match bio {
        Some(b) => (b.height_cm as f32 / 180.0).clamp(0.86, 1.14),
        None => 0.95 + (seed % 9) as f32 * 0.011 + if keeper { 0.04 } else { 0.0 },
    };
    // Build from body-mass index: slight, average or stocky.
    let bulk = match bio {
        Some(b) => {
            let bmi = b.weight_kg as f32 / (b.height_cm as f32 / 100.0).powi(2);
            if bmi < 21.6 { 0 } else if bmi < 23.6 { 1 } else { 2 }
        }
        None => seed % 3,
    };
    humanoid::spawn_figure(
        commands,
        Transform::from_translation(sim_to_world(at, 0.0)).with_scale(Vec3::splat(height)),
        FigureParts {
            body: kit.body(bulk, keeper),
            bindposes: kit.bindposes.clone(),
            material,
            hair: &kit.hair[(seed / 11) % kit.hair.len()],
            hair_material,
            beard,
            back,
            front: sponsor.map(|m| (kit.sponsor.clone(), m)),
        },
        StateScoped(AppState::Match),
    )
}

pub fn spawn_players(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    images: &mut Assets<Image>,
    bindposes: &mut Assets<bevy::render::mesh::skinning::SkinnedMeshInverseBindposes>,
    m: &Match,
) {
    let scoped = StateScoped(AppState::Match);
    let mut bodies = Vec::new();
    for bulk in [0.93f32, 1.0, 1.08] {
        for keeper in [false, true] {
            let b = Build { bulk, long_sleeves: keeper, gloves: keeper };
            bodies.push((b, meshes.add(humanoid::body_mesh(b))));
        }
    }
    let sponsors = [0, 1].map(|t| {
        let team = &m.teams[t];
        let pick = team.name.bytes().fold(0usize, |a, b| a.wrapping_mul(17).wrapping_add(b as usize)) % SPONSORS.len();
        let img = images.add(super::font5x7::text_image(SPONSORS[pick], print_fg(m.kit[t]), [0, 0, 0, 0], 3, 1));
        materials.add(StandardMaterial { base_color_texture: Some(img), alpha_mode: AlphaMode::Mask(0.5), perceptual_roughness: 0.8, ..default() })
    });
    let kit = Kit {
        sponsors,
        bodies,
        bindposes: bindposes.add(humanoid::inverse_bindposes()),
        back: meshes.add(Plane3d::new(Vec3::Z, Vec2::new(0.12, 0.13))),
        sponsor: meshes.add(Plane3d::new(Vec3::Z, Vec2::new(0.13, 0.03))),
        beard: meshes.add(Sphere::new(0.085).mesh().uv(16, 10)),
        hair: (0..6).map(|s| humanoid::hair_meshes(s, meshes)).collect(),
    };

    for i in 0..PLAYERS {
        spawn_player(commands, materials, images, &kit, m, i);
    }

    // Officials in black, the assistants with flags.
    let flag_mesh = meshes.add(Cuboid::new(0.02, 0.32, 0.4));
    let pole_mesh = meshes.add(Cylinder::new(0.012, 0.5));
    let flag_mat = materials.add(StandardMaterial { base_color: Color::srgb(1.0, 0.85, 0.0), double_sided: true, cull_mode: None, ..default() });
    let pole_mat = materials.add(StandardMaterial { base_color: Color::srgb(0.1, 0.1, 0.1), ..default() });
    for kind in 0..3 {
        let seed = 31 + kind * 17;
        let look = Look {
            crest: Crest::Shield,
            shirt: 0x161618,
            accent: 0x161618,
            sleeve: 0x161618,
            shorts: 0x161618,
            socks: 0x161618,
            trim: 0xE8FF3A,
            skin: SKINS[(seed * 7) % SKINS.len()],
            hair: HAIRS[seed % HAIRS.len()],
            boots: 0x141418,
            gloves: 0xE8FF3A,
        };
        let at = match kind {
            0 => Vec2::new(-6.0, 8.0),
            1 => Vec2::new(10.0, HALF_W + 1.0),
            _ => Vec2::new(-10.0, -HALF_W - 1.0),
        };
        let fig = dress(commands, materials, images, &kit, look, seed, false, None, None, None, at);
        if kind > 0 {
            let hand = fig.joints[humanoid::FOREARM_R];
            commands.spawn((Mesh3d(pole_mesh.clone()), MeshMaterial3d(pole_mat.clone()), Transform::from_xyz(0.0, -0.36, -0.12).with_rotation(Quat::from_rotation_x(FRAC_PI_2 * 0.9)), ChildOf(hand)));
            commands.spawn((Mesh3d(flag_mesh.clone()), MeshMaterial3d(flag_mat.clone()), Transform::from_xyz(0.0, -0.3, -0.45), ChildOf(hand)));
        }
        commands.entity(fig.root).insert(Official { kind, pos: at, vel: Vec2::ZERO, facing: Vec2::new(0.0, -1.0), phase: 0.0, joints: fig.joints });
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
    commands.insert_resource(kit);
}

fn print_fg(bg: u32) -> [u8; 4] {
    if lum(bg) > 140.0 { [20, 20, 28, 255] } else { [250, 250, 250, 255] }
}

/// Dresses and spawns player `i` as they are now (used at kick-off and after substitutions).
fn spawn_player(commands: &mut Commands, materials: &mut Assets<StandardMaterial>, images: &mut Assets<Image>, kit: &Kit, m: &Match, i: usize) {
    let p = &m.players[i];
    let t = p.team;
    let team = &m.teams[t];
    let shirt_c = m.kit[t];
    let trim_c = if shirt_c == team.home { team.trim } else { team.home };
    let shorts_c = if (lum(shirt_c) - lum(trim_c)).abs() > 60.0 { trim_c } else if lum(shirt_c) > 128.0 { 0x15151A } else { 0xF2F2F2 };
    let keeper = p.role == Role::Gk;
    let bio = team.squad.get(p.card).and_then(|c| c.bio);
    let seed = p.name.bytes().fold(7usize, |a, b| a.wrapping_mul(31).wrapping_add(b as usize));
    // Some veterans have gone grey.
    let hair = if bio.is_some_and(|b| b.age >= 33) && seed % 2 == 0 { 0x8E8B86 } else { HAIRS[(seed / 7) % HAIRS.len()] };
    let (shirt, accent, sleeve, shorts, socks, trim, crest) = if keeper {
        (team.keeper, team.keeper, team.keeper, team.keeper, team.keeper, 0x111111, Crest::Shield)
    } else {
        (shirt_c, trim_c, shirt_c, shorts_c, shirt_c, trim_c, team.crest)
    };
    let look = Look {
        crest,
        shirt,
        accent,
        sleeve,
        shorts,
        socks,
        trim,
        skin: SKINS[seed % SKINS.len()],
        hair,
        boots: BOOTS[(seed / 3) % BOOTS.len()],
        gloves: 0xE8FF3A,
    };
    let fg = print_fg(if keeper { team.keeper } else { shirt_c });
    let sponsor = (!keeper).then(|| kit.sponsors[t].clone());
    let fig = dress(commands, materials, images, kit, look, seed, keeper, Some((&p.name, p.num, fg)), sponsor, bio, p.pos);
    let left_foot = bio.is_some_and(|b| b.left_foot);
    commands.entity(fig.root).insert(Rig {
        idx: i,
        card: p.card,
        left_foot,
        phase: (seed % 100) as f32 / 100.0,
        joints: fig.joints,
        turn: 0.0,
        last_yaw: 0.0,
        had_ball: false,
        touch: None,
        header: None,
    });
}

/// Rebuilds a figure when a substitution changes who is in that position.
pub fn redress_substitutes(
    mut commands: Commands,
    res: Option<Res<MatchRes>>,
    kit: Option<Res<Kit>>,
    rigs: Query<(Entity, &Rig)>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    let (Some(res), Some(kit)) = (res, kit) else { return };
    for (e, rig) in &rigs {
        if res.m.players[rig.idx].card != rig.card {
            commands.entity(e).despawn();
            spawn_player(&mut commands, &mut materials, &mut images, &kit, &res.m, rig.idx);
        }
    }
}

fn yaw(facing: Vec2) -> Quat {
    // Models face -Z; simulation facing (x, y) maps to world (x, 0, y).
    Quat::from_rotation_y((-facing.x).atan2(-facing.y))
}

/// Head turn towards `target`, in the body's frame, limited to a comfortable range.
fn look_at(rot: Quat, from: Vec2, target: Vec2) -> f32 {
    let d = target - from;
    if d.length() < 0.5 {
        return 0.0;
    }
    let local = rot.inverse() * Vec3::new(d.x, 0.0, d.y);
    (-local.x).atan2(-local.z).clamp(-1.0, 1.0)
}

fn motion(p: &PlayerView, phase: f32, t: f32, look: f32) -> Motion {
    let dive = (p.dive > 0.0).then(|| {
        let right = Vec2::new(-p.facing.y, p.facing.x);
        (if p.vel.dot(right) >= 0.0 { 1.0 } else { -1.0 }, p.dive / 0.65)
    });
    Motion {
        speed: p.vel.length(),
        phase,
        kick: (p.kick > 0.0).then(|| 1.0 - p.kick / 0.35),
        sliding: p.slide > 0.0,
        dive,
        holding: p.holding,
        celebrating: p.celebrating,
        stunned: p.stun > 0.0,
        look: if p.celebrating { 0.0 } else { look },
        time: t,
        flag_up: false,
        routine: None,
        left_footed: false,
        ..default()
    }
}

pub fn animate_players(
    view: Res<View>,
    time: Res<Time>,
    mut rigs: Query<(&mut Rig, &mut Transform, &mut Visibility)>,
    mut joints: Query<&mut Transform, (Without<Rig>, Without<Official>)>,
) {
    let dt = time.delta_secs();
    let t = time.elapsed_secs();
    for (mut rig, mut tf, mut vis) in &mut rigs {
        let Some(p) = view.players.get(rig.idx) else { continue };
        *vis = if p.sent_off { Visibility::Hidden } else { Visibility::Inherited };
        let speed = p.vel.length();
        // Stride length grows with speed: about 1.6 m jogging, 2.3 m sprinting.
        rig.phase = (rig.phase + speed * dt / (1.5 + speed * 0.1)).fract();
        tf.translation = sim_to_world(p.pos, 0.0);
        let target = yaw(p.facing);
        tf.rotation = tf.rotation.slerp(target, (dt * 16.0).min(1.0));
        let look = look_at(tf.rotation, p.pos, view.ball.pos);

        // Turning: smoothed yaw rate drives a lean into the turn.
        let yaw_now = (-p.facing.x).atan2(-p.facing.y);
        let mut d = yaw_now - rig.last_yaw;
        if d > PI {
            d -= 2.0 * PI;
        } else if d < -PI {
            d += 2.0 * PI;
        }
        rig.last_yaw = yaw_now;
        let rate = if dt > 0.0 { (d / dt).clamp(-6.0, 6.0) } else { 0.0 };
        rig.turn += (rate - rig.turn) * (dt * 8.0).min(1.0);

        // A first touch cushions the ball as it arrives; a kick with the ball up high is a header.
        if p.has_ball && !rig.had_ball {
            rig.touch = Some(0.0);
        }
        rig.had_ball = p.has_ball;
        if p.kick > 0.3 && rig.header.is_none() && view.ball.z > 0.9 && view.ball.pos.distance(p.pos) < 2.5 {
            rig.header = Some(0.0);
        }
        let r = &mut *rig;
        for timer in [&mut r.touch, &mut r.header] {
            if let Some(s) = timer {
                *s += dt;
                if *s > 0.45 {
                    *timer = None;
                }
            }
        }

        let mut mo = motion(p, rig.phase, t + rig.idx as f32, look);
        mo.left_footed = rig.left_foot;
        mo.turn = rig.turn / 6.0;
        mo.dribbling = p.has_ball;
        mo.first_touch = rig.touch.map(|s| s / 0.3);
        mo.header = rig.header.map(|s| s / 0.45);
        mo.throw_in = p.throw_in;
        mo.keeper_ready = p.keeper && !p.holding && speed < 2.5 && view.ball.pos.distance(p.pos) < 30.0;
        if let Some((_, kind, ct)) = view.celebration.filter(|c| c.0 == rig.idx) {
            mo.routine = Some((kind, ct));
            if kind == 4 && ct > 1.2 {
                tf.rotation = Quat::from_rotation_y((ct - 1.2) * 9.0);
            }
        }
        let pose = humanoid::pose(mo);
        humanoid::apply(&pose, &rig.joints, &mut joints);
    }
}

/// The referee shadows play from a diagonal; assistants track the play along their touchlines.
pub fn animate_officials(
    view: Res<View>,
    res: Option<Res<MatchRes>>,
    time: Res<Time>,
    mut officials: Query<(&mut Official, &mut Transform), Without<Rig>>,
    mut joints: Query<&mut Transform, (Without<Rig>, Without<Official>)>,
) {
    let dt = time.delta_secs().max(1e-4);
    let t = time.elapsed_secs();
    let ball = view.ball.pos;
    let offside = res.as_ref().is_some_and(|r| r.m.banner.as_ref().is_some_and(|b| b.0.starts_with("OFFSIDE")));
    for (mut o, mut tf) in &mut officials {
        let target = match o.kind {
            0 => {
                let side = if ball.y > 0.0 { -1.0 } else { 1.0 };
                let v = Vec2::new(ball.x - ball.x.signum() * 7.0, ball.y + side * 9.0);
                Vec2::new(v.x.clamp(-HALF_L + 8.0, HALF_L - 8.0), v.y.clamp(-HALF_W + 3.0, HALF_W - 3.0))
            }
            1 => Vec2::new(ball.x.clamp(2.0, HALF_L - 1.0), HALF_W + 1.2),
            _ => Vec2::new(ball.x.clamp(-HALF_L + 1.0, -2.0), -HALF_W - 1.2),
        };
        let to = target - o.pos;
        let max = if o.kind == 0 { 7.0 } else { 6.5 };
        let want = to.clamp_length_max(max) * (to.length() / 3.0).min(1.0);
        o.vel = o.vel.lerp(want, (dt * 4.0).min(1.0));
        let step = o.vel * dt;
        o.pos += step;
        // Assistants side-step along the line facing the pitch; the referee faces the ball.
        let face = if o.kind == 0 { (ball - o.pos).normalize_or(o.facing) } else { Vec2::new(0.0, -o.pos.y.signum()) };
        o.facing = o.facing.lerp(face, (dt * 6.0).min(1.0)).normalize_or(face);
        let speed = o.vel.length();
        o.phase = (o.phase + speed * dt / 1.6).fract();
        tf.translation = sim_to_world(o.pos, 0.0);
        tf.rotation = yaw(o.facing);
        let look = look_at(tf.rotation, o.pos, ball);
        let mut m = Motion { speed, phase: o.phase, look, time: t + o.kind as f32 * 3.0, ..default() };
        m.flag_up = o.kind > 0 && offside;
        let pose = humanoid::pose(m);
        humanoid::apply(&pose, &o.joints, &mut joints);
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
