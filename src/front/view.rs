//! A render-side snapshot of the match. Visuals read only this, so the same code draws live
//! play and replays.

use crate::sim::game::{Match, Phase, Role};
use bevy::prelude::*;
use std::collections::VecDeque;

#[derive(Clone, Default)]
pub struct PlayerView {
    pub pos: Vec2,
    pub vel: Vec2,
    pub facing: Vec2,
    pub kick: f32,
    pub slide: f32,
    pub dive: f32,
    pub stun: f32,
    pub holding: bool,
    pub celebrating: bool,
    pub sent_off: bool,
    pub has_ball: bool,
    /// Taking a throw-in (ball held overhead).
    pub throw_in: bool,
    pub keeper: bool,
}

#[derive(Clone, Default)]
pub struct BallView {
    pub pos: Vec2,
    pub z: f32,
    pub vel: Vec2,
}

#[derive(Resource, Clone, Default)]
pub struct View {
    pub players: Vec<PlayerView>,
    pub ball: BallView,
    /// Player index each human controls, if any.
    pub controlled: Vec<Option<usize>>,
    pub replay: bool,
    /// Scorer, celebration routine and seconds into it, while one is playing.
    pub celebration: Option<(usize, usize, f32)>,
}

impl View {
    pub fn from_match(m: &Match) -> View {
        let celebrating = match m.phase {
            Phase::Goal { .. } => Some(m.ball.touch_team),
            _ => None,
        };
        View {
            players: m
                .players
                .iter()
                .enumerate()
                .map(|(i, p)| PlayerView {
                    pos: p.pos,
                    vel: p.vel,
                    facing: p.facing,
                    kick: p.kick_anim,
                    slide: p.slide,
                    dive: p.dive,
                    stun: p.stun,
                    holding: p.role == Role::Gk && m.ball.owner == Some(i),
                    celebrating: celebrating == Some(p.team) && p.role != Role::Gk,
                    sent_off: p.sent_off,
                    has_ball: m.ball.owner == Some(i),
                    throw_in: m.lock.is_some_and(|l| l.taker == i && l.kind == crate::sim::game::Restart::ThrowIn),
                    keeper: p.role == Role::Gk,
                })
                .collect(),
            ball: BallView { pos: m.ball.pos, z: m.ball.z, vel: m.ball.vel },
            controlled: m.humans.iter().map(|h| Some(h.player)).collect(),
            replay: false,
            celebration: None,
        }
    }
}

/// Rolling recording of recent frames, and goal-replay playback.
#[derive(Resource, Default)]
pub struct Replay {
    frames: VecDeque<(f32, View)>,
    /// Seconds until the replay of the latest goal starts.
    pub armed: Option<f32>,
    playing: Option<(Vec<(f32, View)>, f32)>,
}

const KEEP_SECS: f32 = 7.0;
const REPLAY_SPEED: f32 = 0.7;

impl Replay {
    pub fn record(&mut self, dt: f32, v: &View) {
        let now = self.frames.back().map_or(0.0, |f| f.0) + dt;
        self.frames.push_back((now, v.clone()));
        while self.frames.front().is_some_and(|f| now - f.0 > KEEP_SECS) {
            self.frames.pop_front();
        }
    }

    pub fn start(&mut self) {
        let frames: Vec<(f32, View)> = self.frames.iter().cloned().collect();
        if let Some(t0) = frames.first().map(|f| f.0) {
            self.playing = Some((frames, t0));
        }
        self.armed = None;
    }

    pub fn is_playing(&self) -> bool {
        self.playing.is_some()
    }

    pub fn stop(&mut self) {
        self.playing = None;
    }

    /// Advances playback and returns the frame to show, or `None` once finished.
    pub fn advance(&mut self, dt: f32) -> Option<View> {
        let (frames, t) = self.playing.as_mut()?;
        *t += dt * REPLAY_SPEED;
        let idx = frames.partition_point(|f| f.0 < *t);
        if idx >= frames.len() {
            self.playing = None;
            return None;
        }
        let mut v = frames[idx].1.clone();
        v.replay = true;
        v.controlled = vec![None; v.controlled.len()];
        Some(v)
    }
}
