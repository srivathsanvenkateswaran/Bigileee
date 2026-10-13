//! Match cameras: broadcast (default), high tele, behind-the-player "pro", and the replay angle.

use super::stadium::sim_to_world;
use super::view::View;
use super::AppState;
use crate::sim::consts::*;
use crate::sim::game::Match;
use bevy::core_pipeline::bloom::Bloom;
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::prelude::*;
use bevy::render::camera::PerspectiveProjection;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum CamMode {
    #[default]
    Broadcast,
    Tele,
    Pro,
}

impl CamMode {
    pub const ALL: [CamMode; 3] = [CamMode::Broadcast, CamMode::Tele, CamMode::Pro];
    pub fn label(self) -> &'static str {
        match self {
            CamMode::Broadcast => "Broadcast",
            CamMode::Tele => "Tele Broadcast",
            CamMode::Pro => "Pro (behind player)",
        }
    }
}

#[derive(Component)]
pub struct MainCam {
    focus: Vec3,
    eye: Vec3,
}

pub fn spawn_camera(commands: &mut Commands) {
    let eye = Vec3::new(0.0, 24.0, 50.0);
    commands.spawn((
        Camera3d::default(),
        Camera { hdr: true, ..default() },
        Projection::Perspective(PerspectiveProjection { fov: 30f32.to_radians(), far: 600.0, ..default() }),
        Tonemapping::TonyMcMapface,
        Bloom::NATURAL,
        Msaa::Sample4,
        Transform::from_translation(eye).looking_at(Vec3::ZERO, Vec3::Y),
        MainCam { focus: Vec3::ZERO, eye },
        StateScoped(AppState::Match),
    ));
}

pub fn update_camera(
    view: Res<View>,
    mode: Res<super::Settings>,
    m: Option<Res<super::MatchRes>>,
    time: Res<Time>,
    mut q: Query<(&mut MainCam, &mut Transform, &mut Projection)>,
) {
    let Ok((mut cam, mut tf, mut proj)) = q.single_mut() else { return };
    let dt = time.delta_secs();
    let ball = view.ball.pos + view.ball.vel * 0.3;
    let (focus, eye, fov): (Vec3, Vec3, f32) = if view.replay {
        let side = view.ball.pos.x.signum();
        let focus = sim_to_world(view.ball.pos, view.ball.z * 0.5);
        let eye = Vec3::new(side * (HALF_L - 14.0), 3.5, view.ball.pos.y * 0.5 + 16.0);
        (focus, eye, 34.0)
    } else {
        match mode.camera {
            CamMode::Broadcast | CamMode::Tele => {
                let (h, back, fov) = if mode.camera == CamMode::Tele { (36.0, 68.0, 27.0) } else { (23.0, 48.0, 31.0) };
                let fx = ball.x.clamp(-HALF_L + 14.0, HALF_L - 14.0);
                let fz = ball.y * 0.45 + 2.0;
                (Vec3::new(fx, 0.0, fz), Vec3::new(fx * 0.9, h, back + fz * 0.25), fov)
            }
            CamMode::Pro => {
                let (p, attack) = match (&m, view.controlled.first().copied().flatten()) {
                    (Some(m), Some(i)) => (view.players[i].pos, m.m.dir(m.m.players[i].team)),
                    _ => (view.ball.pos, 1.0),
                };
                let a = Vec2::new(attack, 0.0);
                (sim_to_world(p + a * 10.0, 0.0), sim_to_world(p - a * 13.0, 6.5), 50.0)
            }
        }
    };
    let k = 1.0 - (-dt * if view.replay { 6.0 } else { 3.2 }).exp();
    cam.focus = cam.focus.lerp(focus, k);
    cam.eye = cam.eye.lerp(eye, k);
    *tf = Transform::from_translation(cam.eye).looking_at(cam.focus, Vec3::Y);
    if let Projection::Perspective(p) = &mut *proj {
        p.fov += (fov.to_radians() - p.fov) * k;
    }
}
