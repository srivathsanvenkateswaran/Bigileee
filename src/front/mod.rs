//! The Bevy front end: menus, the 3D match scene, controls, HUD and sound.

pub mod audio;
pub mod camera;
pub mod controls;
pub mod font5x7;
pub mod hud;
pub mod menu;
pub mod players;
pub mod stadium;
pub mod textures;
pub mod view;

use crate::data;
use crate::sim::events::Sfx;
use crate::sim::game::{Match, MatchConfig, Mode, Phase};
use crate::sim::squad::TeamSetup;
use bevy::audio::Volume;
use bevy::input::gamepad::{Gamepad, GamepadButton};
use bevy::prelude::*;
use camera::{CamMode, MainCam};
use view::{Replay, View};

#[derive(States, Default, Clone, Copy, Eq, PartialEq, Hash, Debug)]
#[states(scoped_entities)]
pub enum AppState {
    #[default]
    Title,
    Setup,
    Match,
}

/// Choices made in the menus. Team setups are cached so their XIs stay stable while browsing.
#[derive(Resource)]
pub struct Settings {
    pub mode: Mode,
    /// (league index, club index)
    pub home: (usize, usize),
    pub away: (usize, usize),
    pub half: usize,
    pub level: usize,
    pub camera: CamMode,
    pub home_team: TeamSetup,
    pub away_team: TeamSetup,
}

impl Settings {
    fn new() -> Settings {
        let l = data::leagues();
        // Default fixture: the two clubs from the first league's opening pair.
        let home = (0, 11.min(l[0].clubs.len() - 1));
        let away = (0, 12.min(l[0].clubs.len() - 1));
        Settings {
            mode: Mode::VsCpu,
            home,
            away,
            half: 1,
            level: 2,
            camera: CamMode::Broadcast,
            home_team: TeamSetup::from_club(&l[home.0].clubs[home.1]),
            away_team: TeamSetup::from_club(&l[away.0].clubs[away.1]),
        }
    }

    pub fn refresh_teams(&mut self) {
        let l = data::leagues();
        if self.home_team.name != l[self.home.0].clubs[self.home.1].name {
            self.home_team = TeamSetup::from_club(&l[self.home.0].clubs[self.home.1]);
        }
        if self.away_team.name != l[self.away.0].clubs[self.away.1].name {
            self.away_team = TeamSetup::from_club(&l[self.away.0].clubs[self.away.1]);
        }
    }
}

#[derive(Resource)]
pub struct MatchRes {
    pub m: Match,
    pub sfx: Sfx,
    pub paused: bool,
    pub pause_sel: usize,
}

#[derive(Resource)]
pub struct Fonts {
    pub bold: Handle<Font>,
    pub regular: Handle<Font>,
}

/// Debug aid: `BIGILEEE_SNAPSHOT=<dir>` drives the menus, plays a CPU match and saves screenshots.
#[derive(Resource)]
pub struct Snapshot {
    dir: String,
    frame: u32,
}

pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<AppState>()
            .insert_resource(Settings::new())
            .insert_resource(menu::MenuState { row: 0, dirty: true })
            .init_resource::<View>()
            .init_resource::<Replay>()
            .add_systems(Startup, (load_fonts, audio::build_sounds))
            .add_systems(OnEnter(AppState::Title), (mark_dirty, menu_camera::<0>))
            .add_systems(OnEnter(AppState::Setup), (mark_dirty, menu_camera::<1>))
            .add_systems(Update, (menu::title_input, menu::draw_title).chain().run_if(in_state(AppState::Title)))
            .add_systems(Update, (menu::setup_input, menu::draw_setup).chain().run_if(in_state(AppState::Setup)))
            .add_systems(OnEnter(AppState::Match), setup_match)
            .add_systems(OnExit(AppState::Match), teardown_match)
            .add_systems(
                Update,
                (
                    step_match,
                    players::animate_players,
                    players::animate_ball,
                    players::place_indicators,
                    camera::update_camera,
                    hud::update_hud,
                )
                    .chain()
                    .run_if(in_state(AppState::Match).and(resource_exists::<MatchRes>)),
            );
        if let Ok(dir) = std::env::var("BIGILEEE_SNAPSHOT") {
            app.insert_resource(Snapshot { dir, frame: 0 }).add_systems(Last, snapshot);
        }
    }
}

/// Menus are pure UI, which still needs a camera to render through.
fn menu_camera<const S: u8>(mut commands: Commands) {
    let state = if S == 0 { AppState::Title } else { AppState::Setup };
    commands.spawn((Camera2d, StateScoped(state)));
}

fn mark_dirty(mut menu: ResMut<menu::MenuState>) {
    menu.dirty = true;
}

fn load_fonts(mut commands: Commands, mut fonts: ResMut<Assets<Font>>) {
    // Use the Mac's own Arial if present (it covers accents); fall back to Bevy's built-in font.
    let mut load = |paths: &[&str]| {
        paths
            .iter()
            .find_map(|p| std::fs::read(p).ok())
            .and_then(|bytes| Font::try_from_bytes(bytes).ok())
            .map(|f| fonts.add(f))
            .unwrap_or_default()
    };
    let bold = load(&["/System/Library/Fonts/Supplemental/Arial Bold.ttf", "/Library/Fonts/Arial Bold.ttf"]);
    let regular = load(&["/System/Library/Fonts/Supplemental/Arial.ttf", "/Library/Fonts/Arial.ttf"]);
    commands.insert_resource(Fonts { bold, regular });
}

#[allow(clippy::too_many_arguments)]
fn setup_match(
    mut commands: Commands,
    settings: Res<Settings>,
    fonts: Res<Fonts>,
    sounds: Res<audio::Sounds>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    mut replay: ResMut<Replay>,
) {
    let cfg = MatchConfig {
        mode: settings.mode,
        half_secs: menu::HALVES[settings.half].1,
        difficulty: menu::LEVELS[settings.level].1,
        knockout: false,
    };
    let m = Match::new(cfg, [settings.home_team.clone(), settings.away_team.clone()]);
    let league = data::leagues()[settings.home.0].name.to_uppercase();
    stadium::spawn_stadium(&mut commands, &mut meshes, &mut materials, &mut images, m.kit[0], m.kit[1], &league);
    players::spawn_players(&mut commands, &mut meshes, &mut materials, &mut images, &m);
    camera::spawn_camera(&mut commands);
    let res = MatchRes { m, sfx: Sfx::default(), paused: false, pause_sel: 0 };
    hud::spawn_hud(&mut commands, &fonts, &mut images, &res);
    commands.spawn((
        AudioPlayer::new(sounds.crowd.clone()),
        PlaybackSettings::LOOP.with_volume(Volume::Linear(0.45)),
        StateScoped(AppState::Match),
    ));
    *replay = Replay::default();
    commands.insert_resource(View::from_match(&res.m));
    commands.insert_resource(res);
}

fn teardown_match(mut commands: Commands) {
    commands.remove_resource::<MatchRes>();
}

#[allow(clippy::too_many_arguments)]
fn step_match(
    mut commands: Commands,
    mut res: ResMut<MatchRes>,
    mut replay: ResMut<Replay>,
    mut view: ResMut<View>,
    mut settings: ResMut<Settings>,
    keys: Res<ButtonInput<KeyCode>>,
    pads: Query<&Gamepad>,
    cam: Query<&GlobalTransform, With<MainCam>>,
    time: Res<Time>,
    sounds: Res<audio::Sounds>,
    mut next: ResMut<NextState<AppState>>,
) {
    let dt = time.delta_secs().min(1.0 / 30.0);
    let pad_list: Vec<&Gamepad> = pads.iter().collect();
    let pad = |b: GamepadButton| pad_list.iter().any(|p| p.just_pressed(b));
    let confirm = keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::Space) || pad(GamepadButton::South);

    if keys.just_pressed(KeyCode::KeyC) {
        let k = CamMode::ALL.iter().position(|c| *c == settings.camera).unwrap_or(0);
        settings.camera = CamMode::ALL[(k + 1) % CamMode::ALL.len()];
    }

    if res.m.phase == Phase::FullTime {
        if confirm || keys.just_pressed(KeyCode::Escape) {
            next.set(AppState::Setup);
        }
        return;
    }

    if keys.just_pressed(KeyCode::Escape) || keys.just_pressed(KeyCode::KeyP) || pad(GamepadButton::Start) {
        res.paused = !res.paused;
        res.pause_sel = 0;
    }
    if res.paused {
        if keys.just_pressed(KeyCode::ArrowDown) || keys.just_pressed(KeyCode::KeyS) || pad(GamepadButton::DPadDown) {
            res.pause_sel = (res.pause_sel + 1) % 3;
        }
        if keys.just_pressed(KeyCode::ArrowUp) || keys.just_pressed(KeyCode::KeyW) || pad(GamepadButton::DPadUp) {
            res.pause_sel = (res.pause_sel + 2) % 3;
        }
        if confirm {
            match res.pause_sel {
                0 => res.paused = false,
                1 => {
                    // Re-enter the match state to rebuild everything from scratch.
                    next.set(AppState::Setup);
                    settings.mode = res.m.cfg.mode;
                    commands.insert_resource(menu::MenuState { row: 7, dirty: true });
                }
                _ => next.set(AppState::Title),
            }
        }
        return;
    }

    // Goal replays: skip with confirm, otherwise play out before the restart.
    if replay.is_playing() {
        if confirm || pad(GamepadButton::East) {
            replay.stop();
        } else if let Some(v) = replay.advance(dt) {
            *view = v;
            return;
        }
    }

    let was_goal = matches!(res.m.phase, Phase::Goal { .. });
    let humans = res.m.humans.len();
    let inputs = controls::read(humans, &keys, &pad_list, cam.single().ok());
    let MatchRes { m, sfx, .. } = &mut *res;
    m.update(dt, &inputs, sfx);
    for e in sfx.drain() {
        audio::play(&mut commands, &sounds, e);
    }
    *view = View::from_match(&res.m);
    replay.record(dt, &view);

    if !was_goal && matches!(res.m.phase, Phase::Goal { .. }) {
        replay.armed = Some(1.8);
    }
    if let Some(t) = replay.armed.as_mut() {
        *t -= dt;
        if *t <= 0.0 {
            replay.start();
        }
    }
}

fn snapshot(
    mut commands: Commands,
    mut snap: ResMut<Snapshot>,
    state: Res<State<AppState>>,
    mut next: ResMut<NextState<AppState>>,
    mut settings: ResMut<Settings>,
    mut menu: ResMut<menu::MenuState>,
    mut exit: EventWriter<AppExit>,
) {
    use bevy::render::view::screenshot::{save_to_disk, Screenshot};
    snap.frame += 1;
    let f = snap.frame;
    let shot = |commands: &mut Commands, name: &str| {
        let path = format!("{}/{}.png", snap.dir, name);
        commands.spawn(Screenshot::primary_window()).observe(save_to_disk(path));
    };
    match (f, state.get()) {
        (60, _) => shot(&mut commands, "1_title"),
        (70, _) => {
            settings.mode = Mode::Watch;
            menu.row = 1;
            next.set(AppState::Setup);
        }
        (130, _) => shot(&mut commands, "2_setup"),
        (140, _) => next.set(AppState::Match),
        (400, AppState::Match) => shot(&mut commands, "3_kickoff"),
        (1100, AppState::Match) => shot(&mut commands, "4_play"),
        (1900, AppState::Match) => shot(&mut commands, "5_play"),
        (2700, AppState::Match) => shot(&mut commands, "6_play"),
        (2800, _) => {
            exit.write(AppExit::Success);
        }
        _ => {}
    }
}
