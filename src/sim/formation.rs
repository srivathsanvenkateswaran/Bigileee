//! Formation shapes. Positions are in a team's own frame: attacking towards +x, left wing at -y.

use crate::data::types::Pos;
use bevy::math::{vec2, Vec2};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Role {
    Gk,
    Def,
    Mid,
    Fwd,
}

#[derive(Clone, Copy, Debug)]
pub struct Slot {
    pub role: Role,
    pub at: Vec2,
    pub pos: Pos,
}

const fn s(role: Role, x: f32, y: f32, pos: Pos) -> Slot {
    Slot { role, at: vec2(x, y), pos }
}

pub const NAMES: [&str; 6] = ["4-4-2", "4-3-3", "4-2-3-1", "3-5-2", "5-3-2", "4-1-2-1-2"];

/// Eleven slots, goalkeeper first. Slot 10 is always the centre-forward who takes kick-offs.
pub fn slots(name: &str) -> [Slot; 11] {
    use Pos::*;
    use Role::*;
    let gk = s(Gk, -50.0, 0.0, GK);
    let back4 = [s(Def, -30.0, -22.0, LB), s(Def, -32.0, -7.5, CB), s(Def, -32.0, 7.5, CB), s(Def, -30.0, 22.0, RB)];
    match name {
        "4-3-3" => [
            gk, back4[0], back4[1], back4[2], back4[3],
            s(Mid, -21.0, 0.0, CDM), s(Mid, -12.0, -12.0, CM), s(Mid, -12.0, 12.0, CM),
            s(Fwd, 0.0, -21.0, LW), s(Fwd, 0.0, 21.0, RW), s(Fwd, 3.0, 0.0, ST),
        ],
        "4-2-3-1" => [
            gk, back4[0], back4[1], back4[2], back4[3],
            s(Mid, -20.0, -7.0, CDM), s(Mid, -20.0, 7.0, CDM),
            s(Mid, -5.0, -22.0, LM), s(Mid, -6.0, 0.0, CAM), s(Mid, -5.0, 22.0, RM),
            s(Fwd, 3.0, 0.0, ST),
        ],
        "3-5-2" => [
            gk, s(Def, -32.0, -13.0, CB), s(Def, -33.5, 0.0, CB), s(Def, -32.0, 13.0, CB),
            s(Mid, -14.0, -26.0, LWB), s(Mid, -15.0, -10.0, CM), s(Mid, -21.0, 0.0, CDM),
            s(Mid, -15.0, 10.0, CM), s(Mid, -14.0, 26.0, RWB),
            s(Fwd, 2.0, 7.0, ST), s(Fwd, 2.0, -7.0, ST),
        ],
        "5-3-2" => [
            gk, s(Def, -26.0, -26.0, LWB), s(Def, -33.0, -13.0, CB), s(Def, -34.0, 0.0, CB),
            s(Def, -33.0, 13.0, CB), s(Def, -26.0, 26.0, RWB),
            s(Mid, -15.0, -12.0, CM), s(Mid, -18.0, 0.0, CDM), s(Mid, -15.0, 12.0, CM),
            s(Fwd, 2.0, 7.0, ST), s(Fwd, 2.0, -7.0, ST),
        ],
        "4-1-2-1-2" => [
            gk, back4[0], back4[1], back4[2], back4[3],
            s(Mid, -21.0, 0.0, CDM), s(Mid, -14.0, -11.0, CM), s(Mid, -14.0, 11.0, CM),
            s(Mid, -5.0, 0.0, CAM), s(Fwd, 3.0, 6.0, ST), s(Fwd, 3.0, -6.0, ST),
        ],
        _ => [
            gk, back4[0], back4[1], back4[2], back4[3],
            s(Mid, -13.0, -24.0, LM), s(Mid, -15.0, -8.0, CM), s(Mid, -15.0, 8.0, CM),
            s(Mid, -13.0, 24.0, RM), s(Fwd, 2.0, 7.0, ST), s(Fwd, 2.0, -7.0, ST),
        ],
    }
}

/// How far a team pushes up, how hard it presses and how many risks it takes on the ball.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Mentality {
    UltraDefensive,
    Defensive,
    #[default]
    Balanced,
    Attacking,
    UltraAttacking,
}

impl Mentality {
    pub const ALL: [Mentality; 5] =
        [Mentality::UltraDefensive, Mentality::Defensive, Mentality::Balanced, Mentality::Attacking, Mentality::UltraAttacking];

    pub fn label(self) -> &'static str {
        match self {
            Mentality::UltraDefensive => "Ultra Defensive",
            Mentality::Defensive => "Defensive",
            Mentality::Balanced => "Balanced",
            Mentality::Attacking => "Attacking",
            Mentality::UltraAttacking => "Ultra Attacking",
        }
    }

    /// -1 (everyone behind the ball) to +1 (all-out attack).
    pub fn push(self) -> f32 {
        match self {
            Mentality::UltraDefensive => -1.0,
            Mentality::Defensive => -0.5,
            Mentality::Balanced => 0.0,
            Mentality::Attacking => 0.5,
            Mentality::UltraAttacking => 1.0,
        }
    }
}
