//! Season and Cup modes: fixtures, quick simulation of other matches, the league table, the
//! knockout bracket, saving, and the hub screen between matches.

use super::hud::text;
use super::textures;
use super::{AppState, Fonts, Settings};
use crate::data;
use crate::sim::rng::gen_range;
use crate::sim::squad::TeamSetup;
use bevy::input::gamepad::{Gamepad, GamepadButton};
use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub struct ClubRef {
    pub league: usize,
    pub club: usize,
}

impl ClubRef {
    pub fn club(self) -> &'static data::types::Club {
        &data::leagues()[self.league].clubs[self.club]
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Fixture {
    pub home: usize,
    pub away: usize,
    pub result: Option<(u8, u8)>,
    /// Shootout winner (0 = home, 1 = away) for drawn cup ties.
    pub pens: Option<usize>,
}

impl Fixture {
    fn new(home: usize, away: usize) -> Fixture {
        Fixture { home, away, result: None, pens: None }
    }

    /// Index into `Career::teams` of the side that went through, for a played cup tie.
    fn winner(&self) -> Option<usize> {
        let (h, a) = self.result?;
        Some(if h > a || (h == a && self.pens == Some(0)) { self.home } else { self.away })
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Season,
    Cup,
}

#[derive(Serialize, Deserialize, Clone, Resource, Debug)]
pub struct Career {
    pub kind: Kind,
    pub name: String,
    pub teams: Vec<ClubRef>,
    /// Index into `teams` of the human's club.
    pub user: usize,
    /// Matchdays for a season, knockout rounds for a cup.
    pub rounds: Vec<Vec<Fixture>>,
    pub round: usize,
    /// Team strengths used to simulate matches the human doesn't play.
    pub ratings: Vec<u8>,
    pub champion: Option<usize>,
}

#[derive(Clone, Debug, Default)]
pub struct Row {
    pub team: usize,
    pub p: u32,
    pub w: u32,
    pub d: u32,
    pub l: u32,
    pub gf: u32,
    pub ga: u32,
    pub pts: u32,
}

fn strength(r: ClubRef) -> u8 {
    let t = TeamSetup::from_club(r.club());
    (t.xi.iter().map(|&i| t.squad[i].ovr as u32).sum::<u32>() / 11) as u8
}

/// Circle-method double round robin. Odd team counts get a bye each round.
pub fn round_robin(n: usize) -> Vec<Vec<(usize, usize)>> {
    let mut ids: Vec<Option<usize>> = (0..n).map(Some).collect();
    if n % 2 == 1 {
        ids.push(None);
    }
    let m = ids.len();
    let mut rounds = Vec::new();
    for r in 0..m - 1 {
        let mut games = Vec::new();
        for i in 0..m / 2 {
            if let (Some(a), Some(b)) = (ids[i], ids[m - 1 - i]) {
                games.push(if (r + i) % 2 == 0 { (a, b) } else { (b, a) });
            }
        }
        rounds.push(games);
        let last = ids.pop().unwrap();
        ids.insert(1, last);
    }
    let second: Vec<Vec<(usize, usize)>> = rounds.iter().map(|g| g.iter().map(|&(a, b)| (b, a)).collect()).collect();
    rounds.extend(second);
    rounds
}

fn poisson(lambda: f32) -> u8 {
    let limit = (-lambda).exp();
    let (mut k, mut p) = (0u8, 1.0f32);
    loop {
        p *= gen_range(0.0, 1.0);
        if p <= limit || k >= 9 {
            return k;
        }
        k += 1;
    }
}

/// Simulates a match the human isn't playing, from team strengths.
pub fn quick_sim(home: u8, away: u8, knockout: bool) -> ((u8, u8), Option<usize>) {
    let diff = home as f32 - away as f32;
    // Stronger sides score more, but even big gaps rarely produce cricket scores.
    let h = poisson((1.45 * (diff / 24.0).exp()).min(3.6));
    let a = poisson((1.1 * (-diff / 24.0).exp()).min(3.2));
    let pens = (knockout && h == a).then(|| if gen_range(0.0, 1.0) < 0.5 + diff * 0.01 { 0 } else { 1 });
    ((h, a), pens)
}

pub fn save_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    PathBuf::from(home).join("Library/Application Support/Bigileee22/career.json")
}

impl Career {
    pub fn new_season(user: ClubRef) -> Career {
        let league = data::leagues()[user.league];
        let teams: Vec<ClubRef> = (0..league.clubs.len()).map(|club| ClubRef { league: user.league, club }).collect();
        let rounds = round_robin(teams.len())
            .into_iter()
            .map(|g| g.into_iter().map(|(h, a)| Fixture::new(h, a)).collect())
            .collect();
        Career {
            kind: Kind::Season,
            name: format!("{} 2021-22", league.name),
            ratings: teams.iter().map(|&r| strength(r)).collect(),
            user: user.club,
            teams,
            rounds,
            round: 0,
            champion: None,
        }
    }

    /// Sixteen clubs: the human's plus the fifteen strongest from every league, drawn at random.
    pub fn new_cup(user: ClubRef) -> Career {
        let mut all: Vec<(ClubRef, u8)> = data::leagues()
            .iter()
            .enumerate()
            .flat_map(|(l, lg)| (0..lg.clubs.len()).map(move |c| ClubRef { league: l, club: c }))
            .filter(|&r| r != user)
            .map(|r| (r, r.club().rating))
            .collect();
        all.sort_by(|a, b| b.1.cmp(&a.1));
        let mut teams: Vec<ClubRef> = all.iter().take(15).map(|x| x.0).collect();
        teams.push(user);
        // Shuffle the draw.
        for i in (1..teams.len()).rev() {
            teams.swap(i, gen_range(0, i + 1));
        }
        let user_idx = teams.iter().position(|&r| r == user).unwrap();
        let first = (0..8).map(|k| Fixture::new(2 * k, 2 * k + 1)).collect();
        Career {
            kind: Kind::Cup,
            name: "Bigileee Champions Cup".into(),
            ratings: teams.iter().map(|&r| strength(r)).collect(),
            user: user_idx,
            teams,
            rounds: vec![first],
            round: 0,
            champion: None,
        }
    }

    pub fn round_name(&self) -> String {
        match self.kind {
            Kind::Season => format!("Matchday {} of {}", (self.round + 1).min(self.rounds.len()), self.rounds.len()),
            Kind::Cup => match self.rounds.get(self.round).map_or(0, |r| r.len()) {
                8 => "Round of 16".into(),
                4 => "Quarter-finals".into(),
                2 => "Semi-finals".into(),
                _ => "Final".into(),
            },
        }
    }

    /// The human's fixture in the current round, if they play in it.
    pub fn user_fixture(&self) -> Option<usize> {
        if self.champion.is_some() {
            return None;
        }
        self.rounds.get(self.round)?.iter().position(|f| (f.home == self.user || f.away == self.user) && f.result.is_none())
    }

    pub fn eliminated(&self) -> bool {
        self.kind == Kind::Cup
            && self.round > 0
            && !self.rounds[self.round - 1].iter().any(|f| f.winner() == Some(self.user))
            && self.rounds.get(self.round).is_none_or(|r| !r.iter().any(|f| f.home == self.user || f.away == self.user))
    }

    pub fn record(&mut self, fixture: usize, score: (u8, u8), pens: Option<usize>) {
        let f = &mut self.rounds[self.round][fixture];
        f.result = Some(score);
        f.pens = pens;
    }

    /// Simulates everything left in the current round, then moves on.
    pub fn finish_round(&mut self) {
        let knockout = self.kind == Kind::Cup;
        let ratings = self.ratings.clone();
        for f in &mut self.rounds[self.round] {
            if f.result.is_none() {
                let (s, p) = quick_sim(ratings[f.home], ratings[f.away], knockout);
                f.result = Some(s);
                f.pens = p;
            }
        }
        match self.kind {
            Kind::Season => {
                self.round += 1;
                if self.round >= self.rounds.len() {
                    self.champion = self.table().first().map(|r| r.team);
                }
            }
            Kind::Cup => {
                let winners: Vec<usize> = self.rounds[self.round].iter().filter_map(|f| f.winner()).collect();
                if winners.len() == 1 {
                    self.champion = Some(winners[0]);
                } else {
                    self.rounds.push(winners.chunks(2).map(|w| Fixture::new(w[0], w[1])).collect());
                    self.round += 1;
                }
            }
        }
    }

    pub fn table(&self) -> Vec<Row> {
        let mut rows: Vec<Row> = (0..self.teams.len()).map(|team| Row { team, ..default() }).collect();
        for f in self.rounds.iter().flatten() {
            let Some((h, a)) = f.result else { continue };
            let (h, a) = (h as u32, a as u32);
            for (t, gf, ga) in [(f.home, h, a), (f.away, a, h)] {
                let r = &mut rows[t];
                r.p += 1;
                r.gf += gf;
                r.ga += ga;
                if gf > ga {
                    r.w += 1;
                    r.pts += 3;
                } else if gf == ga {
                    r.d += 1;
                    r.pts += 1;
                } else {
                    r.l += 1;
                }
            }
        }
        rows.sort_by(|a, b| {
            (b.pts, b.gf as i32 - b.ga as i32, b.gf).cmp(&(a.pts, a.gf as i32 - a.ga as i32, a.gf))
        });
        rows
    }

    pub fn save(&self) {
        let path = save_path();
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(json) = serde_json::to_string(self) {
            let _ = std::fs::write(path, json);
        }
    }

    pub fn load() -> Option<Career> {
        let json = std::fs::read_to_string(save_path()).ok()?;
        serde_json::from_str(&json).ok()
    }

    pub fn delete_save() {
        let _ = std::fs::remove_file(save_path());
    }

    pub fn team_name(&self, t: usize) -> &'static str {
        self.teams[t].club().name
    }
}

// ---------- hub screen ----------

#[derive(Component)]
pub struct HubRoot;

#[derive(Resource, Default)]
pub struct HubState {
    pub row: usize,
    pub dirty: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum HubAction {
    Play,
    Simulate,
    SimulateAll,
    Quit,
    Done,
}

fn actions(c: &Career) -> Vec<(HubAction, &'static str)> {
    if c.champion.is_some() || c.eliminated() {
        return vec![(HubAction::Done, "Finish and return to title")];
    }
    let mut v = Vec::new();
    if c.user_fixture().is_some() {
        v.push((HubAction::Play, "Play match"));
        v.push((HubAction::Simulate, "Simulate match"));
    } else {
        v.push((HubAction::Simulate, "Continue (sim this round)"));
    }
    if c.kind == Kind::Season {
        v.push((HubAction::SimulateAll, "Simulate rest of season"));
    }
    v.push((HubAction::Quit, "Save and quit to title"));
    v
}

#[allow(clippy::too_many_arguments)]
pub fn hub_input(
    keys: Res<ButtonInput<KeyCode>>,
    pads: Query<&Gamepad>,
    mut hub: ResMut<HubState>,
    career: Option<ResMut<Career>>,
    mut settings: ResMut<Settings>,
    mut next: ResMut<NextState<AppState>>,
    mut commands: Commands,
) {
    let Some(mut career) = career else {
        next.set(AppState::Title);
        return;
    };
    let k = |c: &[KeyCode]| c.iter().any(|&c| keys.just_pressed(c));
    let p = |b: GamepadButton| pads.iter().any(|g| g.just_pressed(b));
    let acts = actions(&career);
    if k(&[KeyCode::ArrowDown, KeyCode::KeyS]) || p(GamepadButton::DPadDown) {
        hub.row = (hub.row + 1) % acts.len();
        hub.dirty = true;
    }
    if k(&[KeyCode::ArrowUp, KeyCode::KeyW]) || p(GamepadButton::DPadUp) {
        hub.row = (hub.row + acts.len() - 1) % acts.len();
        hub.dirty = true;
    }
    hub.row = hub.row.min(acts.len() - 1);
    if !(k(&[KeyCode::Enter, KeyCode::Space]) || p(GamepadButton::South)) {
        return;
    }
    hub.dirty = true;
    match acts[hub.row].0 {
        HubAction::Play => {
            let fx = career.user_fixture().unwrap();
            let f = career.rounds[career.round][fx].clone();
            settings.home_team = TeamSetup::from_club(career.teams[f.home].club());
            settings.away_team = TeamSetup::from_club(career.teams[f.away].club());
            settings.home = (career.teams[f.home].league, career.teams[f.home].club);
            settings.away = (career.teams[f.away].league, career.teams[f.away].club);
            settings.mode = crate::sim::game::Mode::VsCpu;
            settings.human_team = if f.home == career.user { 0 } else { 1 };
            settings.knockout = career.kind == Kind::Cup;
            settings.career_fixture = Some(fx);
            next.set(AppState::Match);
        }
        HubAction::Simulate => {
            career.finish_round();
            career.save();
        }
        HubAction::SimulateAll => {
            while career.champion.is_none() {
                career.finish_round();
            }
            career.save();
        }
        HubAction::Quit => {
            career.save();
            commands.remove_resource::<Career>();
            next.set(AppState::Title);
        }
        HubAction::Done => {
            Career::delete_save();
            commands.remove_resource::<Career>();
            next.set(AppState::Title);
        }
    }
    hub.row = 0;
}

const GOLD: Color = Color::srgb(0.97, 0.78, 0.25);

pub fn draw_hub(
    mut commands: Commands,
    fonts: Res<Fonts>,
    mut images: ResMut<Assets<Image>>,
    mut hub: ResMut<HubState>,
    career: Option<Res<Career>>,
    old: Query<Entity, With<HubRoot>>,
) {
    let Some(c) = career else { return };
    if !hub.dirty && !c.is_changed() {
        return;
    }
    hub.dirty = false;
    for e in &old {
        commands.entity(e).despawn();
    }
    let dim = Color::srgba(1.0, 1.0, 1.0, 0.6);
    let user_club = c.teams[c.user].club();
    let crest = |images: &mut Assets<Image>, t: usize| {
        let club = c.teams[t].club();
        images.add(textures::crest(club.crest, club.home, club.trim, 96))
    };
    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                padding: UiRect::all(Val::Px(24.0)),
                row_gap: Val::Px(14.0),
                ..default()
            },
            BackgroundColor(Color::srgb(0.03, 0.05, 0.1)),
            HubRoot,
            StateScoped(AppState::Hub),
        ))
        .with_children(|root| {
            root.spawn(text(&fonts, c.name.to_uppercase(), 40.0, Color::WHITE));
            let status = if let Some(ch) = c.champion {
                if ch == c.user {
                    format!("CHAMPIONS!  {} win the {}", user_club.name, c.name)
                } else {
                    format!("{} are champions", c.team_name(ch))
                }
            } else if c.eliminated() {
                format!("{} are out of the cup", user_club.name)
            } else {
                c.round_name()
            };
            root.spawn(text(&fonts, status, 24.0, GOLD));

            root.spawn(Node { column_gap: Val::Px(40.0), align_items: AlignItems::FlexStart, ..default() }).with_children(|cols| {
                // Left: next fixture and actions.
                cols.spawn(Node { flex_direction: FlexDirection::Column, align_items: AlignItems::Center, row_gap: Val::Px(12.0), width: Val::Px(480.0), ..default() })
                    .with_children(|left| {
                        if let Some(fx) = c.user_fixture() {
                            let f = &c.rounds[c.round][fx];
                            left.spawn(text(&fonts, "NEXT MATCH", 16.0, dim));
                            left.spawn(Node { column_gap: Val::Px(18.0), align_items: AlignItems::Center, ..default() }).with_children(|m| {
                                for (k, t) in [f.home, f.away].iter().enumerate() {
                                    if k == 1 {
                                        m.spawn(text(&fonts, "vs", 22.0, GOLD));
                                    }
                                    m.spawn(Node { flex_direction: FlexDirection::Column, align_items: AlignItems::Center, width: Val::Px(200.0), ..default() })
                                        .with_children(|t_| {
                                            t_.spawn((Node { width: Val::Px(84.0), height: Val::Px(84.0), ..default() }, ImageNode::new(crest(&mut images, *t))));
                                            let colour = if *t == c.user { GOLD } else { Color::WHITE };
                                            t_.spawn(text(&fonts, c.team_name(*t), 18.0, colour));
                                            t_.spawn(text(&fonts, format!("Strength {}", c.ratings[*t]), 14.0, dim));
                                        });
                                }
                            });
                        }
                        left.spawn(Node { height: Val::Px(8.0), ..default() });
                        for (k, (_, label)) in actions(&c).iter().enumerate() {
                            let sel = hub.row == k;
                            left.spawn((
                                Node { width: Val::Px(380.0), padding: UiRect::axes(Val::Px(20.0), Val::Px(10.0)), justify_content: JustifyContent::Center, ..default() },
                                BackgroundColor(if sel { GOLD } else { Color::srgba(1.0, 1.0, 1.0, 0.07) }),
                            ))
                            .with_child(text(&fonts, *label, 22.0, if sel { Color::BLACK } else { Color::WHITE }));
                        }
                        // Recent results for the human's club.
                        let mine: Vec<String> = c
                            .rounds
                            .iter()
                            .flatten()
                            .filter(|f| f.result.is_some() && (f.home == c.user || f.away == c.user))
                            .map(|f| {
                                let (h, a) = f.result.unwrap();
                                let pens = match f.pens {
                                    Some(0) => " (home on pens)",
                                    Some(_) => " (away on pens)",
                                    None => "",
                                };
                                format!("{} {}-{} {}{}", c.teams[f.home].club().short, h, a, c.teams[f.away].club().short, pens)
                            })
                            .collect();
                        if !mine.is_empty() {
                            left.spawn(Node { height: Val::Px(8.0), ..default() });
                            left.spawn(text(&fonts, "YOUR RESULTS", 16.0, dim));
                            for r in mine.iter().rev().take(6) {
                                left.spawn(text(&fonts, r.clone(), 16.0, Color::WHITE));
                            }
                        }
                    });

                // Right: table or bracket.
                cols.spawn(Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(3.0), width: Val::Px(560.0), ..default() }).with_children(|right| {
                    match c.kind {
                        Kind::Season => {
                            right.spawn(Node { justify_content: JustifyContent::SpaceBetween, ..default() }).with_children(|row| {
                                row.spawn(text(&fonts, " #   CLUB", 14.0, dim));
                                row.spawn(text(&fonts, "  P    W   D   L    GD    PTS", 14.0, dim));
                            });
                            for (pos, r) in c.table().iter().enumerate() {
                                let colour = if r.team == c.user { GOLD } else { Color::WHITE };
                                right.spawn(Node { justify_content: JustifyContent::SpaceBetween, ..default() }).with_children(|row| {
                                    row.spawn(text(&fonts, format!("{:>2}  {}", pos + 1, c.team_name(r.team)), 15.0, colour));
                                    let gd = r.gf as i32 - r.ga as i32;
                                    row.spawn(text(&fonts, format!("{:>3}  {:>3} {:>3} {:>3}  {:>+4}  {:>4}", r.p, r.w, r.d, r.l, gd, r.pts), 15.0, colour));
                                });
                            }
                        }
                        Kind::Cup => {
                            for (k, round) in c.rounds.iter().enumerate() {
                                let name = match round.len() {
                                    8 => "ROUND OF 16",
                                    4 => "QUARTER-FINALS",
                                    2 => "SEMI-FINALS",
                                    _ => "FINAL",
                                };
                                right.spawn((Node { margin: UiRect::top(Val::Px(if k == 0 { 0.0 } else { 10.0 })), ..default() }, text(&fonts, name, 15.0, dim)));
                                for f in round {
                                    let score = match (f.result, f.pens) {
                                        (Some((h, a)), Some(w)) => format!("{h}-{a} ({} on pens)", if w == 0 { "home" } else { "away" }),
                                        (Some((h, a)), None) => format!("{h}-{a}"),
                                        _ => "v".into(),
                                    };
                                    let colour = if f.home == c.user || f.away == c.user { GOLD } else { Color::WHITE };
                                    right.spawn(text(&fonts, format!("{}  {}  {}", c.team_name(f.home), score, c.team_name(f.away)), 15.0, colour));
                                }
                            }
                        }
                    }
                });
            });
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn season_and_cup_complete() {
        crate::sim::rng::srand(3);
        let mut s = Career::new_season(ClubRef { league: 0, club: 0 });
        assert_eq!(s.rounds.len(), 38);
        while s.champion.is_none() {
            s.finish_round();
        }
        let t = s.table();
        assert!(t.iter().all(|r| r.p == 38));
        println!("season champion: {}", s.team_name(t[0].team));

        let odd = Career::new_season(ClubRef { league: 5, club: 0 });
        assert_eq!(odd.teams.len(), 11);
        assert!(odd.rounds.iter().all(|r| r.len() == 5), "odd leagues get one bye per round");

        let mut c = Career::new_cup(ClubRef { league: 5, club: 2 });
        assert_eq!(c.teams.len(), 16);
        while c.champion.is_none() {
            c.finish_round();
        }
        assert_eq!(c.rounds.len(), 4);
        println!("cup champion: {}", c.team_name(c.champion.unwrap()));
    }
}
