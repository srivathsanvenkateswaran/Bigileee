//! Keyboard and gamepad bindings. Directions are made camera-relative, so "up" always pushes
//! a player away from the screen whichever camera is active.

use super::camera::MainCam;
use crate::sim::input::Input;
use bevy::input::gamepad::{Gamepad, GamepadButton};
use bevy::prelude::*;

struct Keys {
    up: &'static [KeyCode],
    down: &'static [KeyCode],
    left: &'static [KeyCode],
    right: &'static [KeyCode],
    sprint: &'static [KeyCode],
    pass: &'static [KeyCode],
    shoot: &'static [KeyCode],
    lob: &'static [KeyCode],
    through: &'static [KeyCode],
    switch: &'static [KeyCode],
    contain: &'static [KeyCode],
}

use KeyCode::*;
const SOLO: Keys = Keys {
    up: &[KeyW, ArrowUp],
    down: &[KeyS, ArrowDown],
    left: &[KeyA, ArrowLeft],
    right: &[KeyD, ArrowRight],
    sprint: &[ShiftLeft, ShiftRight],
    pass: &[KeyJ],
    shoot: &[KeyK, Space],
    lob: &[KeyL],
    through: &[KeyI],
    switch: &[KeyQ],
    contain: &[KeyE],
};
const P1: Keys = Keys {
    up: &[KeyW],
    down: &[KeyS],
    left: &[KeyA],
    right: &[KeyD],
    sprint: &[ShiftLeft],
    pass: &[KeyJ],
    shoot: &[KeyK],
    lob: &[KeyL],
    through: &[KeyI],
    switch: &[KeyQ],
    contain: &[KeyE],
};
const P2: Keys = Keys {
    up: &[ArrowUp],
    down: &[ArrowDown],
    left: &[ArrowLeft],
    right: &[ArrowRight],
    sprint: &[ShiftRight],
    pass: &[Comma],
    shoot: &[Period],
    lob: &[Slash],
    through: &[Semicolon],
    switch: &[KeyM],
    contain: &[KeyN],
};

pub fn read(
    humans: usize,
    keys: &ButtonInput<KeyCode>,
    pads: &[&Gamepad],
    cam: Option<&GlobalTransform>,
) -> Vec<Input> {
    // Screen axes projected onto the pitch: up = camera forward, right = camera right.
    let (fwd, right) = match cam {
        Some(tf) => {
            let f = tf.forward();
            let f = Vec2::new(f.x, f.z).normalize_or(Vec2::new(0.0, -1.0));
            (f, Vec2::new(-f.y, f.x))
        }
        None => (Vec2::new(0.0, -1.0), Vec2::new(1.0, 0.0)),
    };
    (0..humans)
        .map(|h| {
            let k = if humans == 1 { &SOLO } else if h == 0 { &P1 } else { &P2 };
            let down = |ks: &[KeyCode]| ks.iter().any(|&c| keys.pressed(c));
            let pressed = |ks: &[KeyCode]| ks.iter().any(|&c| keys.just_pressed(c));
            let released = |ks: &[KeyCode]| ks.iter().any(|&c| keys.just_released(c));
            let mut stick = Vec2::ZERO;
            if down(k.up) {
                stick.y += 1.0;
            }
            if down(k.down) {
                stick.y -= 1.0;
            }
            if down(k.left) {
                stick.x -= 1.0;
            }
            if down(k.right) {
                stick.x += 1.0;
            }
            let mut i = Input {
                sprint: down(k.sprint),
                pass: pressed(k.pass),
                through: pressed(k.through),
                lob: pressed(k.lob),
                switch: pressed(k.switch),
                contain: down(k.contain),
                shoot_pressed: pressed(k.shoot),
                shoot_held: down(k.shoot),
                shoot_released: released(k.shoot),
                ..default()
            };
            // FIFA-style pad layout: A pass, B shoot/tackle, Y through, X lob/slide, RT sprint, LB switch, LT jockey.
            if let Some(pad) = pads.get(h) {
                let ls = pad.left_stick();
                if ls.length() > 0.2 {
                    stick = ls;
                }
                if pad.dpad().length() > 0.5 {
                    stick = pad.dpad();
                }
                use GamepadButton as B;
                i.sprint |= pad.pressed(B::RightTrigger2) || pad.pressed(B::RightTrigger);
                i.pass |= pad.just_pressed(B::South);
                i.shoot_pressed |= pad.just_pressed(B::East);
                i.shoot_held |= pad.pressed(B::East);
                i.shoot_released |= pad.just_released(B::East);
                i.through |= pad.just_pressed(B::North);
                i.lob |= pad.just_pressed(B::West);
                i.switch |= pad.just_pressed(B::LeftTrigger);
                i.contain |= pad.pressed(B::LeftTrigger2);
            }
            let stick = stick.clamp_length_max(1.0);
            i.dir = (right * stick.x + fwd * stick.y).clamp_length_max(1.0);
            i
        })
        .collect()
}
