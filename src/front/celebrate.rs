//! Goal celebrations. The scorer performs one of a set of original routines: a human scorer
//! picks it in the first moments after the goal, otherwise it's chosen at random.

use super::commentary::Commentary;
use super::view::View;
use super::MatchRes;
use crate::sim::game::Phase;
use crate::sim::rng::gen_range;
use bevy::input::gamepad::{Gamepad, GamepadButton};
use bevy::prelude::*;

pub const NAMES: [&str; 5] = ["Sambar Slide", "Dosa Flip", "Filter Kaapi Pour", "Biryani Bow", "Helicopter"];

#[derive(Resource, Default)]
pub struct Celebration {
    was_goal: bool,
    /// Scorer, routine, seconds since the goal, and whether the routine is locked in.
    pub active: Option<(usize, usize, f32, bool)>,
}

const CHOOSE_SECS: f32 = 1.4;

pub fn celebrations(
    mut c: ResMut<Celebration>,
    res: Option<Res<MatchRes>>,
    mut view: ResMut<View>,
    keys: Res<ButtonInput<KeyCode>>,
    pads: Query<&Gamepad>,
    talk: Option<ResMut<Commentary>>,
    time: Res<Time>,
) {
    let Some(res) = res else { return };
    let m = &res.m;
    let goal = matches!(m.phase, Phase::Goal { .. });
    if goal && !c.was_goal {
        // The scorer is the last kicker if they're on the scoring side.
        let team = m.ball.touch_team;
        let scorer = m.ball.kicker.filter(|&k| m.players[k].team == team);
        c.active = scorer.map(|s| (s, gen_range(0, NAMES.len()), 0.0, false));
    }
    c.was_goal = goal;
    if !goal {
        c.active = None;
    }
    let Some((scorer, mut kind, mut t, mut locked)) = c.active else { return };
    if res.paused {
        return;
    }
    t += time.delta_secs();

    let human_scored = m.humans.iter().any(|h| h.team == m.players[scorer].team);
    let mut talk = talk;
    if human_scored && !locked && t < CHOOSE_SECS {
        let picks = [
            (KeyCode::Digit1, GamepadButton::DPadUp),
            (KeyCode::Digit2, GamepadButton::DPadRight),
            (KeyCode::Digit3, GamepadButton::DPadDown),
            (KeyCode::Digit4, GamepadButton::DPadLeft),
            (KeyCode::Digit5, GamepadButton::RightTrigger),
        ];
        for (k, (key, button)) in picks.iter().enumerate() {
            if keys.just_pressed(*key) || pads.iter().any(|p| p.just_pressed(*button)) {
                kind = k;
                locked = true;
            }
        }
        if let Some(talk) = talk.as_mut() {
            if !locked {
                talk.hint = Some("Celebrate:  1 Sambar Slide   2 Dosa Flip   3 Filter Kaapi   4 Biryani Bow   5 Helicopter".into());
            }
        }
    }
    if (locked || t >= CHOOSE_SECS) && !c.active.is_some_and(|a| a.3) {
        locked = true;
        if let Some(talk) = talk.as_mut() {
            talk.hint = None;
            talk.celebration(&m.players[scorer].name, NAMES[kind]);
        }
    }
    c.active = Some((scorer, kind, t, locked));
    if !view.replay {
        view.celebration = Some((scorer, kind, t));
    }
}
