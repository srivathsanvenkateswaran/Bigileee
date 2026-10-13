//! A procedural, skinned human figure: one continuous body mesh over a 14-joint skeleton, so
//! knees, elbows, hips and spine bend smoothly instead of the body being stacked primitives.
//!
//! The model faces -Z with +Y up and stands about 1.8 m tall. Every part of the body samples a
//! small per-person texture atlas: a kit-pattern region for the shirt and solid cells for skin,
//! shorts, socks, sock trim, gloves, boots and hair.

use super::font5x7::text_image;
use super::textures::{self, rgb};
use crate::data::types::Crest;
use bevy::asset::RenderAssetUsages;
use bevy::image::{ImageSampler, ImageSamplerDescriptor};
use bevy::prelude::*;
use bevy::render::mesh::skinning::{SkinnedMesh, SkinnedMeshInverseBindposes};
use bevy::render::mesh::{Indices, PrimitiveTopology, VertexAttributeValues};
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::render::view::NoFrustumCulling;
use std::f32::consts::TAU;

pub const HIPS: usize = 0;
pub const SPINE: usize = 1;
pub const CHEST: usize = 2;
pub const HEAD: usize = 3;
pub const ARM_L: usize = 4;
pub const FOREARM_L: usize = 5;
pub const ARM_R: usize = 6;
pub const FOREARM_R: usize = 7;
pub const THIGH_L: usize = 8;
pub const SHIN_L: usize = 9;
pub const FOOT_L: usize = 10;
pub const THIGH_R: usize = 11;
pub const SHIN_R: usize = 12;
pub const FOOT_R: usize = 13;
pub const JOINTS: usize = 14;

/// (parent, offset from parent) in the bind pose.
const SKELETON: [(Option<usize>, Vec3); JOINTS] = [
    (None, Vec3::new(0.0, 0.95, 0.0)),
    (Some(HIPS), Vec3::new(0.0, 0.22, 0.0)),
    (Some(SPINE), Vec3::new(0.0, 0.18, 0.0)),
    (Some(CHEST), Vec3::new(0.0, 0.25, 0.0)),
    (Some(CHEST), Vec3::new(-0.2, 0.1, 0.0)),
    (Some(ARM_L), Vec3::new(-0.02, -0.29, 0.0)),
    (Some(CHEST), Vec3::new(0.2, 0.1, 0.0)),
    (Some(ARM_R), Vec3::new(0.02, -0.29, 0.0)),
    (Some(HIPS), Vec3::new(-0.1, -0.06, 0.0)),
    (Some(THIGH_L), Vec3::new(0.0, -0.43, 0.0)),
    (Some(SHIN_L), Vec3::new(0.0, -0.4, 0.0)),
    (Some(HIPS), Vec3::new(0.1, -0.06, 0.0)),
    (Some(THIGH_R), Vec3::new(0.0, -0.43, 0.0)),
    (Some(SHIN_R), Vec3::new(0.0, -0.4, 0.0)),
];

pub fn bind_position(j: usize) -> Vec3 {
    let (parent, offset) = SKELETON[j];
    parent.map_or(offset, |p| bind_position(p) + offset)
}

// ---------- texture atlas ----------

const ATLAS: u32 = 128;

#[derive(Clone, Copy)]
enum Cell {
    Skin,
    Shorts,
    Socks,
    Trim,
    Gloves,
    Boots,
    Hair,
    Sleeve,
}

/// Centre UV of a solid colour cell.
fn cell(c: Cell) -> [f32; 2] {
    let (cx, cy) = match c {
        Cell::Sleeve => (2, 0),
        Cell::Shorts => (3, 0),
        Cell::Skin => (2, 1),
        Cell::Socks => (3, 1),
        Cell::Trim => (2, 2),
        Cell::Gloves => (3, 2),
        Cell::Boots => (2, 3),
        Cell::Hair => (3, 3),
    };
    [(cx as f32 + 0.5) / 4.0, (cy as f32 + 0.5) / 4.0]
}

/// UV inside the kit-pattern region (the top-left 64x64 of the atlas), inset to avoid bleeding.
fn shirt_uv(u: f32, v: f32) -> [f32; 2] {
    [0.01 + u.fract() * 0.48, 0.01 + v.clamp(0.0, 1.0) * 0.48]
}

/// Colours for one person's atlas.
pub struct Look {
    pub crest: Crest,
    pub shirt: u32,
    pub accent: u32,
    pub sleeve: u32,
    pub shorts: u32,
    pub socks: u32,
    pub trim: u32,
    pub skin: u32,
    pub hair: u32,
    pub boots: u32,
    pub gloves: u32,
}

pub fn atlas(look: &Look) -> Image {
    let n = ATLAS as usize;
    let mut data = vec![255u8; n * n * 4];
    let pattern = textures::kit(look.crest, look.shirt, look.accent);
    let pd = pattern.data.as_ref().expect("kit texture has pixel data");
    for y in 0..64usize {
        for x in 0..64usize {
            let s = (y * 64 + x) * 4;
            let d = (y * n + x) * 4;
            data[d..d + 4].copy_from_slice(&pd[s..s + 4]);
        }
    }
    let fill = |data: &mut Vec<u8>, c: Cell, colour: u32| {
        let uv = cell(c);
        let (x0, y0) = ((uv[0] * 4.0).floor() as usize * 32, (uv[1] * 4.0).floor() as usize * 32);
        let [r, g, b] = rgb(colour);
        for y in y0..y0 + 32 {
            for x in x0..x0 + 32 {
                let d = (y * n + x) * 4;
                // A touch of noise keeps solid areas from looking like plastic.
                let k = ((x * 7 + y * 13) % 9) as i32 - 4;
                data[d] = (r as i32 + k).clamp(0, 255) as u8;
                data[d + 1] = (g as i32 + k).clamp(0, 255) as u8;
                data[d + 2] = (b as i32 + k).clamp(0, 255) as u8;
                data[d + 3] = 255;
            }
        }
    };
    fill(&mut data, Cell::Sleeve, look.sleeve);
    fill(&mut data, Cell::Shorts, look.shorts);
    fill(&mut data, Cell::Skin, look.skin);
    fill(&mut data, Cell::Socks, look.socks);
    fill(&mut data, Cell::Trim, look.trim);
    fill(&mut data, Cell::Gloves, look.gloves);
    fill(&mut data, Cell::Boots, look.boots);
    fill(&mut data, Cell::Hair, look.hair);
    paint_face(&mut data, n, look);
    let mut img = Image::new(
        Extent3d { width: ATLAS, height: ATLAS, depth_or_array_layers: 1 },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD,
    );
    img.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor::linear());
    img
}

/// Paints a simple generic face into the bottom-left quarter: skin with soft shading, eyes,
/// brows and a mouth centred on the front (u = 0.75 of the head's wrap).
fn paint_face(data: &mut [u8], n: usize, look: &Look) {
    let [r, g, b] = rgb(look.skin).map(|c| c as f32);
    let set = |data: &mut [u8], x: usize, y: usize, c: [f32; 3]| {
        let d = ((64 + y) * n + x) * 4;
        data[d..d + 4].copy_from_slice(&[c[0].clamp(0.0, 255.0) as u8, c[1].clamp(0.0, 255.0) as u8, c[2].clamp(0.0, 255.0) as u8, 255]);
    };
    for y in 0..64 {
        for x in 0..64 {
            let (u, v) = (x as f32 / 64.0, y as f32 / 64.0);
            // Slightly darker towards the back of the head and under the jaw.
            let front = (1.0 - ((u - 0.75).abs() * 2.0).min(1.0)) * 0.08;
            let under = (v - 0.7).max(0.0) * 0.25;
            let k = 0.94 + front - under;
            set(data, x, y, [r * k, g * k, b * k]);
        }
    }
    let px = |u: f32| (u * 64.0).round() as usize;
    let py = |v: f32| (v * 64.0).round() as usize;
    let [hr, hg, hb] = rgb(look.hair).map(|c| c as f32);
    let lip = [r * 0.72 + 20.0, g * 0.55, b * 0.55];
    for side in [-1.0f32, 1.0] {
        let ex = px(0.75 + side * 0.048);
        let ey = py(0.47);
        for dx in 0..3 {
            set(data, ex + dx - 1, ey, [235.0, 232.0, 226.0]);
        }
        set(data, ex, ey, [40.0, 28.0, 22.0]);
        for dx in 0..4 {
            set(data, ex + dx - 2, py(0.41), [hr * 0.9, hg * 0.9, hb * 0.9]);
        }
    }
    for dx in 0..5 {
        set(data, px(0.75) + dx - 2, py(0.645), lip);
    }
}

/// Name above number, as printed on the back of a shirt.
pub fn back_print(name: &str, num: u8, fg: [u8; 4]) -> Image {
    let surname = name.rsplit(' ').next().unwrap_or(name).to_uppercase();
    let top = text_image(&surname, fg, [0, 0, 0, 0], 2, 0);
    let bottom = text_image(&num.to_string(), fg, [0, 0, 0, 0], 9, 0);
    let (tw, th) = (top.width(), top.height());
    let (bw, bh) = (bottom.width(), bottom.height());
    let w = tw.max(bw) + 8;
    let h = th + bh + 10;
    let mut data = vec![0u8; (w * h * 4) as usize];
    let blit = |data: &mut Vec<u8>, img: &Image, ox: u32, oy: u32| {
        let src = img.data.as_ref().unwrap();
        for y in 0..img.height() {
            for x in 0..img.width() {
                let s = ((y * img.width() + x) * 4) as usize;
                let d = (((oy + y) * w + ox + x) * 4) as usize;
                data[d..d + 4].copy_from_slice(&src[s..s + 4]);
            }
        }
    };
    blit(&mut data, &top, (w - tw) / 2, 2);
    blit(&mut data, &bottom, (w - bw) / 2, th + 8);
    let mut img = Image::new(
        Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    );
    img.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor::linear());
    img
}

// ---------- body mesh ----------

#[derive(Default)]
struct Builder {
    pos: Vec<[f32; 3]>,
    uv: Vec<[f32; 2]>,
    joints: Vec<[u16; 4]>,
    weights: Vec<[f32; 4]>,
    idx: Vec<u32>,
}

type Skin = ([u16; 4], [f32; 4]);

fn one(j: usize) -> Skin {
    ([j as u16, 0, 0, 0], [1.0, 0.0, 0.0, 0.0])
}

fn blend(a: usize, b: usize, t: f32) -> Skin {
    let t = t.clamp(0.0, 1.0);
    let t = t * t * (3.0 - 2.0 * t);
    ([a as u16, b as u16, 0, 0], [1.0 - t, t, 0.0, 0.0])
}

const AROUND: usize = 16;

impl Builder {
    fn vertex(&mut self, p: Vec3, uv: [f32; 2], s: Skin) -> u32 {
        self.pos.push(p.to_array());
        self.uv.push(uv);
        self.joints.push(s.0);
        self.weights.push(s.1);
        (self.pos.len() - 1) as u32
    }

    /// A lofted tube from `a` to `b`. `radius(t)` gives the (side, front) half-widths along it.
    #[allow(clippy::too_many_arguments)]
    fn tube(
        &mut self,
        a: Vec3,
        b: Vec3,
        rings: usize,
        radius: impl Fn(f32) -> (f32, f32),
        skin: impl Fn(f32) -> Skin,
        uv: impl Fn(f32, f32) -> [f32; 2],
        caps: (bool, bool),
    ) {
        let axis = (b - a).normalize();
        let side = (Vec3::X - axis * axis.x).normalize_or(Vec3::Z);
        let front = axis.cross(side).normalize();
        let base = self.pos.len() as u32;
        for r in 0..=rings {
            let t = r as f32 / rings as f32;
            let c = a.lerp(b, t);
            let (rx, rz) = radius(t);
            for k in 0..=AROUND {
                let ang = k as f32 / AROUND as f32 * TAU;
                let p = c + side * ang.cos() * rx + front * ang.sin() * rz;
                self.vertex(p, uv(t, k as f32 / AROUND as f32), skin(t));
            }
        }
        let row = (AROUND + 1) as u32;
        for r in 0..rings as u32 {
            for k in 0..AROUND as u32 {
                let i0 = base + r * row + k;
                let i1 = i0 + 1;
                let i2 = i0 + row;
                let i3 = i2 + 1;
                self.idx.extend_from_slice(&[i0, i1, i2, i1, i3, i2]);
            }
        }
        for (end, t) in [(caps.0, 0.0f32), (caps.1, 1.0)] {
            if !end {
                continue;
            }
            let ring = if t == 0.0 { 0 } else { rings as u32 };
            let centre = self.vertex(a.lerp(b, t) + axis * if t == 0.0 { -0.01 } else { 0.01 }, uv(t, 0.5), skin(t));
            for k in 0..AROUND as u32 {
                let i0 = base + ring * row + k;
                if t == 0.0 {
                    self.idx.extend_from_slice(&[centre, i0 + 1, i0]);
                } else {
                    self.idx.extend_from_slice(&[centre, i0, i0 + 1]);
                }
            }
        }
    }

    fn ellipsoid(&mut self, c: Vec3, r: Vec3, skin: Skin, uv: [f32; 2]) {
        self.ellipsoid_uv(c, r, skin, move |_, _| uv);
    }

    /// An ellipsoid whose UVs come from `uv(v, u)`: v runs top to bottom, u around (0.75 faces -Z).
    fn ellipsoid_uv(&mut self, c: Vec3, r: Vec3, skin: Skin, uv: impl Fn(f32, f32) -> [f32; 2]) {
        let (lat, lon) = (10, AROUND);
        let base = self.pos.len() as u32;
        for i in 0..=lat {
            let phi = i as f32 / lat as f32 * std::f32::consts::PI;
            for k in 0..=lon {
                let th = k as f32 / lon as f32 * TAU;
                let p = c + Vec3::new(phi.sin() * th.cos() * r.x, phi.cos() * r.y, phi.sin() * th.sin() * r.z);
                self.vertex(p, uv(i as f32 / lat as f32, k as f32 / lon as f32), skin);
            }
        }
        let row = (lon + 1) as u32;
        for i in 0..lat as u32 {
            for k in 0..lon as u32 {
                let i0 = base + i * row + k;
                self.idx.extend_from_slice(&[i0, i0 + row, i0 + 1, i0 + 1, i0 + row, i0 + row + 1]);
            }
        }
    }

    fn build(self) -> Mesh {
        let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
            .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, self.pos)
            .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, self.uv)
            .with_inserted_attribute(Mesh::ATTRIBUTE_JOINT_INDEX, VertexAttributeValues::Uint16x4(self.joints))
            .with_inserted_attribute(Mesh::ATTRIBUTE_JOINT_WEIGHT, self.weights)
            .with_inserted_indices(Indices::U32(self.idx));
        mesh.compute_smooth_normals();
        mesh
    }
}

fn lerp_keys(keys: &[(f32, f32, f32)], t: f32) -> (f32, f32) {
    let k = keys.partition_point(|k| k.0 < t).clamp(1, keys.len() - 1);
    let (a, b) = (keys[k - 1], keys[k]);
    let s = ((t - a.0) / (b.0 - a.0).max(1e-4)).clamp(0.0, 1.0);
    let s = s * s * (3.0 - 2.0 * s);
    (a.1 + (b.1 - a.1) * s, a.2 + (b.2 - a.2) * s)
}

/// Body proportions that vary between people.
#[derive(Clone, Copy)]
pub struct Build {
    /// Overall breadth multiplier (0.9 slight .. 1.12 stocky).
    pub bulk: f32,
    /// Keepers wear long sleeves.
    pub long_sleeves: bool,
    pub gloves: bool,
}

pub fn body_mesh(b: Build) -> Mesh {
    let mut m = Builder::default();
    let w = b.bulk;
    let jp = bind_position;

    // Torso: crotch to neck, shorts below the waistband, shirt above.
    let bottom = Vec3::new(0.0, 0.83, 0.0);
    let top = Vec3::new(0.0, 1.55, 0.0);
    let torso_keys = [
        (0.0, 0.13, 0.10),
        (0.12, 0.175, 0.125),
        (0.26, 0.165, 0.115),
        (0.38, 0.16, 0.112),
        (0.55, 0.185, 0.125),
        (0.72, 0.205, 0.128),
        (0.86, 0.2, 0.11),
        (0.95, 0.11, 0.08),
        (1.0, 0.06, 0.06),
    ];
    let waist = 0.26;
    m.tube(
        bottom,
        top,
        18,
        |t| {
            let (rx, rz) = lerp_keys(&torso_keys, t);
            (rx * w, rz * w)
        },
        |t| {
            let y = bottom.y + (top.y - bottom.y) * t;
            if y < 1.06 {
                one(HIPS)
            } else if y < 1.22 {
                blend(HIPS, SPINE, (y - 1.06) / 0.16)
            } else if y < 1.36 {
                blend(SPINE, CHEST, (y - 1.22) / 0.14)
            } else if y < 1.5 {
                one(CHEST)
            } else {
                blend(CHEST, HEAD, (y - 1.5) / 0.08)
            }
        },
        |t, u| if t < waist { cell(Cell::Shorts) } else if t > 0.94 { cell(Cell::Trim) } else { shirt_uv(u, (t - waist) / (0.94 - waist)) },
        (true, false),
    );
    // Neck and head.
    m.tube(Vec3::new(0.0, 1.5, 0.0), Vec3::new(0.0, 1.66, 0.0), 3, |_| (0.055, 0.055), |t| blend(CHEST, HEAD, 0.4 + t * 0.6), |_, _| cell(Cell::Skin), (false, false));
    // The head samples the face region (bottom-left quarter of the atlas).
    m.ellipsoid_uv(jp(HEAD) + Vec3::new(0.0, 0.11, -0.005), Vec3::new(0.1, 0.122, 0.112), one(HEAD), |v, u| [0.005 + u * 0.49, 0.505 + v * 0.49]);
    m.ellipsoid(jp(HEAD) + Vec3::new(0.0, 0.055, -0.085), Vec3::new(0.016, 0.026, 0.02), one(HEAD), cell(Cell::Skin)); // nose
    for s in [-1.0f32, 1.0] {
        m.ellipsoid(jp(HEAD) + Vec3::new(s * 0.1, 0.1, 0.0), Vec3::new(0.016, 0.03, 0.02), one(HEAD), cell(Cell::Skin)); // ears
    }

    // Arms.
    for (arm, fore) in [(ARM_L, FOREARM_L), (ARM_R, FOREARM_R)] {
        let (s, e) = (jp(arm), jp(fore));
        let wrist = e + (e - s).normalize() * 0.26;
        let sleeve_end = if b.long_sleeves { 1.01 } else { 0.42 };
        m.ellipsoid(s + Vec3::new(0.0, -0.02, 0.0), Vec3::new(0.075, 0.07, 0.07) * w, blend(CHEST, arm, 0.6), cell(Cell::Sleeve));
        m.tube(
            s,
            e,
            8,
            |t| {
                let r = (0.058 - 0.012 * t) * w;
                (r, r * 0.95)
            },
            |t| if t < 0.15 { blend(CHEST, arm, 0.5 + t * 3.3) } else if t > 0.85 { blend(arm, fore, (t - 0.85) / 0.3) } else { one(arm) },
            |t, _| if t < sleeve_end { cell(Cell::Sleeve) } else { cell(Cell::Skin) },
            (false, false),
        );
        m.tube(
            e,
            wrist,
            8,
            |t| {
                let r = (0.046 - 0.012 * t) * w;
                (r, r * 0.9)
            },
            |t| if t < 0.12 { blend(arm, fore, 0.5 + t * 4.0) } else { one(fore) },
            |_, _| if b.long_sleeves { cell(Cell::Sleeve) } else { cell(Cell::Skin) },
            (false, false),
        );
        let hand = if b.gloves { cell(Cell::Gloves) } else { cell(Cell::Skin) };
        let hs = if b.gloves { 1.3 } else { 1.0 };
        m.ellipsoid(wrist + (wrist - e).normalize() * 0.06, Vec3::new(0.03, 0.065, 0.045) * hs, one(fore), hand);
    }

    // Legs.
    for (thigh, shin, foot) in [(THIGH_L, SHIN_L, FOOT_L), (THIGH_R, SHIN_R, FOOT_R)] {
        let (h, k, a) = (jp(thigh), jp(shin), jp(foot));
        let hip_top = h + Vec3::new(0.0, 0.07, 0.0);
        m.tube(
            hip_top,
            k,
            10,
            |t| {
                let r = (0.092 - 0.03 * t) * w;
                (r, r * 1.05)
            },
            |t| if t < 0.2 { blend(HIPS, thigh, 0.5 + t * 2.5) } else if t > 0.82 { blend(thigh, shin, (t - 0.82) / 0.36) } else { one(thigh) },
            |t, _| if t < 0.36 { cell(Cell::Shorts) } else { cell(Cell::Skin) },
            (false, false),
        );
        let calf = [(0.0, 0.058, 0.06), (0.3, 0.064, 0.072), (0.75, 0.045, 0.047), (1.0, 0.04, 0.042)];
        m.tube(
            k,
            a,
            10,
            |t| {
                let (rx, rz) = lerp_keys(&calf, t);
                (rx * w, rz * w)
            },
            |t| if t < 0.12 { blend(thigh, shin, 0.5 + t * 4.0) } else if t > 0.9 { blend(shin, foot, (t - 0.9) / 0.2) } else { one(shin) },
            |t, _| if t < 0.1 { cell(Cell::Trim) } else { cell(Cell::Socks) },
            (false, false),
        );
        // Boot: heel under the ankle, toe forward (-Z).
        m.ellipsoid(a + Vec3::new(0.0, -0.025, -0.055), Vec3::new(0.05, 0.042, 0.135), one(foot), cell(Cell::Boots));
    }
    m.build()
}

/// Rigid hair pieces, attached to the head joint.
pub fn hair_meshes(style: usize, meshes: &mut Assets<Mesh>) -> Vec<(Handle<Mesh>, Transform)> {
    let cap = |sx: f32, sy: f32, sz: f32| Sphere::new(1.0).mesh().uv(20, 12).scaled_by(Vec3::new(sx, sy, sz));
    let head = Vec3::new(0.0, 0.11, -0.005);
    match style % 6 {
        0 | 1 => vec![(meshes.add(cap(0.104, 0.1, 0.116)), Transform::from_translation(head + Vec3::new(0.0, 0.035, 0.012)))],
        2 => vec![(meshes.add(cap(0.122, 0.12, 0.13)), Transform::from_translation(head + Vec3::new(0.0, 0.05, 0.01)))],
        3 => vec![
            (meshes.add(cap(0.106, 0.1, 0.118)), Transform::from_translation(head + Vec3::new(0.0, 0.035, 0.012))),
            (meshes.add(cap(0.09, 0.11, 0.06)), Transform::from_translation(head + Vec3::new(0.0, -0.04, 0.075))),
        ],
        4 => vec![(meshes.add(cap(0.102, 0.08, 0.113)), Transform::from_translation(head + Vec3::new(0.0, 0.045, 0.01)))],
        _ => vec![
            (meshes.add(cap(0.103, 0.085, 0.115)), Transform::from_translation(head + Vec3::new(0.0, 0.04, 0.012))),
            (meshes.add(cap(0.04, 0.04, 0.04)), Transform::from_translation(head + Vec3::new(0.0, 0.14, 0.04))),
        ],
    }
}

/// A spawned figure: the root (placed and turned on the pitch) and its joint entities.
pub struct Figure {
    pub root: Entity,
    pub joints: [Entity; JOINTS],
}

pub struct FigureParts<'a> {
    pub body: Handle<Mesh>,
    pub bindposes: Handle<SkinnedMeshInverseBindposes>,
    pub material: Handle<StandardMaterial>,
    pub hair: &'a [(Handle<Mesh>, Transform)],
    pub hair_material: Handle<StandardMaterial>,
    pub beard: Option<(Handle<Mesh>, Handle<StandardMaterial>)>,
    pub back: Option<(Handle<Mesh>, Handle<StandardMaterial>)>,
    /// Chest sponsor print.
    pub front: Option<(Handle<Mesh>, Handle<StandardMaterial>)>,
}

pub fn inverse_bindposes() -> SkinnedMeshInverseBindposes {
    let v: Vec<Mat4> = (0..JOINTS).map(|j| Mat4::from_translation(-bind_position(j))).collect();
    SkinnedMeshInverseBindposes::from(v)
}

pub fn spawn_figure(commands: &mut Commands, root_tf: Transform, parts: FigureParts, extra: impl Bundle + Clone) -> Figure {
    let root = commands.spawn((root_tf, Visibility::default(), extra.clone())).id();
    let mut joints = [Entity::PLACEHOLDER; JOINTS];
    for j in 0..JOINTS {
        let (parent, offset) = SKELETON[j];
        let parent = parent.map_or(root, |p| joints[p]);
        joints[j] = commands.spawn((Transform::from_translation(offset), Visibility::default(), ChildOf(parent))).id();
    }
    commands.spawn((
        Mesh3d(parts.body),
        MeshMaterial3d(parts.material),
        SkinnedMesh { inverse_bindposes: parts.bindposes, joints: joints.to_vec() },
        Transform::default(),
        NoFrustumCulling,
        ChildOf(root),
    ));
    for (mesh, tf) in parts.hair {
        commands.spawn((Mesh3d(mesh.clone()), MeshMaterial3d(parts.hair_material.clone()), *tf, ChildOf(joints[HEAD])));
    }
    if let Some((mesh, mat)) = parts.beard {
        commands.spawn((Mesh3d(mesh), MeshMaterial3d(mat), Transform::from_xyz(0.0, 0.045, -0.06).with_scale(Vec3::new(1.0, 0.75, 0.75)), ChildOf(joints[HEAD])));
    }
    if let Some((mesh, mat)) = parts.back {
        // Printed on the back of the shirt (+Z), just proud of the surface.
        commands.spawn((Mesh3d(mesh), MeshMaterial3d(mat), Transform::from_xyz(0.0, 0.0, 0.132), ChildOf(joints[CHEST])));
    }
    if let Some((mesh, mat)) = parts.front {
        commands.spawn((
            Mesh3d(mesh),
            MeshMaterial3d(mat),
            Transform::from_xyz(0.0, 0.01, -0.13).with_rotation(Quat::from_rotation_y(std::f32::consts::PI)),
            ChildOf(joints[CHEST]),
        ));
    }
    Figure { root, joints }
}

// ---------- posing ----------

/// Joint rotations (and hip height) for one frame.
pub struct Pose {
    pub hip_y: f32,
    pub rot: [Quat; JOINTS],
}

impl Default for Pose {
    fn default() -> Pose {
        Pose { hip_y: 0.95, rot: [Quat::IDENTITY; JOINTS] }
    }
}

/// What a figure is doing this frame.
#[derive(Clone, Copy, Default)]
pub struct Motion {
    /// Ground speed, m/s.
    pub speed: f32,
    /// Stride phase, 0..1.
    pub phase: f32,
    /// 0..1 progress through a kick, if kicking.
    pub kick: Option<f32>,
    pub sliding: bool,
    /// Dive side (-1 left, +1 right) and remaining airtime, if diving.
    pub dive: Option<(f32, f32)>,
    pub holding: bool,
    pub celebrating: bool,
    pub stunned: bool,
    /// Head yaw towards the ball, radians.
    pub look: f32,
    /// Seconds of animation time, for idle motion.
    pub time: f32,
    /// Raised flag arm (linesmen).
    pub flag_up: bool,
    /// A goal celebration routine and seconds into it.
    pub routine: Option<(usize, f32)>,
    /// Strikes the ball with the left foot.
    pub left_footed: bool,
    /// Normalised turn rate (-1 hard left .. 1 hard right in body terms), for leaning.
    pub turn: f32,
    pub dribbling: bool,
    /// 0..1 progress through a first touch / header.
    pub first_touch: Option<f32>,
    pub header: Option<f32>,
    pub throw_in: bool,
    /// A keeper set and ready while play is near his goal.
    pub keeper_ready: bool,
}

pub fn pose(m: Motion) -> Pose {
    let mut p = Pose::default();
    let run = (m.speed / 8.5).min(1.0);
    let phi = m.phase * TAU;
    let rx = Quat::from_rotation_x;
    let rz = Quat::from_rotation_z;
    let ry = Quat::from_rotation_y;

    // Legs: thighs swing, knees fold most while the leg comes through.
    for (thigh, shin, foot, off) in [(THIGH_L, SHIN_L, FOOT_L, 0.0), (THIGH_R, SHIN_R, FOOT_R, std::f32::consts::PI)] {
        let a = phi + off;
        let swing = a.sin() * (0.15 + 0.85 * run) * 0.95;
        let knee = 0.08 + run * (0.2 + 1.25 * (a + 0.6).cos().max(0.0));
        p.rot[thigh] = rx(swing);
        p.rot[shin] = rx(-knee);
        p.rot[foot] = rx(knee * 0.35 - swing * 0.3);
    }
    // Arms counter-swing with bent elbows.
    for (arm, fore, side, off) in [(ARM_L, FOREARM_L, -1.0f32, std::f32::consts::PI), (ARM_R, FOREARM_R, 1.0, 0.0)] {
        let a = phi + off;
        p.rot[arm] = rx(a.sin() * 0.85 * run) * rz(side * (0.12 + 0.08 * run));
        p.rot[fore] = rx(0.25 + 1.15 * run + a.sin().max(0.0) * 0.25 * run);
    }
    p.hip_y = 0.95 - 0.05 * run + (phi * 2.0).cos().abs() * 0.05 * run;
    p.rot[HIPS] = rx(-0.1 * run) * ry(phi.sin() * 0.14 * run);
    p.rot[SPINE] = rx(-0.08 * run) * ry(-phi.sin() * 0.1 * run);
    p.rot[CHEST] = ry(-phi.sin() * 0.12 * run);
    p.rot[HEAD] = ry(m.look) * rx(0.12 * run);

    if m.speed < 0.4 {
        // Idle: soft knees, slight weight shift and breathing.
        let b = (m.time * 1.7).sin();
        p.hip_y = 0.93 + b * 0.004;
        p.rot[HIPS] = rz(b * 0.03);
        for (thigh, shin) in [(THIGH_L, SHIN_L), (THIGH_R, SHIN_R)] {
            p.rot[thigh] = rx(0.12);
            p.rot[shin] = rx(-0.22);
        }
        p.rot[FOOT_L] = rx(0.1);
        p.rot[FOOT_R] = rx(0.1);
        p.rot[ARM_L] = rz(-0.1 - b * 0.02) * rx(0.05);
        p.rot[ARM_R] = rz(0.1 + b * 0.02) * rx(0.05);
        p.rot[FOREARM_L] = rx(0.3);
        p.rot[FOREARM_R] = rx(0.3);
    }

    if let Some(k) = m.kick {
        // Plant, wind back, strike through, follow through.
        let (thigh, knee) = if k < 0.3 {
            let s = k / 0.3;
            (-0.6 * s, 0.4 + 1.1 * s)
        } else if k < 0.55 {
            let s = (k - 0.3) / 0.25;
            (-0.6 + 1.9 * s, 1.5 - 1.4 * s)
        } else {
            let s = (k - 0.55) / 0.45;
            (1.3 - 0.9 * s, 0.1 + 0.3 * s)
        };
        // Left-footers mirror the whole action.
        let (kt, ks, kf, pt, ps, ka, oa, side) = if m.left_footed {
            (THIGH_L, SHIN_L, FOOT_L, THIGH_R, SHIN_R, ARM_R, ARM_L, -1.0)
        } else {
            (THIGH_R, SHIN_R, FOOT_R, THIGH_L, SHIN_L, ARM_L, ARM_R, 1.0)
        };
        p.rot[kt] = rx(thigh);
        p.rot[ks] = rx(-knee);
        p.rot[kf] = rx(-0.3);
        p.rot[pt] = rx(-0.15);
        p.rot[ps] = rx(-0.35);
        p.rot[ka] = rz(-0.9 * side) * rx(0.3);
        p.rot[oa] = rz(0.6 * side) * rx(-0.3);
        p.rot[SPINE] = rx(0.1) * ry(0.25 * (k - 0.4) * side);
        p.hip_y = 0.9;
    }
    if m.sliding {
        p.hip_y = 0.3;
        p.rot[HIPS] = rx(1.0) * rz(0.35);
        p.rot[SPINE] = rx(0.25);
        p.rot[THIGH_L] = rx(0.55);
        p.rot[SHIN_L] = rx(-0.05);
        p.rot[THIGH_R] = rx(-0.4);
        p.rot[SHIN_R] = rx(-1.7);
        p.rot[ARM_L] = rz(-1.1);
        p.rot[ARM_R] = rz(0.7) * rx(-0.6);
    }
    if let Some((side, air)) = m.dive {
        let air = air.clamp(0.0, 1.0);
        p.hip_y = 0.5 + air * 0.55;
        p.rot[HIPS] = rz(-side * 1.35);
        p.rot[SPINE] = rz(-side * 0.15);
        p.rot[ARM_L] = rz(-2.75) * rx(0.2);
        p.rot[ARM_R] = rz(2.75) * rx(0.2);
        p.rot[FOREARM_L] = rx(0.1);
        p.rot[FOREARM_R] = rx(0.1);
        p.rot[THIGH_L] = rx(0.2) * rz(0.15);
        p.rot[THIGH_R] = rx(-0.1) * rz(-0.15);
        p.rot[SHIN_L] = rx(-0.4);
        p.rot[SHIN_R] = rx(-0.3);
    }
    if m.holding {
        p.rot[ARM_L] = rx(1.2) * rz(0.15);
        p.rot[ARM_R] = rx(1.2) * rz(-0.15);
        p.rot[FOREARM_L] = rx(0.7);
        p.rot[FOREARM_R] = rx(0.7);
    }
    if m.celebrating {
        let jump = (m.time * 6.5 + m.phase * 4.0).sin().max(0.0);
        p.hip_y += jump * 0.28;
        p.rot[ARM_L] = rz(-2.5 - jump * 0.3);
        p.rot[ARM_R] = rz(2.5 + jump * 0.3);
        p.rot[FOREARM_L] = rx(0.3);
        p.rot[FOREARM_R] = rx(0.3);
        p.rot[THIGH_L] = rx(0.3 * (1.0 - jump));
        p.rot[THIGH_R] = rx(0.3 * (1.0 - jump));
        p.rot[SHIN_L] = rx(-0.6 * (1.0 - jump));
        p.rot[SHIN_R] = rx(-0.6 * (1.0 - jump));
        p.rot[HEAD] = rx(-0.35);
    }
    if let Some((kind, t)) = m.routine {
        routine(&mut p, kind, t);
    }
    // Lean into turns, more at speed.
    if m.turn.abs() > 0.02 {
        p.rot[HIPS] = rz(m.turn * 0.35 * (0.3 + run)) * p.rot[HIPS];
        p.rot[HEAD] = rz(-m.turn * 0.2) * p.rot[HEAD];
    }
    // Dribbling: head over the ball, a slightly more crouched, shorter-striding action.
    if m.dribbling && m.kick.is_none() {
        p.rot[SPINE] = rx(0.12) * p.rot[SPINE];
        p.rot[HEAD] = rx(0.3) * p.rot[HEAD];
        p.hip_y -= 0.02;
    }
    if let Some(s) = m.first_touch.filter(|s| *s < 1.0) {
        // Lift the receiving foot and cushion the ball down.
        let lift = (s * std::f32::consts::PI).sin();
        let (thigh, shin, foot) = if m.left_footed { (THIGH_L, SHIN_L, FOOT_L) } else { (THIGH_R, SHIN_R, FOOT_R) };
        p.rot[thigh] = rx(0.55 * lift);
        p.rot[shin] = rx(-0.55 * lift - 0.1);
        p.rot[foot] = rx(0.45 * lift);
        p.rot[HEAD] = rx(0.35);
    }
    if let Some(s) = m.header.filter(|s| *s < 1.0) {
        // Leap, arch back, then snap the head through the ball.
        let up = (s * std::f32::consts::PI).sin();
        p.hip_y += 0.38 * up;
        p.rot[SPINE] = rx(if s < 0.45 { -0.35 * (s / 0.45) } else { -0.35 + 0.95 * ((s - 0.45) / 0.55) });
        p.rot[HEAD] = rx(if s < 0.45 { -0.3 } else { 0.4 });
        p.rot[ARM_L] = rz(-1.1) * rx(0.3);
        p.rot[ARM_R] = rz(1.1) * rx(0.3);
        p.rot[THIGH_L] = rx(0.25 * up);
        p.rot[THIGH_R] = rx(0.1 * up);
        p.rot[SHIN_L] = rx(-0.9 * up);
        p.rot[SHIN_R] = rx(-0.6 * up);
    }
    if m.throw_in {
        // Ball held behind the head, back arched.
        p.rot[ARM_L] = rz(-2.75) * rx(-0.45);
        p.rot[ARM_R] = rz(2.75) * rx(-0.45);
        p.rot[FOREARM_L] = rx(1.5);
        p.rot[FOREARM_R] = rx(1.5);
        p.rot[SPINE] = rx(-0.25);
        p.rot[HEAD] = rx(-0.1);
    }
    if m.keeper_ready && m.dive.is_none() && !m.holding && m.speed < 2.5 {
        // Set position: knees bent, weight forward, hands out.
        p.hip_y = 0.8;
        p.rot[HIPS] = rx(0.2);
        p.rot[SPINE] = rx(0.25);
        for (thigh, shin, foot) in [(THIGH_L, SHIN_L, FOOT_L), (THIGH_R, SHIN_R, FOOT_R)] {
            p.rot[thigh] = rx(0.55);
            p.rot[shin] = rx(-1.0);
            p.rot[foot] = rx(0.3);
        }
        p.rot[THIGH_L] = rz(0.12) * p.rot[THIGH_L];
        p.rot[THIGH_R] = rz(-0.12) * p.rot[THIGH_R];
        p.rot[ARM_L] = rz(-0.55) * rx(0.55);
        p.rot[ARM_R] = rz(0.55) * rx(0.55);
        p.rot[FOREARM_L] = rx(0.45);
        p.rot[FOREARM_R] = rx(0.45);
    }
    if m.stunned && !m.sliding && m.dive.is_none() {
        p.rot[SPINE] = rz((m.time * 11.0).sin() * 0.1) * rx(0.35);
        p.rot[HEAD] = rx(0.3);
    }
    if m.flag_up {
        p.rot[ARM_R] = rz(2.8);
        p.rot[FOREARM_R] = rx(0.1);
    }
    p
}

/// The five celebration routines. Most play once the scorer has wheeled away (about 1.2 s in).
fn routine(p: &mut Pose, kind: usize, t: f32) {
    let rx = Quat::from_rotation_x;
    let rz = Quat::from_rotation_z;
    let start = 1.2;
    if t < start && kind != 4 {
        return;
    }
    let s = t - start;
    match kind {
        // Sambar Slide: on the knees, leaning back, arms flung wide.
        0 => {
            let lean = (s * 3.0).min(1.0);
            p.hip_y = 0.95 - 0.5 * lean;
            p.rot[HIPS] = rx(-0.35 * lean);
            p.rot[SPINE] = rx(-0.25 * lean);
            for (thigh, shin) in [(THIGH_L, SHIN_L), (THIGH_R, SHIN_R)] {
                p.rot[thigh] = rx(0.35 * lean);
                p.rot[shin] = rx(-1.9 * lean);
            }
            p.rot[ARM_L] = rz(-1.45) * rx(-0.3);
            p.rot[ARM_R] = rz(1.45) * rx(-0.3);
            p.rot[FOREARM_L] = rx(0.1);
            p.rot[FOREARM_R] = rx(0.1);
            p.rot[HEAD] = rx(-0.5);
        }
        // Dosa Flip: a tucked backflip, then a proud landing.
        1 => {
            let f = (s / 0.8).clamp(0.0, 1.0);
            if f < 1.0 {
                p.hip_y = 0.95 + (f * std::f32::consts::PI).sin() * 1.3;
                p.rot[HIPS] = rx(-std::f32::consts::TAU * f);
                for (thigh, shin) in [(THIGH_L, SHIN_L), (THIGH_R, SHIN_R)] {
                    p.rot[thigh] = rx(1.6 * (f * std::f32::consts::PI).sin());
                    p.rot[shin] = rx(-2.0 * (f * std::f32::consts::PI).sin());
                }
                p.rot[ARM_L] = rx(1.0);
                p.rot[ARM_R] = rx(1.0);
            } else {
                p.rot[ARM_L] = rz(-2.6);
                p.rot[ARM_R] = rz(2.6);
            }
        }
        // Filter Kaapi Pour: one hand high, one low, pouring between them.
        2 => {
            let pour = (s * 3.5).sin();
            p.rot[ARM_L] = rz(-2.4) * rx(0.4);
            p.rot[FOREARM_L] = rx(0.6 + pour * 0.25);
            p.rot[ARM_R] = rx(0.9) * rz(0.2);
            p.rot[FOREARM_R] = rx(1.2 - pour * 0.25);
            p.rot[HEAD] = rx(-0.25);
            p.hip_y = 0.93;
        }
        // Biryani Bow: a deep, slow bow to the crowd.
        3 => {
            let bow = ((s * 1.2).sin() * 0.5 + 0.5).max(0.0);
            p.rot[SPINE] = rx(0.9 * bow);
            p.rot[CHEST] = rx(0.3 * bow);
            p.rot[HEAD] = rx(0.4 * bow);
            p.rot[ARM_L] = rz(-0.7) * rx(-0.6 * bow);
            p.rot[ARM_R] = rz(0.7) * rx(-0.6 * bow);
            p.rot[THIGH_L] = rx(0.0);
            p.rot[THIGH_R] = rx(0.0);
            p.rot[SHIN_L] = rx(-0.1);
            p.rot[SHIN_R] = rx(-0.1);
            p.hip_y = 0.94;
        }
        // Helicopter: arms out like rotor blades (the spin is applied to the root).
        _ => {
            p.rot[ARM_L] = rz(-1.55);
            p.rot[ARM_R] = rz(1.55);
            p.rot[FOREARM_L] = Quat::IDENTITY;
            p.rot[FOREARM_R] = Quat::IDENTITY;
            p.rot[HEAD] = rx(-0.3);
        }
    }
}

pub fn apply<F: bevy::ecs::query::QueryFilter>(pose: &Pose, joints: &[Entity; JOINTS], q: &mut Query<&mut Transform, F>) {
    for j in 0..JOINTS {
        if let Ok(mut tf) = q.get_mut(joints[j]) {
            tf.rotation = pose.rot[j];
            if j == HIPS {
                tf.translation.y = pose.hip_y;
            }
        }
    }
}
