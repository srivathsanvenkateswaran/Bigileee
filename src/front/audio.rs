//! Sound effects synthesized at startup, so the game ships without audio files.

use macroquad::audio::{load_sound_from_bytes, play_sound, stop_sound, PlaySoundParams, Sound};
use std::f32::consts::TAU;

const SR: u32 = 44_100;

pub struct Sfx {
    kick: Option<Sound>,
    whistle: Option<Sound>,
    whistle_long: Option<Sound>,
    cheer: Option<Sound>,
    crowd: Option<Sound>,
    tackle: Option<Sound>,
    post: Option<Sound>,
}

struct Noise(u32);

impl Noise {
    fn next(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        (self.0 as f32 / u32::MAX as f32) * 2.0 - 1.0
    }
}

fn wav(samples: &[f32]) -> Vec<u8> {
    let data_len = (samples.len() * 2) as u32;
    let mut v = Vec::with_capacity(44 + data_len as usize);
    v.extend_from_slice(b"RIFF");
    v.extend_from_slice(&(36 + data_len).to_le_bytes());
    v.extend_from_slice(b"WAVEfmt ");
    v.extend_from_slice(&16u32.to_le_bytes());
    v.extend_from_slice(&1u16.to_le_bytes());
    v.extend_from_slice(&1u16.to_le_bytes());
    v.extend_from_slice(&SR.to_le_bytes());
    v.extend_from_slice(&(SR * 2).to_le_bytes());
    v.extend_from_slice(&2u16.to_le_bytes());
    v.extend_from_slice(&16u16.to_le_bytes());
    v.extend_from_slice(b"data");
    v.extend_from_slice(&data_len.to_le_bytes());
    for s in samples {
        v.extend_from_slice(&((s.clamp(-1.0, 1.0) * 32767.0) as i16).to_le_bytes());
    }
    v
}

fn render(secs: f32, mut f: impl FnMut(f32) -> f32) -> Vec<f32> {
    let n = (secs * SR as f32) as usize;
    (0..n).map(|i| f(i as f32 / SR as f32)).collect()
}

fn kick() -> Vec<f32> {
    let mut noise = Noise(7);
    let mut phase = 0.0;
    render(0.14, |t| {
        let freq = 60.0 + 140.0 * (-t * 45.0).exp();
        phase += TAU * freq / SR as f32;
        phase.sin() * (-t * 32.0).exp() * 0.9 + noise.next() * (-t * 220.0).exp() * 0.5
    })
}

fn whistle_blast(len: f32) -> impl FnMut(f32) -> f32 {
    let mut noise = Noise(11);
    let mut phase = 0.0;
    move |t| {
        let freq = 2750.0 + 140.0 * (TAU * 32.0 * t).sin();
        phase += TAU * freq / SR as f32;
        let env = (t / 0.02).min(1.0) * ((len - t) / 0.05).clamp(0.0, 1.0);
        (phase.sin() * 0.35 + noise.next() * 0.05) * env
    }
}

fn whistle() -> Vec<f32> {
    render(0.38, whistle_blast(0.38))
}

fn whistle_long() -> Vec<f32> {
    let mut out = Vec::new();
    for (len, gap) in [(0.32, 0.16), (0.32, 0.16), (0.95, 0.0)] {
        out.extend(render(len, whistle_blast(len)));
        out.extend(std::iter::repeat(0.0).take((gap * SR as f32) as usize));
    }
    out
}

fn crowd_noise(seed: u32, secs: f32, mut env: impl FnMut(f32) -> f32) -> Vec<f32> {
    let mut noise = Noise(seed);
    let (mut lo, mut mid, mut brown) = (0.0f32, 0.0f32, 0.0f32);
    render(secs, |t| {
        let w = noise.next();
        lo += (w - lo) * 0.02;
        mid += (w - mid) * 0.18;
        brown = (brown + w * 0.02) * 0.995;
        let chatter = (mid - lo) * (0.6 + 0.4 * (TAU * 3.1 * t).sin().abs());
        (lo * 2.2 + chatter * 0.9 + brown * 1.5) * env(t)
    })
}

fn crowd_loop() -> Vec<f32> {
    let secs = 6.0;
    let fade = (0.6 * SR as f32) as usize;
    let mut s = crowd_noise(23, secs, |t| 0.5 + 0.12 * (TAU * 0.35 * t).sin());
    // Crossfade the tail into the head so the loop point is seamless.
    let n = s.len();
    for i in 0..fade {
        let a = i as f32 / fade as f32;
        s[i] = s[i] * a + s[n - fade + i] * (1.0 - a);
    }
    s.truncate(n - fade);
    s
}

fn cheer() -> Vec<f32> {
    crowd_noise(41, 4.0, |t| {
        let attack = (t / 0.25).min(1.0);
        let decay = if t < 1.6 { 1.0 } else { (-(t - 1.6) * 1.3).exp() };
        attack * decay * 1.4
    })
}

fn tackle() -> Vec<f32> {
    let mut noise = Noise(5);
    let mut lp = 0.0;
    render(0.18, |t| {
        lp += (noise.next() - lp) * 0.3;
        lp * (-t * 22.0).exp() * 1.6
    })
}

fn post() -> Vec<f32> {
    render(0.7, |t| {
        let env = (-t * 7.0).exp();
        ((TAU * 880.0 * t).sin() * 0.5 + (TAU * 1340.0 * t).sin() * 0.3 + (TAU * 2210.0 * t).sin() * 0.2)
            * env
            * 0.6
    })
}

async fn load(samples: Vec<f32>) -> Option<Sound> {
    load_sound_from_bytes(&wav(&samples)).await.ok()
}

impl Sfx {
    /// No audio at all, for headless simulation.
    #[cfg(test)]
    pub fn silent() -> Sfx {
        Sfx { kick: None, whistle: None, whistle_long: None, cheer: None, crowd: None, tackle: None, post: None }
    }

    pub async fn load() -> Sfx {
        Sfx {
            kick: load(kick()).await,
            whistle: load(whistle()).await,
            whistle_long: load(whistle_long()).await,
            cheer: load(cheer()).await,
            crowd: load(crowd_loop()).await,
            tackle: load(tackle()).await,
            post: load(post()).await,
        }
    }

    fn play(sound: &Option<Sound>, volume: f32) {
        if let Some(s) = sound {
            play_sound(s, PlaySoundParams { looped: false, volume });
        }
    }

    pub fn kick(&self, power: f32) {
        Self::play(&self.kick, (0.25 + power * 0.6).min(1.0));
    }
    pub fn whistle(&self) {
        Self::play(&self.whistle, 0.6);
    }
    pub fn whistle_long(&self) {
        Self::play(&self.whistle_long, 0.6);
    }
    pub fn cheer(&self) {
        Self::play(&self.cheer, 0.8);
    }
    pub fn tackle(&self) {
        Self::play(&self.tackle, 0.6);
    }
    pub fn post(&self) {
        Self::play(&self.post, 0.7);
    }

    pub fn crowd(&self, on: bool) {
        if let Some(s) = &self.crowd {
            if on {
                play_sound(s, PlaySoundParams { looped: true, volume: 0.35 });
            } else {
                stop_sound(s);
            }
        }
    }
}
