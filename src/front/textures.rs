//! Procedurally generated textures: grass, crowd, goal net, ball and club crests.

use crate::data::types::Crest;
use crate::sim::rng::gen_range;
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

/// Mown grass: alternating stripes with fine noise. One texel column per stripe band is enough;
/// the noise adds texture up close.
pub fn grass(stripes: u32) -> Image {
    let (w, h) = (stripes * 64, 256);
    let mut data = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        for x in 0..w {
            let band = (x / 64) % 2;
            let n = gen_range(-9i32, 10);
            let (r, g, b) = if band == 0 { (52, 118, 48) } else { (62, 134, 56) };
            let wave = (((y as f32) * 0.21).sin() * 2.0) as i32;
            data.extend_from_slice(&[
                (r + n / 2) as u8,
                (g + n + wave).clamp(0, 255) as u8,
                (b + n / 2) as u8,
                255,
            ]);
        }
    }
    image(w, h, data, false, true)
}

/// Rows of spectators in assorted shirts, tiled along the stands.
pub fn crowd(home: u32, away: u32) -> Image {
    let (w, h) = (256u32, 128u32);
    let palette = [
        rgb(home),
        rgb(home),
        rgb(home),
        rgb(away),
        [235, 235, 235],
        [30, 30, 36],
        [90, 90, 100],
        [170, 60, 50],
        [60, 80, 160],
        [200, 170, 70],
    ];
    let mut data = vec![0u8; (w * h * 4) as usize];
    let (cw, ch) = (4u32, 6u32);
    for cy in 0..h / ch {
        for cx in 0..w / cw {
            let shirt = palette[gen_range(0, palette.len())];
            let skin = [[224, 172, 140], [190, 135, 95], [140, 95, 60], [95, 62, 40]][gen_range(0, 4)];
            let empty = gen_range(0.0, 1.0) < 0.06;
            for dy in 0..ch {
                for dx in 0..cw {
                    let i = (((cy * ch + dy) * w + cx * cw + dx) * 4) as usize;
                    let px = if empty {
                        [45, 45, 52]
                    } else if dy < 2 && (1..3).contains(&dx) {
                        skin
                    } else if dy >= 2 && dx < 4 {
                        shirt
                    } else {
                        [40, 40, 46]
                    };
                    data[i..i + 4].copy_from_slice(&[px[0], px[1], px[2], 255]);
                }
            }
        }
    }
    image(w, h, data, true, false)
}

/// A square mesh with transparent holes, for goal nets.
pub fn net() -> Image {
    let n = 64u32;
    let mut data = vec![0u8; (n * n * 4) as usize];
    for y in 0..n {
        for x in 0..n {
            let on = x % 16 < 2 || y % 16 < 2;
            let i = ((y * n + x) * 4) as usize;
            data[i..i + 4].copy_from_slice(&if on { [245, 245, 245, 255] } else { [0, 0, 0, 0] });
        }
    }
    image(n, n, data, true, true)
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
