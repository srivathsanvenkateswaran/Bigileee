//! Computer-controlled decisions: outfield positioning, the ball carrier, set pieces and keepers.

use crate::consts::*;
use crate::game::*;
use macroquad::prelude::*;
use macroquad::rand::gen_range;

pub struct Intent {
    pub vel: Vec2,
    pub sprint: bool,
    pub action: Option<Action>,
    pub rethink: bool,
}

impl Intent {
    fn idle() -> Intent {
        Intent { vel: Vec2::ZERO, sprint: false, action: None, rethink: false }
    }
}

impl Match {
    pub fn ai_intent(&self, i: usize) -> Intent {
        let p = &self.players[i];
        if p.stun > 0.0 || p.slide > 0.0 || p.dive > 0.0 {
            return Intent::idle();
        }
        if self.ball.owner == Some(i) {
            if let Some(l) = self.lock.filter(|l| l.taker == i) {
                return self.setpiece_ai(i, l);
            }
            return if p.role == Role::Gk { self.gk_distribute(i) } else { self.carrier_ai(i) };
        }
        if p.role == Role::Gk {
            return self.gk_ai(i);
        }
        self.offball_ai(i)
    }

    fn seek(&self, i: usize, target: Vec2, sprint: bool) -> Vec2 {
        let to = target - self.players[i].pos;
        let d = to.length();
        if d < 0.25 {
            return Vec2::ZERO;
        }
        to / d * self.max_speed(i, sprint).min(d * 2.5)
    }

    fn nearest_opponent(&self, i: usize) -> (usize, f32) {
        let p = &self.players[i];
        Self::team_range(1 - p.team)
            .map(|j| (j, self.players[j].pos.distance(p.pos)))
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .unwrap()
    }

    /// Where `i` can first meet the ball, by stepping the ball's path forward.
    fn intercept_point(&self, i: usize) -> Vec2 {
        let p = &self.players[i];
        let speed = self.max_speed(i, true);
        let (mut pos, mut vel, mut z, mut vz) = (self.ball.pos, self.ball.vel, self.ball.z, self.ball.vz);
        let dt = 0.05;
        let mut t = 0.0;
        while t < 3.0 {
            if pos.distance(p.pos) <= speed * t + 0.6 && z < 1.8 {
                return pos;
            }
            if z > 0.0 || vz > 0.0 {
                vel *= (-AIR_DRAG * dt).exp();
                vz -= G * dt;
                z = (z + vz * dt).max(0.0);
                if z == 0.0 {
                    vz = 0.0;
                }
            } else {
                vel *= (-GROUND_DRAG * dt).exp();
            }
            pos += vel * dt;
            t += dt;
        }
        pos
    }

    /// The outfield player of `team` best placed to win the ball, ignoring human-controlled players.
    fn chaser(&self, team: usize) -> Option<usize> {
        let target = self.ball.pos + self.ball.vel * 0.4;
        Self::team_range(team)
            .skip(1)
            .filter(|&j| !self.is_human(j) && self.players[j].stun <= 0.0)
            .min_by(|&a, &b| self.players[a].pos.distance(target).total_cmp(&self.players[b].pos.distance(target)))
    }

    fn second_chaser(&self, team: usize, first: Option<usize>) -> Option<usize> {
        let target = self.ball.pos;
        Self::team_range(team)
            .skip(1)
            .filter(|&j| !self.is_human(j) && Some(j) != first)
            .min_by(|&a, &b| self.players[a].pos.distance(target).total_cmp(&self.players[b].pos.distance(target)))
    }

    /// Scores every teammate as a pass option and returns the best one.
    pub fn best_pass(&self, i: usize, max_dist: f32) -> Option<(usize, f32, PassKind)> {
        let p = &self.players[i];
        let t = p.team;
        let my_x = self.frame(t, p.pos).x;
        let line = {
            let mut xs: Vec<f32> = Self::team_range(1 - t).map(|j| self.frame(t, self.players[j].pos).x).collect();
            xs.sort_by(|a, b| b.total_cmp(a));
            xs[1]
        };
        Self::team_range(t)
            .filter(|&j| j != i)
            .filter_map(|j| {
                let r = &self.players[j];
                let d = r.pos.distance(p.pos);
                if !(6.0..=max_dist).contains(&d) {
                    return None;
                }
                let rx = self.frame(t, r.pos).x;
                let prog = rx - my_x;
                let opps = Self::team_range(1 - t).map(|o| self.players[o].pos);
                let lane = opps.clone().map(|o| dist_to_segment(o, p.pos, r.pos)).fold(f32::MAX, f32::min);
                let space = opps.map(|o| o.distance(r.pos)).fold(f32::MAX, f32::min);
                let mut kind = PassKind::Ground;
                let mut s = prog * 0.07 + space.min(8.0) * 0.28 + lane.min(4.0) * 0.35 - d * 0.025;
                if lane < 1.6 {
                    if d > 16.0 {
                        kind = PassKind::Lob;
                        s -= 0.8;
                    } else {
                        s -= 3.0;
                    }
                }
                if r.role == Role::Fwd && prog > 5.0 && space > 4.0 && lane > 2.0 && rx < line + 2.0 {
                    kind = PassKind::Through;
                    s += 0.4;
                }
                if r.role == Role::Gk {
                    s -= 2.5;
                }
                Some((j, s, kind))
            })
            .max_by(|a, b| a.1.total_cmp(&b.1))
    }

    fn carrier_ai(&self, i: usize) -> Intent {
        let p = &self.players[i];
        let t = p.team;
        let skill = self.skill[t];
        let goal = self.opp_goal(t);
        let pf = self.frame(t, p.pos);
        let dist_goal = goal.distance(p.pos);
        let (_, press) = self.nearest_opponent(i);
        let mut action = None;
        let thinking = p.think <= 0.0;

        if thinking {
            let angle_ok = pf.y.abs() < 20.0 && pf.x > 18.0;
            let chance = if dist_goal < 13.0 {
                0.85
            } else if dist_goal < 20.0 {
                0.5 + skill * 0.15
            } else if dist_goal < 27.0 {
                0.12 + skill * 0.1
            } else {
                0.0
            };
            if angle_ok && gen_range(0.0, 1.0) < chance {
                let power = (0.5 + dist_goal / 70.0 + gen_range(0.0, 0.15)).min(0.88);
                action = Some(Action::Shoot(power, Vec2::ZERO));
            } else if pf.x > 33.0 && pf.y.abs() > 17.0 && gen_range(0.0, 1.0) < 0.45 {
                let box_target = Self::team_range(t)
                    .filter(|&j| j != i && self.in_box(1 - t, self.players[j].pos))
                    .max_by(|&a, &b| self.players[a].pos.x.abs().total_cmp(&self.players[b].pos.x.abs()));
                let aim = match box_target {
                    Some(j) => self.players[j].pos - p.pos,
                    None => goal - vec2(9.0 * self.dir(t), 0.0) - p.pos,
                };
                action = Some(Action::Pass(PassKind::Lob, aim));
            } else if let Some((j, score, kind)) = self.best_pass(i, 40.0) {
                let pressured = press < 3.2;
                let threshold = if pressured { -1.0 } else { 2.0 + gen_range(0.0, 1.6) };
                if score > threshold {
                    action = Some(Action::Pass(kind, self.players[j].pos - p.pos));
                }
            }
        }

        // Dribble towards goal, steering around nearby opponents and away from the touchline.
        let mut want = (goal - p.pos).normalize();
        let mut avoid = Vec2::ZERO;
        for j in Self::team_range(1 - t) {
            let o = self.players[j].pos;
            let to = p.pos - o;
            let d = to.length();
            if d < 7.0 && (o - p.pos).dot(want) > -1.0 {
                avoid += to.normalize_or_zero() * (1.0 - d / 7.0);
            }
        }
        want += avoid * 1.6;
        if p.pos.y.abs() > HALF_W - 4.0 {
            want.y -= p.pos.y.signum() * 0.8;
        }
        let want = want.normalize_or(vec2(self.dir(t), 0.0));
        let open = press > 5.0;
        Intent { vel: want * self.max_speed(i, open), sprint: open, action, rethink: thinking }
    }

    fn setpiece_ai(&self, i: usize, l: Lock) -> Intent {
        let wait = match l.kind {
            Restart::Penalty => 1.4,
            Restart::KickOff => 0.4,
            _ => 0.8,
        };
        if l.t < wait {
            return Intent::idle();
        }
        let p = &self.players[i];
        let t = p.team;
        let goal = self.opp_goal(t);
        let pf = self.frame(t, p.pos);
        let to = |j: usize| self.players[j].pos - p.pos;
        let nearest = |max: f32| {
            Self::team_range(t)
                .filter(|&j| j != i && to(j).length() < max && self.players[j].role != Role::Gk)
                .min_by(|&a, &b| to(a).length().total_cmp(&to(b).length()))
        };
        let action = match l.kind {
            Restart::Penalty => Action::Shoot(0.7 + gen_range(0.0, 0.14), Vec2::ZERO),
            Restart::KickOff => Action::Pass(PassKind::Ground, nearest(30.0).map_or(vec2(-self.dir(t), 0.0), to)),
            Restart::Corner => {
                let j = Self::team_range(t)
                    .filter(|&j| self.in_box(1 - t, self.players[j].pos))
                    .max_by(|&a, &b| {
                        let sa = self.nearest_opponent(a).1 + gen_range(0.0, 2.0);
                        let sb = self.nearest_opponent(b).1 + gen_range(0.0, 2.0);
                        sa.total_cmp(&sb)
                    });
                Action::Pass(PassKind::Lob, j.map_or(goal - vec2(10.0 * self.dir(t), 0.0) - p.pos, to))
            }
            Restart::FreeKick if goal.distance(p.pos) < 28.0 && pf.y.abs() < 17.0 => {
                Action::Shoot(0.8 + gen_range(0.0, 0.07), Vec2::ZERO)
            }
            Restart::GoalKick => match self.best_pass(i, 55.0) {
                Some((j, s, _)) if s > 1.0 && gen_range(0.0, 1.0) < 0.5 => Action::Pass(PassKind::Ground, to(j)),
                _ => {
                    let mids: Vec<usize> = Self::team_range(t)
                        .filter(|&j| matches!(self.players[j].role, Role::Mid | Role::Fwd))
                        .collect();
                    let j = mids[gen_range(0, mids.len())];
                    Action::Pass(PassKind::Lob, to(j))
                }
            },
            _ => match self.best_pass(i, 30.0) {
                Some((j, _, kind)) => Action::Pass(kind, to(j)),
                None => Action::Pass(PassKind::Ground, nearest(40.0).map_or(vec2(self.dir(t), 0.0), to)),
            },
        };
        Intent { vel: Vec2::ZERO, sprint: false, action: Some(action), rethink: true }
    }

    fn offball_ai(&self, i: usize) -> Intent {
        let p = &self.players[i];
        let t = p.team;
        let skill = self.skill[t];
        let loose = self.ball.owner.is_none();
        let possession = self.possession_team();

        if loose && self.ball.pass_target == Some(i) {
            let target = self.intercept_point(i);
            return Intent { vel: self.seek(i, target, true), sprint: true, action: None, rethink: false };
        }

        let chaser = self.chaser(t);
        if loose && chaser == Some(i) && self.ball.pass_target.map_or(true, |r| self.players[r].team != t) {
            let human_close = self
                .human_of_team(t)
                .map(|hi| self.players[self.humans[hi].player].pos.distance(self.ball.pos) < 6.0)
                .unwrap_or(false);
            if !human_close {
                let target = self.intercept_point(i);
                return Intent { vel: self.seek(i, target, true), sprint: true, action: None, rethink: false };
            }
        }

        if possession == t {
            let mut target = self.formation_target(i);
            if let Some(o) = self.ball.owner {
                let carrier = self.players[o].pos;
                if p.role == Role::Fwd && self.players[o].team == t {
                    // Forwards look to run in behind, staying level with the last defender.
                    let mut f = self.frame(t, target);
                    let line = {
                        let mut xs: Vec<f32> =
                            Self::team_range(1 - t).map(|j| self.frame(t, self.players[j].pos).x).collect();
                        xs.sort_by(|a, b| b.total_cmp(a));
                        xs[1].max(0.0)
                    };
                    f.x = (f.x + 5.0).min(line - 0.3).max(f.x);
                    target = self.frame(t, f);
                }
                if target.distance(carrier) < 5.0 {
                    target = carrier + (target - carrier).normalize_or(vec2(0.0, 1.0)) * 7.0;
                }
            }
            let far = target.distance(p.pos) > 9.0;
            return Intent { vel: self.seek(i, target, far), sprint: far, action: None, rethink: false };
        }

        // Defending.
        let carrier = self.ball.owner;
        let human_engaged = self
            .human_of_team(t)
            .map(|hi| self.players[self.humans[hi].player].pos.distance(self.ball.pos) < 10.0)
            .unwrap_or(false);
        if chaser == Some(i) && !human_engaged {
            let target = if let Some(c) = carrier {
                let cp = self.players[c].pos;
                cp + (self.own_goal(t) - cp).normalize() * 0.6
            } else {
                self.intercept_point(i)
            };
            let mut action = None;
            let mut rethink = false;
            if let Some(c) = carrier {
                let d = self.players[c].pos.distance(p.pos);
                if p.think <= 0.0 && p.tackle_cd <= 0.0 {
                    rethink = true;
                    if d < 1.7 && gen_range(0.0, 1.0) < 0.25 + skill * 0.45 {
                        action = Some(Action::Tackle);
                    } else if d < 2.6
                        && d > 1.2
                        && (self.players[c].pos - p.pos).normalize().dot(p.facing) > 0.85
                        && gen_range(0.0, 1.0) < 0.05 + skill * 0.06
                    {
                        action = Some(Action::Slide);
                    }
                }
            }
            return Intent { vel: self.seek(i, target, true), sprint: true, action, rethink };
        }
        if carrier.is_some() && self.second_chaser(t, chaser) == Some(i) {
            let b = self.ball.pos;
            let target = b + (self.own_goal(t) - b).normalize() * 5.0;
            return Intent { vel: self.seek(i, target, true), sprint: true, action: None, rethink: false };
        }

        // Hold shape and mark the nearest attacker in the zone.
        let mut target = self.formation_target(i);
        let mark = Self::team_range(1 - t)
            .skip(1)
            .map(|j| (j, self.players[j].pos.distance(target)))
            .filter(|&(_, d)| d < 12.0)
            .min_by(|a, b| a.1.total_cmp(&b.1));
        if let Some((j, _)) = mark {
            let o = self.players[j].pos;
            let goal_side = o + (self.own_goal(t) - o).normalize() * 1.8;
            let w = if p.role == Role::Def { 0.6 } else { 0.4 };
            target = target.lerp(goal_side, w);
        }
        let far = target.distance(p.pos) > 6.0;
        Intent { vel: self.seek(i, target, far), sprint: far, action: None, rethink: false }
    }

    fn gk_distribute(&self, i: usize) -> Intent {
        let p = &self.players[i];
        if p.hold < 1.1 {
            return Intent::idle();
        }
        let t = p.team;
        let action = match self.best_pass(i, 50.0) {
            Some((j, s, kind)) if s > 0.5 => Action::Pass(kind, self.players[j].pos - p.pos),
            _ => Action::Pass(PassKind::Lob, vec2(self.dir(t), gen_range(-0.5, 0.5))),
        };
        Intent { vel: Vec2::ZERO, sprint: false, action: Some(action), rethink: true }
    }

    fn gk_ai(&self, i: usize) -> Intent {
        let p = &self.players[i];
        let t = p.team;
        let d = self.dir(t);
        let own = self.own_goal(t);
        let b = self.ball.pos;

        let to_ball = b - own;
        let depth = (to_ball.length() * 0.07).clamp(0.8, 4.5);
        let mut target = own + to_ball.normalize_or(vec2(d, 0.0)) * depth;
        target.y = target.y.clamp(-GOAL_HW + 0.3, GOAL_HW - 0.3);
        let mut sprint = false;
        let mut action = None;
        let mut rethink = false;

        if self.ball.owner.is_none() && self.in_box(t, b) && self.ball.z < 2.0 && !self.ball.shot {
            let mine = p.pos.distance(b);
            let theirs = Self::team_range(1 - t).map(|j| self.players[j].pos.distance(b)).fold(f32::MAX, f32::min);
            if mine < theirs + 2.0 {
                target = b;
                sprint = true;
            }
        }
        if let Some(o) = self.ball.owner {
            if self.players[o].team != t && self.in_box(t, b) && own.distance(b) < 16.0 {
                target = b + (own - b).normalize() * 1.5;
                sprint = true;
            }
        }

        // React to shots: get across, and dive if the ball is going past.
        let vx = self.ball.vel.x;
        if self.ball.owner.is_none() && vx * -d > 4.0 {
            let time = (p.pos.x - b.x) / vx;
            if time > 0.0 && time < 1.4 {
                let pred = b + self.ball.vel * time;
                if pred.y.abs() < GOAL_HW + 1.5 {
                    target = vec2(p.pos.x, pred.y.clamp(-GOAL_HW, GOAL_HW));
                    sprint = true;
                    let gap = pred - p.pos;
                    if p.think <= 0.0 {
                        rethink = true;
                        if time < 0.6 && gap.y.abs() > 0.9 && self.ball.vel.length() > 11.0 {
                            action = Some(Action::Dive(vec2(gap.x * 0.2, gap.y).normalize()));
                        }
                    }
                }
            }
        }

        Intent { vel: self.seek(i, target, sprint) * 1.05, sprint, action, rethink }
    }
}
