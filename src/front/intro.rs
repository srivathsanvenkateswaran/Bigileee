//! Pre-match presentation: the camera sweeps around the stadium while both line-ups are shown
//! and the commentator welcomes everyone. Skippable with Enter, Space or A.

use super::commentary::Commentary;
use super::hud::text;
use super::textures::{self, color};
use super::{AppState, Fonts, MatchRes};
use bevy::input::gamepad::{Gamepad, GamepadButton};
use bevy::prelude::*;

pub const INTRO_SECS: f32 = 8.0;

#[derive(Resource)]
pub struct Intro {
    pub t: f32,
    pub active: bool,
    spoke: bool,
}

#[derive(Component)]
pub struct IntroRoot;

/// Spawns the line-up overlay once the match resource exists.
pub fn spawn_intro(mut commands: Commands, res: Res<MatchRes>, fonts: Res<Fonts>, mut images: ResMut<Assets<Image>>) {
    let m = &res.m;
    commands.insert_resource(Intro { t: 0.0, active: true, spoke: false });
    let gold = Color::srgb(0.97, 0.78, 0.25);
    let dim = Color::srgba(1.0, 1.0, 1.0, 0.65);
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                padding: UiRect::axes(Val::Px(40.0), Val::Px(30.0)),
                ..default()
            },
            IntroRoot,
            StateScoped(AppState::Match),
        ))
        .with_children(|root| {
            for t in 0..2 {
                let team = &m.teams[t];
                let crest = images.add(textures::crest(team.crest, team.home, team.trim, 96));
                let slots = team.slots();
                root.spawn((
                    Node {
                        flex_direction: FlexDirection::Column,
                        align_items: AlignItems::Center,
                        row_gap: Val::Px(4.0),
                        padding: UiRect::all(Val::Px(18.0)),
                        width: Val::Px(330.0),
                        border: UiRect::top(Val::Px(5.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.03, 0.04, 0.09, 0.82)),
                    BorderColor(color(m.kit[t])),
                ))
                .with_children(|card| {
                    card.spawn((Node { width: Val::Px(72.0), height: Val::Px(72.0), ..default() }, ImageNode::new(crest)));
                    card.spawn(text(&fonts, team.name.clone(), 24.0, Color::WHITE));
                    card.spawn(text(&fonts, format!("{}  ·  {}", if t == 0 { "HOME" } else { "AWAY" }, team.formation), 15.0, dim));
                    card.spawn(Node { height: Val::Px(6.0), ..default() });
                    for k in 0..11 {
                        let p = &m.players[t * 11 + k];
                        card.spawn(Node { width: Val::Percent(100.0), justify_content: JustifyContent::SpaceBetween, ..default() })
                            .with_children(|row| {
                                row.spawn(text(&fonts, format!("{:?}", slots[k].pos), 14.0, dim));
                                row.spawn(text(&fonts, format!("{:>2}  {}", p.num, p.name), 15.0, Color::WHITE));
                            });
                    }
                });
                if t == 0 {
                    root.spawn((Node { flex_direction: FlexDirection::Column, align_items: AlignItems::Center, row_gap: Val::Px(8.0), ..default() },))
                        .with_children(|mid| {
                            mid.spawn(text(&fonts, "BIGILEEE 22", 44.0, Color::WHITE));
                            mid.spawn(text(&fonts, format!("{}  v  {}", m.teams[0].short, m.teams[1].short), 30.0, gold));
                            mid.spawn(text(&fonts, "Enter / A to skip", 16.0, dim));
                        });
                }
            }
        });
}

/// Advances the intro, handles skipping, and says the welcome line.
pub fn tick_intro(
    mut commands: Commands,
    intro: Option<ResMut<Intro>>,
    res: Option<Res<MatchRes>>,
    talk: Option<ResMut<Commentary>>,
    keys: Res<ButtonInput<KeyCode>>,
    pads: Query<&Gamepad>,
    time: Res<Time>,
    roots: Query<Entity, With<IntroRoot>>,
) {
    let Some(mut intro) = intro else { return };
    if !intro.active {
        return;
    }
    intro.t += time.delta_secs();
    if !intro.spoke {
        intro.spoke = true;
        if let (Some(mut talk), Some(res)) = (talk, res) {
            talk.welcome(&res.m.teams[0].name, &res.m.teams[1].name);
        }
    }
    let skip = keys.just_pressed(KeyCode::Enter)
        || keys.just_pressed(KeyCode::Space)
        || pads.iter().any(|p| p.just_pressed(GamepadButton::South) || p.just_pressed(GamepadButton::Start));
    if skip || intro.t >= INTRO_SECS {
        intro.active = false;
        for e in &roots {
            commands.entity(e).despawn();
        }
    }
}

/// Camera path for the intro: a slow, high orbit of the bowl that settles towards the
/// broadcast position. Returns (eye, focus, fov in degrees).
pub fn intro_camera(t: f32) -> (Vec3, Vec3, f32) {
    let s = (t / INTRO_SECS).clamp(0.0, 1.0);
    let angle = -0.9 + s * 2.4;
    let radius = 95.0 - s * 30.0;
    let height = 42.0 - s * 14.0;
    (Vec3::new(angle.sin() * radius, height, angle.cos() * radius), Vec3::new(0.0, 2.0, 0.0), 40.0)
}
