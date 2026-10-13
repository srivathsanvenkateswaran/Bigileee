//! Bigileee 22: a parody 3D football game for macOS.

mod data;
mod front;
mod sim;

use bevy::prelude::*;
use bevy::window::{PresentMode, WindowResolution};

fn main() {
    sim::rng::srand(
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(1),
    );
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Bigileee 22".into(),
                resolution: WindowResolution::new(1440.0, 900.0),
                present_mode: PresentMode::AutoVsync,
                ..default()
            }),
            ..default()
        }))
        .add_plugins(front::GamePlugin)
        .run();
}
