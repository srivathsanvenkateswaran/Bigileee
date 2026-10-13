//! Sound effects synthesised at startup, so the game ships without audio files.

use std::f32::consts::TAU;

const SR: u32 = 44_100;

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

/// Crowd babble: many synthetic voices, each a buzzing "vowel" (a pitched sawtooth plus breath
/// through a resonant formant filter) chopped into syllables, summed and gently filtered. This
/// reads as a murmuring crowd rather than static.
fn crowd_noise(seed: u32, secs: f32, voices: usize, excitement: f32, mut env: impl FnMut(f32) -> f32) -> Vec<f32> {
    let n = (secs * SR as f32) as usize;
    let mut out = vec![0.0f32; n];
    let mut rng = Noise(seed);
    let mut uniform = |lo: f32, hi: f32| lo + (hi - lo) * (rng.next() * 0.5 + 0.5);
    for _ in 0..voices {
        let pitch = uniform(95.0, 230.0) * (1.0 + excitement * 0.35);
        let formant = uniform(380.0, 1100.0) * (1.0 + excitement * 0.25);
        let rate = uniform(2.5, 5.5);
        let phase = uniform(0.0, TAU);
        let drift = uniform(0.08, 0.3);
        let gain = uniform(0.5, 1.0);
        // State-variable band-pass around the formant.
        let f = 2.0 * (std::f32::consts::PI * formant / SR as f32).sin();
        let q = 0.25;
        let (mut low, mut band) = (0.0f32, 0.0f32);
        let mut saw = uniform(0.0, 1.0);
        let mut breath = Noise(seed.wrapping_mul(31).wrapping_add((pitch * 10.0) as u32) | 1);
        for (i, o) in out.iter_mut().enumerate() {
            let t = i as f32 / SR as f32;
            saw = (saw + pitch * (1.0 + 0.02 * (TAU * 5.0 * t).sin()) / SR as f32).fract();
            let input = (saw * 2.0 - 1.0) * 0.6 + breath.next() * 0.4;
            let high = input - low - q * band;
            band += f * high;
            low += f * band;
            // Syllables, with longer pauses that come and go.
            let syl = (0.5 + 0.5 * (TAU * rate * t + phase).sin()).powf(2.0);
            let talk = (0.5 + 0.5 * (TAU * drift * t + phase * 1.7).sin()).powf(1.5);
            *o += band * syl * talk * gain;
        }
    }
    // Soften the top end, drop rumble, normalise, then shape.
    let (mut lp, mut hp_in, mut hp) = (0.0f32, 0.0f32, 0.0f32);
    for o in out.iter_mut() {
        lp += (*o - lp) * 0.35;
        hp = 0.995 * (hp + lp - hp_in);
        hp_in = lp;
        *o = hp;
    }
    let peak = out.iter().fold(1e-6f32, |m, v| m.max(v.abs()));
    for (i, o) in out.iter_mut().enumerate() {
        *o = *o / peak * 0.5 * env(i as f32 / SR as f32);
    }
    out
}

fn crowd_loop() -> Vec<f32> {
    let secs = 6.0;
    let fade = (0.6 * SR as f32) as usize;
    let mut s = crowd_noise(23, secs, 48, 0.0, |t| 0.85 + 0.15 * (TAU * 0.35 * t).sin());
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
    crowd_noise(41, 4.0, 90, 1.0, |t| {
        let attack = (t / 0.25).min(1.0);
        let decay = if t < 1.6 { 1.0 } else { (-(t - 1.6) * 1.3).exp() };
        attack * decay
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

fn ooh() -> Vec<f32> {
    // A crowd groan: the cheer texture with a falling, shorter envelope.
    crowd_noise(77, 2.2, 70, 0.4, |t| (t / 0.15).min(1.0) * (-(t * 1.4)).exp())
}

use crate::sim::events::SoundEvent;
use bevy::audio::Volume;
use bevy::prelude::*;

#[derive(Resource)]
pub struct Sounds {
    kick: Handle<AudioSource>,
    whistle: Handle<AudioSource>,
    whistle_long: Handle<AudioSource>,
    cheer: Handle<AudioSource>,
    pub crowd: Handle<AudioSource>,
    tackle: Handle<AudioSource>,
    post: Handle<AudioSource>,
    ooh: Handle<AudioSource>,
}

pub fn build_sounds(mut commands: Commands, mut sources: ResMut<Assets<AudioSource>>) {
    let mut add = |s: Vec<f32>| sources.add(AudioSource { bytes: wav(&s).into() });
    commands.insert_resource(Sounds {
        kick: add(kick()),
        whistle: add(whistle()),
        whistle_long: add(whistle_long()),
        cheer: add(cheer()),
        crowd: add(crowd_loop()),
        tackle: add(tackle()),
        post: add(post()),
        ooh: add(ooh()),
    });
}

/// Plays one simulation sound event as a fire-and-forget entity.
pub fn play(commands: &mut Commands, sounds: &Sounds, e: SoundEvent) {
    let (handle, volume) = match e {
        SoundEvent::Kick(p) => (&sounds.kick, 0.25 + p * 0.6),
        SoundEvent::Whistle => (&sounds.whistle, 0.6),
        SoundEvent::WhistleLong => (&sounds.whistle_long, 0.6),
        SoundEvent::Cheer => (&sounds.cheer, 0.55),
        SoundEvent::Tackle => (&sounds.tackle, 0.6),
        SoundEvent::Post => (&sounds.post, 0.7),
        SoundEvent::Ooh => (&sounds.ooh, 0.4),
    };
    commands.spawn((AudioPlayer::new(handle.clone()), PlaybackSettings::DESPAWN.with_volume(Volume::Linear(volume.min(1.0)))));
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The crowd bed stays quiet and isn't dominated by hiss.
    #[test]
    fn crowd_is_murmur_not_static() {
        let s = crowd_loop();
        let peak = s.iter().fold(0.0f32, |m, v| m.max(v.abs()));
        let rms = (s.iter().map(|v| v * v).sum::<f32>() / s.len() as f32).sqrt();
        // High-frequency share: energy of the first difference relative to the signal.
        let diff = (s.windows(2).map(|w| (w[1] - w[0]).powi(2)).sum::<f32>() / s.len() as f32).sqrt();
        println!("crowd peak {peak:.2} rms {rms:.3} hf ratio {:.3}", diff / rms);
        assert!(peak <= 0.55);
        assert!(diff / rms < 0.5, "too much hiss");
        std::fs::write("/tmp/bigileee_crowd.wav", wav(&s)).unwrap();
        std::fs::write("/tmp/bigileee_cheer.wav", wav(&cheer())).unwrap();
    }
}
