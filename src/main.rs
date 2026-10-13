mod ai;
mod audio;
mod consts;
mod game;
mod input;
mod render;
mod teams;

use game::{Match, MatchConfig, Mode};
use macroquad::prelude::*;
use teams::TeamDef;

fn window_conf() -> Conf {
    Conf {
        window_title: "KaalPanthu".to_owned(),
        window_width: 1280,
        window_height: 800,
        high_dpi: true,
        sample_count: 4,
        ..Default::default()
    }
}

const MODES: [(&str, Mode); 3] = [
    ("1 Player vs CPU", Mode::VsCpu),
    ("2 Players (same keyboard)", Mode::TwoPlayer),
    ("Watch CPU vs CPU", Mode::Watch),
];
const HALVES: [(&str, f32); 4] = [("2 min", 120.0), ("3 min", 180.0), ("5 min", 300.0), ("8 min", 480.0)];
const LEVELS: [(&str, f32); 4] = [("Amateur", 0.25), ("Pro", 0.5), ("World Class", 0.72), ("Legend", 0.92)];

struct Menu {
    row: usize,
    mode: usize,
    home: usize,
    away: usize,
    half: usize,
    level: usize,
}

impl Menu {
    fn update(&mut self, teams: &[TeamDef]) -> Option<MatchConfig> {
        const ROWS: usize = 6;
        if is_key_pressed(KeyCode::Down) || is_key_pressed(KeyCode::S) {
            self.row = (self.row + 1) % ROWS;
        }
        if is_key_pressed(KeyCode::Up) || is_key_pressed(KeyCode::W) {
            self.row = (self.row + ROWS - 1) % ROWS;
        }
        let delta: i32 = if is_key_pressed(KeyCode::Right) || is_key_pressed(KeyCode::D) {
            1
        } else if is_key_pressed(KeyCode::Left) || is_key_pressed(KeyCode::A) {
            -1
        } else {
            0
        };
        let cycle = |v: &mut usize, n: usize| *v = (*v as i32 + delta).rem_euclid(n as i32) as usize;
        match self.row {
            0 => cycle(&mut self.mode, MODES.len()),
            1 => {
                cycle(&mut self.home, teams.len());
                if self.home == self.away {
                    cycle(&mut self.home, teams.len());
                }
            }
            2 => {
                cycle(&mut self.away, teams.len());
                if self.home == self.away {
                    cycle(&mut self.away, teams.len());
                }
            }
            3 => cycle(&mut self.half, HALVES.len()),
            4 => cycle(&mut self.level, LEVELS.len()),
            _ => {}
        }
        if is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::Space) {
            return Some(MatchConfig {
                home: self.home,
                away: self.away,
                mode: MODES[self.mode].1,
                half_secs: HALVES[self.half].1,
                difficulty: LEVELS[self.level].1,
            });
        }
        None
    }

    fn draw(&self, teams: &[TeamDef], t: f32) {
        let (sw, sh) = (screen_width(), screen_height());
        clear_background(Color::new(0.07, 0.2, 0.1, 1.0));
        for k in 0..14 {
            let w = sw / 14.0;
            if k % 2 == 0 {
                draw_rectangle(k as f32 * w, 0.0, w, sh, Color::new(1.0, 1.0, 1.0, 0.025));
            }
        }
        draw_circle_lines(sw / 2.0, sh * 0.52, sh * 0.32, 3.0, Color::new(1.0, 1.0, 1.0, 0.07));
        draw_line(sw / 2.0, 0.0, sw / 2.0, sh, 3.0, Color::new(1.0, 1.0, 1.0, 0.07));

        let center = |text: &str, y: f32, size: f32, c: Color| {
            let d = measure_text(text, None, size as u16, 1.0);
            draw_text(text, (sw - d.width) / 2.0, y, size, c);
        };
        let gold = Color::new(0.97, 0.78, 0.25, 1.0);
        center("KAALPANTHU", 120.0, 96.0, WHITE);
        center("11-a-side football, written in Rust", 160.0, 26.0, gold);

        let rows: [(&str, String); 5] = [
            ("Mode", MODES[self.mode].0.to_string()),
            ("Home", teams[self.home].name.to_string()),
            ("Away", teams[self.away].name.to_string()),
            ("Half length", HALVES[self.half].0.to_string()),
            ("CPU level", LEVELS[self.level].0.to_string()),
        ];
        let x = sw / 2.0 - 260.0;
        let mut y = 230.0;
        for (k, (label, value)) in rows.iter().enumerate() {
            let selected = self.row == k;
            if selected {
                draw_rectangle(x - 16.0, y - 30.0, 560.0, 44.0, Color::new(1.0, 1.0, 1.0, 0.1));
                draw_rectangle(x - 16.0, y - 30.0, 4.0, 44.0, gold);
            }
            draw_text(label, x, y, 26.0, Color::new(1.0, 1.0, 1.0, 0.7));
            let v = if selected { format!("<  {}  >", value) } else { value.clone() };
            draw_text(&v, x + 200.0, y, 28.0, if selected { gold } else { WHITE });
            if k == 1 || k == 2 {
                let team = &teams[if k == 1 { self.home } else { self.away }];
                render::draw_crest(team, x + 500.0, y - 9.0, 34.0);
                let stats = format!("ATT {:.0}  DEF {:.0}  PAC {:.0}", team.attack * 100.0, team.defense * 100.0, team.pace * 100.0);
                if selected {
                    draw_text(&stats, x + 200.0, y + 22.0, 18.0, Color::new(1.0, 1.0, 1.0, 0.6));
                }
            }
            y += if (k == 1 || k == 2) && selected { 62.0 } else { 52.0 };
        }
        let start_sel = self.row == 5;
        let pulse = if start_sel { 0.75 + 0.25 * (t * 5.0).sin() } else { 0.6 };
        let label = "KICK OFF";
        let d = measure_text(label, None, 40, 1.0);
        let bx = (sw - d.width) / 2.0 - 30.0;
        draw_rectangle(bx, y - 6.0, d.width + 60.0, 58.0, Color::new(0.97, 0.78, 0.25, pulse));
        draw_text(label, bx + 30.0, y + 36.0, 40.0, Color::new(0.05, 0.1, 0.05, 1.0));

        let help = [
            "Up/Down choose · Left/Right change · Enter to play",
            "In match: WASD move · Shift sprint · J pass · I through ball · L lob/cross · K or Space shoot (hold for power)",
            "Defending: K tackle · L slide · J or Q switch player · Esc pause",
        ];
        let mut hy = sh - 86.0;
        for line in help {
            center(line, hy, 20.0, Color::new(1.0, 1.0, 1.0, 0.6));
            hy += 26.0;
        }
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    rand::srand((miniquad::date::now() * 1000.0) as u64);
    let sfx = audio::Sfx::load().await;
    let teams = teams::all();
    let crowd = render::Crowd::new();
    let mut menu = Menu { row: 5, mode: 0, home: 0, away: 1, half: 1, level: 1 };
    let mut current: Option<Match> = None;
    // Debug aid: KAALPANTHU_SNAPSHOT=<dir> saves the menu and a CPU match as PNGs, then quits.
    let snapshot = std::env::var("KAALPANTHU_SNAPSHOT").ok();
    let mut frame = 0u32;

    loop {
        frame += 1;
        if snapshot.is_some() {
            match frame {
                41 => {
                    menu.mode = 2;
                    current = Some(Match::new(
                        MatchConfig { home: menu.home, away: menu.away, mode: Mode::Watch, half_secs: 120.0, difficulty: 0.6 },
                        &teams,
                    ));
                }
                1501 => break,
                _ => {}
            }
        }
        let dt = get_frame_time().min(1.0 / 30.0);
        match &mut current {
            None => {
                if let Some(cfg) = menu.update(&teams) {
                    current = Some(Match::new(cfg, &teams));
                    sfx.crowd(true);
                }
                menu.draw(&teams, get_time() as f32);
            }
            Some(m) => {
                m.update(dt, &sfx);
                render::draw_match(m, &crowd);
                if m.exit {
                    current = None;
                    sfx.crowd(false);
                }
            }
        }
        if let Some(dir) = &snapshot {
            match frame {
                40 => get_screen_data().export_png(&format!("{dir}/menu.png")),
                400 | 900 | 1500 => get_screen_data().export_png(&format!("{dir}/match_{frame}.png")),
                _ => {}
            }
        }
        next_frame().await
    }
}
