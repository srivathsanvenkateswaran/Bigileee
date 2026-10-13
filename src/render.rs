//! Drawing the pitch, players, ball and HUD.

use crate::consts::*;
use crate::game::*;
use crate::teams::{CrestStyle, TeamDef};
use macroquad::prelude::*;

const VIEW_W: f32 = 64.0;
const LINE: Color = Color::new(1.0, 1.0, 1.0, 0.88);
const P_COLORS: [Color; 2] = [Color::new(1.0, 0.87, 0.2, 1.0), Color::new(0.3, 0.9, 1.0, 1.0)];
const HAIR: [Color; 5] = [
    Color::new(0.08, 0.06, 0.05, 1.0),
    Color::new(0.25, 0.15, 0.08, 1.0),
    Color::new(0.45, 0.3, 0.15, 1.0),
    Color::new(0.75, 0.6, 0.35, 1.0),
    Color::new(0.15, 0.12, 0.12, 1.0),
];

pub struct Crowd {
    dots: Vec<(Vec2, Color)>,
}

impl Crowd {
    pub fn new() -> Crowd {
        let mut dots = Vec::new();
        let palette = [
            Color::new(0.85, 0.2, 0.2, 1.0),
            Color::new(0.95, 0.95, 0.95, 1.0),
            Color::new(0.2, 0.3, 0.8, 1.0),
            Color::new(0.95, 0.75, 0.2, 1.0),
            Color::new(0.15, 0.15, 0.18, 1.0),
            Color::new(0.4, 0.4, 0.45, 1.0),
            Color::new(0.9, 0.5, 0.15, 1.0),
        ];
        let mut x = -HALF_L - 18.0;
        while x < HALF_L + 18.0 {
            let mut y = -HALF_W - 18.0;
            while y < HALF_W + 18.0 {
                if x.abs() > HALF_L + 8.0 || y.abs() > HALF_W + 8.0 {
                    let jitter = vec2(rand::gen_range(-0.2, 0.2), rand::gen_range(-0.2, 0.2));
                    dots.push((vec2(x, y) + jitter, palette[rand::gen_range(0, palette.len())]));
                }
                y += 0.9;
            }
            x += 0.9;
        }
        Crowd { dots }
    }
}

struct View {
    cam: Vec2,
    scale: f32,
    center: Vec2,
}

impl View {
    fn w(&self, p: Vec2) -> Vec2 {
        (p - self.cam) * self.scale + self.center
    }
    fn s(&self, m: f32) -> f32 {
        m * self.scale
    }
    fn line(&self, a: Vec2, b: Vec2, color: Color) {
        let (a, b) = (self.w(a), self.w(b));
        draw_line(a.x, a.y, b.x, b.y, self.s(0.14).max(1.5), color);
    }
    fn arc(&self, c: Vec2, r: f32, from: f32, to: f32, color: Color) {
        let n = 48;
        for k in 0..n {
            let a0 = from + (to - from) * k as f32 / n as f32;
            let a1 = from + (to - from) * (k + 1) as f32 / n as f32;
            self.line(c + Vec2::from_angle(a0) * r, c + Vec2::from_angle(a1) * r, color);
        }
    }
    fn rect_lines(&self, x0: f32, y0: f32, x1: f32, y1: f32, color: Color) {
        self.line(vec2(x0, y0), vec2(x1, y0), color);
        self.line(vec2(x1, y0), vec2(x1, y1), color);
        self.line(vec2(x1, y1), vec2(x0, y1), color);
        self.line(vec2(x0, y1), vec2(x0, y0), color);
    }
    fn fill(&self, x0: f32, y0: f32, x1: f32, y1: f32, color: Color) {
        let a = self.w(vec2(x0.min(x1), y0.min(y1)));
        let b = self.w(vec2(x0.max(x1), y0.max(y1)));
        draw_rectangle(a.x, a.y, b.x - a.x, b.y - a.y, color);
    }
}

fn view_for(m: &Match) -> View {
    let (sw, sh) = (screen_width(), screen_height());
    let scale = sw / VIEW_W;
    let hw = sw / scale / 2.0;
    let hh = sh / scale / 2.0;
    let mx = (HALF_L + 9.0 - hw).max(0.0);
    let my = (HALF_W + 7.0 - hh).max(0.0);
    let cam = vec2(m.cam.x.clamp(-mx, mx), m.cam.y.clamp(-my, my));
    View { cam, scale, center: vec2(sw / 2.0, sh / 2.0) }
}

pub fn draw_match(m: &Match, crowd: &Crowd) {
    clear_background(Color::new(0.1, 0.1, 0.12, 1.0));
    let v = view_for(m);
    draw_stadium(&v, crowd);
    draw_pitch(&v);
    draw_goal_back(&v);
    draw_players(m, &v);
    draw_ball(m, &v);
    draw_goal_front(&v);
    draw_hud(m);
}

fn draw_stadium(v: &View, crowd: &Crowd) {
    // Run-off grass.
    v.fill(-HALF_L - 8.0, -HALF_W - 8.0, HALF_L + 8.0, HALF_W + 8.0, Color::new(0.16, 0.42, 0.18, 1.0));
    let size = v.s(0.55);
    for (p, c) in &crowd.dots {
        let s = v.w(*p);
        if s.x > -10.0 && s.y > -10.0 && s.x < screen_width() + 10.0 && s.y < screen_height() + 10.0 {
            draw_rectangle(s.x - size / 2.0, s.y - size / 2.0, size, size, *c);
        }
    }
    // Advertising boards.
    let board = Color::new(0.08, 0.12, 0.3, 1.0);
    let edge = Color::new(0.95, 0.75, 0.2, 1.0);
    for side in [-1.0, 1.0] {
        let y = side * (HALF_W + 5.0);
        v.fill(-HALF_L - 5.0, y - 0.5, HALF_L + 5.0, y + 0.5, board);
        v.line(vec2(-HALF_L - 5.0, y + 0.5 * side), vec2(HALF_L + 5.0, y + 0.5 * side), edge);
        let x = side * (HALF_L + 5.0);
        v.fill(x - 0.5, -HALF_W - 5.0, x + 0.5, HALF_W + 5.0, board);
    }
    let font = v.s(0.8) as u16;
    let mut x = -HALF_L;
    while x < HALF_L {
        for side in [-1.0, 1.0] {
            let p = v.w(vec2(x, side * (HALF_W + 5.0) + 0.3));
            draw_text("KAALPANTHU", p.x, p.y, font as f32, Color::new(1.0, 1.0, 1.0, 0.85));
        }
        x += 16.0;
    }
}

fn draw_pitch(v: &View) {
    let stripes = 18;
    let w = HALF_L * 2.0 / stripes as f32;
    for k in 0..stripes {
        let x0 = -HALF_L + k as f32 * w;
        let c = if k % 2 == 0 { Color::new(0.2, 0.52, 0.22, 1.0) } else { Color::new(0.23, 0.57, 0.25, 1.0) };
        v.fill(x0, -HALF_W, x0 + w, HALF_W, c);
    }
    v.rect_lines(-HALF_L, -HALF_W, HALF_L, HALF_W, LINE);
    v.line(vec2(0.0, -HALF_W), vec2(0.0, HALF_W), LINE);
    v.arc(Vec2::ZERO, CIRCLE_R, 0.0, std::f32::consts::TAU, LINE);
    let c = v.w(Vec2::ZERO);
    draw_circle(c.x, c.y, v.s(0.25), LINE);
    for side in [-1.0f32, 1.0] {
        let gx = side * HALF_L;
        v.rect_lines(gx, -BOX_HW, gx - side * BOX_D, BOX_HW, LINE);
        v.rect_lines(gx, -SIX_HW, gx - side * SIX_D, SIX_HW, LINE);
        let spot = vec2(gx - side * PEN_SPOT, 0.0);
        let s = v.w(spot);
        draw_circle(s.x, s.y, v.s(0.22), LINE);
        // Penalty arc: the part of the circle outside the box.
        let half = ((BOX_D - PEN_SPOT) / CIRCLE_R).acos();
        let facing = if side > 0.0 { std::f32::consts::PI } else { 0.0 };
        v.arc(spot, CIRCLE_R, facing - half, facing + half, LINE);
        for ys in [-1.0f32, 1.0] {
            let corner = vec2(gx, ys * HALF_W);
            let start = if side > 0.0 { std::f32::consts::PI } else { 0.0 };
            let sweep = -side * ys * std::f32::consts::FRAC_PI_2;
            v.arc(corner, 1.0, start, start + sweep, LINE);
        }
    }
}

/// Height is drawn as an upward screen offset, giving the top-down view some depth.
fn lift(z: f32) -> Vec2 {
    vec2(0.0, -z * 0.8)
}

fn draw_goal_back(v: &View) {
    let net = Color::new(1.0, 1.0, 1.0, 0.13);
    let mesh = Color::new(1.0, 1.0, 1.0, 0.28);
    for side in [-1.0f32, 1.0] {
        let x0 = side * HALF_L;
        let x1 = side * (HALF_L + GOAL_DEPTH);
        v.fill(x0, -GOAL_HW, x1, GOAL_HW, Color::new(0.0, 0.0, 0.0, 0.18));
        let top = lift(GOAL_H * 0.85);
        let a = v.w(vec2(x0.min(x1), -GOAL_HW) + top);
        let b = v.w(vec2(x0.max(x1), GOAL_HW) + top);
        draw_rectangle(a.x, a.y, b.x - a.x, b.y - a.y, net);
        let mut y = -GOAL_HW;
        while y <= GOAL_HW + 0.01 {
            v.line(vec2(x0, y) + top, vec2(x1, y) + top, mesh);
            y += 0.6;
        }
        let mut x = 0.0;
        while x <= GOAL_DEPTH + 0.01 {
            let gx = side * (HALF_L + x);
            v.line(vec2(gx, -GOAL_HW) + top, vec2(gx, GOAL_HW) + top, mesh);
            x += 0.55;
        }
        for ys in [-1.0f32, 1.0] {
            v.line(vec2(x1, ys * GOAL_HW), vec2(x1, ys * GOAL_HW) + top, mesh);
        }
    }
}

fn draw_goal_front(v: &View) {
    let white = Color::new(0.97, 0.97, 0.97, 1.0);
    for side in [-1.0f32, 1.0] {
        let x0 = side * HALF_L;
        let top = lift(GOAL_H);
        let thick = v.s(0.16).max(2.0);
        for ys in [-1.0f32, 1.0] {
            let a = v.w(vec2(x0, ys * GOAL_HW));
            let b = v.w(vec2(x0, ys * GOAL_HW) + top);
            draw_line(a.x, a.y, b.x, b.y, thick, white);
        }
        let a = v.w(vec2(x0, -GOAL_HW) + top);
        let b = v.w(vec2(x0, GOAL_HW) + top);
        draw_line(a.x, a.y, b.x, b.y, thick, white);
    }
}

fn draw_players(m: &Match, v: &View) {
    let mut order: Vec<usize> = (0..PLAYERS).collect();
    order.sort_by(|&a, &b| m.players[a].pos.y.total_cmp(&m.players[b].pos.y));

    for i in order {
        let p = &m.players[i];
        let team = &m.teams[p.team];
        let shirt = if p.role == Role::Gk { team.keeper } else { team.shirt };
        let c = v.w(p.pos);
        let r = v.s(PLAYER_R);

        let controller = m.humans.iter().position(|h| h.player == i);
        if let Some(hi) = controller {
            let col = P_COLORS[m.humans[hi].team.min(1)];
            draw_circle_lines(c.x, c.y, v.s(0.95), v.s(0.12).max(2.0), col);
        }

        draw_circle(c.x + v.s(0.2), c.y + v.s(0.25), r, Color::new(0.0, 0.0, 0.0, 0.28));
        if p.slide > 0.0 || p.dive > 0.0 {
            let tail = v.w(p.pos - p.facing * 0.6);
            draw_circle(tail.x, tail.y, r * 0.8, shirt);
            let legs = v.w(p.pos + p.facing * 0.5);
            draw_circle(legs.x, legs.y, r * 0.55, Color::new(0.1, 0.1, 0.1, 1.0));
        }
        // Shoulders, body, then head.
        let side = p.facing.perp() * 0.32;
        for s in [-1.0, 1.0] {
            let sh = v.w(p.pos + side * s);
            draw_circle(sh.x, sh.y, r * 0.5, team.trim);
        }
        draw_circle(c.x, c.y, r, shirt);
        draw_circle_lines(c.x, c.y, r, v.s(0.07).max(1.0), team.trim);
        let head = v.w(p.pos + p.facing * 0.1);
        draw_circle(head.x, head.y, r * 0.48, HAIR[(i * 7 + p.num as usize) % HAIR.len()]);

        if p.stun > 0.0 && p.slide <= 0.0 && p.dive <= 0.0 {
            draw_circle_lines(c.x, c.y - r * 1.4, r * 0.35, 1.5, Color::new(1.0, 1.0, 0.6, 0.8));
        }

        if let Some(hi) = controller {
            let h = &m.humans[hi];
            let col = P_COLORS[h.team.min(1)];
            let tip = vec2(c.x, c.y - v.s(1.25));
            draw_triangle(tip, tip + vec2(-7.0, -10.0), tip + vec2(7.0, -10.0), col);
            let label = format!("{} {}", p.num, p.name);
            let size = 18.0;
            let dims = measure_text(&label, None, size as u16, 1.0);
            draw_text(&label, c.x - dims.width / 2.0 + 1.0, tip.y - 13.0, size, Color::new(0.0, 0.0, 0.0, 0.6));
            draw_text(&label, c.x - dims.width / 2.0, tip.y - 14.0, size, WHITE);

            let bar_w = v.s(1.8);
            let bx = c.x - bar_w / 2.0;
            let by = c.y + v.s(1.0);
            draw_rectangle(bx, by, bar_w, 4.0, Color::new(0.0, 0.0, 0.0, 0.5));
            draw_rectangle(bx, by, bar_w * p.stamina, 4.0, Color::new(0.3, 0.85, 0.4, 0.9));
            if h.charging {
                let by = by + 6.0;
                draw_rectangle(bx - 1.0, by - 1.0, bar_w + 2.0, 9.0, Color::new(0.0, 0.0, 0.0, 0.7));
                let col = if h.charge > 0.88 { RED } else if h.charge > 0.6 { ORANGE } else { YELLOW };
                draw_rectangle(bx, by, bar_w * h.charge, 7.0, col);
            }
        }
    }
}

fn draw_ball(m: &Match, v: &View) {
    let b = &m.ball;
    let s = v.w(b.pos);
    let shadow_r = v.s(0.2 * (1.0 - (b.z / 8.0).min(0.5)));
    draw_circle(s.x + v.s(0.08), s.y + v.s(0.08), shadow_r, Color::new(0.0, 0.0, 0.0, 0.4));
    let p = v.w(b.pos + lift(b.z));
    let r = v.s(0.22 + b.z * 0.025);
    draw_circle(p.x, p.y, r, WHITE);
    draw_circle_lines(p.x, p.y, r, 1.0, Color::new(0.2, 0.2, 0.2, 1.0));
    let spin = (b.pos.x + b.pos.y) * 2.0;
    for k in 0..3 {
        let a = spin + k as f32 * 2.1;
        draw_circle(p.x + a.cos() * r * 0.45, p.y + a.sin() * r * 0.45, r * 0.25, Color::new(0.15, 0.15, 0.15, 1.0));
    }
}

fn text_center(text: &str, y: f32, size: f32, color: Color) {
    let d = measure_text(text, None, size as u16, 1.0);
    draw_text(text, (screen_width() - d.width) / 2.0, y, size, color);
}

fn draw_hud(m: &Match) {
    // Scorebug.
    let (min, sec) = m.display_minute();
    let x = 18.0;
    let y = 16.0;
    draw_rectangle(x, y, 400.0, 38.0, Color::new(0.05, 0.06, 0.1, 0.85));
    for (k, tx) in [(0usize, x), (1, x + 206.0)] {
        draw_rectangle(tx, y, 6.0, 38.0, m.teams[k].shirt);
        draw_crest(&m.teams[k], tx + 30.0, y + 19.0, 30.0);
        draw_text(m.teams[k].short, tx + 52.0, y + 27.0, 28.0, WHITE);
    }
    let score = format!("{} - {}", m.score[0], m.score[1]);
    draw_rectangle(x + 116.0, y, 86.0, 38.0, Color::new(0.95, 0.95, 0.95, 1.0));
    let d = measure_text(&score, None, 30, 1.0);
    draw_text(&score, x + 159.0 - d.width / 2.0, y + 28.0, 30.0, BLACK);
    let clock = format!("{:02}:{:02}", min, sec);
    draw_rectangle(x + 328.0, y, 72.0, 38.0, Color::new(0.85, 0.2, 0.25, 1.0));
    draw_text(&clock, x + 334.0, y + 27.0, 26.0, WHITE);
    draw_text(if m.half == 1 { "1st half" } else { "2nd half" }, x, y + 56.0, 18.0, Color::new(1.0, 1.0, 1.0, 0.7));

    draw_minimap(m);

    if let Some((text, t)) = &m.banner {
        if m.phase != Phase::FullTime {
            let alpha = t.min(0.4) / 0.4;
            let big = text.starts_with("GOAL");
            let size = if big { 72.0 } else { 44.0 };
            let y = if big { screen_height() * 0.32 } else { 120.0 };
            draw_rectangle(0.0, y - size * 0.95, screen_width(), size * 1.3, Color::new(0.0, 0.0, 0.0, 0.45 * alpha));
            text_center(text, y, size, Color::new(1.0, 1.0, 1.0, alpha));
        }
    }

    if let Some(l) = m.lock {
        if let Some(h) = m.humans.iter().find(|h| h.player == l.taker) {
            let col = P_COLORS[h.team.min(1)];
            let msg = match l.kind {
                Restart::Penalty => "Aim with direction, hold SHOOT for power",
                Restart::Corner => "LOB to cross into the box",
                _ => "PASS / THROUGH / LOB to restart",
            };
            text_center(msg, screen_height() - 30.0, 22.0, col);
        }
    }

    if !m.humans.is_empty() && m.phase != Phase::FullTime {
        let help = if m.humans.len() == 1 {
            "Move WASD/Arrows · Sprint Shift · Pass J · Through I · Lob L · Shoot K/Space (hold) · Switch Q · Tackle K · Slide L · Pause Esc"
        } else {
            "P1: WASD J I L K LShift Q    P2: Arrows , ; / . RShift M    Pause Esc"
        };
        draw_text(help, 18.0, screen_height() - 10.0, 16.0, Color::new(1.0, 1.0, 1.0, 0.55));
    }

    match m.phase {
        Phase::HalfTime { .. } => stats_panel(m, "HALF TIME", false),
        Phase::FullTime => stats_panel(m, "FULL TIME", true),
        _ => {}
    }

    if m.paused {
        draw_rectangle(0.0, 0.0, screen_width(), screen_height(), Color::new(0.0, 0.0, 0.0, 0.6));
        text_center("PAUSED", screen_height() / 2.0 - 20.0, 64.0, WHITE);
        text_center("Esc  resume      Q  quit to menu", screen_height() / 2.0 + 30.0, 26.0, LIGHTGRAY);
    }
}

fn draw_minimap(m: &Match) {
    let w = 230.0;
    let h = w * (HALF_W * 2.0) / (HALF_L * 2.0);
    let x0 = (screen_width() - w) / 2.0;
    let y0 = screen_height() - h - 28.0;
    draw_rectangle(x0 - 4.0, y0 - 4.0, w + 8.0, h + 8.0, Color::new(0.0, 0.0, 0.0, 0.45));
    draw_rectangle(x0, y0, w, h, Color::new(0.15, 0.45, 0.2, 0.65));
    draw_rectangle_lines(x0, y0, w, h, 1.0, Color::new(1.0, 1.0, 1.0, 0.5));
    draw_line(x0 + w / 2.0, y0, x0 + w / 2.0, y0 + h, 1.0, Color::new(1.0, 1.0, 1.0, 0.5));
    let map = |p: Vec2| vec2(x0 + (p.x + HALF_L) / (HALF_L * 2.0) * w, y0 + (p.y + HALF_W) / (HALF_W * 2.0) * h);
    for (i, p) in m.players.iter().enumerate() {
        let s = map(p.pos);
        let c = if p.role == Role::Gk { m.teams[p.team].keeper } else { m.teams[p.team].shirt };
        draw_circle(s.x, s.y, 3.2, c);
        if let Some(h) = m.humans.iter().find(|h| h.player == i) {
            draw_circle_lines(s.x, s.y, 5.0, 1.5, P_COLORS[h.team.min(1)]);
        }
    }
    let b = map(m.ball.pos);
    draw_circle(b.x, b.y, 2.6, WHITE);
}

fn stats_panel(m: &Match, title: &str, final_whistle: bool) {
    let w = 520.0;
    let h = 300.0 + m.scorers.len().min(8) as f32 * 22.0;
    let x = (screen_width() - w) / 2.0;
    let y = (screen_height() - h) / 2.0;
    draw_rectangle(x, y, w, h, Color::new(0.04, 0.05, 0.09, 0.92));
    draw_rectangle(x, y, w, 4.0, Color::new(0.95, 0.75, 0.2, 1.0));
    text_center(title, y + 50.0, 40.0, WHITE);
    let line = format!("{}  {} - {}  {}", m.teams[0].name, m.score[0], m.score[1], m.teams[1].name);
    text_center(&line, y + 95.0, 28.0, WHITE);

    let total = (m.possession[0] + m.possession[1]).max(0.01);
    let pos0 = (m.possession[0] / total * 100.0).round();
    let rows = [
        ("Possession", format!("{}%", pos0), format!("{}%", 100.0 - pos0)),
        ("Shots", m.shots[0].to_string(), m.shots[1].to_string()),
    ];
    let mut ry = y + 140.0;
    for (label, a, b) in rows {
        draw_text(&a, x + 60.0, ry, 24.0, m.teams[0].shirt);
        text_center(label, ry, 22.0, LIGHTGRAY);
        let d = measure_text(&b, None, 24, 1.0);
        draw_text(&b, x + w - 60.0 - d.width, ry, 24.0, m.teams[1].shirt);
        ry += 32.0;
    }
    ry += 8.0;
    for (team, name, minute) in m.scorers.iter().take(8) {
        let s = format!("{}'  {}", minute, name);
        if *team == 0 {
            draw_text(&s, x + 40.0, ry, 20.0, WHITE);
        } else {
            let d = measure_text(&s, None, 20, 1.0);
            draw_text(&s, x + w - 40.0 - d.width, ry, 20.0, WHITE);
        }
        ry += 22.0;
    }
    if final_whistle {
        text_center("Enter  back to menu", y + h - 24.0, 22.0, Color::new(0.95, 0.75, 0.2, 1.0));
    }
}

/// Draws a club's crest centred at (cx, cy). Crests are original geometric designs in the club colours.
pub fn draw_crest(team: &TeamDef, cx: f32, cy: f32, size: f32) {
    let (p, q) = (team.shirt, team.trim);
    let h = size / 2.0;
    let border = (size * 0.07).max(1.5);
    let ink = if p.r * 0.3 + p.g * 0.59 + p.b * 0.11 > 0.6 { Color::new(0.08, 0.08, 0.1, 1.0) } else { WHITE };

    if team.crest == CrestStyle::Roundel {
        draw_circle(cx, cy, h, q);
        draw_circle(cx, cy, h - border, p);
        draw_circle_lines(cx, cy, h * 0.68, border * 0.6, q);
    } else {
        // Shield: a block on top of a downward point.
        let shield = |grow: f32, c: Color| {
            let (l, r, top, mid) = (cx - h - grow, cx + h + grow, cy - h - grow, cy + h * 0.15);
            draw_rectangle(l, top, r - l, mid - top, c);
            draw_triangle(vec2(l, mid), vec2(r, mid), vec2(cx, cy + h + grow * 1.6), c);
        };
        shield(border, q);
        shield(0.0, p);
        let (top, mid) = (cy - h, cy + h * 0.15);
        match team.crest {
            CrestStyle::Stripes => {
                let w = size / 7.0;
                for k in [1.0, 3.0, 5.0] {
                    draw_rectangle(cx - h + k * w, top, w, mid - top, q);
                }
            }
            CrestStyle::Halves => draw_rectangle(cx, top, h, mid - top, q),
            CrestStyle::Chevron => {
                let t = size * 0.14;
                draw_line(cx - h, top + size * 0.1, cx, top + size * 0.42, t, q);
                draw_line(cx, top + size * 0.42, cx + h, top + size * 0.1, t, q);
            }
            CrestStyle::Sash => draw_line(cx - h, top, cx + h, mid, size * 0.16, q),
            CrestStyle::Roundel => {}
        }
        // A label band carries the initials.
        draw_rectangle(cx - h, cy - size * 0.1, size, size * 0.3, Color::new(0.0, 0.0, 0.0, 0.35));
    }
    let fs = size * 0.42;
    let d = measure_text(team.initials, None, fs as u16, 1.0);
    let y = if team.crest == CrestStyle::Roundel { cy + fs * 0.35 } else { cy + size * 0.12 };
    let color = if team.crest == CrestStyle::Roundel { ink } else { WHITE };
    draw_text(team.initials, cx - d.width / 2.0, y, fs, color);
}
