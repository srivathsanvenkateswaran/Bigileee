//! One controller's intent for a frame, independent of keyboard or gamepad.

use bevy::math::Vec2;

#[derive(Clone, Copy, Default, Debug)]
pub struct Input {
    /// Movement, in pitch space (x towards the right-hand goal, y towards the near touchline).
    pub dir: Vec2,
    pub sprint: bool,
    pub pass: bool,
    pub through: bool,
    pub lob: bool,
    pub switch: bool,
    pub contain: bool,
    pub shoot_pressed: bool,
    pub shoot_held: bool,
    pub shoot_released: bool,
}

impl Input {
    /// The same input with one-shot presses cleared, for physics sub-steps after the first.
    pub fn held_only(&self) -> Input {
        Input {
            dir: self.dir,
            sprint: self.sprint,
            contain: self.contain,
            shoot_held: self.shoot_held,
            ..Default::default()
        }
    }
}
