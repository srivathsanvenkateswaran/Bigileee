//! Match state, rules, ball physics and player movement.

use super::consts::*;
pub use super::events::Sfx;
pub use super::formation::Role;
use super::input::Input;
use super::rng::gen_range;
use super::squad::TeamSetup;
use bevy::math::{vec2, Vec2};

pub struct Player {
    pub team: usize,
    pub num: u8,
    pub name: String,
    pub role: Role,
    /// Formation slot in the team's own frame (attacking towards +x).
    pub base: Vec2,
    pub pos: Vec2,
    pub vel: Vec2,
    pub facing: Vec2,
    pub stamina: f32,
    pub pace: f32,
    pub shooting: f32,
    pub passing: f32,
    pub defending: f32,
    pub dribbling: f32,
    /// Goalkeeping ability, 0..1 (only meaningful for keepers).
    pub keeping: f32,
    pub ovr: u8,
    /// Index into the team's squad.
    pub card: usize,
    /// Counts down after a kick, for the striking animation.
    pub kick_anim: f32,
    pub stun: f32,
    pub tackle_cd: f32,
    pub slide: f32,
    pub slide_hit: bool,
    pub dive: f32,
    /// Seconds during which this player cannot touch the ball (after kicking or being beaten).
    pub no_touch: f32,
    /// Seconds a goalkeeper has been holding the ball.
    pub hold: f32,
    pub think: f32,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Restart {
    KickOff,
    ThrowIn,
    GoalKick,
    Corner,
    FreeKick,
    Penalty,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Phase {
    SetPiece { kind: Restart, team: usize, spot: Vec2, timer: f32 },
    Play,
    Goal { timer: f32 },
    HalfTime { timer: f32 },
    FullTime,
}

/// A restart in progress: the taker owns the ball and may not dribble until it is played.
#[derive(Clone, Copy)]
pub struct Lock {
    pub taker: usize,
    pub kind: Restart,
    pub team: usize,
    pub spot: Vec2,
    pub t: f32,
}

pub struct Ball {
    pub pos: Vec2,
    pub z: f32,
    pub vel: Vec2,
    pub vz: f32,
    pub owner: Option<usize>,
    pub touch_team: usize,
    pub kicker: Option<usize>,
    pub pass_target: Option<usize>,
    pub pass_point: Vec2,
    pub shot: bool,
    pub spin: f32,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PassKind {
    Ground,
    Through,
    Lob,
}

#[derive(Clone, Copy, Debug)]
pub enum Action {
    Pass(PassKind, Vec2),
    Shoot(f32, Vec2),
    Tackle,
    Slide,
    Dive(Vec2),
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    VsCpu,
    TwoPlayer,
    Watch,
}

#[derive(Clone)]
pub struct MatchConfig {
    pub mode: Mode,
    pub half_secs: f32,
    /// CPU skill, 0..1.
    pub difficulty: f32,
}

pub struct Human {
    pub team: usize,
    pub player: usize,
    pub charge: f32,
    pub charging: bool,
    pub buffered: Option<(Action, f32)>,
}

pub struct Match {
    pub teams: [TeamSetup; 2],
    /// Shirt colour each side wears (0xRRGGBB), chosen to avoid a clash.
    pub kit: [u32; 2],
    /// Attackers caught offside when the last pass was played.
    pub offside: Vec<usize>,
    pub players: Vec<Player>,
    pub ball: Ball,
    pub phase: Phase,
    pub score: [u32; 2],
    pub shots: [u32; 2],
    pub possession: [f32; 2],
    pub clock: f32,
    pub half: u8,
    pub cfg: MatchConfig,
    pub humans: Vec<Human>,
    pub banner: Option<(String, f32)>,
    pub lock: Option<Lock>,
    pub setpiece_taker: Option<usize>,
    pub first_kickoff: usize,
    pub skill: [f32; 2],
    pub scorers: Vec<(usize, String, u32)>,
}

pub fn rotate_towards(cur: Vec2, want: Vec2, max_angle: f32) -> Vec2 {
    if want.length_squared() < 1e-6 {
        return cur;
    }
    let want = want.normalize();
    let angle = cur.angle_between(want);
    if angle.abs() <= max_angle {
        want
    } else {
        Vec2::from_angle(max_angle * angle.signum()).rotate(cur).normalize()
    }
}

pub fn dist_to_segment(p: Vec2, a: Vec2, b: Vec2) -> f32 {
    let ab = b - a;
    let t = ((p - a).dot(ab) / ab.length_squared().max(1e-6)).clamp(0.0, 1.0);
    p.distance(a + ab * t)
}

fn gauss() -> f32 {
    (0..4).map(|_| gen_range(-1.0f32, 1.0)).sum::<f32>() * 0.5
}

impl Match {
    pub fn new(cfg: MatchConfig, teams: [TeamSetup; 2]) -> Match {
        let mut players = Vec::with_capacity(PLAYERS);
        for (t, setup) in teams.iter().enumerate() {
            let slots = setup.slots();
            for (k, slot) in slots.iter().enumerate() {
                let c = &setup.squad[setup.xi[k]];
                let f = |v: u8| v as f32 / 100.0;
                players.push(Player {
                    team: t,
                    num: c.num,
                    name: c.name.clone(),
                    role: slot.role,
                    base: slot.at,
                    pos: slot.at,
                    vel: Vec2::ZERO,
                    facing: vec2(1.0, 0.0),
                    stamina: 1.0,
                    pace: f(c.pac),
                    shooting: f(c.sho),
                    passing: f(c.pas),
                    defending: f(c.def),
                    dribbling: f(c.dri),
                    keeping: if slot.role == Role::Gk { f(c.ovr) } else { 0.0 },
                    ovr: c.ovr,
                    card: setup.xi[k],
                    kick_anim: 0.0,
                    stun: 0.0,
                    tackle_cd: 0.0,
                    slide: 0.0,
                    slide_hit: false,
                    dive: 0.0,
                    no_touch: 0.0,
                    hold: 0.0,
                    think: gen_range(0.0, 0.3),
                });
            }
        }
        let kit = choose_kits(&teams);

        let mut humans = Vec::new();
        match cfg.mode {
            Mode::VsCpu => humans.push(Human::new(0, 10)),
            Mode::TwoPlayer => {
                humans.push(Human::new(0, 10));
                humans.push(Human::new(1, PER_TEAM + 10));
            }
            Mode::Watch => {}
        }

        let skill = match cfg.mode {
            Mode::VsCpu => [0.6, cfg.difficulty],
            Mode::TwoPlayer => [0.6, 0.6],
            Mode::Watch => [cfg.difficulty, cfg.difficulty],
        };

        let first_kickoff = if gen_range(0, 2) == 0 { 0 } else { 1 };
        let mut m = Match {
            teams,
            kit,
            offside: Vec::new(),
            players,
            ball: Ball {
                pos: Vec2::ZERO,
                z: 0.0,
                vel: Vec2::ZERO,
                vz: 0.0,
                owner: None,
                touch_team: first_kickoff,
                kicker: None,
                pass_target: None,
                pass_point: Vec2::ZERO,
                shot: false,
                spin: 0.0,
            },
            phase: Phase::Play,
            score: [0, 0],
            shots: [0, 0],
            possession: [0.0, 0.0],
            clock: 0.0,
            half: 1,
            cfg,
            humans,
            banner: None,
            lock: None,
            setpiece_taker: None,
            first_kickoff,
            skill,
            scorers: Vec::new(),
        };
        m.begin_restart(Restart::KickOff, first_kickoff, Vec2::ZERO, None);
        m.snap_to_setpiece();
        m
    }

    // ---------- frames of reference ----------

    /// +1 if `team` attacks towards +x this half, -1 otherwise.
    pub fn dir(&self, team: usize) -> f32 {
        if (team == 0) == (self.half == 1) {
            1.0
        } else {
            -1.0
        }
    }

    /// Converts between world space and a team's frame (where it attacks +x). The map is its own inverse.
    pub fn frame(&self, team: usize, v: Vec2) -> Vec2 {
        v * self.dir(team)
    }

    pub fn opp_goal(&self, team: usize) -> Vec2 {
        vec2(HALF_L * self.dir(team), 0.0)
    }

    pub fn own_goal(&self, team: usize) -> Vec2 {
        vec2(-HALF_L * self.dir(team), 0.0)
    }

    pub fn team_range(team: usize) -> std::ops::Range<usize> {
        team * PER_TEAM..(team + 1) * PER_TEAM
    }

    pub fn gk(team: usize) -> usize {
        team * PER_TEAM
    }

    pub fn in_box(&self, defending: usize, p: Vec2) -> bool {
        let f = self.frame(defending, p);
        f.x < -HALF_L + BOX_D && f.x > -HALF_L - 1.0 && f.y.abs() < BOX_HW
    }

    pub fn is_human(&self, i: usize) -> bool {
        self.humans.iter().any(|h| h.player == i)
    }

    pub fn human_of_team(&self, team: usize) -> Option<usize> {
        self.humans.iter().position(|h| h.team == team)
    }

    pub fn display_minute(&self) -> (u32, u32) {
        let total = (self.clock / self.cfg.half_secs).min(1.0) * 45.0 * 60.0
            + if self.half == 2 { 45.0 * 60.0 } else { 0.0 };
        ((total / 60.0) as u32, (total % 60.0) as u32)
    }

    fn show(&mut self, text: impl Into<String>, secs: f32) {
        self.banner = Some((text.into(), secs));
    }

    // ---------- restarts ----------

    fn choose_taker(&self, kind: Restart, team: usize, spot: Vec2) -> usize {
        let range = Self::team_range(team);
        match kind {
            Restart::KickOff => range.start + 10,
            Restart::GoalKick => Self::gk(team),
            Restart::Penalty => range
                .skip(1)
                .max_by(|&a, &b| self.players[a].shooting.total_cmp(&self.players[b].shooting))
                .unwrap(),
            _ => range
                .skip(1)
                .min_by(|&a, &b| {
                    self.players[a].pos.distance(spot).total_cmp(&self.players[b].pos.distance(spot))
                })
                .unwrap(),
        }
    }

    fn begin_restart(&mut self, kind: Restart, team: usize, spot: Vec2, sfx: Option<&Sfx>) {
        self.ball.pos = spot;
        self.ball.vel = Vec2::ZERO;
        self.ball.z = 0.0;
        self.ball.vz = 0.0;
        self.ball.owner = None;
        self.ball.pass_target = None;
        self.ball.shot = false;
        self.ball.touch_team = team;
        self.lock = None;
        for p in &mut self.players {
            p.slide = 0.0;
            p.dive = 0.0;
            p.stun = 0.0;
            p.hold = 0.0;
        }
        let taker = self.choose_taker(kind, team, spot);
        self.setpiece_taker = Some(taker);
        self.phase = Phase::SetPiece { kind, team, spot, timer: if kind == Restart::KickOff { 1.2 } else { 1.6 } };

        let label = match kind {
            Restart::KickOff => None,
            Restart::ThrowIn => Some("THROW-IN"),
            Restart::GoalKick => Some("GOAL KICK"),
            Restart::Corner => Some("CORNER"),
            Restart::FreeKick => Some("FREE KICK"),
            Restart::Penalty => Some("PENALTY!"),
        };
        if let Some(l) = label {
            self.show(format!("{}  ·  {}", l, self.teams[team].short), 1.6);
        }
        if let (Some(s), Restart::FreeKick | Restart::Penalty) = (sfx, kind) {
            s.whistle();
        }

        // Hand human control to the taker, or to the defender nearest the ball.
        for h in 0..self.humans.len() {
            let ht = self.humans[h].team;
            let pick = if ht == team && taker != Self::gk(team) {
                taker
            } else {
                self.nearest_outfield(ht, spot, None)
            };
            self.humans[h].player = pick;
            self.humans[h].charging = false;
            self.humans[h].charge = 0.0;
            self.humans[h].buffered = None;
        }
    }

    fn nearest_outfield(&self, team: usize, to: Vec2, exclude: Option<usize>) -> usize {
        Self::team_range(team)
            .skip(1)
            .filter(|&j| Some(j) != exclude)
            .min_by(|&a, &b| self.players[a].pos.distance(to).total_cmp(&self.players[b].pos.distance(to)))
            .unwrap()
    }

    fn setpiece_target(&self, i: usize, kind: Restart, team: usize, spot: Vec2) -> (Vec2, Vec2) {
        let p = &self.players[i];
        let t = p.team;
        let d = self.dir(t);
        let face_goal = vec2(d, 0.0);

        if Some(i) == self.setpiece_taker {
            let approach = match kind {
                Restart::ThrowIn => vec2(0.0, -spot.y.signum()),
                Restart::Corner => (self.opp_goal(t) - vec2(11.0 * d, 0.0) - spot).normalize(),
                _ => (self.opp_goal(t) - spot).normalize_or(face_goal),
            };
            let back = if kind == Restart::ThrowIn { 0.0 } else { 0.8 };
            return (spot - approach * back, approach);
        }

        if p.role == Role::Gk {
            let line = if kind == Restart::Penalty { 0.2 } else { 1.5 };
            let g = self.own_goal(t) + vec2(line * d, 0.0);
            return (g, face_goal);
        }

        let mut target = match kind {
            Restart::KickOff => {
                let mut f = p.base;
                f.x = f.x.min(-1.5);
                if t == team && i == Self::team_range(t).start + 9 {
                    f = vec2(-1.0, 7.0);
                }
                let mut w = self.frame(t, f);
                if t != team && w.length() < CIRCLE_R + 0.5 {
                    w = w.normalize_or(vec2(-d, 0.0)) * (CIRCLE_R + 0.5);
                }
                w
            }
            Restart::Corner => {
                let attacking = t == team;
                let k = i - Self::team_range(t).start;
                let slots_att = [(0, 0.0), (9, 44.0), (10, 45.5), (6, 40.0), (7, 41.0), (2, 38.0)];
                let ys_att = [0.0, -3.5, 3.0, -6.0, 6.0, 0.0];
                let slots_def = [(1, -47.0), (2, -47.5), (3, -47.5), (4, -47.0), (6, -44.0), (7, -44.0)];
                let ys_def = [-6.0, -2.0, 2.0, 6.0, -4.0, 4.0];
                let (slots, ys) = if attacking { (&slots_att, &ys_att) } else { (&slots_def, &ys_def) };
                let slot = slots.iter().position(|s| s.0 == k && s.1 != 0.0);
                match slot {
                    Some(n) => self.frame(t, vec2(slots[n].1, ys[n] + gen_range(-0.5, 0.5))),
                    None => self.formation_target(i),
                }
            }
            _ => self.formation_target(i),
        };

        // Opponents give the taker room.
        if t != team {
            let r = if kind == Restart::ThrowIn { 3.0 } else { CIRCLE_R + 0.3 };
            if kind != Restart::KickOff && target.distance(spot) < r {
                target = spot + (target - spot).normalize_or(vec2(-self.dir(team), 0.0)) * r;
            }
        }
        // Goal kicks and penalties clear the box.
        let box_team = match kind {
            Restart::GoalKick => Some(team),
            Restart::Penalty => Some(1 - team),
            _ => None,
        };
        if let Some(bt) = box_team {
            if self.in_box(bt, target) {
                let mut f = self.frame(bt, target);
                f.x = -HALF_L + BOX_D + 1.5 + gen_range(0.0, 3.0);
                target = self.frame(bt, f);
            }
        }
        let face = (self.ball.pos - target).normalize_or(face_goal);
        (target, face)
    }

    fn snap_to_setpiece(&mut self) {
        if let Phase::SetPiece { kind, team, spot, .. } = self.phase {
            for i in 0..PLAYERS {
                let (t, f) = self.setpiece_target(i, kind, team, spot);
                let p = &mut self.players[i];
                if p.pos.distance(t) > 1.5 || Some(i) == self.setpiece_taker {
                    p.pos = t;
                }
                p.vel = Vec2::ZERO;
                p.facing = f;
            }
        }
    }

    // ---------- tactical positioning ----------

    pub fn possession_team(&self) -> usize {
        match self.ball.owner {
            Some(o) => self.players[o].team,
            None => self.ball.touch_team,
        }
    }

    /// The deepest line an attacker of `team` may run to: the second-last defender, or halfway.
    fn offside_line(&self, team: usize) -> f32 {
        let mut xs: Vec<f32> = Self::team_range(1 - team)
            .map(|j| self.frame(team, self.players[j].pos).x)
            .collect();
        xs.sort_by(|a, b| b.total_cmp(a));
        xs[1].max(0.0)
    }

    pub fn formation_target(&self, i: usize) -> Vec2 {
        let p = &self.players[i];
        let t = p.team;
        let b = self.frame(t, self.ball.pos);
        let attacking = self.possession_team() == t;
        let shift = if attacking { 9.0 } else { -5.0 };
        let mut x = p.base.x + b.x * 0.62 + shift;
        let (lo, hi) = match p.role {
            Role::Gk => (-51.0, -40.0),
            Role::Def => (-47.0, 22.0),
            Role::Mid => (-42.0, 38.0),
            Role::Fwd => (-25.0, 46.0),
        };
        x = x.clamp(lo, hi);
        if matches!(p.role, Role::Fwd | Role::Mid) {
            x = x.min(self.offside_line(t).max(b.x) - 0.5);
        }
        if !attacking && p.role == Role::Def {
            // Defenders hold a line that never sits deeper than the ball allows.
            x = x.min(b.x - 4.0).max(-47.0);
        }
        let width = if attacking { 1.1 } else { 0.72 };
        let y = (p.base.y * width + b.y * 0.33).clamp(-HALF_W + 2.0, HALF_W - 2.0);
        self.frame(t, vec2(x, y))
    }

    pub fn max_speed(&self, i: usize, sprint: bool) -> f32 {
        let p = &self.players[i];
        let mut s = 5.4 + p.pace * 1.7;
        if sprint && p.stamina > 0.05 {
            s *= 1.33;
        }
        if self.ball.owner == Some(i) {
            s *= 0.88;
        }
        s * (0.94 + self.skill[p.team] * 0.1)
    }

    // ---------- main update ----------

    /// Advances the match by `dt` seconds. `inputs[k]` drives `humans[k]`.
    pub fn update(&mut self, dt: f32, inputs: &[Input], sfx: &Sfx) {
        if let Some((_, t)) = &mut self.banner {
            *t -= dt;
            if *t <= 0.0 {
                self.banner = None;
            }
        }
        if self.phase == Phase::FullTime {
            return;
        }
        let steps = (dt / (1.0 / 120.0)).ceil().max(1.0) as usize;
        let h = dt / steps as f32;
        for s in 0..steps {
            let frame_inputs: Vec<Input> = inputs.iter().map(|i| if s == 0 { *i } else { i.held_only() }).collect();
            self.step(h, &frame_inputs, sfx);
        }
    }

    pub(crate)     fn step(&mut self, h: f32, inputs: &[Input], sfx: &Sfx) {
        match self.phase {
            Phase::Goal { timer } => {
                self.ball_physics(h, true);
                self.move_players(h, &self.celebration_targets(), &[false; PLAYERS], None);
                let timer = timer - h;
                if timer <= 0.0 {
                    let conceded = 1 - self.ball.touch_team_for_goal();
                    self.begin_restart(Restart::KickOff, conceded, Vec2::ZERO, Some(sfx));
                } else {
                    self.phase = Phase::Goal { timer };
                }
            }
            Phase::HalfTime { timer } => {
                let timer = timer - h;
                if timer <= 0.0 {
                    self.half = 2;
                    self.clock = 0.0;
                    for p in &mut self.players {
                        p.stamina = 1.0;
                    }
                    self.begin_restart(Restart::KickOff, 1 - self.first_kickoff, Vec2::ZERO, Some(sfx));
                    self.snap_to_setpiece();
                    sfx.whistle();
                } else {
                    self.phase = Phase::HalfTime { timer };
                }
            }
            Phase::SetPiece { kind, team, spot, timer } => {
                let mut targets = [Vec2::ZERO; PLAYERS];
                let mut faces = [Vec2::ZERO; PLAYERS];
                for i in 0..PLAYERS {
                    let (t, f) = self.setpiece_target(i, kind, team, spot);
                    targets[i] = t;
                    faces[i] = f;
                }
                let desired: Vec<Vec2> = (0..PLAYERS)
                    .map(|i| {
                        let to = targets[i] - self.players[i].pos;
                        to.clamp_length_max(1.0) * if to.length() > 0.3 { 9.0 } else { to.length() * 9.0 }
                    })
                    .collect();
                self.move_players(h, &desired, &[true; PLAYERS], None);
                for i in 0..PLAYERS {
                    if self.players[i].vel.length() < 1.0 {
                        self.players[i].facing = faces[i];
                    }
                }
                let timer = timer - h;
                if timer > 0.0 {
                    self.phase = Phase::SetPiece { kind, team, spot, timer };
                } else {
                    self.snap_to_setpiece();
                    let taker = self.setpiece_taker.unwrap();
                    self.ball.pos = spot;
                    self.ball.owner = Some(taker);
                    self.lock = Some(Lock { taker, kind, team, spot, t: 0.0 });
                    self.phase = Phase::Play;
                    if kind == Restart::KickOff {
                        sfx.whistle();
                    }
                }
            }
            Phase::Play => self.play_step(h, inputs, sfx),
            Phase::FullTime => {}
        }
    }

    fn celebration_targets(&self) -> [Vec2; PLAYERS] {
        let mut out = [Vec2::ZERO; PLAYERS];
        let scorer_team = self.ball.touch_team_for_goal();
        let corner = vec2(self.dir(scorer_team) * (HALF_L - 6.0), self.ball.pos.y.signum() * (HALF_W - 6.0));
        for i in 0..PLAYERS {
            let p = &self.players[i];
            let to = if p.team == scorer_team && p.role != Role::Gk {
                corner + vec2(((i * 37) % 7) as f32 - 3.0, ((i * 53) % 7) as f32 - 3.0) - p.pos
            } else {
                Vec2::ZERO
            };
            out[i] = to.clamp_length_max(1.0) * if to.length() > 1.0 { 6.5 } else { 0.0 };
        }
        out
    }

    fn play_step(&mut self, h: f32, inputs: &[Input], sfx: &Sfx) {
        self.clock += h;
        let pt = self.possession_team();
        self.possession[pt] += h;
        for p in &mut self.players {
            p.stun = (p.stun - h).max(0.0);
            p.tackle_cd = (p.tackle_cd - h).max(0.0);
            p.no_touch = (p.no_touch - h).max(0.0);
            p.kick_anim = (p.kick_anim - h).max(0.0);
            p.think -= h;
            if p.slide > 0.0 {
                p.slide = (p.slide - h).max(0.0);
                if p.slide == 0.0 {
                    p.slide_hit = false;
                    p.stun = 0.35;
                }
            }
            if p.dive > 0.0 {
                p.dive = (p.dive - h).max(0.0);
                if p.dive == 0.0 {
                    p.stun = 0.45;
                }
            }
        }
        if let Some(l) = &mut self.lock {
            l.t += h;
        }

        let mut desired = [Vec2::ZERO; PLAYERS];
        let mut sprint = [false; PLAYERS];
        let mut actions: Vec<(usize, Action)> = Vec::new();

        for hi in 0..self.humans.len() {
            self.human_step(hi, &inputs[hi], h, &mut desired, &mut sprint, &mut actions);
        }
        for i in 0..PLAYERS {
            if self.is_human(i) {
                continue;
            }
            let intent = self.ai_intent(i);
            desired[i] = intent.vel;
            sprint[i] = intent.sprint;
            if let Some(a) = intent.action {
                actions.push((i, a));
            }
            if intent.rethink {
                let s = self.skill[self.players[i].team];
                self.players[i].think = gen_range(0.18, 0.42) * (1.4 - s * 0.6);
            }
        }

        if let Some(l) = self.lock {
            desired[l.taker] = Vec2::ZERO;
        }
        self.move_players(h, &desired, &sprint, self.lock);

        for (i, a) in actions {
            self.do_action(i, a, sfx);
        }

        let prev = self.ball.pos;
        let prev_z = self.ball.z;
        self.ball_physics(h, false);
        if self.ball.owner.is_none() {
            self.goalkeeper_saves(sfx);
            self.ball_contact(sfx);
        }
        self.slide_contacts(sfx);
        self.update_ownership_locks(h);
        self.check_bounds(prev, prev_z, sfx);

        if self.phase == Phase::Play && self.clock >= self.cfg.half_secs {
            sfx.whistle_long();
            if self.half == 1 {
                self.phase = Phase::HalfTime { timer: 3.5 };
                self.show("HALF TIME", 3.5);
            } else {
                self.phase = Phase::FullTime;
                self.show("FULL TIME", 1000.0);
            }
            self.ball.owner = None;
            self.ball.vel = Vec2::ZERO;
        }
    }

    fn update_ownership_locks(&mut self, h: f32) {
        if let Some(l) = self.lock {
            if self.ball.owner != Some(l.taker) {
                self.lock = None;
            }
        }
        // Keepers holding the ball start their distribution clock.
        if let Some(o) = self.ball.owner {
            if self.players[o].role == Role::Gk {
                self.players[o].hold += h;
            }
        }
    }

    // ---------- humans ----------

    fn human_step(
        &mut self,
        hi: usize,
        inp: &Input,
        h: f32,
        desired: &mut [Vec2; PLAYERS],
        sprint: &mut [bool; PLAYERS],
        actions: &mut Vec<(usize, Action)>,
    ) {
        let team = self.humans[hi].team;

        // Follow the ball when a teammate wins it.
        if let Some(o) = self.ball.owner {
            if self.players[o].team == team && self.players[o].role != Role::Gk {
                self.humans[hi].player = o;
            }
        }
        let opp_has_ball = self.ball.owner.map_or(false, |o| self.players[o].team != team);
        if inp.switch || (inp.pass && opp_has_ball) {
            let cur = self.humans[hi].player;
            let target = self.ball.pos + self.ball.vel * 0.3;
            self.humans[hi].player = self.nearest_outfield(team, target, Some(cur));
        }
        // When the ball is loose and far away, hand control to someone nearer.
        let pi = self.humans[hi].player;
        if self.ball.owner.is_none() && self.ball.pass_target.is_none() {
            let near = self.nearest_outfield(team, self.ball.pos, None);
            if self.players[pi].pos.distance(self.ball.pos) > 20.0
                && self.players[near].pos.distance(self.ball.pos) + 10.0 < self.players[pi].pos.distance(self.ball.pos)
            {
                self.humans[hi].player = near;
            }
        }

        let pi = self.humans[hi].player;
        let has_ball = self.ball.owner == Some(pi);
        let aim = if inp.dir.length() > 0.1 { inp.dir } else { self.players[pi].facing };

        desired[pi] = inp.dir * self.max_speed(pi, inp.sprint);
        sprint[pi] = inp.sprint;
        if let (true, Some(c)) = (inp.contain && opp_has_ball && inp.dir.length() < 0.1, self.ball.owner) {
            // Jockey: hold a position goal-side of the carrier.
            let cp = self.players[c].pos;
            let spot = cp + (self.own_goal(team) - cp).normalize_or_zero() * 1.6;
            let to = spot - self.players[pi].pos;
            desired[pi] = to.clamp_length_max(1.0) * self.max_speed(pi, true).min(to.length() * 3.0);
            sprint[pi] = true;
        }
        if has_ball && inp.dir.length() > 0.1 {
            let turn = if inp.sprint { 6.0 } else { 9.0 };
            self.players[pi].facing = rotate_towards(self.players[pi].facing, inp.dir, turn * h);
        }
        if self.lock.map_or(false, |l| l.taker == pi) && inp.dir.length() > 0.1 {
            self.players[pi].facing = inp.dir;
        }

        let hum = &mut self.humans[hi];
        if let Some((_, t)) = &mut hum.buffered {
            *t -= h;
            if *t <= 0.0 {
                hum.buffered = None;
            }
        }

        // Shooting: hold to charge, release to strike.
        if inp.shoot_pressed && opp_has_ball {
            actions.push((pi, Action::Tackle));
        } else if inp.shoot_held && !opp_has_ball {
            hum.charging = true;
            hum.charge = (hum.charge + h * 1.05).min(1.0);
        }
        if inp.shoot_released && hum.charging {
            let a = Action::Shoot(hum.charge.max(0.15), inp.dir);
            hum.charging = false;
            hum.charge = 0.0;
            if has_ball {
                actions.push((pi, a));
            } else {
                hum.buffered = Some((a, 0.45));
            }
        }

        let pass = if inp.pass {
            Some(PassKind::Ground)
        } else if inp.through {
            Some(PassKind::Through)
        } else if inp.lob {
            Some(PassKind::Lob)
        } else {
            None
        };
        if let Some(kind) = pass {
            if has_ball {
                actions.push((pi, Action::Pass(kind, aim)));
            } else if opp_has_ball && kind == PassKind::Lob {
                actions.push((pi, Action::Slide));
            } else if !opp_has_ball {
                hum.buffered = Some((Action::Pass(kind, aim), 0.45));
            }
        }

        if has_ball {
            if let Some((a, _)) = hum.buffered.take() {
                actions.push((pi, a));
            }
        }
    }

    // ---------- actions ----------

    fn do_action(&mut self, i: usize, a: Action, sfx: &Sfx) {
        match a {
            Action::Pass(kind, aim) if self.ball.owner == Some(i) => self.do_pass(i, kind, aim, sfx),
            Action::Shoot(power, aim) if self.ball.owner == Some(i) => self.do_shoot(i, power, aim, sfx),
            Action::Tackle => self.do_tackle(i, sfx),
            Action::Slide => {
                let p = &mut self.players[i];
                if p.tackle_cd <= 0.0 && p.stun <= 0.0 && p.slide <= 0.0 {
                    p.slide = 0.55;
                    p.slide_hit = false;
                    p.vel = p.facing * 10.0;
                    p.tackle_cd = 1.6;
                }
            }
            Action::Dive(d) => {
                let p = &mut self.players[i];
                if p.dive <= 0.0 && p.stun <= 0.0 {
                    p.dive = 0.65;
                    p.vel = d * 7.2;
                }
            }
            _ => {}
        }
    }

    fn release_ball(&mut self, i: usize) {
        let t = self.players[i].team;
        self.ball.owner = None;
        self.ball.kicker = Some(i);
        self.ball.touch_team = t;
        self.ball.shot = false;
        self.ball.pass_target = None;
        self.ball.spin = 0.0;
        self.players[i].no_touch = 0.28;
        self.players[i].kick_anim = 0.35;
        self.players[i].hold = 0.0;
    }

    pub fn do_pass(&mut self, i: usize, kind: PassKind, aim: Vec2, sfx: &Sfx) {
        let p = &self.players[i];
        let t = p.team;
        let attack = vec2(self.dir(t), 0.0);
        let aim = aim.normalize_or(p.facing);

        let receiver = Self::team_range(t)
            .filter(|&j| j != i)
            .filter_map(|j| {
                let v = self.players[j].pos - p.pos;
                let d = v.length();
                if d < 2.5 {
                    return None;
                }
                let cos = v.normalize().dot(aim);
                if cos < 0.55 {
                    return None;
                }
                let ideal = match kind {
                    PassKind::Ground => 14.0,
                    PassKind::Through => 22.0,
                    PassKind::Lob => 30.0,
                };
                let gk_penalty = if self.players[j].role == Role::Gk { 1.0 } else { 0.0 };
                Some((j, cos * 3.0 - (d - ideal).abs() / 22.0 - gk_penalty))
            })
            .max_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(j, _)| j);

        let target = match receiver {
            Some(j) => {
                let r = &self.players[j];
                match kind {
                    PassKind::Ground => r.pos + r.vel * 0.35,
                    PassKind::Through => r.pos + attack * 7.0 + r.vel * 0.5,
                    PassKind::Lob => r.pos + r.vel * 0.6 + attack * 1.5,
                }
            }
            None => {
                p.pos + aim
                    * match kind {
                        PassKind::Ground => 14.0,
                        PassKind::Through => 22.0,
                        PassKind::Lob => 30.0,
                    }
            }
        };
        let target = vec2(target.x.clamp(-HALF_L + 1.0, HALF_L - 1.0), target.y.clamp(-HALF_W + 1.0, HALF_W - 1.0));

        let sprinting = p.vel.length() > 7.0;
        let err = gauss() * ((1.0 - p.passing) * 0.09 + if sprinting { 0.03 } else { 0.0 });
        let to = target - p.pos;
        let d = to.length().max(1.0);
        let dirv = Vec2::from_angle(err).rotate(to / d);

        self.release_ball(i);
        match kind {
            PassKind::Ground => {
                let v = (d * GROUND_DRAG + 4.5).clamp(8.0, 30.0);
                self.ball.vel = dirv * v;
                self.ball.vz = 0.0;
            }
            PassKind::Through => {
                let v = (d * GROUND_DRAG + 2.5).clamp(8.0, 30.0);
                self.ball.vel = dirv * v;
                self.ball.vz = 0.0;
            }
            PassKind::Lob => {
                let flight = (d / 16.0).clamp(0.9, 2.3);
                let vz = G * flight / 2.0;
                let v = d * AIR_DRAG / (1.0 - (-AIR_DRAG * flight).exp()) * 0.96;
                self.ball.vel = dirv * v;
                self.ball.vz = vz;
                self.ball.z = 0.05;
            }
        }
        self.ball.pass_target = receiver;
        self.ball.pass_point = target;
        self.mark_offside(i);
        sfx.kick(if kind == PassKind::Lob { 0.5 } else { 0.3 });

        if let (Some(r), Some(hi)) = (receiver, self.human_of_team(t)) {
            if self.players[r].role != Role::Gk {
                self.humans[hi].player = r;
            }
        }
    }

    pub fn do_shoot(&mut self, i: usize, power: f32, aim: Vec2, sfx: &Sfx) {
        let p = &self.players[i];
        let t = p.team;
        let goal = self.opp_goal(t);
        let human = self.is_human(i);

        let mut aim_y = if human && aim.y.abs() > 0.3 {
            aim.y.signum() * 2.9
        } else if human {
            -p.pos.y.signum() * 1.2
        } else {
            let far = -p.pos.y.signum();
            let side = if gen_range(0.0, 1.0) < 0.7 { far } else { -far };
            side * gen_range(1.6, 3.2)
        };
        let dist = vec2(goal.x, aim_y).distance(p.pos);
        let over = (power - 0.88).max(0.0);
        let sigma = 0.35 + dist * 0.045 * (1.25 - p.shooting) + over * 9.0;
        aim_y += gauss() * sigma;

        let speed = 17.0 + power * 15.0 + p.shooting * 3.0;
        let to = vec2(goal.x, aim_y) - p.pos;
        let d = to.length();
        let arg = 1.0 - d * AIR_DRAG / speed;
        let flight = if arg > 0.05 { -arg.ln() / AIR_DRAG } else { d / speed * 1.6 };
        let height = (0.25 + power * 1.25 + over * 5.0 + gen_range(-0.35, 0.45)).max(0.05);
        let vz = ((height + 0.5 * G * flight * flight) / flight).clamp(0.5, 14.0);

        self.release_ball(i);
        self.ball.vel = to / d * speed;
        self.ball.vz = vz;
        self.ball.z = 0.05;
        self.ball.shot = true;
        self.ball.spin = gauss() * 1.5;
        self.shots[t] += 1;
        sfx.kick(0.6 + power * 0.4);
    }

    fn do_tackle(&mut self, i: usize, sfx: &Sfx) {
        let p = &self.players[i];
        if p.tackle_cd > 0.0 || p.stun > 0.0 {
            return;
        }
        let Some(c) = self.ball.owner else { return };
        if self.players[c].team == p.team || self.ball.pos.distance(p.pos) > 1.9 {
            self.players[i].tackle_cd = 0.4;
            return;
        }
        let carrier = &self.players[c];
        let chance = (0.42 + p.defending * 0.45 - carrier.dribbling * 0.2 + (self.skill[p.team] - 0.5) * 0.2)
            .clamp(0.15, 0.85);
        let facing = p.facing;
        let team = p.team;
        self.players[i].tackle_cd = 0.9;
        if gen_range(0.0, 1.0) < chance {
            self.ball.owner = None;
            self.ball.touch_team = team;
            self.ball.pass_target = None;
            self.ball.vel = facing * 3.5 + vec2(gen_range(-2.0, 2.0), gen_range(-2.0, 2.0));
            self.players[c].stun = 0.45;
            self.players[c].no_touch = 0.35;
            sfx.tackle();
        } else {
            self.players[i].stun = 0.45;
        }
    }

    fn slide_contacts(&mut self, sfx: &Sfx) {
        for i in 0..PLAYERS {
            let p = &self.players[i];
            if p.slide <= 0.0 || p.slide_hit {
                continue;
            }
            let foot = p.pos + p.facing * 0.7;
            let team = p.team;
            let facing = p.facing;
            if foot.distance(self.ball.pos) < 1.1 && self.ball.z < 0.6 {
                if let Some(c) = self.ball.owner {
                    if self.players[c].team != team {
                        self.players[c].stun = 0.7;
                        self.players[c].no_touch = 0.4;
                    } else {
                        continue;
                    }
                }
                self.ball.owner = None;
                self.ball.touch_team = team;
                self.ball.pass_target = None;
                self.ball.vel = facing * 8.0 + vec2(gen_range(-2.0, 2.0), gen_range(-2.0, 2.0));
                self.players[i].slide_hit = true;
                sfx.tackle();
                continue;
            }
            // Clattering the man before the ball is a foul.
            if let Some(c) = self.ball.owner {
                if self.players[c].team != team && self.players[c].pos.distance(p.pos) < 0.95 {
                    let spot = self.players[c].pos;
                    let fouled = self.players[c].team;
                    self.players[i].slide_hit = true;
                    self.players[c].stun = 0.8;
                    sfx.tackle();
                    if self.in_box(team, spot) {
                        let pen = self.own_goal(team) + vec2(PEN_SPOT * self.dir(team), 0.0);
                        self.begin_restart(Restart::Penalty, fouled, pen, Some(sfx));
                    } else {
                        let spot = vec2(spot.x.clamp(-HALF_L + 1.0, HALF_L - 1.0), spot.y.clamp(-HALF_W + 1.0, HALF_W - 1.0));
                        self.begin_restart(Restart::FreeKick, fouled, spot, Some(sfx));
                    }
                    self.show("FOUL!", 1.2);
                    return;
                }
            }
        }
    }

    // ---------- movement ----------

    fn move_players(&mut self, h: f32, desired: &[Vec2], sprint: &[bool], lock: Option<Lock>) {
        for i in 0..PLAYERS {
            let owner = self.ball.owner == Some(i);
            let p = &mut self.players[i];
            if p.slide > 0.0 {
                p.vel *= (-2.2 * h).exp();
            } else if p.dive > 0.0 {
                p.vel *= (-1.5 * h).exp();
            } else {
                let want = if p.stun > 0.0 { Vec2::ZERO } else { desired[i] };
                let accel = if p.stun > 0.0 { 14.0 } else { 24.0 };
                p.vel += (want - p.vel).clamp_length_max(accel * h);
                if owner {
                    if want.length() > 0.5 {
                        p.facing = rotate_towards(p.facing, want, 7.0 * h);
                    }
                } else if p.vel.length() > 0.6 {
                    p.facing = rotate_towards(p.facing, p.vel, 10.0 * h);
                }
            }
            p.pos += p.vel * h;
            p.pos.x = p.pos.x.clamp(-HALF_L - 4.0, HALF_L + 4.0);
            p.pos.y = p.pos.y.clamp(-HALF_W - 3.0, HALF_W + 3.0);
            if sprint[i] && p.vel.length() > 6.5 {
                p.stamina = (p.stamina - 0.07 * h).max(0.0);
            } else {
                p.stamina = (p.stamina + 0.03 * h).min(1.0);
            }
        }

        // Keep opponents away from a dead ball until it is played.
        if let Some(l) = lock {
            for i in Self::team_range(1 - l.team) {
                let p = &mut self.players[i];
                let r = match l.kind {
                    Restart::ThrowIn => 3.0,
                    Restart::KickOff => CIRCLE_R,
                    _ => CIRCLE_R,
                };
                if p.role != Role::Gk && p.pos.distance(l.spot) < r {
                    p.pos = l.spot + (p.pos - l.spot).normalize_or(vec2(1.0, 0.0)) * r;
                }
            }
        }

        // Simple body separation.
        for a in 0..PLAYERS {
            for b in a + 1..PLAYERS {
                let d = self.players[b].pos - self.players[a].pos;
                let len = d.length();
                if len < 0.95 && len > 1e-4 {
                    let push = d / len * (0.95 - len) * 0.5;
                    self.players[a].pos -= push;
                    self.players[b].pos += push;
                }
            }
        }
    }

    // ---------- ball ----------

    fn ball_physics(&mut self, h: f32, in_net: bool) {
        if let Some(o) = self.ball.owner {
            let p = &self.players[o];
            let target = p.pos + p.facing * 0.72;
            self.ball.pos += (target - self.ball.pos) * (1.0 - (-h * 30.0).exp());
            self.ball.vel = p.vel;
            self.ball.z = 0.0;
            self.ball.vz = 0.0;
            return;
        }
        let b = &mut self.ball;
        if b.z > 0.0 || b.vz > 0.0 {
            b.vel *= (-AIR_DRAG * h).exp();
            if b.spin.abs() > 0.01 {
                let side = b.vel.perp().normalize_or_zero();
                b.vel += side * b.spin * h;
            }
            b.vz -= G * h;
            b.z += b.vz * h;
            if b.z <= 0.0 {
                b.z = 0.0;
                if b.vz < -1.8 {
                    b.vz = -b.vz * 0.5;
                    b.vel *= 0.82;
                } else {
                    b.vz = 0.0;
                    b.spin = 0.0;
                }
            }
        } else {
            b.vel *= (-GROUND_DRAG * h).exp();
            if b.vel.length() < 0.2 {
                b.vel = Vec2::ZERO;
            }
        }
        b.pos += b.vel * h;

        if in_net {
            let side = b.pos.x.signum();
            if b.pos.x.abs() > HALF_L + GOAL_DEPTH - 0.2 {
                b.pos.x = side * (HALF_L + GOAL_DEPTH - 0.2);
                b.vel.x *= -0.15;
            }
            if b.pos.y.abs() > GOAL_HW - 0.15 && b.pos.x.abs() > HALF_L {
                b.pos.y = b.pos.y.signum() * (GOAL_HW - 0.15);
                b.vel.y *= -0.2;
            }
            if b.z > GOAL_H - 0.15 {
                b.z = GOAL_H - 0.15;
                b.vz = -b.vz.abs() * 0.2;
            }
        }
    }

    fn goalkeeper_saves(&mut self, sfx: &Sfx) {
        for team in 0..2 {
            let g = Self::gk(team);
            let k = &self.players[g];
            if k.no_touch > 0.0 || k.stun > 0.0 || self.ball.owner.is_some() {
                continue;
            }
            if !self.in_box(team, self.ball.pos) {
                continue;
            }
            let diving = k.dive > 0.0;
            let reach = if diving { 1.75 } else { 1.15 };
            let max_z = if diving { 2.1 } else { 2.6 };
            let d = k.pos.distance(self.ball.pos);
            if d > reach || self.ball.z > max_z {
                continue;
            }
            let speed = self.ball.vel.length();
            if speed < 9.0 && !self.ball.shot {
                // Gather a loose or slow ball.
                self.take_possession(g);
                continue;
            }
            let keeping = self.players[g].keeping;
            let p = (0.3 + keeping * 0.75 - (speed - 16.0).max(0.0) * 0.022 - if diving { d / reach * 0.25 } else { 0.0 }
                + (self.skill[team] - 0.6) * 0.15)
                .clamp(0.08, 0.96);
            if gen_range(0.0, 1.0) < p {
                if speed < 21.0 && gen_range(0.0, 1.0) < 0.65 {
                    self.take_possession(g);
                } else {
                    let away = vec2(self.dir(team), 0.0);
                    let lateral = if self.ball.vel.y.abs() > 1.0 { self.ball.vel.y.signum() } else { if gen_range(0, 2) == 0 { -1.0 } else { 1.0 } };
                    self.ball.vel = away * gen_range(4.0, 9.0) + vec2(0.0, lateral * gen_range(5.0, 10.0));
                    self.ball.vz = gen_range(1.5, 4.0);
                    self.ball.shot = false;
                    self.ball.touch_team = team;
                    self.ball.pass_target = None;
                    self.players[g].no_touch = 0.45;
                    sfx.tackle();
                }
                if self.ball.owner == Some(g) {
                    self.show("SAVE!", 1.0);
                } else {
                    self.show("PARRIED!", 1.0);
                }
            } else {
                self.players[g].no_touch = 0.6;
            }
        }
    }

    /// Records which attackers are in an offside position as `passer` plays the ball.
    fn mark_offside(&mut self, passer: usize) {
        let t = self.players[passer].team;
        let ball_x = self.frame(t, self.ball.pos).x;
        let line = self.offside_line(t);
        self.offside = Self::team_range(t)
            .filter(|&j| j != passer)
            .filter(|&j| {
                let x = self.frame(t, self.players[j].pos).x;
                x > 0.0 && x > ball_x + 0.3 && x > line + 0.3
            })
            .collect();
    }

    /// Whistles for offside if `j` is an attacker who was offside when the ball was played to them.
    fn offside_called(&mut self, j: usize, sfx: &Sfx) -> bool {
        let t = self.players[j].team;
        if t != self.ball.touch_team || !self.offside.contains(&j) {
            return false;
        }
        let at = self.players[j].pos;
        let spot = vec2(at.x.clamp(-HALF_L + 1.0, HALF_L - 1.0), at.y.clamp(-HALF_W + 1.0, HALF_W - 1.0));
        self.offside.clear();
        self.begin_restart(Restart::FreeKick, 1 - t, spot, Some(sfx));
        self.show(format!("OFFSIDE  ·  {}", self.players[j].name), 1.8);
        true
    }

    fn take_possession(&mut self, j: usize) {
        self.offside.clear();
        let team = self.players[j].team;
        self.ball.owner = Some(j);
        self.ball.touch_team = team;
        self.ball.pass_target = None;
        self.ball.shot = false;
        self.ball.z = 0.0;
        self.ball.vz = 0.0;
        self.ball.spin = 0.0;
        self.players[j].hold = 0.0;
        if self.ball.vel.length() > 0.5 && self.players[j].vel.length() < 0.5 {
            self.players[j].facing = (-self.ball.vel).normalize();
        }
    }

    fn ball_contact(&mut self, sfx: &Sfx) {
        if self.ball.owner.is_some() {
            return;
        }
        let b = self.ball.pos;
        let z = self.ball.z;

        // Headers for balls at head height.
        if (1.1..2.7).contains(&z) {
            let header = (0..PLAYERS)
                .filter(|&j| {
                    let p = &self.players[j];
                    p.role != Role::Gk && p.no_touch <= 0.0 && p.stun <= 0.0 && p.slide <= 0.0 && p.pos.distance(b) < 0.9
                })
                .min_by(|&a, &c| self.players[a].pos.distance(b).total_cmp(&self.players[c].pos.distance(b)));
            if let Some(j) = header {
                if self.offside_called(j, sfx) {
                    return;
                }
                self.offside.clear();
                self.header(j, sfx);
            }
            return;
        }
        if z >= 1.1 {
            return;
        }

        let candidate = (0..PLAYERS)
            .filter(|&j| {
                let p = &self.players[j];
                p.no_touch <= 0.0 && p.stun <= 0.0 && p.slide <= 0.0 && p.dive <= 0.0 && p.pos.distance(b) < CONTROL_R
            })
            .min_by(|&a, &c| self.players[a].pos.distance(b).total_cmp(&self.players[c].pos.distance(b)));
        let Some(j) = candidate else { return };

        let rel = (self.ball.vel - self.players[j].vel).length();
        let team = self.players[j].team;
        if self.ball.shot && team != self.ball.touch_team && rel > 16.0 {
            // A defender blocks the shot.
            self.ball.vel = -self.ball.vel * 0.25 + vec2(gen_range(-5.0, 5.0), gen_range(-5.0, 5.0));
            self.ball.vz = gen_range(0.0, 3.0);
            self.ball.shot = false;
            self.ball.touch_team = team;
            self.players[j].no_touch = 0.3;
            sfx.tackle();
            return;
        }
        if self.offside_called(j, sfx) {
            return;
        }
        let control = (1.0 - (rel - 15.0).max(0.0) / 14.0).clamp(0.2, 1.0);
        if gen_range(0.0, 1.0) < control {
            self.take_possession(j);
        } else {
            self.ball.vel = self.ball.vel * 0.35 + vec2(gen_range(-3.0, 3.0), gen_range(-3.0, 3.0));
            self.ball.touch_team = team;
            self.ball.pass_target = None;
            self.ball.shot = false;
            self.players[j].no_touch = 0.3;
        }
    }

    fn header(&mut self, j: usize, sfx: &Sfx) {
        let (t, pos, facing, shooting) = {
            let p = &self.players[j];
            (p.team, p.pos, p.facing, p.shooting)
        };
        let goal = self.opp_goal(t);
        let human = self.humans.iter().position(|h| h.player == j);
        let wants_shot = human.map_or(false, |hi| {
            let h = &self.humans[hi];
            matches!(h.buffered, Some((Action::Shoot(..), _))) || h.charging
        });
        let attacking_box = self.in_box(1 - t, pos);
        let shoot = wants_shot || (human.is_none() && attacking_box) || (attacking_box && goal.distance(pos) < 14.0);
        self.release_ball(j);
        if shoot {
            let aim = vec2(goal.x, gen_range(-3.2, 3.2));
            self.ball.vel = (aim - pos).normalize() * gen_range(12.0, 16.0) * (0.85 + shooting * 0.2);
            self.ball.vz = gen_range(-1.0, 1.5);
            self.ball.shot = true;
            self.shots[t] += 1;
        } else {
            let dirv = if human.is_some() { facing } else { vec2(self.dir(t), gen_range(-0.6, 0.6)).normalize() };
            self.ball.vel = dirv * 12.0;
            self.ball.vz = 3.5;
        }
        if let Some(hi) = human {
            let h = &mut self.humans[hi];
            h.buffered = None;
            h.charging = false;
            h.charge = 0.0;
        }
        sfx.kick(0.35);
    }

    // ---------- rules: goal lines and touchlines ----------

    fn check_bounds(&mut self, prev: Vec2, prev_z: f32, sfx: &Sfx) {
        if self.phase != Phase::Play {
            return;
        }
        let b = self.ball.pos;
        let _ = prev_z;

        if b.x.abs() > HALF_L && prev.x.abs() <= HALF_L {
            let side = b.x.signum();
            let y = b.y.abs();
            let z = self.ball.z;
            // Woodwork.
            let hits_post = (y - GOAL_HW).abs() < 0.16 && z < GOAL_H;
            let hits_bar = y < GOAL_HW && (GOAL_H - 0.05..GOAL_H + 0.18).contains(&z);
            if hits_post || hits_bar {
                self.ball.owner = None;
                self.ball.pos.x = side * (HALF_L - 0.05);
                self.ball.vel.x *= -0.55;
                if hits_post {
                    self.ball.vel.y += -b.y.signum() * gen_range(1.0, 4.0);
                } else {
                    self.ball.vz = -self.ball.vz.abs() * 0.3;
                }
                self.ball.shot = false;
                sfx.post();
                self.show(if hits_post { "OFF THE POST!" } else { "OFF THE BAR!" }, 1.0);
                return;
            }
            let attacking_team = if self.dir(0) == side { 0 } else { 1 };
            let defending_team = 1 - attacking_team;
            if y < GOAL_HW && z < GOAL_H {
                self.goal(attacking_team, sfx);
                return;
            }
            self.ball.owner = None;
            let last = self.ball.touch_team;
            if last == defending_team {
                let spot = vec2(side * (HALF_L - 0.4), b.y.signum() * (HALF_W - 0.4));
                self.begin_restart(Restart::Corner, attacking_team, spot, Some(sfx));
            } else {
                let spot = vec2(side * (HALF_L - SIX_D), b.y.signum() * 4.0);
                self.begin_restart(Restart::GoalKick, defending_team, spot, Some(sfx));
            }
            return;
        }
        if b.y.abs() > HALF_W {
            self.ball.owner = None;
            let spot = vec2(b.x.clamp(-HALF_L + 1.0, HALF_L - 1.0), b.y.signum() * HALF_W);
            self.begin_restart(Restart::ThrowIn, 1 - self.ball.touch_team, spot, Some(sfx));
        }
    }

    fn goal(&mut self, team: usize, sfx: &Sfx) {
        self.score[team] += 1;
        let scorer = match self.ball.kicker {
            Some(k) if self.players[k].team == team => self.players[k].name.clone(),
            Some(k) => format!("{} (OG)", self.players[k].name),
            None => "—".to_string(),
        };
        let (min, _) = self.display_minute();
        self.scorers.push((team, scorer.clone(), min + 1));
        self.ball.touch_team = team;
        self.ball.owner = None;
        self.ball.shot = false;
        self.ball.vel *= 0.6;
        self.lock = None;
        self.phase = Phase::Goal { timer: 3.8 };
        self.show(format!("GOAL!  {}", scorer), 3.6);
        sfx.cheer();
    }
}

impl Ball {
    /// After a goal the scoring team is recorded in `touch_team`.
    fn touch_team_for_goal(&self) -> usize {
        self.touch_team
    }
}

impl Human {
    fn new(team: usize, player: usize) -> Human {
        Human { team, player, charge: 0.0, charging: false, buffered: None }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::types::{Club, Crest};
    use crate::sim::rng::srand;

    fn club(name: &'static str, rating: u8, formation: &'static str) -> Club {
        Club {
            name,
            short: "TST",
            home: 0xff0000,
            trim: 0xffffff,
            away: 0x0000ff,
            keeper: 0x00ff00,
            crest: Crest::Shield,
            formation,
            rating,
            players: &[],
        }
    }

    /// Plays full CPU-vs-CPU matches headlessly and checks the game flows end to end.
    #[test]
    fn cpu_matches_complete() {
        let formations = crate::sim::formation::NAMES;
        let (mut goals, mut shots, mut restarts) = (0, 0, 0);
        for seed in 0..6u64 {
            srand(seed * 7919 + 1);
            let a = club("Home", 70 + seed as u8 * 3, formations[seed as usize % 6]);
            let b = club("Away", 85 - seed as u8 * 3, formations[(seed as usize + 2) % 6]);
            let teams = [TeamSetup::from_club(&a), TeamSetup::from_club(&b)];
            let cfg = MatchConfig { mode: Mode::Watch, half_secs: 180.0, difficulty: 0.6 };
            let mut m = Match::new(cfg, teams);
            let sfx = Sfx::default();
            let mut was_setpiece = false;
            let mut steps = 0;
            let mut banners: std::collections::BTreeMap<String, u32> = Default::default();
            let mut last_banner: Option<String> = None;
            let (mut owner_x, mut owner_n) = ([0.0f32; 2], [0.0f32; 2]);
            while m.phase != Phase::FullTime {
                m.step(1.0 / 120.0, &[], &sfx);
                sfx.drain();
                if let Some((b, _)) = &m.banner {
                    if last_banner.as_deref() != Some(b.as_str()) {
                        let key = b.split("  ").next().unwrap_or("").to_string();
                        *banners.entry(key).or_insert(0) += 1;
                        last_banner = Some(b.clone());
                    }
                } else {
                    last_banner = None;
                }
                if let Some(o) = m.ball.owner {
                    owner_x[m.players[o].team] += m.frame(m.players[o].team, m.ball.pos).x;
                    owner_n[m.players[o].team] += 1.0;
                }
                let sp = matches!(m.phase, Phase::SetPiece { .. });
                if sp && !was_setpiece {
                    restarts += 1;
                }
                was_setpiece = sp;
                assert!(m.ball.pos.x.abs() < HALF_L + 5.0 && m.ball.pos.y.abs() < HALF_W + 5.0, "ball escaped: {:?}", m.ball.pos);
                assert!(m.players.iter().all(|p| p.pos.is_finite()));
                steps += 1;
                assert!(steps < 120 * 60 * 20, "match never finished");
            }
            println!(
                "{} {}-{} {}  shots {:?}  possession {:.0}%",
                m.teams[0].formation, m.score[0], m.score[1], m.teams[1].formation, m.shots,
                m.possession[0] / (m.possession[0] + m.possession[1]) * 100.0
            );
            println!("   events {:?}  avg carrier x (team frame) {:.1} / {:.1}", banners, owner_x[0] / owner_n[0].max(1.0), owner_x[1] / owner_n[1].max(1.0));
            goals += m.score[0] + m.score[1];
            shots += m.shots[0] + m.shots[1];
        }
        println!("goals {goals} shots {shots} restarts {restarts}");
        assert!(shots > 0, "nobody ever shot");
    }
}

/// Home side wears its home kit; the away side switches to its change kit if the colours are too close.
fn choose_kits(teams: &[TeamSetup; 2]) -> [u32; 2] {
    let rgb = |c: u32| [(c >> 16 & 255) as f32, (c >> 8 & 255) as f32, (c & 255) as f32];
    let dist = |a: u32, b: u32| {
        let (a, b) = (rgb(a), rgb(b));
        ((a[0] - b[0]).powi(2) * 0.3 + (a[1] - b[1]).powi(2) * 0.59 + (a[2] - b[2]).powi(2) * 0.11).sqrt()
    };
    let home = teams[0].home;
    let away = if dist(home, teams[1].home) < 70.0 { teams[1].away } else { teams[1].home };
    [home, away]
}
