//! Procedurally generated textures: pitch surface, LED boards, goal net, ball and club crests.

use crate::data::types::Crest;
use bevy::asset::RenderAssetUsages;
use bevy::image::{ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

pub fn rgb(c: u32) -> [u8; 3] {
    [(c >> 16) as u8, (c >> 8) as u8, c as u8]
}

pub fn color(c: u32) -> Color {
    let [r, g, b] = rgb(c);
    Color::srgb_u8(r, g, b)
}

fn image(w: u32, h: u32, data: Vec<u8>, repeat: bool, linear: bool) -> Image {
    let mut img = Image::new(
        Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD,
    );
    let mode = if repeat { ImageAddressMode::Repeat } else { ImageAddressMode::ClampToEdge };
    let filter = if linear { ImageFilterMode::Linear } else { ImageFilterMode::Nearest };
    img.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: mode,
        address_mode_v: mode,
        mag_filter: filter,
        min_filter: filter,
        mipmap_filter: filter,
        anisotropy_clamp: if linear { 16 } else { 1 },
        ..default()
    });
    img
}

// ---------------------------------------------------------------------------------------------
// Procedural noise. Kept separate from the simulation RNG so building the stadium never shifts
// the match's random stream.

/// Deterministic hash of an integer lattice point to `[0, 1)`.
pub fn hash2(x: i32, y: i32, seed: u32) -> f32 {
    let mut h = (x as u32).wrapping_mul(0x8da6_b343) ^ (y as u32).wrapping_mul(0xd816_3841) ^ seed.wrapping_mul(0xcb1a_b31f);
    h ^= h >> 13;
    h = h.wrapping_mul(0x5bd1_e995);
    h ^= h >> 15;
    (h & 0x00ff_ffff) as f32 / 16_777_216.0
}

/// Smooth value noise in `[0, 1)`.
fn vnoise(x: f32, y: f32, seed: u32) -> f32 {
    let (xf, yf) = (x.floor(), y.floor());
    let (fx, fy) = (x - xf, y - yf);
    let (sx, sy) = (fx * fx * (3.0 - 2.0 * fx), fy * fy * (3.0 - 2.0 * fy));
    let (xi, yi) = (xf as i32, yf as i32);
    let a = hash2(xi, yi, seed);
    let b = hash2(xi + 1, yi, seed);
    let c = hash2(xi, yi + 1, seed);
    let d = hash2(xi + 1, yi + 1, seed);
    a + (b - a) * sx + (c - a) * sy + (a - b - c + d) * sx * sy
}

/// Two-octave value noise in roughly `[0, 1)`.
fn fbm2(x: f32, y: f32, seed: u32) -> f32 {
    (vnoise(x, y, seed) * 2.0 + vnoise(x * 2.03 + 7.1, y * 2.03 - 3.7, seed ^ 0x55)) / 3.0
}

fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Builds an image with a full box-filtered mip chain (Bevy does not generate mips itself).
/// `normal_map` renormalises the averaged vectors and stores the result as linear data.
fn mipped(w: u32, h: u32, base: Vec<u8>, normal_map: bool, repeat: bool) -> Image {
    let format = if normal_map { TextureFormat::Rgba8Unorm } else { TextureFormat::Rgba8UnormSrgb };
    let mut data = base.clone();
    let (mut cw, mut ch, mut cur) = (w, h, base);
    let mut levels = 1;
    while cw > 1 || ch > 1 {
        let (nw, nh) = ((cw / 2).max(1), (ch / 2).max(1));
        let mut next = vec![0u8; (nw * nh * 4) as usize];
        for y in 0..nh {
            for x in 0..nw {
                let mut acc = [0f32; 4];
                for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                    let sx = (x * 2 + dx).min(cw - 1);
                    let sy = (y * 2 + dy).min(ch - 1);
                    let i = ((sy * cw + sx) * 4) as usize;
                    for (c, a) in acc.iter_mut().enumerate() {
                        *a += cur[i + c] as f32 * 0.25;
                    }
                }
                if normal_map {
                    let v = Vec3::new(acc[0] / 127.5 - 1.0, acc[1] / 127.5 - 1.0, acc[2] / 127.5 - 1.0).normalize_or(Vec3::Z);
                    acc[0] = (v.x * 0.5 + 0.5) * 255.0;
                    acc[1] = (v.y * 0.5 + 0.5) * 255.0;
                    acc[2] = (v.z * 0.5 + 0.5) * 255.0;
                }
                let o = ((y * nw + x) * 4) as usize;
                for (c, a) in acc.iter().enumerate() {
                    next[o + c] = a.round().clamp(0.0, 255.0) as u8;
                }
            }
        }
        data.extend_from_slice(&next);
        levels += 1;
        cw = nw;
        ch = nh;
        cur = next;
    }
    let mut img = Image::new_uninit(
        Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        TextureDimension::D2,
        format,
        RenderAssetUsages::RENDER_WORLD,
    );
    img.texture_descriptor.mip_level_count = levels;
    img.data = Some(data);
    let mode = if repeat { ImageAddressMode::Repeat } else { ImageAddressMode::ClampToEdge };
    img.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: mode,
        address_mode_v: mode,
        mag_filter: ImageFilterMode::Linear,
        min_filter: ImageFilterMode::Linear,
        mipmap_filter: ImageFilterMode::Linear,
        anisotropy_clamp: 16,
        ..default()
    });
    img
}

/// Fills an RGBA buffer on all cores. `f(x, y)` returns the pixel.
fn par_fill(w: u32, h: u32, f: impl Fn(u32, u32) -> [u8; 4] + Sync) -> Vec<u8> {
    let mut data = vec![0u8; (w * h * 4) as usize];
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get()).clamp(1, 12);
    let rows_per = (h as usize).div_ceil(threads);
    std::thread::scope(|s| {
        for (k, chunk) in data.chunks_mut(rows_per * w as usize * 4).enumerate() {
            let f = &f;
            s.spawn(move || {
                for (r, row) in chunk.chunks_mut(w as usize * 4).enumerate() {
                    let y = (k * rows_per + r) as u32;
                    for x in 0..w {
                        let i = x as usize * 4;
                        row[i..i + 4].copy_from_slice(&f(x, y));
                    }
                }
            });
        }
    });
    data
}

/// Geometry of the painted pitch markings, in simulation metres.
enum Paint {
    Seg(Vec2, Vec2),
    /// Ring of radius `r` around a centre; the predicate (centre, direction) keeps some arcs only.
    Ring(Vec2, f32, fn(Vec2, Vec2) -> bool),
    Disc(Vec2, f32),
}

fn markings() -> Vec<Paint> {
    use crate::sim::consts::*;
    let mut p = Vec::new();
    let rect = |x0: f32, y0: f32, x1: f32, y1: f32, p: &mut Vec<Paint>| {
        let (a, b, c, d) = (Vec2::new(x0, y0), Vec2::new(x1, y0), Vec2::new(x1, y1), Vec2::new(x0, y1));
        p.extend([Paint::Seg(a, b), Paint::Seg(b, c), Paint::Seg(c, d), Paint::Seg(d, a)]);
    };
    rect(-HALF_L, -HALF_W, HALF_L, HALF_W, &mut p);
    p.push(Paint::Seg(Vec2::new(0.0, -HALF_W), Vec2::new(0.0, HALF_W)));
    p.push(Paint::Ring(Vec2::ZERO, CIRCLE_R, |_, _| true));
    p.push(Paint::Disc(Vec2::ZERO, 0.15));
    for side in [-1.0f32, 1.0] {
        let gx = side * HALF_L;
        rect(gx, -BOX_HW, gx - side * BOX_D, BOX_HW, &mut p);
        rect(gx, -SIX_HW, gx - side * SIX_D, SIX_HW, &mut p);
        let spot = Vec2::new(gx - side * PEN_SPOT, 0.0);
        p.push(Paint::Disc(spot, 0.15));
        // The "D": only the part of the circle outside the penalty area.
        p.push(Paint::Ring(spot, CIRCLE_R, |c, d| {
            let side = c.x.signum();
            ((c + d * CIRCLE_R).x - side * (HALF_L - BOX_D)) * -side > 0.0
        }));
        for ys in [-1.0f32, 1.0] {
            // Corner quarter circles, pointing into the pitch.
            p.push(Paint::Ring(Vec2::new(gx, ys * HALF_W), 1.0, |c, d| d.x * -c.x.signum() >= 0.0 && d.y * -c.y.signum() >= 0.0));
        }
    }
    p
}

/// Broadcast-style pitch surface covering the pitch plus `margin` metres of run-off on every side,
/// at `ppm` texels per metre. Returns (albedo, tangent-space normal map), both with mip chains.
///
/// Wide mown stripes along the length with a faint cross-cut, blade-level noise, worn goalmouths
/// and centre spot, a darker run-off band, and the line markings painted in with soft edges.
/// Texel (0, 0) is the (-x, -y) corner in simulation space; u runs along x and v along y.
pub fn pitch(margin: f32, ppm: f32) -> (Image, Image) {
    use crate::sim::consts::*;
    let (ex, ey) = (HALF_L + margin, HALF_W + margin);
    let (w, h) = ((ex * 2.0 * ppm) as u32, (ey * 2.0 * ppm) as u32);
    let to_sim = |x: u32, y: u32| Vec2::new(-ex + (x as f32 + 0.5) / ppm, -ey + (y as f32 + 0.5) / ppm);

    // Paint coverage, rasterised per primitive inside its bounding box.
    let mut paint = vec![0f32; (w * h) as usize];
    let lw = 0.12;
    let soft = 0.6 / ppm;
    for prim in markings() {
        let (lo, hi) = match &prim {
            Paint::Seg(a, b) => (a.min(*b), a.max(*b)),
            Paint::Ring(c, r, _) | Paint::Disc(c, r) => (*c - Vec2::splat(*r), *c + Vec2::splat(*r)),
        };
        let pad = Vec2::splat(lw + 2.0 / ppm);
        let (lo, hi) = (lo - pad, hi + pad);
        let x0 = ((lo.x + ex) * ppm).floor().max(0.0) as u32;
        let x1 = (((hi.x + ex) * ppm).ceil() as u32).min(w - 1);
        let y0 = ((lo.y + ey) * ppm).floor().max(0.0) as u32;
        let y1 = (((hi.y + ey) * ppm).ceil() as u32).min(h - 1);
        for y in y0..=y1 {
            for x in x0..=x1 {
                let p = to_sim(x, y);
                let d = match &prim {
                    Paint::Seg(a, b) => {
                        let ab = *b - *a;
                        let t = ((p - *a).dot(ab) / ab.length_squared()).clamp(0.0, 1.0);
                        (p - (*a + ab * t)).length() - lw / 2.0
                    }
                    Paint::Ring(c, r, keep) => {
                        let dir = (p - *c).normalize_or_zero();
                        if keep(*c, dir) {
                            ((p - *c).length() - r).abs() - lw / 2.0
                        } else {
                            f32::MAX
                        }
                    }
                    Paint::Disc(c, r) => (p - *c).length() - r,
                };
                let cov = (0.5 - d / (2.0 * soft)).clamp(0.0, 1.0);
                let i = (y * w + x) as usize;
                paint[i] = paint[i].max(cov);
            }
        }
    }

    let stripe_w = HALF_L * 2.0 / 18.0;
    let cross_w = HALF_W * 2.0 / 12.0;
    // Per-texel surface description shared by the albedo and normal passes.
    let surface = |x: u32, y: u32| {
        let p = to_sim(x, y);
        let band = ((p.x + HALF_L) / stripe_w).floor() as i32;
        let cross = ((p.y + HALF_W) / cross_w).floor() as i32;
        let s = if band.rem_euclid(2) == 0 { 1.0 } else { -1.0 };
        let c = if cross.rem_euclid(2) == 0 { 1.0 } else { -1.0 };
        // Distance outside the touch/goal lines: run-off starts a metre and a half out.
        let out = (p.x.abs() - HALF_L).max(p.y.abs() - HALF_W);
        let runoff = smoothstep(1.0, 2.5, out);
        // Wear: goalmouths, penalty spots and the centre spot take a beating.
        let mut wear = 0.0f32;
        for side in [-1.0f32, 1.0] {
            let g = Vec2::new(side * (HALF_L - 2.5), 0.0);
            let e = ((p - g) / Vec2::new(5.0, 7.5)).length();
            wear = wear.max((1.0 - e).clamp(0.0, 1.0).powf(0.8) * 0.85);
            let spot = Vec2::new(side * (HALF_L - PEN_SPOT), 0.0);
            wear = wear.max((1.0 - (p - spot).length() / 1.3).clamp(0.0, 1.0) * 0.6);
        }
        wear = wear.max((1.0 - p.length() / 2.2).clamp(0.0, 1.0) * 0.45);
        let patch = fbm2(p.x * 0.9, p.y * 0.9, 11);
        wear = (wear * (0.55 + patch * 0.9)).clamp(0.0, 1.0);
        (p, s, c, runoff, wear)
    };

    let albedo = par_fill(w, h, |x, y| {
        let (p, s, c, runoff, wear) = surface(x, y);
        let light = Vec3::new(80.0, 138.0, 62.0);
        let dark = Vec3::new(64.0, 117.0, 52.0);
        let mut col = if s > 0.0 { light } else { dark };
        col = col.lerp((light + dark) * 0.5, runoff * 0.55);
        col *= 1.0 + c * 0.025 * (1.0 - runoff);
        col = col.lerp(Vec3::new(64.0, 104.0, 52.0), runoff * 0.6);
        // Large soft patches, then mid-scale clumps, then individual blades.
        let macro_n = fbm2(p.x * 0.12, p.y * 0.12, 3) - 0.5;
        let mid = vnoise(p.x * 1.7, p.y * 1.7, 5) - 0.5;
        let blade = hash2(x as i32, y as i32, 9) - 0.5;
        let streak = hash2(x as i32, (y / 3) as i32, 13) - 0.5;
        col *= 1.0 + macro_n * 0.12 + mid * 0.07;
        col.y *= 1.0 + blade * 0.16 + streak * 0.06;
        col.x *= 1.0 + blade * 0.08 + (hash2(x as i32, y as i32, 21) - 0.5) * 0.1;
        // Worn turf: duller, browner, a touch lighter.
        col = col.lerp(Vec3::new(122.0, 118.0, 74.0) * (0.9 + blade * 0.3), wear * 0.7);
        // Paint sits on top of the grass and lets some texture through.
        let pa = paint[(y * w + x) as usize];
        let paint_col = Vec3::splat(232.0) * (0.95 + blade * 0.08);
        col = col.lerp(paint_col, pa * 0.94);
        [col.x.clamp(0.0, 255.0) as u8, col.y.clamp(0.0, 255.0) as u8, col.z.clamp(0.0, 255.0) as u8, 255]
    });
    let normal = par_fill(w, h, |x, y| {
        let (_, s, c, runoff, wear) = surface(x, y);
        let pa = paint[(y * w + x) as usize];
        let flat = (1.0 - runoff * 0.6) * (1.0 - wear * 0.7);
        // Mown blades lean with the mower: the stripes tilt along v, the cross-cut along u.
        let mut t = Vec2::new(c * 0.05, s * 0.14) * flat;
        let jitter = Vec2::new(hash2(x as i32, y as i32, 31) - 0.5, hash2(x as i32, y as i32, 37) - 0.5);
        t += jitter * 0.55 * (1.0 - pa * 0.6);
        let n = Vec3::new(t.x, t.y, 1.0).normalize();
        [((n.x * 0.5 + 0.5) * 255.0) as u8, ((n.y * 0.5 + 0.5) * 255.0) as u8, ((n.z * 0.5 + 0.5) * 255.0) as u8, 255]
    });
    (mipped(w, h, albedo, false, false), mipped(w, h, normal, true, false))
}

/// Light concrete with faint weathering, tiled (stands, walls, tunnel).
pub fn concrete() -> Image {
    let n = 128u32;
    let data = par_fill(n, n, |x, y| {
        let v = 0.82 + vnoise(x as f32 / 9.0, y as f32 / 9.0, 41) * 0.12 + (hash2(x as i32, y as i32, 43) - 0.5) * 0.1;
        let g = (v * 150.0) as u8;
        [g, g, (g as f32 * 1.04).min(255.0) as u8, 255]
    });
    mipped(n, n, data, false, true)
}

/// A square goal-net mesh with transparent holes: 8 cells per tile.
pub fn net() -> Image {
    let (n, cells) = (128u32, 8u32);
    let cell = n / cells;
    let mut data = vec![0u8; (n * n * 4) as usize];
    for y in 0..n {
        for x in 0..n {
            let on = x % cell < 2 || y % cell < 2;
            let i = ((y * n + x) * 4) as usize;
            data[i..i + 4].copy_from_slice(&if on { [245, 245, 245, 255] } else { [0, 0, 0, 0] });
        }
    }
    image(n, n, data, true, true)
}

/// A continuous LED ribbon: each text gets a `seg_px`-wide panel and the strip repeats along u.
/// A faint dot grid gives it the look of an LED panel up close.
pub fn led_strip(texts: &[&str], seg_px: u32) -> Image {
    use super::font5x7::text_image;
    let h = 48u32;
    let w = seg_px * texts.len() as u32;
    let bgs: [[u8; 4]; 5] = [[12, 28, 96, 255], [160, 18, 30, 255], [8, 8, 12, 255], [0, 112, 70, 255], [230, 120, 0, 255]];
    let mut data = vec![0u8; (w * h * 4) as usize];
    for (k, text) in texts.iter().enumerate() {
        let bg = bgs[k % bgs.len()];
        let fg = if k % bgs.len() == 4 { [20, 20, 20, 255] } else if k % 2 == 0 { [255, 255, 255, 255] } else { [255, 222, 60, 255] };
        // Fit the text into the panel: the biggest integer scale that leaves some margin.
        let chars = text.chars().count().max(1) as u32;
        let scale = ((seg_px - 24) / (chars * 6)).min((h - 10) / 7).max(1);
        let t = text_image(text, fg, bg, scale, 0);
        let (tw, th) = (t.texture_descriptor.size.width, t.texture_descriptor.size.height);
        let td = t.data.as_deref().unwrap_or(&[]);
        let (ox, oy) = (k as u32 * seg_px + seg_px.saturating_sub(tw) / 2, h.saturating_sub(th) / 2);
        for y in 0..h {
            for x in 0..seg_px {
                let gx = k as u32 * seg_px + x;
                let mut px = bg;
                if gx >= ox && gx < ox + tw && y >= oy && y < oy + th {
                    let i = (((y - oy) * tw + (gx - ox)) * 4) as usize;
                    if i + 4 <= td.len() {
                        px.copy_from_slice(&td[i..i + 4]);
                    }
                }
                // Separator bar between panels.
                if x < 3 {
                    px = [0, 0, 0, 255];
                }
                let dim = if gx % 2 == 1 || y % 2 == 1 { 0.82 } else { 1.0 };
                let i = ((y * w + gx) * 4) as usize;
                data[i..i + 4].copy_from_slice(&[(px[0] as f32 * dim) as u8, (px[1] as f32 * dim) as u8, (px[2] as f32 * dim) as u8, 255]);
            }
        }
    }
    let mut img = mipped(w, h, data, false, true);
    if let ImageSampler::Descriptor(d) = &mut img.sampler {
        d.address_mode_v = ImageAddressMode::ClampToEdge;
    }
    img
}

/// The stadium big screen: centred lines of text on a dark panel, with a coloured header bar.
pub fn screen(lines: &[(&str, [u8; 4])], header: [u8; 4]) -> Image {
    use super::font5x7::text_image;
    let (w, h) = (352u32, 160u32);
    let bg = [6u8, 10, 24, 255];
    let mut data = vec![0u8; (w * h * 4) as usize];
    for y in 0..h {
        for x in 0..w {
            let i = ((y * w + x) * 4) as usize;
            let px = if y < 14 { header } else { bg };
            data[i..i + 4].copy_from_slice(&px);
        }
    }
    let n = lines.len().max(1) as u32;
    let band = (h - 20) / n;
    for (k, (text, fg)) in lines.iter().enumerate() {
        let chars = text.chars().count().max(1) as u32;
        let scale = ((w - 24) / (chars * 6)).min((band - 8) / 7).max(1);
        let t = text_image(text, *fg, bg, scale, 0);
        let (tw, th) = (t.texture_descriptor.size.width, t.texture_descriptor.size.height);
        let td = t.data.as_deref().unwrap_or(&[]);
        let ox = w.saturating_sub(tw) / 2;
        let oy = 18 + k as u32 * band + band.saturating_sub(th) / 2;
        for y in 0..th.min(h - oy) {
            for x in 0..tw.min(w - ox) {
                let si = ((y * tw + x) * 4) as usize;
                let di = (((oy + y) * w + ox + x) * 4) as usize;
                if si + 4 <= td.len() {
                    data[di..di + 4].copy_from_slice(&td[si..si + 4]);
                }
            }
        }
    }
    image(w, h, data, false, true)
}

/// Equirectangular ball skin: white with dark panels.
pub fn ball() -> Image {
    let (w, h) = (256u32, 128u32);
    let mut data = vec![255u8; (w * h * 4) as usize];
    // Panel centres spread roughly evenly over the sphere.
    let mut centres = vec![Vec3::Y, -Vec3::Y];
    for k in 0..5 {
        let a = k as f32 * std::f32::consts::TAU / 5.0;
        centres.push(Vec3::new(a.cos() * 0.89, 0.45, a.sin() * 0.89));
        let b = a + std::f32::consts::PI / 5.0;
        centres.push(Vec3::new(b.cos() * 0.89, -0.45, b.sin() * 0.89));
    }
    for y in 0..h {
        for x in 0..w {
            let lon = x as f32 / w as f32 * std::f32::consts::TAU;
            let lat = (0.5 - y as f32 / h as f32) * std::f32::consts::PI;
            let p = Vec3::new(lat.cos() * lon.cos(), lat.sin(), lat.cos() * lon.sin());
            let near = centres.iter().map(|c| p.dot(c.normalize())).fold(-1.0, f32::max);
            let i = ((y * w + x) * 4) as usize;
            let px = if near > 0.93 {
                [24, 24, 30]
            } else if near > 0.915 {
                [120, 120, 130]
            } else {
                [248, 248, 248]
            };
            data[i..i + 4].copy_from_slice(&[px[0], px[1], px[2], 255]);
        }
    }
    image(w, h, data, true, true)
}

/// An original crest design in the club colours, drawn at `size`² with 3x supersampling.
pub fn crest(style: Crest, primary: u32, secondary: u32, size: u32) -> Image {
    let ss = 3u32;
    let n = size * ss;
    let p = rgb(primary);
    let q = rgb(secondary);
    // Pick a dark or light outline depending on the colours, so crests read on any background.
    let lum = |c: [u8; 3]| c[0] as f32 * 0.3 + c[1] as f32 * 0.59 + c[2] as f32 * 0.11;
    let outline = if lum(p).min(lum(q)) > 150.0 { [30, 30, 40] } else { [245, 245, 245] };

    let sample = |u: f32, v: f32| -> Option<[u8; 3]> {
        // u, v in -1..1, v down.
        let (inside, edge) = match style {
            Crest::Roundel => {
                let r = (u * u + v * v).sqrt();
                (r <= 1.0, r > 0.86)
            }
            _ => {
                let top = -0.92;
                let shoulder = 0.25;
                let inside = if v < top || u.abs() > 0.86 {
                    false
                } else if v < shoulder {
                    true
                } else {
                    // Taper to a point at v = 1.
                    u.abs() <= 0.86 * (1.0 - (v - shoulder) / (1.0 - shoulder)).max(0.0).sqrt()
                };
                let edge = inside && {
                    let d = 0.12;
                    let inner = if v < top + d || u.abs() > 0.86 - d {
                        false
                    } else if v < shoulder {
                        true
                    } else {
                        u.abs() <= (0.86 - d) * (1.0 - (v - shoulder) / (1.0 - shoulder - d * 0.6)).max(0.0).sqrt()
                    };
                    !inner
                };
                (inside, edge)
            }
        };
        if !inside {
            return None;
        }
        if edge {
            return Some(outline);
        }
        let body = match style {
            Crest::Stripes => ((u + 1.0) * 3.5) as i32 % 2 == 1,
            Crest::Hoops => ((v + 1.0) * 3.0) as i32 % 2 == 1,
            Crest::Halves => u > 0.0,
            Crest::Chevron => {
                let y = -0.35 + u.abs() * -0.55;
                v > y + 0.55 && v < y + 0.85
            }
            Crest::Sash => (u - v).abs() < 0.28,
            Crest::Roundel => (u * u + v * v).sqrt() > 0.62,
            Crest::Shield => v > 0.15,
        };
        Some(if body { q } else { p })
    };

    let mut data = vec![0u8; (size * size * 4) as usize];
    for y in 0..size {
        for x in 0..size {
            let (mut acc, mut hits) = ([0u32; 3], 0u32);
            for sy in 0..ss {
                for sx in 0..ss {
                    let u = ((x * ss + sx) as f32 + 0.5) / n as f32 * 2.0 - 1.0;
                    let v = ((y * ss + sy) as f32 + 0.5) / n as f32 * 2.0 - 1.0;
                    if let Some(c) = sample(u * 1.04, v * 1.04) {
                        acc[0] += c[0] as u32;
                        acc[1] += c[1] as u32;
                        acc[2] += c[2] as u32;
                        hits += 1;
                    }
                }
            }
            let i = ((y * size + x) * 4) as usize;
            if hits > 0 {
                data[i] = (acc[0] / hits) as u8;
                data[i + 1] = (acc[1] / hits) as u8;
                data[i + 2] = (acc[2] / hits) as u8;
                data[i + 3] = (hits * 255 / (ss * ss)) as u8;
            }
        }
    }
    image(size, size, data, false, true)
}

/// A shirt texture for the torso capsule (u wraps around the body, v runs top to bottom), patterned
/// after the club's crest style so kits read at a distance.
pub fn kit(style: Crest, main: u32, accent: u32) -> Image {
    let (w, h) = (64u32, 64u32);
    let (a, b) = (rgb(main), rgb(accent));
    let mut data = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        for x in 0..w {
            let (u, v) = (x as f32 / w as f32, y as f32 / h as f32);
            let accent = match style {
                Crest::Stripes => ((u * 12.0) as u32) % 2 == 1,
                Crest::Hoops => (0.25..0.8).contains(&v) && ((v * 10.0) as u32) % 2 == 1,
                Crest::Halves => u < 0.5,
                Crest::Sash => ((u * 2.0 + v) % 1.0) < 0.18,
                Crest::Chevron => (v - 0.35 - (u * 2.0 % 1.0 - 0.5).abs() * 0.3).abs() < 0.05,
                Crest::Roundel | Crest::Shield => false,
            };
            // Collar and hem trims in the accent colour.
            let trim = v < 0.06 || v > 0.94;
            let c = if accent || trim { b } else { a };
            data.extend_from_slice(&[c[0], c[1], c[2], 255]);
        }
    }
    image(w, h, data, true, true)
}
