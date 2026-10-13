//! Keyboard bindings for one or two local players.

use macroquad::prelude::*;

#[derive(Clone, Copy, Default, Debug)]
pub struct Input {
    pub dir: Vec2,
    pub sprint: bool,
    pub pass: bool,
    pub through: bool,
    pub lob: bool,
    pub switch: bool,
    pub shoot_pressed: bool,
    pub shoot_held: bool,
    pub shoot_released: bool,
}

impl Input {
    /// The same input with one-shot presses cleared, for physics sub-steps after the first.
    pub fn held_only(&self) -> Input {
        Input { dir: self.dir, sprint: self.sprint, shoot_held: self.shoot_held, ..Default::default() }
    }
}

pub struct Controls {
    pub up: &'static [KeyCode],
    pub down: &'static [KeyCode],
    pub left: &'static [KeyCode],
    pub right: &'static [KeyCode],
    pub sprint: &'static [KeyCode],
    pub pass: &'static [KeyCode],
    pub shoot: &'static [KeyCode],
    pub lob: &'static [KeyCode],
    pub through: &'static [KeyCode],
    pub switch: &'static [KeyCode],
}

fn down(keys: &[KeyCode]) -> bool {
    keys.iter().any(|&k| is_key_down(k))
}
fn pressed(keys: &[KeyCode]) -> bool {
    keys.iter().any(|&k| is_key_pressed(k))
}
fn released(keys: &[KeyCode]) -> bool {
    keys.iter().any(|&k| is_key_released(k))
}

impl Controls {
    /// Single player: WASD or arrow keys both steer.
    pub fn solo() -> Controls {
        use KeyCode::*;
        Controls {
            up: &[W, Up],
            down: &[S, Down],
            left: &[A, Left],
            right: &[D, Right],
            sprint: &[LeftShift, RightShift],
            pass: &[J],
            shoot: &[K, Space],
            lob: &[L],
            through: &[I],
            switch: &[Q],
        }
    }

    pub fn p1() -> Controls {
        use KeyCode::*;
        Controls {
            up: &[W],
            down: &[S],
            left: &[A],
            right: &[D],
            sprint: &[LeftShift],
            pass: &[J],
            shoot: &[K],
            lob: &[L],
            through: &[I],
            switch: &[Q],
        }
    }

    pub fn p2() -> Controls {
        use KeyCode::*;
        Controls {
            up: &[Up],
            down: &[Down],
            left: &[Left],
            right: &[Right],
            sprint: &[RightShift],
            pass: &[Comma],
            shoot: &[Period],
            lob: &[Slash],
            through: &[Semicolon],
            switch: &[M],
        }
    }

    pub fn read(&self) -> Input {
        let mut dir = Vec2::ZERO;
        if down(self.up) {
            dir.y -= 1.0;
        }
        if down(self.down) {
            dir.y += 1.0;
        }
        if down(self.left) {
            dir.x -= 1.0;
        }
        if down(self.right) {
            dir.x += 1.0;
        }
        Input {
            dir: dir.normalize_or_zero(),
            sprint: down(self.sprint),
            pass: pressed(self.pass),
            through: pressed(self.through),
            lob: pressed(self.lob),
            switch: pressed(self.switch),
            shoot_pressed: pressed(self.shoot),
            shoot_held: down(self.shoot),
            shoot_released: released(self.shoot),
        }
    }
}
