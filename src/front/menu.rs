//! Title screen and match setup (league, club, length, difficulty, camera), with team sheets.

use super::camera::CamMode;
use super::hud::text;
use super::textures::{self, color};
use super::{AppState, Flow, Fonts, Settings};
use crate::data;
use crate::sim::game::Mode;
use crate::sim::squad::TeamSetup;
use bevy::input::gamepad::{Gamepad, GamepadButton};
use bevy::prelude::*;

pub const HALVES: [(&str, f32); 5] = [("2 min", 120.0), ("3 min", 180.0), ("4 min", 240.0), ("6 min", 360.0), ("10 min", 600.0)];
pub const LEVELS: [(&str, f32); 5] =
    [("Beginner", 0.15), ("Amateur", 0.32), ("Semi-Pro", 0.5), ("World Class", 0.72), ("Legendary", 0.92)];

const GOLD: Color = Color::srgb(0.97, 0.78, 0.25);
const BG: Color = Color::srgb(0.03, 0.05, 0.1);

#[derive(Component)]
pub struct MenuRoot;

#[derive(Resource, Default)]
pub struct MenuState {
    pub row: usize,
    pub dirty: bool,
}

/// One frame of menu navigation from keyboard and the first gamepad.
struct Nav {
    up: bool,
    down: bool,
    left: bool,
    right: bool,
    ok: bool,
    back: bool,
}

fn nav(keys: &ButtonInput<KeyCode>, pads: &Query<&Gamepad>) -> Nav {
    use KeyCode::*;
    let k = |c: &[KeyCode]| c.iter().any(|&c| keys.just_pressed(c));
    let p = |b: GamepadButton| pads.iter().any(|g| g.just_pressed(b));
    Nav {
        up: k(&[ArrowUp, KeyW]) || p(GamepadButton::DPadUp),
        down: k(&[ArrowDown, KeyS]) || p(GamepadButton::DPadDown),
        left: k(&[ArrowLeft, KeyA]) || p(GamepadButton::DPadLeft),
        right: k(&[ArrowRight, KeyD]) || p(GamepadButton::DPadRight),
        ok: k(&[Enter, Space]) || p(GamepadButton::South) || p(GamepadButton::Start),
        back: k(&[Escape, Backspace]) || p(GamepadButton::East),
    }
}

// ---------- title ----------

fn title_items() -> Vec<&'static str> {
    let mut v = Vec::new();
    if super::career::save_path().exists() {
        v.push("Continue Career");
    }
    v.extend(["Kick Off  (vs CPU)", "Season", "Cup", "2 Players  (same Mac)", "Watch CPU vs CPU", "Quit"]);
    v
}

pub fn title_input(
    keys: Res<ButtonInput<KeyCode>>,
    pads: Query<&Gamepad>,
    mut menu: ResMut<MenuState>,
    mut settings: ResMut<Settings>,
    mut next: ResMut<NextState<AppState>>,
    mut exit: EventWriter<AppExit>,
    mut commands: Commands,
) {
    let n = nav(&keys, &pads);
    let items = title_items();
    let len = items.len();
    if n.up {
        menu.row = (menu.row + len - 1) % len;
        menu.dirty = true;
    }
    if n.down {
        menu.row = (menu.row + 1) % len;
        menu.dirty = true;
    }
    if n.ok {
        settings.flow = Flow::Friendly;
        settings.human_team = 0;
        settings.knockout = false;
        settings.career_fixture = None;
        match items[menu.row.min(len - 1)] {
            "Continue Career" => {
                if let Some(c) = super::career::Career::load() {
                    commands.insert_resource(c);
                    next.set(AppState::Hub);
                }
                return;
            }
            "Season" => {
                settings.flow = Flow::Season;
                settings.mode = Mode::VsCpu;
            }
            "Cup" => {
                settings.flow = Flow::Cup;
                settings.mode = Mode::VsCpu;
            }
            "2 Players  (same Mac)" => settings.mode = Mode::TwoPlayer,
            "Watch CPU vs CPU" => settings.mode = Mode::Watch,
            "Quit" => {
                exit.write(AppExit::Success);
                return;
            }
            _ => settings.mode = Mode::VsCpu,
        }
        menu.row = START_ROW;
        next.set(AppState::Setup);
    }
}

pub fn draw_title(mut commands: Commands, fonts: Res<Fonts>, mut menu: ResMut<MenuState>, old: Query<Entity, With<MenuRoot>>) {
    if !menu.dirty {
        return;
    }
    menu.dirty = false;
    for e in &old {
        commands.entity(e).despawn();
    }
    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                row_gap: Val::Px(14.0),
                ..default()
            },
            BackgroundColor(BG),
            MenuRoot,
            StateScoped(AppState::Title),
        ))
        .with_children(|c| {
            c.spawn(text(&fonts, "BIGILEEE 22", 104.0, Color::WHITE));
            c.spawn(text(&fonts, "The parody football game  ·  KaalPanthu engine", 24.0, GOLD));
            c.spawn(Node { height: Val::Px(40.0), ..default() });
            for (k, item) in title_items().iter().enumerate() {
                let sel = menu.row == k;
                c.spawn((
                    Node { width: Val::Px(420.0), padding: UiRect::axes(Val::Px(24.0), Val::Px(12.0)), justify_content: JustifyContent::Center, ..default() },
                    BackgroundColor(if sel { GOLD } else { Color::srgba(1.0, 1.0, 1.0, 0.06) }),
                ))
                .with_child(text(&fonts, *item, 28.0, if sel { Color::BLACK } else { Color::WHITE }));
            }
            c.spawn(Node { height: Val::Px(30.0), ..default() });
            c.spawn(text(&fonts, "Arrows / D-pad to choose  ·  Enter / A to select  ·  Xbox and PlayStation pads supported", 17.0, Color::srgba(1.0, 1.0, 1.0, 0.5)));
        });
}

// ---------- setup ----------

const ROWS: usize = 11;
pub const START_ROW: usize = ROWS - 1;

pub fn setup_input(
    mut commands: Commands,
    mut time_of_day: ResMut<super::stadium::TimeOfDay>,
    keys: Res<ButtonInput<KeyCode>>,
    pads: Query<&Gamepad>,
    mut menu: ResMut<MenuState>,
    mut settings: ResMut<Settings>,
    mut next: ResMut<NextState<AppState>>,
) {
    let n = nav(&keys, &pads);
    if n.back {
        menu.row = 0;
        next.set(AppState::Title);
        return;
    }
    let career = settings.flow != Flow::Friendly;
    let skip = |r: usize| career && (r == 2 || r == 3);
    if n.up {
        menu.row = (menu.row + ROWS - 1) % ROWS;
        while skip(menu.row) {
            menu.row = (menu.row + ROWS - 1) % ROWS;
        }
        menu.dirty = true;
    }
    if n.down {
        menu.row = (menu.row + 1) % ROWS;
        while skip(menu.row) {
            menu.row = (menu.row + 1) % ROWS;
        }
        menu.dirty = true;
    }
    let d: i32 = if n.right { 1 } else if n.left { -1 } else { 0 };
    if d != 0 {
        let leagues = data::leagues();
        let cycle = |v: usize, len: usize| (v as i32 + d).rem_euclid(len as i32) as usize;
        match menu.row {
            0 => {
                settings.home.0 = cycle(settings.home.0, leagues.len());
                settings.home.1 = 0;
            }
            1 => settings.home.1 = cycle(settings.home.1, leagues[settings.home.0].clubs.len()),
            2 => {
                settings.away.0 = cycle(settings.away.0, leagues.len());
                settings.away.1 = 0;
            }
            3 => settings.away.1 = cycle(settings.away.1, leagues[settings.away.0].clubs.len()),
            4 => settings.half = cycle(settings.half, HALVES.len()),
            5 => settings.level = cycle(settings.level, LEVELS.len()),
            6 => settings.camera = CamMode::ALL[cycle(CamMode::ALL.iter().position(|c| *c == settings.camera).unwrap_or(0), 3)],
            7 => settings.commentary = !settings.commentary,
            8 => settings.crowd = cycle(settings.crowd, super::CROWD_LEVELS.len()),
            9 => {
                use super::stadium::TimeOfDay;
                *time_of_day = if *time_of_day == TimeOfDay::Day { TimeOfDay::Night } else { TimeOfDay::Day };
            }
            _ => {}
        }
        if menu.row <= 3 {
            settings.refresh_teams();
        }
        menu.dirty = true;
    }
    if n.ok && (menu.row == ROWS - 1 || keys.just_pressed(KeyCode::Enter)) && career {
        use super::career::{Career, ClubRef};
        let user = ClubRef { league: settings.home.0, club: settings.home.1 };
        let c = if settings.flow == Flow::Season { Career::new_season(user) } else { Career::new_cup(user) };
        c.save();
        commands.insert_resource(c);
        next.set(AppState::Hub);
        return;
    }
    if n.ok && (menu.row == ROWS - 1 || keys.just_pressed(KeyCode::Enter)) {
        if settings.home == settings.away {
            // Mirror match: nudge the away side so both teams differ.
            let len = data::leagues()[settings.away.0].clubs.len();
            settings.away.1 = (settings.away.1 + 1) % len;
            settings.refresh_teams();
        }
        next.set(AppState::Match);
    }
}

fn stat_line(t: &TeamSetup) -> String {
    use crate::sim::formation::Role;
    let slots = t.slots();
    let avg = |want: &[Role]| {
        let v: Vec<u32> = (0..11).filter(|&k| want.contains(&slots[k].role)).map(|k| t.squad[t.xi[k]].ovr as u32).collect();
        if v.is_empty() { 0 } else { v.iter().sum::<u32>() / v.len() as u32 }
    };
    let ovr = (0..11).map(|k| t.squad[t.xi[k]].ovr as u32).sum::<u32>() / 11;
    format!("OVR {}   ATT {}   MID {}   DEF {}", ovr, avg(&[Role::Fwd]), avg(&[Role::Mid]), avg(&[Role::Def, Role::Gk]))
}

fn team_card(c: &mut ChildSpawnerCommands, fonts: &Fonts, images: &mut Assets<Image>, t: &TeamSetup, league: &str, label: &str, sel: [bool; 2]) {
    let crest = images.add(textures::crest(t.crest, t.home, t.trim, 128));
    c.spawn((
        Node {
            width: Val::Px(440.0),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            padding: UiRect::all(Val::Px(14.0)),
            row_gap: Val::Px(3.0),
            border: UiRect::top(Val::Px(5.0)),
            ..default()
        },
        BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.05)),
        BorderColor(color(t.home)),
    ))
    .with_children(|card| {
        card.spawn(text(fonts, label, 16.0, Color::srgba(1.0, 1.0, 1.0, 0.6)));
        let arrow = |s: bool, v: &str| if s { format!("<  {v}  >") } else { v.to_string() };
        card.spawn(text(fonts, arrow(sel[0], league), 20.0, if sel[0] { GOLD } else { Color::srgba(1.0, 1.0, 1.0, 0.8) }));
        card.spawn((Node { width: Val::Px(92.0), height: Val::Px(92.0), margin: UiRect::vertical(Val::Px(4.0)), ..default() }, ImageNode::new(crest)));
        card.spawn(text(fonts, arrow(sel[1], &t.name), 28.0, if sel[1] { GOLD } else { Color::WHITE }));
        card.spawn(text(fonts, format!("{}   ·   {}", t.short, t.formation), 16.0, Color::srgba(1.0, 1.0, 1.0, 0.6)));
        card.spawn(text(fonts, stat_line(t), 18.0, Color::WHITE));
        card.spawn(Node { height: Val::Px(6.0), ..default() });
        let slots = t.slots();
        for k in 0..11 {
            let p = &t.squad[t.xi[k]];
            card.spawn(Node { width: Val::Percent(100.0), justify_content: JustifyContent::SpaceBetween, ..default() }).with_children(|row| {
                row.spawn(text(fonts, format!("{:?}", slots[k].pos), 14.0, Color::srgba(1.0, 1.0, 1.0, 0.5)));
                let name = if p.generated { format!("{} *", p.name) } else { p.name.clone() };
                row.spawn(text(fonts, format!("{:>2}  {}", p.num, name), 15.0, Color::WHITE));
                row.spawn(text(fonts, p.ovr.to_string(), 15.0, GOLD));
            });
        }
    });
}

pub fn draw_setup(
    mut commands: Commands,
    fonts: Res<Fonts>,
    mut images: ResMut<Assets<Image>>,
    mut menu: ResMut<MenuState>,
    settings: Res<Settings>,
    time_of_day: Res<super::stadium::TimeOfDay>,
    old: Query<Entity, With<MenuRoot>>,
) {
    if !menu.dirty {
        return;
    }
    menu.dirty = false;
    for e in &old {
        commands.entity(e).despawn();
    }
    let leagues = data::leagues();
    let row = menu.row;
    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                padding: UiRect::top(Val::Px(14.0)),
                row_gap: Val::Px(8.0),
                ..default()
            },
            BackgroundColor(BG),
            MenuRoot,
            StateScoped(AppState::Setup),
        ))
        .with_children(|c| {
            let mode = match (settings.flow, settings.mode) {
                (Flow::Season, _) => "SEASON  ·  choose your club",
                (Flow::Cup, _) => "CUP  ·  choose your club",
                (_, Mode::VsCpu) => "KICK OFF",
                (_, Mode::TwoPlayer) => "2 PLAYERS",
                (_, Mode::Watch) => "CPU vs CPU",
            };
            c.spawn(text(&fonts, mode, 32.0, Color::WHITE));
            c.spawn(Node { column_gap: Val::Px(30.0), align_items: AlignItems::Center, ..default() }).with_children(|teams| {
                let (hl, al) = (leagues[settings.home.0].name, leagues[settings.away.0].name);
                if settings.flow != Flow::Friendly {
                    team_card(teams, &fonts, &mut images, &settings.home_team, hl, "YOUR CLUB", [row == 0, row == 1]);
                } else {
                    team_card(teams, &fonts, &mut images, &settings.home_team, hl, "HOME", [row == 0, row == 1]);
                    teams.spawn(text(&fonts, "VS", 34.0, GOLD));
                    team_card(teams, &fonts, &mut images, &settings.away_team, al, "AWAY", [row == 2, row == 3]);
                }
            });
            let opts = [
                ("Half length", HALVES[settings.half].0.to_string()),
                ("CPU level", LEVELS[settings.level].0.to_string()),
                ("Camera", settings.camera.label().to_string()),
                ("Commentary", if settings.commentary { "On".to_string() } else { "Off".to_string() }),
                ("Crowd", super::CROWD_LEVELS[settings.crowd].0.to_string()),
                ("Kick-off", if *time_of_day == super::stadium::TimeOfDay::Day { "Afternoon".to_string() } else { "Floodlit night".to_string() }),
            ];
            c.spawn(Node {
                column_gap: Val::Px(18.0),
                row_gap: Val::Px(10.0),
                flex_wrap: FlexWrap::Wrap,
                justify_content: JustifyContent::Center,
                max_width: Val::Px(1100.0),
                ..default()
            })
            .with_children(|o| {
                for (k, (label, value)) in opts.iter().enumerate() {
                    let sel = row == 4 + k;
                    o.spawn((
                        Node { padding: UiRect::axes(Val::Px(16.0), Val::Px(8.0)), column_gap: Val::Px(10.0), ..default() },
                        BackgroundColor(if sel { Color::srgba(1.0, 1.0, 1.0, 0.12) } else { Color::srgba(1.0, 1.0, 1.0, 0.04) }),
                    ))
                    .with_children(|b| {
                        b.spawn(text(&fonts, *label, 18.0, Color::srgba(1.0, 1.0, 1.0, 0.6)));
                        let v = if sel { format!("<  {value}  >") } else { value.clone() };
                        b.spawn(text(&fonts, v, 18.0, if sel { GOLD } else { Color::WHITE }));
                    });
                }
            });
            let sel = row == ROWS - 1;
            c.spawn((
                Node { padding: UiRect::axes(Val::Px(44.0), Val::Px(12.0)), ..default() },
                BackgroundColor(if sel { GOLD } else { Color::srgba(0.97, 0.78, 0.25, 0.35) }),
            ))
            .with_child(text(&fonts, if settings.flow == Flow::Friendly { "PLAY MATCH" } else { "START" }, 30.0, Color::BLACK));
            c.spawn(text(
                &fonts,
                "Up/Down choose  ·  Left/Right change  ·  Enter play  ·  Esc back      * = generated squad filler",
                15.0,
                Color::srgba(1.0, 1.0, 1.0, 0.45),
            ));
        });
}
