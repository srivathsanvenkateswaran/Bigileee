//! Season and Cup modes: fixtures, quick simulation of other matches, the league table, the
//! knockout bracket, transfers and squad selection, saving, and the hub screens between matches.

use super::hud::text;
use super::textures;
use super::{AppState, Fonts, Settings};
use crate::data;
use crate::data::types::Pos;
use crate::sim::formation;
use crate::sim::rng::gen_range;
use crate::sim::squad::{fit_penalty, pick_xi, refit, Card, TeamSetup};
use std::collections::HashMap;
use bevy::input::gamepad::{Gamepad, GamepadButton};
use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct ClubRef {
    pub league: usize,
    pub club: usize,
}

impl ClubRef {
    pub fn club(self) -> &'static data::types::Club {
        &data::leagues()[self.league].clubs[self.club]
    }
}

/// A player, identified by his original club and his index in that club's built squad. Squads
/// are built deterministically (generated fillers included), so the index is stable.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub struct PlayerRef {
    pub club: ClubRef,
    pub idx: usize,
}

/// A member of the human's squad and the shirt number he wears there.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub struct Signing {
    pub player: PlayerRef,
    pub num: u8,
}

/// A player on the transfer list (or in the human's squad, when selling).
#[derive(Clone, Debug)]
pub struct Listing {
    pub player: PlayerRef,
    pub card: Card,
    pub club: &'static str,
    pub value: u32,
}

pub const MIN_SQUAD: usize = 14;
pub const MAX_SQUAD: usize = 30;

/// Transfer value in thousands, from overall rating and position: attackers cost the most,
/// keepers the least, and every extra point of overall adds about 18%.
pub fn market_value(ovr: u8, pos: Pos) -> u32 {
    use Pos::*;
    let mult = match pos {
        GK => 0.6,
        CB | LB | RB | LWB | RWB => 0.85,
        CDM | CM => 1.0,
        CAM | LM | RM => 1.05,
        LW | RW | CF | ST => 1.2,
    };
    let v = 500.0 * 1.18f32.powi(ovr as i32 - 60) * mult;
    ((v / 50.0).round() as u32).max(1) * 50
}

/// Starting transfer kitty, from the club's rating.
pub fn starting_budget(rating: u8) -> u32 {
    market_value(rating, Pos::CM) * 2 + 1000
}

pub fn money(k: u32) -> String {
    if k >= 1000 {
        format!("£{:.1}m", k as f32 / 1000.0)
    } else {
        format!("£{k}k")
    }
}

/// Looks up cards for a set of players, building each club's squad once.
fn cards_of(refs: impl Iterator<Item = PlayerRef>) -> Vec<Card> {
    let mut built: HashMap<ClubRef, TeamSetup> = HashMap::new();
    refs.map(|r| built.entry(r.club).or_insert_with(|| TeamSetup::from_club(r.club.club())).squad[r.idx].clone()).collect()
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
    /// Transfer budget in thousands. Saves from before transfers existed have none and are
    /// given the club's starting budget on load.
    #[serde(default)]
    pub budget: Option<u32>,
    /// The human's squad; empty in old saves until filled from the club's own squad.
    #[serde(default)]
    pub squad: Vec<Signing>,
    /// The human's starting XI, in formation-slot order.
    #[serde(default)]
    pub xi: Vec<PlayerRef>,
    /// The human's formation; empty means the club's usual one.
    #[serde(default)]
    pub formation: String,
    /// Players who have left their original club, bought or sold by the human.
    #[serde(default)]
    pub departed: Vec<PlayerRef>,
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

fn strength(t: &TeamSetup) -> u8 {
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
        let mut c = Career {
            kind: Kind::Season,
            name: format!("{} 2021-22", league.name),
            ratings: teams.iter().map(|&r| strength(&TeamSetup::from_club(r.club()))).collect(),
            user: user.club,
            teams,
            rounds,
            round: 0,
            champion: None,
            budget: None,
            squad: Vec::new(),
            xi: Vec::new(),
            formation: String::new(),
            departed: Vec::new(),
        };
        c.ensure_squad();
        c
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
        let mut c = Career {
            kind: Kind::Cup,
            name: "Bigileee Champions Cup".into(),
            ratings: teams.iter().map(|&r| strength(&TeamSetup::from_club(r.club()))).collect(),
            user: user_idx,
            teams,
            rounds: vec![first],
            round: 0,
            champion: None,
            budget: None,
            squad: Vec::new(),
            xi: Vec::new(),
            formation: String::new(),
            departed: Vec::new(),
        };
        c.ensure_squad();
        c
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
        Career::from_json(&json)
    }

    /// Parses a save, filling in transfer and squad fields that older saves don't have.
    pub fn from_json(json: &str) -> Option<Career> {
        let mut c: Career = serde_json::from_str(json).ok()?;
        c.ensure_squad();
        Some(c)
    }

    pub fn delete_save() {
        let _ = std::fs::remove_file(save_path());
    }

    pub fn team_name(&self, t: usize) -> &'static str {
        self.teams[t].club().name
    }

    // ---------- squad and transfers ----------

    pub fn user_club(&self) -> ClubRef {
        self.teams[self.user]
    }

    pub fn budget(&self) -> u32 {
        self.budget.unwrap_or(0)
    }

    /// Gives a fresh (or pre-transfers) career its budget, squad, formation and XI.
    pub fn ensure_squad(&mut self) {
        let club = self.user_club();
        if self.budget.is_none() {
            self.budget = Some(starting_budget(club.club().rating));
        }
        if self.squad.is_empty() {
            let s = TeamSetup::from_club(club.club());
            self.squad = s.squad.iter().enumerate().map(|(idx, c)| Signing { player: PlayerRef { club, idx }, num: c.num }).collect();
            self.xi = s.xi.iter().map(|&k| self.squad[k].player).collect();
            self.formation = s.formation;
        }
        if self.formation.is_empty() {
            self.formation = club.club().formation.to_string();
        }
        self.fix_xi();
    }

    /// Cards for the human's squad, in squad order, wearing their squad numbers.
    pub fn squad_cards(&self) -> Vec<Card> {
        let mut cards = cards_of(self.squad.iter().map(|g| g.player));
        for (c, g) in cards.iter_mut().zip(&self.squad) {
            c.num = g.num;
        }
        cards
    }

    fn xi_indices(&self) -> Option<[usize; 11]> {
        if self.xi.len() != 11 {
            return None;
        }
        let mut out = [0usize; 11];
        for (k, r) in self.xi.iter().enumerate() {
            out[k] = self.squad.iter().position(|g| g.player == *r)?;
            if out[..k].contains(&out[k]) {
                return None;
            }
        }
        Some(out)
    }

    /// Keeps the XI valid after squad changes: anyone no longer at the club is replaced by the
    /// best available fit for his slot.
    fn fix_xi(&mut self) {
        let cards = self.squad_cards();
        let slots = formation::slots(&self.formation);
        let mut xi: Vec<Option<usize>> =
            (0..11).map(|k| self.xi.get(k).and_then(|r| self.squad.iter().position(|g| g.player == *r))).collect();
        for k in 0..11 {
            if xi[k].is_some() && xi[..k].contains(&xi[k]) {
                xi[k] = None;
            }
        }
        for k in 0..11 {
            if xi[k].is_none() {
                xi[k] = (0..cards.len()).filter(|i| !xi.contains(&Some(*i))).max_by_key(|&i| {
                    let wrong_keeper = (slots[k].pos == Pos::GK) != (cards[i].pos == Pos::GK);
                    cards[i].ovr as i32 - fit_penalty(slots[k].pos, cards[i].pos) - if wrong_keeper { 100 } else { 0 }
                });
            }
        }
        self.xi = xi.into_iter().flatten().map(|i| self.squad[i].player).collect();
    }

    /// Match setup for career team `t`, with every transfer applied.
    pub fn setup(&self, t: usize) -> TeamSetup {
        if t == self.user {
            self.user_setup()
        } else {
            self.club_setup(self.teams[t])
        }
    }

    pub fn user_setup(&self) -> TeamSetup {
        let mut s = TeamSetup::from_club(self.user_club().club());
        if self.squad.is_empty() {
            return s;
        }
        s.squad = self.squad_cards();
        if !self.formation.is_empty() {
            s.formation = self.formation.clone();
        }
        s.xi = self.xi_indices().unwrap_or_else(|| pick_xi(&s.squad, &s.slots()));
        s
    }

    /// A CPU club's squad minus anyone the human has bought from it.
    pub fn club_setup(&self, r: ClubRef) -> TeamSetup {
        let mut s = TeamSetup::from_club(r.club());
        let gone: Vec<usize> = self.departed.iter().filter(|p| p.club == r).map(|p| p.idx).collect();
        s.drop_players(&gone);
        s
    }

    fn refresh_ratings(&mut self) {
        self.ratings = (0..self.teams.len()).map(|t| strength(&self.setup(t))).collect();
    }

    /// Everyone the human could sign: every club in every league except his own, minus anyone
    /// who has already moved.
    pub fn market(&self) -> Vec<Listing> {
        let user = self.user_club();
        let mut out = Vec::new();
        for (l, lg) in data::leagues().iter().enumerate() {
            for (c, club) in lg.clubs.iter().enumerate() {
                let r = ClubRef { league: l, club: c };
                if r == user {
                    continue;
                }
                for (idx, card) in TeamSetup::from_club(club).squad.into_iter().enumerate() {
                    let player = PlayerRef { club: r, idx };
                    if self.departed.contains(&player) {
                        continue;
                    }
                    let value = market_value(card.ovr, card.pos);
                    out.push(Listing { player, card, club: club.name, value });
                }
            }
        }
        out
    }

    /// The human's own squad as sale listings.
    pub fn my_players(&self) -> Vec<Listing> {
        let club = self.user_club().club().name;
        self.squad
            .iter()
            .zip(self.squad_cards())
            .map(|(g, card)| Listing { player: g.player, value: market_value(card.ovr, card.pos), card, club })
            .collect()
    }

    /// Signs a player for his market value. Returns a message for the transfer screen.
    pub fn buy(&mut self, r: PlayerRef) -> Result<String, String> {
        self.ensure_squad();
        if r.club == self.user_club() || self.squad.iter().any(|g| g.player == r) || self.departed.contains(&r) {
            return Err("That player isn't available".into());
        }
        if self.squad.len() >= MAX_SQUAD {
            return Err(format!("Squad is full ({MAX_SQUAD} players): sell someone first"));
        }
        let card = cards_of(std::iter::once(r)).remove(0);
        let fee = market_value(card.ovr, card.pos);
        if fee > self.budget() {
            return Err(format!("Can't afford {} ({} needed, {} available)", card.name, money(fee), money(self.budget())));
        }
        self.budget = Some(self.budget() - fee);
        let taken = |n: u8| self.squad.iter().any(|g| g.num == n);
        let num = if taken(card.num) { (2..99u8).find(|n| !taken(*n)).unwrap_or(99) } else { card.num };
        self.squad.push(Signing { player: r, num });
        self.departed.push(r);
        self.refresh_ratings();
        Ok(format!("Signed {} from {} for {}", card.name, r.club.club().name, money(fee)))
    }

    /// Sells a squad member for his market value.
    pub fn sell(&mut self, r: PlayerRef) -> Result<String, String> {
        self.ensure_squad();
        let Some(k) = self.squad.iter().position(|g| g.player == r) else {
            return Err("He isn't in your squad".into());
        };
        if self.squad.len() <= MIN_SQUAD {
            return Err(format!("You need at least {MIN_SQUAD} players"));
        }
        let cards = self.squad_cards();
        let card = &cards[k];
        if card.pos == Pos::GK && cards.iter().filter(|c| c.pos == Pos::GK).count() <= 1 {
            return Err("You can't sell your only goalkeeper".into());
        }
        let fee = market_value(card.ovr, card.pos);
        let name = card.name.clone();
        self.budget = Some(self.budget() + fee);
        self.squad.remove(k);
        if !self.departed.contains(&r) {
            self.departed.push(r);
        }
        self.fix_xi();
        self.refresh_ratings();
        Ok(format!("Sold {name} for {}", money(fee)))
    }

    /// Switches formation, re-seating the current XI by best positional fit.
    pub fn set_formation(&mut self, name: &str) {
        let Some(xi) = self.xi_indices() else { return };
        let cards = self.squad_cards();
        let pos = std::array::from_fn(|k| cards[xi[k]].pos);
        let fit = refit(&pos, &[true; 11], &formation::slots(&self.formation), &formation::slots(name));
        self.xi = fit.iter().map(|&old| self.xi[old]).collect();
        self.formation = name.to_string();
        self.refresh_ratings();
    }

    /// Picks the strongest XI for the current formation.
    pub fn auto_pick(&mut self) {
        let xi = pick_xi(&self.squad_cards(), &formation::slots(&self.formation));
        self.xi = xi.iter().map(|&i| self.squad[i].player).collect();
        self.refresh_ratings();
    }

    /// Squad members not in the starting XI.
    pub fn bench(&self) -> Vec<PlayerRef> {
        self.squad.iter().map(|g| g.player).filter(|r| !self.xi.contains(r)).collect()
    }

    /// Swaps two XI slots, or puts a bench player into a slot.
    pub fn swap(&mut self, slot: usize, with: Swap) {
        match with {
            Swap::Slot(other) => self.xi.swap(slot, other),
            Swap::Bench(r) if self.squad.iter().any(|g| g.player == r) && !self.xi.contains(&r) => self.xi[slot] = r,
            Swap::Bench(_) => {}
        }
        self.refresh_ratings();
    }
}

#[derive(Clone, Copy, Debug)]
pub enum Swap {
    Slot(usize),
    Bench(PlayerRef),
}

// ---------- hub screens ----------

#[derive(Component)]
pub struct HubRoot;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Screen {
    #[default]
    Main,
    Squad,
    Transfers,
}

#[derive(Resource, Default)]
pub struct HubState {
    pub row: usize,
    pub dirty: bool,
    pub screen: Screen,
    pub squad: SquadUi,
    pub market: MarketUi,
}

/// Squad screen: column 0 holds formation, auto-pick and the XI; column 1 the bench.
#[derive(Default)]
pub struct SquadUi {
    pub col: usize,
    pub row: usize,
    /// First player chosen for a swap, as (column, row).
    pub picked: Option<(usize, usize)>,
}

const SQUAD_TOP: usize = 2;

/// Transfer screen: filter rows first, then the player list.
#[derive(Default)]
pub struct MarketUi {
    pub row: usize,
    pub sell: bool,
    pub league: Option<usize>,
    pub club: Option<usize>,
    /// Index into `POS_FILTERS`.
    pub pos: usize,
    pub by_value: bool,
    /// A deal awaiting a second press to confirm.
    pub pending: Option<PlayerRef>,
    pub msg: String,
    cache: Option<Vec<Listing>>,
}

const POS_FILTERS: [&str; 5] = ["All", "Goalkeepers", "Defenders", "Midfielders", "Forwards"];
const FILTER_ROWS: usize = 5;
const LIST_ROWS: usize = 16;

fn pos_group(p: Pos) -> usize {
    use Pos::*;
    match p {
        GK => 1,
        CB | LB | RB | LWB | RWB => 2,
        CDM | CM | CAM | LM | RM => 3,
        LW | RW | CF | ST => 4,
    }
}

/// The filtered, sorted list the transfer screen shows. The full market is cached between
/// presses and rebuilt after a deal.
fn market_view(c: &Career, ui: &mut MarketUi) -> Vec<Listing> {
    let mut list = if ui.sell {
        c.my_players()
    } else {
        ui.cache.get_or_insert_with(|| c.market()).clone()
    };
    list.retain(|l| {
        (ui.pos == 0 || pos_group(l.card.pos) == ui.pos)
            && (ui.sell || ui.league.is_none_or(|lg| l.player.club.league == lg))
            && (ui.sell || ui.club.is_none_or(|cl| l.player.club.club == cl))
    });
    if ui.by_value {
        list.sort_by(|a, b| b.value.cmp(&a.value).then(b.card.ovr.cmp(&a.card.ovr)));
    } else {
        list.sort_by(|a, b| b.card.ovr.cmp(&a.card.ovr).then(b.value.cmp(&a.value)));
    }
    list
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum HubAction {
    Play,
    Simulate,
    SimulateAll,
    Squad,
    Transfers,
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
    v.push((HubAction::Squad, "Squad & formation"));
    v.push((HubAction::Transfers, "Transfers"));
    v.push((HubAction::Quit, "Save and quit to title"));
    v
}

/// One frame of hub navigation from keyboard and gamepads.
struct HubNav {
    up: bool,
    down: bool,
    left: bool,
    right: bool,
    ok: bool,
    back: bool,
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
    let nav = HubNav {
        up: k(&[KeyCode::ArrowUp, KeyCode::KeyW]) || p(GamepadButton::DPadUp),
        down: k(&[KeyCode::ArrowDown, KeyCode::KeyS]) || p(GamepadButton::DPadDown),
        left: k(&[KeyCode::ArrowLeft, KeyCode::KeyA]) || p(GamepadButton::DPadLeft),
        right: k(&[KeyCode::ArrowRight, KeyCode::KeyD]) || p(GamepadButton::DPadRight),
        ok: k(&[KeyCode::Enter, KeyCode::Space]) || p(GamepadButton::South),
        back: k(&[KeyCode::Escape, KeyCode::Backspace]) || p(GamepadButton::East),
    };
    if !(nav.up || nav.down || nav.left || nav.right || nav.ok || nav.back) {
        return;
    }
    hub.dirty = true;
    match hub.screen {
        Screen::Main => main_input(&nav, &mut hub, &mut career, &mut settings, &mut next, &mut commands),
        Screen::Squad => squad_input(&nav, &mut hub, &mut career),
        Screen::Transfers => market_input(&nav, &mut hub, &mut career),
    }
}

fn main_input(
    nav: &HubNav,
    hub: &mut HubState,
    career: &mut Career,
    settings: &mut Settings,
    next: &mut NextState<AppState>,
    commands: &mut Commands,
) {
    let acts = actions(career);
    if nav.down {
        hub.row = (hub.row + 1) % acts.len();
    }
    if nav.up {
        hub.row = (hub.row + acts.len() - 1) % acts.len();
    }
    hub.row = hub.row.min(acts.len() - 1);
    if !nav.ok {
        return;
    }
    let action = acts[hub.row].0;
    match action {
        HubAction::Play => {
            let fx = career.user_fixture().unwrap();
            let f = career.rounds[career.round][fx].clone();
            settings.home_team = career.setup(f.home);
            settings.away_team = career.setup(f.away);
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
        HubAction::Squad => {
            hub.screen = Screen::Squad;
            hub.squad = SquadUi::default();
            return;
        }
        HubAction::Transfers => {
            hub.screen = Screen::Transfers;
            hub.market = MarketUi::default();
            return;
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

fn squad_input(nav: &HubNav, hub: &mut HubState, career: &mut Career) {
    let bench = career.bench();
    let ui = &mut hub.squad;
    if nav.back {
        if ui.picked.is_some() {
            ui.picked = None;
        } else {
            career.save();
            hub.screen = Screen::Main;
        }
        return;
    }
    let len = if ui.col == 0 { SQUAD_TOP + 11 } else { bench.len().max(1) };
    if nav.down {
        ui.row = (ui.row + 1) % len;
    }
    if nav.up {
        ui.row = (ui.row + len - 1) % len;
    }
    if ui.col == 0 && ui.row == 0 && (nav.left || nav.right) {
        let names = formation::NAMES;
        let k = names.iter().position(|f| *f == career.formation).unwrap_or(0);
        let d = if nav.right { 1 } else { names.len() - 1 };
        career.set_formation(names[(k + d) % names.len()]);
        return;
    }
    if nav.right && ui.col == 0 && !bench.is_empty() {
        ui.col = 1;
        ui.row = ui.row.saturating_sub(SQUAD_TOP).min(bench.len() - 1);
        return;
    }
    if nav.left && ui.col == 1 {
        ui.col = 0;
        ui.row = (ui.row + SQUAD_TOP).min(SQUAD_TOP + 10);
        return;
    }
    if !nav.ok {
        return;
    }
    if ui.col == 0 && ui.row == 1 {
        career.auto_pick();
        ui.picked = None;
        return;
    }
    if ui.col == 0 && ui.row == 0 {
        return;
    }
    let here = (ui.col, ui.row);
    let Some(first) = ui.picked.take() else {
        ui.picked = Some(here);
        return;
    };
    let slot = |(c, r): (usize, usize)| (c == 0).then(|| r - SQUAD_TOP);
    match (slot(first), slot(here)) {
        (Some(a), Some(b)) => career.swap(a, Swap::Slot(b)),
        (Some(a), None) => career.swap(a, Swap::Bench(bench[here.1])),
        (None, Some(b)) => career.swap(b, Swap::Bench(bench[first.1])),
        (None, None) => ui.picked = Some(here),
    }
    if career.bench().is_empty() && ui.col == 1 {
        ui.col = 0;
        ui.row = SQUAD_TOP;
    }
}

fn market_input(nav: &HubNav, hub: &mut HubState, career: &mut Career) {
    let ui = &mut hub.market;
    if nav.back {
        if ui.pending.is_some() {
            ui.pending = None;
            ui.msg.clear();
        } else {
            career.save();
            hub.screen = Screen::Main;
        }
        return;
    }
    let list = market_view(career, ui);
    let len = FILTER_ROWS + list.len();
    // League and club filters only apply when buying.
    let hidden = |ui: &MarketUi, r: usize| ui.sell && (r == 1 || r == 2) || (r == 2 && ui.league.is_none());
    if nav.down || nav.up {
        ui.pending = None;
        loop {
            ui.row = if nav.down { (ui.row + 1) % len } else { (ui.row + len - 1) % len };
            if !hidden(ui, ui.row) {
                break;
            }
        }
    }
    let d: i32 = if nav.right { 1 } else if nav.left { -1 } else { 0 };
    if d != 0 && ui.row < FILTER_ROWS {
        // Options cycle through "All" (None) and then each entry.
        let cycle = |v: Option<usize>, len: usize| {
            let k = v.map_or(0, |x| x + 1) as i32;
            let k = (k + d).rem_euclid(len as i32 + 1) as usize;
            k.checked_sub(1)
        };
        match ui.row {
            0 => ui.sell = !ui.sell,
            1 => {
                ui.league = cycle(ui.league, data::leagues().len());
                ui.club = None;
            }
            2 => {
                if let Some(lg) = ui.league {
                    ui.club = cycle(ui.club, data::leagues()[lg].clubs.len());
                }
            }
            3 => ui.pos = (ui.pos as i32 + d).rem_euclid(POS_FILTERS.len() as i32) as usize,
            _ => ui.by_value = !ui.by_value,
        }
        ui.pending = None;
        ui.msg.clear();
    }
    if !nav.ok || ui.row < FILTER_ROWS {
        return;
    }
    let Some(pick) = list.get(ui.row - FILTER_ROWS) else { return };
    if ui.pending != Some(pick.player) {
        ui.pending = Some(pick.player);
        ui.msg = if ui.sell {
            format!("Sell {} for {}?  Press Enter / A again to confirm, Esc / B to cancel", pick.card.name, money(pick.value))
        } else {
            format!("Sign {} from {} for {}?  Press Enter / A again to confirm, Esc / B to cancel", pick.card.name, pick.club, money(pick.value))
        };
        return;
    }
    ui.pending = None;
    let result = if ui.sell { career.sell(pick.player) } else { career.buy(pick.player) };
    ui.msg = match result {
        Ok(m) => {
            ui.cache = None;
            career.save();
            m
        }
        Err(e) => e,
    };
    let len = FILTER_ROWS + market_view(career, ui).len();
    ui.row = ui.row.min(len - 1);
}

const GOLD: Color = Color::srgb(0.97, 0.78, 0.25);
const DIM: Color = Color::srgba(1.0, 1.0, 1.0, 0.6);

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
    match hub.screen {
        Screen::Main => draw_main(&mut commands, &fonts, &mut images, &hub, &c),
        Screen::Squad => draw_squad(&mut commands, &fonts, &hub, &c),
        Screen::Transfers => {
            let list = market_view(&c, &mut hub.market);
            draw_market(&mut commands, &fonts, &hub.market, &c, &list);
        }
    }
}

fn hub_root<'a>(commands: &'a mut Commands) -> EntityCommands<'a> {
    commands.spawn((
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
}

/// A list row: gold when under the cursor, teal when picked for a swap.
fn row_bg(cursor: bool, picked: bool) -> BackgroundColor {
    BackgroundColor(if cursor {
        GOLD
    } else if picked {
        Color::srgba(0.3, 0.8, 0.75, 0.35)
    } else {
        Color::srgba(1.0, 1.0, 1.0, 0.04)
    })
}

fn cells(row: &mut ChildSpawnerCommands, fonts: &Fonts, cols: &[(String, f32)], colour: Color) {
    for (s, w) in cols {
        row.spawn((Node { width: Val::Px(*w), overflow: Overflow::clip(), ..default() }, text(fonts, s.clone(), 15.0, colour)));
    }
}

fn draw_squad(commands: &mut Commands, fonts: &Fonts, hub: &HubState, c: &Career) {
    let ui = &hub.squad;
    let cards = c.squad_cards();
    let card = |r: &PlayerRef| &cards[c.squad.iter().position(|g| g.player == *r).unwrap()];
    let slots = formation::slots(&c.formation);
    let bench = c.bench();
    let club = c.user_club().club();
    let ovr = c.xi.iter().map(|r| card(r).ovr as u32).sum::<u32>() / c.xi.len().max(1) as u32;
    hub_root(commands).with_children(|root| {
        root.spawn(text(fonts, format!("SQUAD  ·  {}", club.name.to_uppercase()), 36.0, Color::WHITE));
        root.spawn(text(fonts, format!("{} players  ·  starting XI overall {}  ·  budget {}", c.squad.len(), ovr, money(c.budget())), 18.0, GOLD));
        root.spawn(Node { column_gap: Val::Px(36.0), align_items: AlignItems::FlexStart, ..default() }).with_children(|cols| {
            cols.spawn(Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(3.0), width: Val::Px(560.0), ..default() }).with_children(|left| {
                let top = [format!("Formation   <  {}  >", c.formation), "Auto-pick best XI".to_string()];
                for (r, label) in top.iter().enumerate() {
                    let sel = ui.col == 0 && ui.row == r;
                    left.spawn((Node { padding: UiRect::axes(Val::Px(12.0), Val::Px(6.0)), ..default() }, row_bg(sel, false)))
                        .with_child(text(fonts, label.clone(), 18.0, if sel { Color::BLACK } else { Color::WHITE }));
                }
                left.spawn((Node { margin: UiRect::top(Val::Px(8.0)), ..default() }, text(fonts, "STARTING XI", 14.0, DIM)));
                for (k, r) in c.xi.iter().enumerate() {
                    let p = card(r);
                    let row = SQUAD_TOP + k;
                    let sel = ui.col == 0 && ui.row == row;
                    let colour = if sel { Color::BLACK } else { Color::WHITE };
                    let fit = if fit_penalty(slots[k].pos, p.pos) > 2 { format!("({:?})", p.pos) } else { String::new() };
                    left.spawn((Node { padding: UiRect::axes(Val::Px(12.0), Val::Px(3.0)), ..default() }, row_bg(sel, ui.picked == Some((0, row)))))
                        .with_children(|row| {
                            let name = if p.generated { format!("{} *", p.name) } else { p.name.clone() };
                            cells(row, fonts, &[(format!("{:?}", slots[k].pos), 56.0), (p.num.to_string(), 34.0), (name, 260.0), (fit, 70.0), (p.ovr.to_string(), 40.0)], colour);
                        });
                }
            });
            cols.spawn(Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(3.0), width: Val::Px(560.0), ..default() }).with_children(|right| {
                right.spawn(text(fonts, "BENCH AND RESERVES", 14.0, DIM));
                for (b, r) in bench.iter().enumerate() {
                    let p = card(r);
                    let sel = ui.col == 1 && ui.row == b;
                    let colour = if sel { Color::BLACK } else { Color::WHITE };
                    right.spawn((Node { padding: UiRect::axes(Val::Px(12.0), Val::Px(3.0)), ..default() }, row_bg(sel, ui.picked == Some((1, b)))))
                        .with_children(|row| {
                            let name = if p.generated { format!("{} *", p.name) } else { p.name.clone() };
                            cells(row, fonts, &[(format!("{:?}", p.pos), 56.0), (p.num.to_string(), 34.0), (name, 290.0), (p.ovr.to_string(), 40.0), (money(market_value(p.ovr, p.pos)), 90.0)], colour);
                        });
                }
            });
        });
        let hint = if ui.picked.is_some() {
            "Pick the player to swap with  ·  Esc / B cancel"
        } else {
            "Up/Down choose  ·  Left/Right change formation or switch list  ·  Enter / A pick a player to swap  ·  Esc / B back"
        };
        root.spawn(text(fonts, hint, 15.0, Color::srgba(1.0, 1.0, 1.0, 0.45)));
    });
}

fn draw_market(commands: &mut Commands, fonts: &Fonts, ui: &MarketUi, c: &Career, list: &[Listing]) {
    let leagues = data::leagues();
    hub_root(commands).with_children(|root| {
        root.spawn(text(fonts, "TRANSFERS", 36.0, Color::WHITE));
        root.spawn(text(
            fonts,
            format!("{}  ·  budget {}  ·  squad {} / {}", c.user_club().club().name, money(c.budget()), c.squad.len(), MAX_SQUAD),
            18.0,
            GOLD,
        ));
        let filters = [
            ("View", if ui.sell { "Sell from my squad".to_string() } else { "Buy".to_string() }),
            ("League", ui.league.map_or("All leagues".to_string(), |l| leagues[l].name.to_string())),
            ("Club", ui.league.zip(ui.club).map_or("All clubs".to_string(), |(l, k)| leagues[l].clubs[k].name.to_string())),
            ("Position", POS_FILTERS[ui.pos].to_string()),
            ("Sort", if ui.by_value { "Value".to_string() } else { "Overall".to_string() }),
        ];
        root.spawn(Node { column_gap: Val::Px(14.0), ..default() }).with_children(|o| {
            for (k, (label, value)) in filters.iter().enumerate() {
                if ui.sell && (k == 1 || k == 2) || (k == 2 && ui.league.is_none()) {
                    continue;
                }
                let sel = ui.row == k;
                o.spawn((
                    Node { padding: UiRect::axes(Val::Px(14.0), Val::Px(7.0)), column_gap: Val::Px(8.0), ..default() },
                    BackgroundColor(if sel { Color::srgba(1.0, 1.0, 1.0, 0.14) } else { Color::srgba(1.0, 1.0, 1.0, 0.04) }),
                ))
                .with_children(|b| {
                    b.spawn(text(fonts, *label, 16.0, DIM));
                    let v = if sel { format!("<  {value}  >") } else { value.clone() };
                    b.spawn(text(fonts, v, 16.0, if sel { GOLD } else { Color::WHITE }));
                });
            }
        });
        root.spawn(Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(3.0), width: Val::Px(1080.0), ..default() }).with_children(|t| {
            t.spawn(Node { padding: UiRect::horizontal(Val::Px(12.0)), ..default() }).with_children(|row| {
                let head = [("POS", 56.0), ("PLAYER", 330.0), ("OVR", 50.0), ("PAC SHO PAS DRI DEF", 220.0), ("CLUB", 280.0), ("VALUE", 100.0)];
                cells(row, fonts, &head.map(|(s, w)| (s.to_string(), w)), DIM);
            });
            if list.is_empty() {
                t.spawn(text(fonts, "   Nobody matches these filters", 15.0, DIM));
            }
            let sel = ui.row.checked_sub(FILTER_ROWS);
            let start = sel.unwrap_or(0).saturating_sub(LIST_ROWS / 2).min(list.len().saturating_sub(LIST_ROWS));
            for (k, l) in list.iter().enumerate().skip(start).take(LIST_ROWS) {
                let cursor = sel == Some(k);
                let pending = ui.pending == Some(l.player);
                let colour = if cursor { Color::BLACK } else if l.value > c.budget() && !ui.sell { Color::srgba(1.0, 1.0, 1.0, 0.45) } else { Color::WHITE };
                let p = &l.card;
                t.spawn((Node { padding: UiRect::axes(Val::Px(12.0), Val::Px(3.0)), ..default() }, row_bg(cursor, pending))).with_children(|row| {
                    let name = if p.generated { format!("{} *", p.name) } else { p.name.clone() };
                    let stats = format!("{:>3} {:>3} {:>3} {:>3} {:>3}", p.pac, p.sho, p.pas, p.dri, p.def);
                    cells(
                        row,
                        fonts,
                        &[(format!("{:?}", p.pos), 56.0), (name, 330.0), (p.ovr.to_string(), 50.0), (stats, 220.0), (l.club.to_string(), 280.0), (money(l.value), 100.0)],
                        colour,
                    );
                });
            }
            if list.len() > LIST_ROWS {
                let shown = (start + 1, (start + LIST_ROWS).min(list.len()));
                t.spawn(text(fonts, format!("   {}-{} of {}", shown.0, shown.1, list.len()), 14.0, DIM));
            }
        });
        if !ui.msg.is_empty() {
            root.spawn(text(fonts, ui.msg.clone(), 18.0, GOLD));
        }
        root.spawn(text(
            fonts,
            "Up/Down choose  ·  Left/Right change filter  ·  Enter / A buy or sell (press twice)  ·  Esc / B back      * = generated player",
            15.0,
            Color::srgba(1.0, 1.0, 1.0, 0.45),
        ));
    });
}

fn draw_main(commands: &mut Commands, fonts: &Fonts, images: &mut Assets<Image>, hub: &HubState, c: &Career) {
    let dim = DIM;
    let user_club = c.teams[c.user].club();
    let crest = |images: &mut Assets<Image>, t: usize| {
        let club = c.teams[t].club();
        images.add(textures::crest(club.crest, club.home, club.trim, 96))
    };
    hub_root(commands)
        .with_children(|root| {
            root.spawn(text(fonts, c.name.to_uppercase(), 40.0, Color::WHITE));
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
            root.spawn(text(fonts, status, 24.0, GOLD));
            root.spawn(text(fonts, format!("Budget {}  ·  {} players  ·  {}", money(c.budget()), c.squad.len(), c.formation), 16.0, dim));

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
                                            t_.spawn((Node { width: Val::Px(84.0), height: Val::Px(84.0), ..default() }, ImageNode::new(crest(images, *t))));
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

    /// Buying and selling move players and money, the selling club loses the player, the
    /// user's match squad reflects it, and all of it survives a save/load round trip.
    #[test]
    fn transfers_update_squads_and_budget_and_persist() {
        crate::sim::rng::srand(4);
        let mut c = Career::new_season(ClubRef { league: 0, club: 3 });
        let start_budget = c.budget();
        let start_len = c.squad.len();
        assert_eq!(start_budget, starting_budget(c.user_club().club().rating));
        assert_eq!(c.xi.len(), 11);

        // The best affordable player from another league.
        let market = c.market();
        assert!(market.iter().all(|l| l.player.club != c.user_club()));
        let target = market
            .iter()
            .filter(|l| l.player.club.league == 2 && l.value <= c.budget() && !l.card.generated && l.card.pos != Pos::GK)
            .max_by_key(|l| l.card.ovr)
            .expect("something affordable")
            .clone();
        let seller = target.player.club;
        let before = c.club_setup(seller);
        assert!(before.squad.iter().any(|p| p.name == target.card.name));

        let msg = c.buy(target.player).expect("purchase");
        println!("{msg}");
        assert_eq!(c.budget(), start_budget - target.value);
        assert_eq!(c.squad.len(), start_len + 1);
        assert!(c.buy(target.player).is_err(), "can't buy the same player twice");
        assert!(!c.market().iter().any(|l| l.player == target.player), "gone from the market");
        let after = c.club_setup(seller);
        assert!(!after.squad.iter().any(|p| p.name == target.card.name), "selling club lost him");
        assert!(after.squad.len() >= 16 && after.squad[after.xi[0]].pos == Pos::GK);
        assert!(c.user_setup().squad.iter().any(|p| p.name == target.card.name), "he plays for the user");

        // Put him in the XI, then sell someone else from the bench.
        let slot = (1..11).find(|&k| formation::slots(&c.formation)[k].pos == target.card.pos).unwrap_or(10);
        c.swap(slot, Swap::Bench(target.player));
        assert_eq!(c.xi[slot], target.player);
        let sold = c.bench()[0];
        let sold_name = c.my_players().iter().find(|l| l.player == sold).unwrap().card.name.clone();
        let fee = c.my_players().iter().find(|l| l.player == sold).unwrap().value;
        let budget = c.budget();
        c.sell(sold).expect("sale");
        assert_eq!(c.budget(), budget + fee);
        assert!(!c.squad.iter().any(|g| g.player == sold));
        assert!(!c.user_setup().squad.iter().any(|p| p.name == sold_name));

        // Change formation: same eleven, keeper first.
        let xi_before: Vec<PlayerRef> = c.xi.clone();
        c.set_formation("3-5-2");
        assert_eq!(c.formation, "3-5-2");
        assert_eq!(c.xi[0], xi_before[0]);
        let (mut a, mut b) = (c.xi.clone(), xi_before);
        a.sort_by_key(|r| (r.club.league, r.club.club, r.idx));
        b.sort_by_key(|r| (r.club.league, r.club.club, r.idx));
        assert_eq!(a, b);

        // Round trip through the save format.
        let json = serde_json::to_string(&c).unwrap();
        let back = Career::from_json(&json).expect("loads");
        assert_eq!(back.budget, c.budget);
        assert_eq!(back.squad, c.squad);
        assert_eq!(back.xi, c.xi);
        assert_eq!(back.formation, c.formation);
        assert_eq!(back.departed, c.departed);
        let setup = back.user_setup();
        assert_eq!(setup.formation, "3-5-2");
        assert!(setup.xi.iter().any(|&i| setup.squad[i].name == target.card.name), "new signing still starts");
        assert!(!back.club_setup(seller).squad.iter().any(|p| p.name == target.card.name));

        // Selling can't strip the squad bare.
        let mut d = back.clone();
        while d.squad.len() > MIN_SQUAD {
            let r = d.bench().first().copied().unwrap_or(d.xi[10]);
            if d.sell(r).is_err() {
                break;
            }
        }
        assert!(d.squad.len() >= MIN_SQUAD);
        assert_eq!(d.xi.len(), 11);
    }

    /// Saves written before transfers existed still load, with a budget and the club's squad.
    #[test]
    fn old_saves_load() {
        let c = Career::new_cup(ClubRef { league: 1, club: 0 });
        let mut v = serde_json::to_value(&c).unwrap();
        for k in ["budget", "squad", "xi", "formation", "departed"] {
            v.as_object_mut().unwrap().remove(k);
        }
        let old = Career::from_json(&v.to_string()).expect("old save loads");
        assert_eq!(old.budget(), c.budget());
        assert_eq!(old.squad, c.squad);
        assert_eq!(old.xi.len(), 11);
        assert_eq!(old.formation, c.user_club().club().formation);
    }
}
