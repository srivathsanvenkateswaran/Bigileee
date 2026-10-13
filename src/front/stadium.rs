//! The static 3D scene: pitch, two-tier bowl with seats and an animated crowd, roofs, goals,
//! LED boards, big screen, dugouts, tunnel, corner flags and lighting.
//!
//! Simulation space is (x along the pitch, y across it, z up). World space is Bevy's
//! y-up, so a simulation point (x, y, z) sits at world (x, z, y).
//!
//! The bowl is one continuous "path" running round the pitch (four straight stands joined by
//! quarter-circle corners). Every stand element (steps, seats, fascia LED ribbon, roof) is a
//! 2D profile in (depth behind the stand front, height) swept along that path, so the corners
//! fill themselves in and everything lines up.

use super::textures::{self, hash2};
use super::{AppState, MatchRes};
use crate::sim::consts::*;
use crate::sim::game::{Match, Phase};
use bevy::asset::RenderAssetUsages;
use bevy::core_pipeline::experimental::taa::TemporalAntiAliasPlugin;
use bevy::pbr::{CascadeShadowConfigBuilder, DirectionalLightShadowMap, NotShadowCaster};
use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};
use bevy::render::view::RenderLayers;
use std::f32::consts::FRAC_PI_2;

pub fn sim_to_world(p: Vec2, z: f32) -> Vec3 {
    Vec3::new(p.x, z, p.y)
}

/// Lighting preset for the match scene. `spawn_stadium` reads it when the match starts.
#[derive(Resource, Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum TimeOfDay {
    /// Afternoon sun with a procedural sky and the main stand's roof shadow across the pitch.
    #[default]
    Day,
    /// Floodlit evening match.
    Night,
}

/// Stadium-side systems (crowd, LED boards, big screen) plus the TAA plugin the camera needs.
pub struct StadiumPlugin;

impl Plugin for StadiumPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<TemporalAntiAliasPlugin>() {
            app.add_plugins(TemporalAntiAliasPlugin);
        }
        app.init_resource::<TimeOfDay>().add_systems(
            Update,
            (animate_crowd, scroll_boards, update_screen).run_if(in_state(AppState::Match).and(resource_exists::<MatchRes>)),
        );
    }
}

#[derive(Component)]
pub struct Stadium;

// ---------------------------------------------------------------------------------------------
// Mesh building

/// Accumulates quads (optionally vertex-coloured) into one mesh.
#[derive(Default)]
struct Geo {
    pos: Vec<[f32; 3]>,
    nor: Vec<[f32; 3]>,
    uv: Vec<[f32; 2]>,
    col: Vec<[f32; 4]>,
    idx: Vec<u32>,
}

const WHITE: [f32; 4] = [1.0; 4];

impl Geo {
    /// A quad from four corners in order round its edge, wound so it faces `normal`.
    /// Degenerate corners are fine (the quad becomes a triangle).
    fn quad(&mut self, c: [Vec3; 4], normal: Vec3, uv: [Vec2; 4], col: [f32; 4]) {
        let base = self.pos.len() as u32;
        for k in 0..4 {
            self.pos.push(c[k].to_array());
            self.nor.push(normal.to_array());
            self.uv.push(uv[k].to_array());
            self.col.push(col);
        }
        // Newell's method gives the polygon normal even when two corners coincide.
        let mut face = Vec3::ZERO;
        for k in 0..4 {
            let (a, b) = (c[k], c[(k + 1) % 4]);
            face += Vec3::new((a.y - b.y) * (a.z + b.z), (a.z - b.z) * (a.x + b.x), (a.x - b.x) * (a.y + b.y));
        }
        if face.dot(normal) >= 0.0 {
            self.idx.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
        } else {
            self.idx.extend_from_slice(&[base, base + 2, base + 1, base, base + 3, base + 2]);
        }
    }

    /// An axis-aligned box. `skip_back_bottom` drops the -z and -y faces (never seen on fans).
    fn cuboid(&mut self, centre: Vec3, half: Vec3, col: [f32; 4], skip_back_bottom: bool) {
        for axis in 0..3 {
            for sign in [-1.0f32, 1.0] {
                if skip_back_bottom && sign < 0.0 && axis != 0 {
                    continue;
                }
                let mut n = Vec3::ZERO;
                n[axis] = sign;
                let (ua, va) = ((axis + 1) % 3, (axis + 2) % 3);
                let mut u = Vec3::ZERO;
                u[ua] = half[ua];
                let mut v = Vec3::ZERO;
                v[va] = half[va];
                let f = centre + n * half[axis];
                let uv = [Vec2::new(0.0, 1.0), Vec2::new(1.0, 1.0), Vec2::new(1.0, 0.0), Vec2::new(0.0, 0.0)];
                self.quad([f - u - v, f + u - v, f + u + v, f - u + v], n, uv, col);
            }
        }
    }

    fn build(self) -> Mesh {
        let coloured = self.col.iter().any(|c| *c != WHITE);
        let mut m = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
            .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, self.pos)
            .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, self.nor)
            .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, self.uv)
            .with_inserted_indices(Indices::U32(self.idx));
        if coloured {
            m.insert_attribute(Mesh::ATTRIBUTE_COLOR, self.col);
        }
        m
    }
}

/// A point on the stand path: where the stand front is (world x, z) and which way is "back".
#[derive(Clone, Copy)]
struct PathPt {
    base: Vec2,
    out: Vec2,
}

impl PathPt {
    fn at(&self, d: f32, h: f32) -> Vec3 {
        let p = self.base + self.out * d;
        Vec3::new(p.x, h, p.y)
    }
}

/// Which part of the bowl a path section belongs to.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Part {
    Far,
    Near,
    End,
    /// A corner; `true` when it touches the camera-side (near) stand.
    Corner(bool),
}

/// Distance from the touchlines / goal lines to the front of the stands.
const STAND_GAP: f32 = 9.0;
const CORNER_SEGS: usize = 12;

/// The bowl path, in order: far side (moving +x), corner, right end, corner, near side, corner,
/// left end, corner. Moving along the path always runs left-to-right as seen from the pitch,
/// so text swept along it reads correctly.
fn bowl_sections() -> Vec<(Part, Vec<PathPt>)> {
    let (fx, fz) = (HALF_L + STAND_GAP, HALF_W + STAND_GAP);
    let corner = |c: Vec2, from: Vec2, to: Vec2| -> Vec<PathPt> {
        let a = from.angle_to(to);
        (0..=CORNER_SEGS).map(|k| PathPt { base: c, out: Vec2::from_angle(a * k as f32 / CORNER_SEGS as f32).rotate(from) }).collect()
    };
    let straight = |a: Vec2, b: Vec2, out: Vec2| vec![PathPt { base: a, out }, PathPt { base: b, out }];
    let (n, s, e, w) = (Vec2::new(0.0, 1.0), Vec2::new(0.0, -1.0), Vec2::new(1.0, 0.0), Vec2::new(-1.0, 0.0));
    vec![
        (Part::Far, straight(Vec2::new(-fx, -fz), Vec2::new(fx, -fz), s)),
        (Part::Corner(false), corner(Vec2::new(fx, -fz), s, e)),
        (Part::End, straight(Vec2::new(fx, -fz), Vec2::new(fx, fz), e)),
        (Part::Corner(true), corner(Vec2::new(fx, fz), e, n)),
        (Part::Near, straight(Vec2::new(fx, fz), Vec2::new(-fx, fz), n)),
        (Part::Corner(true), corner(Vec2::new(-fx, fz), n, w)),
        (Part::End, straight(Vec2::new(-fx, fz), Vec2::new(-fx, -fz), w)),
        (Part::Corner(false), corner(Vec2::new(-fx, -fz), w, s)),
    ]
}

/// Sweeps a (depth, height) profile polyline along a path. Faces point to the left of the
/// direction of travel along the profile (so a profile climbing away from the pitch faces the
/// pitch and the sky). `uv_m` is metres per texture repeat; `flip_v` runs v from the profile end.
fn sweep(g: &mut Geo, path: &[PathPt], profile: &[Vec2], uv_m: Vec2, flip_v: bool, col: [f32; 4]) {
    if path.len() < 2 || profile.len() < 2 {
        return;
    }
    let mut prof_len = vec![0.0f32; profile.len()];
    for j in 1..profile.len() {
        prof_len[j] = prof_len[j - 1] + (profile[j] - profile[j - 1]).length();
    }
    let total = prof_len[profile.len() - 1];
    // Distance travelled along the path at each profile depth, so u is continuous in corners.
    let along: Vec<Vec<f32>> = profile
        .iter()
        .map(|q| {
            let mut acc = vec![0.0f32; path.len()];
            for i in 1..path.len() {
                acc[i] = acc[i - 1] + (path[i].at(q.x, q.y) - path[i - 1].at(q.x, q.y)).length();
            }
            acc
        })
        .collect();
    let v = |j: usize| if flip_v { (total - prof_len[j]) / uv_m.y } else { prof_len[j] / uv_m.y };
    for i in 0..path.len() - 1 {
        let o = (path[i].out + path[i + 1].out).normalize_or_zero();
        for j in 0..profile.len() - 1 {
            let (a, b) = (profile[j], profile[j + 1]);
            let c = [path[i].at(a.x, a.y), path[i + 1].at(a.x, a.y), path[i + 1].at(b.x, b.y), path[i].at(b.x, b.y)];
            let area = (c[2] - c[0]).cross(c[3] - c[1]).length();
            if area < 1e-4 {
                continue;
            }
            let d = b - a;
            let n = (Vec3::new(o.x, 0.0, o.y) * -d.y + Vec3::Y * d.x).normalize_or(Vec3::Y);
            let uv = [
                Vec2::new(along[j][i] / uv_m.x, v(j)),
                Vec2::new(along[j][i + 1] / uv_m.x, v(j)),
                Vec2::new(along[j + 1][i + 1] / uv_m.x, v(j + 1)),
                Vec2::new(along[j + 1][i] / uv_m.x, v(j + 1)),
            ];
            g.quad(c, n, uv, col);
        }
    }
}

/// One row of seats: depth of the tread's front edge and its height.
#[derive(Clone, Copy)]
struct Row {
    d: f32,
    h: f32,
    tread: f32,
    lower: bool,
    index: usize,
}

const LOWER_ROWS: usize = 15;
const UPPER_ROWS: usize = 13;
/// Depth and height range of the fascia LED ribbon between the tiers.
const FASCIA_D: f32 = 14.2;
const BACK_D: f32 = LOWER_ROWS as f32 * 0.85 + 1.5 + UPPER_ROWS as f32 * 0.8;
const ROOF_FRONT: Vec2 = Vec2::new(5.0, 23.0);
const ROOF_BACK: Vec2 = Vec2::new(BACK_D + 1.8, 26.0);
const ROOF_THICK: f32 = 0.7;

/// The stepped two-tier bowl profile and its seat rows.
fn bowl_profile() -> (Vec<Vec2>, Vec<Row>) {
    let mut p = vec![Vec2::new(0.0, 0.0), Vec2::new(0.0, 1.2)];
    let mut rows = Vec::new();
    let (mut d, mut h) = (0.0f32, 1.2f32);
    for index in 0..LOWER_ROWS {
        rows.push(Row { d, h, tread: 0.85, lower: true, index });
        d += 0.85;
        p.push(Vec2::new(d, h));
        h += 0.4;
        p.push(Vec2::new(d, h));
    }
    // Concourse walkway, then the fascia wall that carries the LED ribbon.
    d += 1.5;
    p.push(Vec2::new(d, h));
    h = 10.0;
    p.push(Vec2::new(d, h));
    for index in 0..UPPER_ROWS {
        rows.push(Row { d, h, tread: 0.8, lower: false, index });
        d += 0.8;
        p.push(Vec2::new(d, h));
        h += 0.6;
        p.push(Vec2::new(d, h));
    }
    // Back wall up into the roof.
    p.push(Vec2::new(d, ROOF_BACK.y + 0.1));
    (p, rows)
}

// ---------------------------------------------------------------------------------------------
// Crowd

/// One block of four seated fans, animated as a unit.
#[derive(Component)]
struct Fan {
    base: Vec3,
    rot: Quat,
    phase: f32,
    team: u8,
    variant: u16,
}

/// Fan block meshes: `sit[k]` and `cheer[k]` share colours, so swapping keeps each fan's shirt.
#[derive(Resource)]
struct CrowdAssets {
    sit: Vec<Handle<Mesh>>,
    cheer: Vec<Handle<Mesh>>,
}

#[derive(Resource, Default)]
struct CrowdState {
    score: [u32; 2],
    cheering: Option<u8>,
}

const FANS_PER_BLOCK: usize = 4;
const SEAT_W: f32 = 0.55;
const VARIANTS_PER_TEAM: u32 = 12;

fn lin(c: [u8; 3]) -> [f32; 4] {
    let l = Color::srgb_u8(c[0], c[1], c[2]).to_linear();
    [l.red, l.green, l.blue, 1.0]
}

fn shade(c: [f32; 4], k: f32) -> [f32; 4] {
    [c[0] * k, c[1] * k, c[2] * k, 1.0]
}

/// A block of four fans. Local +z faces the pitch, y = 0 is the tread they sit on.
/// Colours are drawn from `seed`, so the sitting and cheering versions match.
fn fan_block(seed: u32, kit: [f32; 4], trim: [f32; 4], rival: [f32; 4], cheering: bool) -> Mesh {
    let mut g = Geo::default();
    let neutral = [[235, 235, 235], [28, 28, 32], [70, 72, 80], [40, 50, 90], [110, 30, 40], [150, 140, 120], [60, 90, 140], [200, 200, 205]].map(lin);
    let skins = [[232, 190, 160], [205, 150, 110], [160, 110, 75], [110, 72, 48], [80, 52, 36]].map(lin);
    let hairs = [[25, 20, 18], [60, 40, 25], [120, 90, 50], [190, 160, 110], [90, 90, 90]].map(lin);
    let trousers = [[30, 32, 40], [40, 55, 85], [60, 60, 62], [90, 80, 65]].map(lin);
    for k in 0..FANS_PER_BLOCK {
        let r = |n: u32| hash2(seed as i32, (k as u32 * 17 + n) as i32, 0xfa);
        if r(0) < 0.07 {
            continue; // empty seat
        }
        let shirt = match r(1) {
            x if x < 0.4 => kit,
            x if x < 0.52 => trim,
            x if x < 0.56 => rival,
            _ => neutral[(r(2) * neutral.len() as f32) as usize % neutral.len()],
        };
        let shirt = shade(shirt, 0.55 + r(3) * 0.3);
        let skin = skins[(r(4) * skins.len() as f32) as usize % skins.len()];
        let hair = hairs[(r(5) * hairs.len() as f32) as usize % hairs.len()];
        let legs = trousers[(r(6) * trousers.len() as f32) as usize % trousers.len()];
        let s = 0.9 + r(7) * 0.18;
        let x = (k as f32 - (FANS_PER_BLOCK as f32 - 1.0) / 2.0) * SEAT_W + (r(8) - 0.5) * 0.08;
        let (torso_bottom, z) = if cheering {
            // Standing: legs straight down to the tread.
            g.cuboid(Vec3::new(x, 0.42 * s, 0.05), Vec3::new(0.14, 0.42 * s, 0.1), legs, true);
            (0.84 * s, 0.02)
        } else {
            // Seated: thighs forward (the row in front hides the shins).
            g.cuboid(Vec3::new(x, 0.47, 0.16), Vec3::new(0.15, 0.07, 0.2), legs, true);
            (0.48, -0.06)
        };
        let torso_h = 0.56 * s;
        let top = torso_bottom + torso_h;
        // Seated fans keep their arms by their sides, folded into a wider torso.
        let torso_hw = if cheering { 0.19 } else { 0.25 };
        g.cuboid(Vec3::new(x, torso_bottom + torso_h / 2.0, z), Vec3::new(torso_hw, torso_h / 2.0, 0.12), shirt, true);
        let head = Vec3::new(x, top + 0.14, z + 0.01);
        g.cuboid(head, Vec3::new(0.095, 0.12, 0.1), skin, true);
        if r(9) < 0.2 {
            g.cuboid(head + Vec3::Y * 0.12, Vec3::new(0.11, 0.035, 0.12), shirt, true); // cap
        } else {
            g.cuboid(head + Vec3::new(0.0, 0.1, -0.02), Vec3::new(0.1, 0.03, 0.095), hair, true);
        }
        if cheering {
            // Arms up, some holding a scarf over their heads.
            for side in [-1.0f32, 1.0] {
                g.cuboid(Vec3::new(x + side * 0.23, top + 0.24, z), Vec3::new(0.05, 0.27, 0.06), shirt, true);
            }
            if r(10) < 0.35 {
                g.cuboid(Vec3::new(x, top + 0.52, z), Vec3::new(0.32, 0.06, 0.02), if r(11) < 0.5 { kit } else { trim }, true);
            }
        }
    }
    g.build()
}

fn animate_crowd(
    time: Res<Time>,
    res: Res<MatchRes>,
    assets: Option<Res<CrowdAssets>>,
    state: Option<ResMut<CrowdState>>,
    mut fans: Query<(&Fan, &mut Transform, &mut Mesh3d)>,
) {
    let (Some(assets), Some(mut state)) = (assets, state) else { return };
    let t = time.elapsed_secs();
    let goal = matches!(res.m.phase, Phase::Goal { .. });
    let score = res.m.score;
    if goal && state.cheering.is_none() {
        let team = if score[1] > state.score[1] && score[0] <= state.score[0] { 1 } else { 0 };
        state.cheering = Some(team);
        for (fan, _, mut mesh) in &mut fans {
            if fan.team == team {
                mesh.0 = assets.cheer[fan.variant as usize].clone();
            }
        }
    } else if !goal && state.cheering.is_some() {
        state.cheering = None;
        for (fan, _, mut mesh) in &mut fans {
            if mesh.0 != assets.sit[fan.variant as usize] {
                mesh.0 = assets.sit[fan.variant as usize].clone();
            }
        }
    }
    if !goal {
        state.score = score;
    }
    let cheer = state.cheering;
    fans.par_iter_mut().for_each(|(fan, mut tf, _)| {
        let ph = fan.phase;
        let (lift, roll) = if cheer == Some(fan.team) {
            ((t * 7.5 + ph).sin().abs() * 0.3, (t * 2.7 + ph).sin() * 0.05)
        } else {
            ((t * 1.6 + ph).sin() * 0.012, (t * 0.9 + ph * 1.7).sin() * 0.018)
        };
        tf.translation = fan.base + Vec3::Y * lift;
        tf.rotation = fan.rot * Quat::from_rotation_z(roll);
    });
}

// ---------------------------------------------------------------------------------------------
// LED boards and big screen

/// Scrolling LED materials and their speeds (texture repeats per second).
#[derive(Resource)]
struct LedBoards(Vec<(Handle<StandardMaterial>, f32)>);

fn scroll_boards(time: Res<Time>, boards: Option<Res<LedBoards>>, mut mats: ResMut<Assets<StandardMaterial>>) {
    let Some(boards) = boards else { return };
    let t = time.elapsed_secs_f64();
    for (h, speed) in &boards.0 {
        if let Some(m) = mats.get_mut(h) {
            m.uv_transform.translation.x = (t * *speed as f64).fract() as f32;
        }
    }
}

#[derive(Component)]
struct BigScreen {
    names: [String; 2],
    league: String,
    shown: Option<([u32; 2], bool)>,
    material: Handle<StandardMaterial>,
}

fn screen_image(names: &[String; 2], score: [u32; 2], league: &str, goal: bool) -> Image {
    let line1 = format!("{} {}-{} {}", names[0], score[0], score[1], names[1]);
    let (line2, c2) = if goal { ("GOAL!".to_string(), [255, 220, 40, 255]) } else { (league.to_string(), [150, 200, 255, 255]) };
    textures::screen(&[(line1.as_str(), [255, 255, 255, 255]), (line2.as_str(), c2)], [200, 20, 40, 255])
}

fn update_screen(
    res: Res<MatchRes>,
    mut screens: Query<&mut BigScreen>,
    mut images: ResMut<Assets<Image>>,
    mut mats: ResMut<Assets<StandardMaterial>>,
) {
    let goal = matches!(res.m.phase, Phase::Goal { .. });
    let key = (res.m.score, goal);
    for mut s in &mut screens {
        if s.shown == Some(key) {
            continue;
        }
        let img = images.add(screen_image(&s.names, key.0, &s.league, goal));
        if let Some(m) = mats.get_mut(&s.material) {
            if let Some(old) = m.base_color_texture.take() {
                images.remove(&old);
            }
            m.base_color_texture = Some(img.clone());
            m.emissive_texture = Some(img);
        }
        s.shown = Some(key);
    }
}

// ---------------------------------------------------------------------------------------------
// The scene

/// Builds the whole match scene. `time_of_day` picks the lighting preset.
#[allow(clippy::too_many_arguments)]
pub fn spawn_stadium(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    images: &mut Assets<Image>,
    m: &Match,
    league: &str,
    time_of_day: TimeOfDay,
) {
    let scoped = StateScoped(AppState::Match);
    let night = time_of_day == TimeOfDay::Night;

    spawn_lights(commands, time_of_day);

    // Pitch and run-off: one textured surface out to the stands, lines painted into the texture.
    let margin = STAND_GAP + 0.5;
    let (albedo, normal) = textures::pitch(margin, 30.0);
    let (ex, ez) = (HALF_L + margin, HALF_W + margin);
    let mut g = Geo::default();
    let corners = [Vec3::new(-ex, 0.0, -ez), Vec3::new(ex, 0.0, -ez), Vec3::new(ex, 0.0, ez), Vec3::new(-ex, 0.0, ez)];
    let uvs = [Vec2::new(0.0, 0.0), Vec2::new(1.0, 0.0), Vec2::new(1.0, 1.0), Vec2::new(0.0, 1.0)];
    g.quad(corners, Vec3::Y, uvs, WHITE);
    let pitch_mesh = g.build().with_generated_tangents().expect("pitch mesh has uvs and normals");
    commands.spawn((
        Mesh3d(meshes.add(pitch_mesh)),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color_texture: Some(images.add(albedo)),
            normal_map_texture: Some(images.add(normal)),
            perceptual_roughness: 0.82,
            reflectance: 0.3,
            ..default()
        })),
        NotShadowCaster,
        scoped.clone(),
    ));

    // Land round the stadium, so the horizon beyond the roofs isn't the atmosphere's bare planet.
    commands.spawn((
        Mesh3d(meshes.add(Circle::new(560.0))),
        MeshMaterial3d(materials.add(StandardMaterial { base_color: Color::srgb(0.2, 0.24, 0.2), perceptual_roughness: 1.0, ..default() })),
        Transform::from_xyz(0.0, -0.3, 0.0).with_rotation(Quat::from_rotation_x(-FRAC_PI_2)),
        NotShadowCaster,
        scoped.clone(),
    ));

    spawn_goals(commands, meshes, materials, images);
    spawn_bowl(commands, meshes, materials, images, m, night);
    spawn_boards(commands, meshes, materials, images, league, night);
    spawn_extras(commands, meshes, materials, images, m, league, night);
}

fn spawn_lights(commands: &mut Commands, time_of_day: TimeOfDay) {
    let scoped = StateScoped(AppState::Match);
    // Broadcast cameras sit ~50 m from play, so the first cascade has to reach well past that.
    commands.insert_resource(DirectionalLightShadowMap { size: 3072 });
    let cascades = CascadeShadowConfigBuilder {
        num_cascades: 3,
        minimum_distance: 0.5,
        first_cascade_far_bound: 65.0,
        maximum_distance: 220.0,
        overlap_proportion: 0.25,
    }
    .build();
    match time_of_day {
        TimeOfDay::Day => {
            commands.insert_resource(ClearColor(Color::srgb(0.45, 0.62, 0.86)));
            commands.insert_resource(AmbientLight { color: Color::srgb(0.78, 0.86, 1.0), brightness: 900.0, ..default() });
            // Afternoon sun behind the main (camera-side) stand, so its roof shades the near
            // touchline. Layer 1 holds shadow-only geometry the camera never draws.
            let sun = Vec3::new(-0.45, 0.6, 0.66).normalize();
            commands.spawn((
                DirectionalLight { illuminance: 11_000.0, shadows_enabled: true, color: Color::srgb(1.0, 0.96, 0.9), ..default() },
                Transform::from_translation(sun * 100.0).looking_at(Vec3::ZERO, Vec3::Y),
                cascades,
                RenderLayers::from_layers(&[0, 1]),
                scoped.clone(),
            ));
        }
        TimeOfDay::Night => {
            commands.insert_resource(ClearColor(Color::srgb(0.015, 0.02, 0.045)));
            commands.insert_resource(AmbientLight { color: Color::srgb(0.7, 0.75, 0.9), brightness: 260.0, ..default() });
            let moon = Vec3::new(0.3, 0.8, 0.5).normalize();
            commands.spawn((
                DirectionalLight { illuminance: 2_500.0, shadows_enabled: true, color: Color::srgb(0.9, 0.93, 1.0), ..default() },
                Transform::from_translation(moon * 100.0).looking_at(Vec3::ZERO, Vec3::Y),
                cascades,
                RenderLayers::from_layers(&[0, 1]),
                scoped.clone(),
            ));
            // Floodlights along the roof fronts (no shadows: one shadowed light is plenty).
            let fz = HALF_W + STAND_GAP + ROOF_FRONT.x;
            let fx = HALF_L + STAND_GAP + ROOF_FRONT.x;
            let mut spots: Vec<Vec3> = [-35.0f32, 0.0, 35.0].iter().flat_map(|&x| [Vec3::new(x, 22.0, -fz), Vec3::new(x, 22.0, fz)]).collect();
            spots.extend([Vec3::new(-fx, 22.0, 0.0), Vec3::new(fx, 22.0, 0.0)]);
            for p in spots {
                commands.spawn((
                    PointLight { intensity: 3.5e7, range: 170.0, radius: 2.0, shadows_enabled: false, color: Color::srgb(1.0, 0.98, 0.94), ..default() },
                    Transform::from_translation(p),
                    scoped.clone(),
                ));
            }
        }
    }
}

/// Stepped bowl, seats, roofs with masts, floodlight strips, fascia LED ribbon and the crowd.
fn spawn_bowl(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    images: &mut Assets<Image>,
    m: &Match,
    night: bool,
) {
    let scoped = StateScoped(AppState::Match);
    let sections = bowl_sections();
    let path: Vec<PathPt> = sections.iter().flat_map(|(_, p)| p.iter().copied()).collect();
    let (profile, rows) = bowl_profile();

    // Steps and walls.
    let concrete_img = images.add(textures::concrete());
    let concrete = materials.add(StandardMaterial {
        base_color: Color::srgb(0.78, 0.78, 0.8),
        base_color_texture: Some(concrete_img.clone()),
        perceptual_roughness: 0.92,
        ..default()
    });
    let mut g = Geo::default();
    sweep(&mut g, &path, &profile, Vec2::splat(3.0), false, WHITE);
    commands.spawn((Mesh3d(meshes.add(g.build())), MeshMaterial3d(concrete.clone()), scoped.clone()));

    // Seats: pan plus back on every row, in a darkened home colour.
    let [r, gg, b] = textures::rgb(m.teams[0].home);
    let l = Color::srgb_u8(r, gg, b).to_linear();
    let seat_col = LinearRgba::rgb(l.red * 0.55 + 0.02, l.green * 0.55 + 0.02, l.blue * 0.55 + 0.025);
    let seats = materials.add(StandardMaterial { base_color: seat_col.into(), perceptual_roughness: 0.6, ..default() });
    let mut g = Geo::default();
    for row in &rows {
        let (d, h) = (row.d, row.h);
        let seat = [Vec2::new(d + 0.12, h + 0.4), Vec2::new(d + row.tread - 0.22, h + 0.42), Vec2::new(d + row.tread - 0.22, h + 0.82)];
        sweep(&mut g, &path, &seat, Vec2::splat(1.0), false, WHITE);
    }
    commands.spawn((Mesh3d(meshes.add(g.build())), MeshMaterial3d(seats), NotShadowCaster, scoped.clone()));

    // Fascia LED ribbon between the tiers, continuous all the way round.
    let ribbon_img = images.add(textures::led_strip(&["BIGILEEE 22", "FILTER KAAPI", "PAROTTA BANK", "SAMBAR ENERGY"], 480));
    let ribbon = materials.add(StandardMaterial {
        base_color_texture: Some(ribbon_img.clone()),
        emissive_texture: Some(ribbon_img),
        emissive: LinearRgba::rgb(1.0, 1.0, 1.0) * if night { 2.2 } else { 1.6 },
        perceptual_roughness: 0.5,
        ..default()
    });
    let mut g = Geo::default();
    let fascia = [Vec2::new(FASCIA_D, 8.1), Vec2::new(FASCIA_D, 9.5)];
    sweep(&mut g, &path, &fascia, Vec2::new(4.0 * 14.0, 1.4), true, WHITE);
    commands.spawn((Mesh3d(meshes.add(g.build())), MeshMaterial3d(ribbon.clone()), NotShadowCaster, scoped.clone()));

    // Roofs. The camera-side roof is drawn from underneath only, so the elevated broadcast
    // cameras can look over it; a closed copy on layer 1 still casts its shadow.
    let roof_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.42, 0.44, 0.48),
        perceptual_roughness: 0.55,
        metallic: 0.2,
        cull_mode: None,
        ..default()
    });
    let slab = [
        ROOF_FRONT,
        ROOF_FRONT + Vec2::Y * ROOF_THICK,
        ROOF_BACK + Vec2::Y * ROOF_THICK,
        ROOF_BACK,
        ROOF_FRONT,
    ];
    let underside = [ROOF_BACK, ROOF_FRONT];
    let (mut far, mut near, mut near_under) = (Geo::default(), Geo::default(), Geo::default());
    for (part, p) in &sections {
        if matches!(part, Part::Near | Part::Corner(true)) {
            sweep(&mut near, p, &slab, Vec2::splat(4.0), false, WHITE);
            sweep(&mut near_under, p, &underside, Vec2::splat(4.0), false, WHITE);
        } else {
            sweep(&mut far, p, &slab, Vec2::splat(4.0), false, WHITE);
        }
    }
    commands.spawn((Mesh3d(meshes.add(far.build())), MeshMaterial3d(roof_mat.clone()), scoped.clone()));
    let under_mat = materials.add(StandardMaterial { base_color: Color::srgb(0.6, 0.62, 0.66), perceptual_roughness: 0.6, ..default() });
    commands.spawn((Mesh3d(meshes.add(near_under.build())), MeshMaterial3d(under_mat), NotShadowCaster, scoped.clone()));
    commands.spawn((Mesh3d(meshes.add(near.build())), MeshMaterial3d(roof_mat), RenderLayers::layer(1), scoped.clone()));

    // Floodlight strip under each roof edge.
    let lamp = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        emissive: if night { LinearRgba::rgb(60.0, 58.0, 52.0) } else { LinearRgba::rgb(6.0, 6.0, 5.6) },
        ..default()
    });
    let mut g = Geo::default();
    let strip = [ROOF_FRONT + Vec2::new(0.1, -0.45), ROOF_FRONT + Vec2::new(0.1, -0.05)];
    for (part, p) in &sections {
        if !matches!(part, Part::Corner(_)) {
            sweep(&mut g, p, &strip, Vec2::splat(1.0), false, WHITE);
        }
    }
    commands.spawn((Mesh3d(meshes.add(g.build())), MeshMaterial3d(lamp), NotShadowCaster, scoped.clone()));

    // Cable-stayed masts behind the far and end stands hold the roof up.
    let steel = materials.add(StandardMaterial { base_color: Color::srgb(0.85, 0.86, 0.88), metallic: 0.6, perceptual_roughness: 0.4, ..default() });
    let rod = meshes.add(Cylinder::new(1.0, 1.0));
    let beam = |commands: &mut Commands, a: Vec3, b: Vec3, r: f32| {
        let d = b - a;
        commands.spawn((
            Mesh3d(rod.clone()),
            MeshMaterial3d(steel.clone()),
            Transform::from_translation((a + b) / 2.0)
                .with_rotation(Quat::from_rotation_arc(Vec3::Y, d.normalize()))
                .with_scale(Vec3::new(r, d.length(), r)),
            NotShadowCaster,
            scoped.clone(),
        ));
    };
    for (part, p) in &sections {
        if !matches!(part, Part::Far | Part::End) {
            continue;
        }
        let (a, b) = (p[0], p[p.len() - 1]);
        let len = (b.base - a.base).length();
        let n = (len / 24.0).round().max(2.0) as usize;
        for k in 0..n {
            let t = (k as f32 + 0.5) / n as f32;
            let pt = PathPt { base: a.base.lerp(b.base, t), out: a.out };
            let foot = pt.at(BACK_D + 2.5, 0.0);
            let top = pt.at(BACK_D + 2.5, 40.0);
            beam(commands, foot, top, 0.45);
            beam(commands, top, pt.at(ROOF_FRONT.x + 0.5, ROOF_FRONT.y + ROOF_THICK), 0.07);
            beam(commands, top, pt.at(14.0, 25.0), 0.07);
        }
    }

    // The crowd: blocks of four, home colours everywhere except the away end (+x).
    let rgbl = |c: u32| lin(textures::rgb(c));
    let teams = [&m.teams[0], &m.teams[1]];
    let mut sit = Vec::new();
    let mut cheer = Vec::new();
    let mut team_of = Vec::new();
    let fan_mat = materials.add(StandardMaterial { base_color: Color::WHITE, perceptual_roughness: 0.85, ..default() });
    for t in 0..2usize {
        let (kit, trim, rival) = (rgbl(teams[t].home), rgbl(teams[t].trim), rgbl(teams[1 - t].home));
        for v in 0..VARIANTS_PER_TEAM {
            let seed = v * 7919 + t as u32 * 104_729;
            sit.push(meshes.add(fan_block(seed, kit, trim, rival, false)));
            cheer.push(meshes.add(fan_block(seed, kit, trim, rival, true)));
            team_of.push(t as u8);
        }
    }
    let block_w = SEAT_W * FANS_PER_BLOCK as f32;
    let mut n = 0u32;
    for row in &rows {
        let d = row.d + row.tread * 0.45;
        // Polyline of seat positions at this depth, continuous round the bowl.
        let pts: Vec<(Vec2, Vec2)> = path.iter().map(|p| (p.base + p.out * d, p.out)).collect();
        let mut carry = block_w / 2.0;
        for w in pts.windows(2) {
            let ((a, oa), (b, ob)) = (w[0], w[1]);
            let seg = (b - a).length();
            if seg < 1e-3 {
                continue;
            }
            let mut s = carry;
            while s < seg {
                let t = s / seg;
                let p = a.lerp(b, t);
                let out = oa.lerp(ob, t).normalize_or_zero();
                s += block_w;
                n += 1;
                let h = hash2(n as i32, row.index as i32, 0xc0);
                if h < 0.03 {
                    continue;
                }
                // Leave the tunnel mouth clear on the far side.
                if p.y < 0.0 && out.y < -0.9 && p.x.abs() < 3.5 && row.lower && row.index < 5 {
                    continue;
                }
                let away = p.x > HALF_L + 2.0;
                let team = if away { 1 } else { 0 };
                let variant = team * VARIANTS_PER_TEAM as usize + (hash2(n as i32, 3, 0xc1) * VARIANTS_PER_TEAM as f32) as usize % VARIANTS_PER_TEAM as usize;
                let toward = -Vec3::new(out.x, 0.0, out.y);
                let rot = Quat::from_rotation_arc(Vec3::Z, toward);
                let base = Vec3::new(p.x, row.h, p.y);
                commands.spawn((
                    Mesh3d(sit[variant].clone()),
                    MeshMaterial3d(fan_mat.clone()),
                    Transform::from_translation(base).with_rotation(rot),
                    Fan { base, rot, phase: hash2(n as i32, 5, 0xc2) * 40.0, team: team_of[variant], variant: variant as u16 },
                    NotShadowCaster,
                    scoped.clone(),
                ));
            }
            carry = s - seg;
        }
    }
    commands.insert_resource(CrowdAssets { sit, cheer });
    commands.insert_resource(CrowdState { score: m.score, cheering: None });
}

/// Pitch-side LED boards: one continuous scrolling ribbon per side, with gaps for the goals
/// and the tunnel.
fn spawn_boards(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    images: &mut Assets<Image>,
    league: &str,
    night: bool,
) {
    let scoped = StateScoped(AppState::Match);
    let ads = ["BIGILEEE 22", "EAT MORE DOSA", "SAMBAR ENERGY", "VADA PAV AIRWAYS", league, "FILTER KAAPI", "KAALPANTHU", "PAROTTA BANK"];
    let seg_m = 9.0;
    let strip_w = seg_m * ads.len() as f32;
    let img = images.add(textures::led_strip(&ads, 480));
    let led = materials.add(StandardMaterial {
        base_color_texture: Some(img.clone()),
        emissive_texture: Some(img),
        emissive: LinearRgba::rgb(1.0, 1.0, 1.0) * if night { 2.4 } else { 2.0 },
        perceptual_roughness: 0.35,
        ..default()
    });
    let frame = materials.add(StandardMaterial { base_color: Color::srgb(0.05, 0.05, 0.06), perceptual_roughness: 0.6, ..default() });
    let (bz, bx) = (HALF_W + 4.0, HALF_L + 4.0);
    // (start, end, outward) in world xz; start is the left end as seen from the pitch.
    let runs = [
        (Vec2::new(-bx + 1.0, -bz), Vec2::new(-2.6, -bz), Vec2::new(0.0, -1.0)),
        (Vec2::new(2.6, -bz), Vec2::new(bx - 1.0, -bz), Vec2::new(0.0, -1.0)),
        (Vec2::new(bx, -bz + 1.0), Vec2::new(bx, -6.0), Vec2::new(1.0, 0.0)),
        (Vec2::new(bx, 6.0), Vec2::new(bx, bz - 1.0), Vec2::new(1.0, 0.0)),
        (Vec2::new(bx - 1.0, bz), Vec2::new(-bx + 1.0, bz), Vec2::new(0.0, 1.0)),
        (Vec2::new(-bx, bz - 1.0), Vec2::new(-bx, 6.0), Vec2::new(-1.0, 0.0)),
        (Vec2::new(-bx, -6.0), Vec2::new(-bx, -bz + 1.0), Vec2::new(-1.0, 0.0)),
    ];
    let mut face = Geo::default();
    for (a, b, out) in runs {
        let right = (b - a).normalize();
        let w = |p: Vec2, h: f32, lean: f32| Vec3::new(p.x + out.x * lean, h, p.y + out.y * lean);
        let (ua, ub) = (a.dot(right) / strip_w, b.dot(right) / strip_w);
        let tilt = 0.12;
        let n = (Vec3::new(-out.x, 0.0, -out.y) + Vec3::Y * tilt).normalize();
        face.quad(
            [w(a, 0.06, 0.0), w(b, 0.06, 0.0), w(b, 0.96, tilt), w(a, 0.96, tilt)],
            n,
            [Vec2::new(ua, 1.0), Vec2::new(ub, 1.0), Vec2::new(ub, 0.0), Vec2::new(ua, 0.0)],
            WHITE,
        );
        // Housing behind the panel.
        let len = (b - a).length();
        let mid = (a + b) / 2.0 + out * 0.45;
        let size = if out.x == 0.0 { Vec3::new(len + 0.1, 1.0, 0.5) } else { Vec3::new(0.5, 1.0, len + 0.1) };
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::from_size(size))),
            MeshMaterial3d(frame.clone()),
            Transform::from_xyz(mid.x, 0.5, mid.y),
            scoped.clone(),
        ));
    }
    commands.spawn((Mesh3d(meshes.add(face.build())), MeshMaterial3d(led.clone()), NotShadowCaster, scoped.clone()));
    commands.insert_resource(LedBoards(vec![(led, 0.012)]));
}

/// Big screen, tunnel, dugouts and corner flags.
#[allow(clippy::too_many_arguments)]
fn spawn_extras(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    images: &mut Assets<Image>,
    m: &Match,
    league: &str,
    night: bool,
) {
    let scoped = StateScoped(AppState::Match);
    let dark = materials.add(StandardMaterial { base_color: Color::srgb(0.08, 0.08, 0.09), perceptual_roughness: 0.7, ..default() });
    let grey = materials.add(StandardMaterial { base_color: Color::srgb(0.55, 0.56, 0.6), perceptual_roughness: 0.6, metallic: 0.3, ..default() });

    // Big screen hanging under the roof in the far-left corner.
    let names = [m.teams[0].short.clone(), m.teams[1].short.clone()];
    let img = images.add(screen_image(&names, m.score, league, false));
    let screen_mat = materials.add(StandardMaterial {
        base_color_texture: Some(img.clone()),
        emissive_texture: Some(img),
        emissive: LinearRgba::rgb(1.0, 1.0, 1.0) * if night { 2.5 } else { 3.0 },
        perceptual_roughness: 0.3,
        ..default()
    });
    let c = Vec2::new(-(HALF_L + STAND_GAP), -(HALF_W + STAND_GAP));
    let out = Vec2::new(-1.0, -1.0).normalize();
    let at = c + out * 15.0;
    let toward = -Vec3::new(out.x, 0.0, out.y);
    let rot = Quat::from_rotation_arc(Vec3::Z, toward);
    let centre = Vec3::new(at.x, 20.0, at.y);
    let (sw, sh) = (11.0, 5.0);
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(sw + 0.6, sh + 0.6, 0.5))),
        MeshMaterial3d(dark.clone()),
        Transform::from_translation(centre - toward * 0.3).with_rotation(rot),
        scoped.clone(),
    ));
    commands.spawn((
        Mesh3d(meshes.add(Rectangle::new(sw, sh))),
        MeshMaterial3d(screen_mat.clone()),
        Transform::from_translation(centre).with_rotation(rot),
        NotShadowCaster,
        BigScreen { names, league: league.to_string(), shown: None, material: screen_mat },
        scoped.clone(),
    ));
    for side in [-1.0f32, 1.0] {
        let x = rot * Vec3::X * side * sw * 0.4;
        commands.spawn((
            Mesh3d(meshes.add(Cylinder::new(0.08, 2.4))),
            MeshMaterial3d(grey.clone()),
            Transform::from_translation(centre + x + Vec3::Y * (sh / 2.0 + 1.2) - toward * 0.3),
            scoped.clone(),
        ));
    }

    // Players' tunnel on the far side, running from the stand to the boards.
    let z0 = -(HALF_W + STAND_GAP + 3.0);
    let z1 = -(HALF_W + 4.6);
    let tunnel_mat = materials.add(StandardMaterial { base_color: Color::srgb(0.22, 0.23, 0.26), perceptual_roughness: 0.45, metallic: 0.3, ..default() });
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(4.4, 2.9, z1 - z0))),
        MeshMaterial3d(tunnel_mat),
        Transform::from_xyz(0.0, 1.45, (z0 + z1) / 2.0),
        scoped.clone(),
    ));
    commands.spawn((
        Mesh3d(meshes.add(Rectangle::new(3.4, 2.3))),
        MeshMaterial3d(materials.add(StandardMaterial { base_color: Color::BLACK, perceptual_roughness: 1.0, ..default() })),
        Transform::from_xyz(0.0, 1.16, z1 + 0.01),
        NotShadowCaster,
        scoped.clone(),
    ));

    // Dugouts either side of the tunnel: back wall, smoked perspex roof and sides, team benches.
    let perspex = materials.add(StandardMaterial {
        base_color: Color::srgba(0.55, 0.65, 0.75, 0.35),
        alpha_mode: AlphaMode::Blend,
        perceptual_roughness: 0.1,
        reflectance: 0.6,
        cull_mode: None,
        ..default()
    });
    for (t, sx) in [(0usize, -1.0f32), (1, 1.0)] {
        let x = sx * 12.0;
        let (zb, zf) = (-(HALF_W + 3.7), -(HALF_W + 1.7));
        let len = 7.0;
        let zm = (zb + zf) / 2.0;
        commands.spawn((Mesh3d(meshes.add(Cuboid::new(len, 2.1, 0.12))), MeshMaterial3d(dark.clone()), Transform::from_xyz(x, 1.05, zb), scoped.clone()));
        commands.spawn((Mesh3d(meshes.add(Cuboid::new(len, 0.06, zf - zb))), MeshMaterial3d(perspex.clone()), Transform::from_xyz(x, 2.12, zm), NotShadowCaster, scoped.clone()));
        for e in [-1.0f32, 1.0] {
            commands.spawn((
                Mesh3d(meshes.add(Cuboid::new(0.05, 2.1, zf - zb))),
                MeshMaterial3d(perspex.clone()),
                Transform::from_xyz(x + e * len / 2.0, 1.05, zm),
                NotShadowCaster,
                scoped.clone(),
            ));
        }
        commands.spawn((Mesh3d(meshes.add(Cuboid::new(len, 0.05, zf - zb))), MeshMaterial3d(grey.clone()), Transform::from_xyz(x, 0.025, zm), scoped.clone()));
        let [r, g, b] = textures::rgb(m.teams[t].home);
        let seat = materials.add(StandardMaterial { base_color: Color::srgb_u8(r, g, b), perceptual_roughness: 0.4, ..default() });
        let seat_mesh = meshes.add(Cuboid::new(0.5, 0.9, 0.5));
        for k in 0..12 {
            let sxk = x - len / 2.0 + 0.4 + k as f32 * 0.56;
            commands.spawn((Mesh3d(seat_mesh.clone()), MeshMaterial3d(seat.clone()), Transform::from_xyz(sxk, 0.45, zb + 0.35), scoped.clone()));
        }
    }

    // Corner flags.
    let pole = meshes.add(Cylinder::new(0.022, 1.5));
    let flag_mesh = meshes.add(Rectangle::new(0.42, 0.3));
    let flag = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.85, 0.1),
        cull_mode: None,
        double_sided: true,
        perceptual_roughness: 0.8,
        ..default()
    });
    let white = materials.add(StandardMaterial { base_color: Color::WHITE, perceptual_roughness: 0.4, ..default() });
    for sx in [-1.0f32, 1.0] {
        for sz in [-1.0f32, 1.0] {
            let p = Vec3::new(sx * HALF_L, 0.0, sz * HALF_W);
            commands.spawn((Mesh3d(pole.clone()), MeshMaterial3d(white.clone()), Transform::from_translation(p + Vec3::Y * 0.75), scoped.clone()));
            commands.spawn((
                Mesh3d(flag_mesh.clone()),
                MeshMaterial3d(flag.clone()),
                Transform::from_translation(p + Vec3::new(-sx * 0.2, 1.33, 0.0)).with_rotation(Quat::from_rotation_y(sz * 0.4)),
                NotShadowCaster,
                scoped.clone(),
            ));
        }
    }
}

/// A subdivided net panel: `origin` plus `u`/`v` edge vectors, bulging by `sag` (a world
/// offset scaled by a sine bump), with the net texture tiled `tiles` times per metre.
fn net_panel(origin: Vec3, u: Vec3, v: Vec3, sag: Vec3, tiles: f32) -> Mesh {
    let (nu, nv) = (((u.length() * 4.0).ceil() as usize).max(2), ((v.length() * 4.0).ceil() as usize).max(2));
    let normal = u.cross(v).normalize();
    let mut pos = Vec::new();
    let mut uvs = Vec::new();
    for j in 0..=nv {
        for i in 0..=nu {
            let (a, b) = (i as f32 / nu as f32, j as f32 / nv as f32);
            let bump = (a * std::f32::consts::PI).sin() * (b * std::f32::consts::PI).sin();
            pos.push((origin + u * a + v * b + sag * bump).to_array());
            uvs.push([a * u.length() * tiles, b * v.length() * tiles]);
        }
    }
    let mut idx = Vec::new();
    for j in 0..nv {
        for i in 0..nu {
            let k = (j * (nu + 1) + i) as u32;
            let w = (nu + 1) as u32;
            idx.extend_from_slice(&[k, k + 1, k + w + 1, k, k + w + 1, k + w]);
        }
    }
    let n = pos.len();
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, pos)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, vec![normal.to_array(); n])
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
        .with_inserted_indices(Indices::U32(idx))
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
    let back_bar = meshes.add(Cylinder::new(0.025, GOAL_HW * 2.0));
    // 8 cells per texture tile, 10 cm cells.
    let tiles = 1.25;
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
        commands.spawn((
            Mesh3d(back_bar.clone()),
            MeshMaterial3d(white.clone()),
            Transform::from_xyz(bx, GOAL_H, 0.0).with_rotation(Quat::from_rotation_x(FRAC_PI_2)),
            scoped.clone(),
        ));
        // Net: back bellies outwards, roof sags, sides bow out a little.
        let (front, back) = (side * HALF_L, bx);
        let panels = [
            (Vec3::new(back, 0.0, -GOAL_HW), Vec3::new(0.0, 0.0, GOAL_HW * 2.0), Vec3::new(0.0, GOAL_H, 0.0), Vec3::new(side * 0.22, -0.06, 0.0)),
            (Vec3::new(front, GOAL_H, -GOAL_HW), Vec3::new(back - front, 0.0, 0.0), Vec3::new(0.0, 0.0, GOAL_HW * 2.0), Vec3::new(0.0, -0.18, 0.0)),
            (Vec3::new(front, 0.0, -GOAL_HW), Vec3::new(back - front, 0.0, 0.0), Vec3::new(0.0, GOAL_H, 0.0), Vec3::new(0.0, -0.03, -0.08)),
            (Vec3::new(front, 0.0, GOAL_HW), Vec3::new(back - front, 0.0, 0.0), Vec3::new(0.0, GOAL_H, 0.0), Vec3::new(0.0, -0.03, 0.08)),
        ];
        for (o, u, v, sag) in panels {
            commands.spawn((Mesh3d(meshes.add(net_panel(o, u, v, sag, tiles))), MeshMaterial3d(net.clone()), NotShadowCaster, scoped.clone()));
        }
    }
}

