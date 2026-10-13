//! Pitch dimensions and physics constants. All distances are in metres.

pub const HALF_L: f32 = 52.5;
pub const HALF_W: f32 = 34.0;
pub const GOAL_HW: f32 = 3.66;
pub const GOAL_H: f32 = 2.44;
pub const GOAL_DEPTH: f32 = 2.2;
pub const BOX_D: f32 = 16.5;
pub const BOX_HW: f32 = 20.16;
pub const SIX_D: f32 = 5.5;
pub const SIX_HW: f32 = 9.16;
pub const PEN_SPOT: f32 = 11.0;
pub const CIRCLE_R: f32 = 9.15;

pub const G: f32 = 9.81;
/// Exponential velocity decay for a rolling ball (per second).
pub const GROUND_DRAG: f32 = 1.05;
/// Exponential velocity decay for a ball in the air (per second).
pub const AIR_DRAG: f32 = 0.25;

pub const PLAYER_R: f32 = 0.5;
pub const CONTROL_R: f32 = 1.0;
pub const PER_TEAM: usize = 11;
pub const PLAYERS: usize = PER_TEAM * 2;
