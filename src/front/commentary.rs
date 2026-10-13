//! Match commentary: picks a line for each simulation call, shows it as a subtitle and speaks it
//! with the Mac's built-in `say` voices. Lines are original to this game.

use crate::sim::events::Call;
use crate::sim::rng::gen_range;
use bevy::prelude::*;
use std::process::{Child, Command, Stdio};

#[derive(Resource)]
pub struct Commentary {
    pub enabled: bool,
    pub line: String,
    /// A control hint that takes over the subtitle line while set.
    pub hint: Option<String>,
    shown_for: f32,
    since_last: f32,
    speaking: Option<Child>,
    voice_main: Option<String>,
    voice_colour: Option<String>,
}

impl Commentary {
    pub fn new() -> Commentary {
        // Pick voices that exist on this Mac; fall back to the system default.
        let voices = Command::new("say").args(["-v", "?"]).output().map(|o| String::from_utf8_lossy(&o.stdout).to_string()).unwrap_or_default();
        let has = |name: &str| voices.lines().any(|l| l.starts_with(name));
        let main = ["Daniel", "Eddy (English (UK))", "Alex"].iter().find(|v| has(v)).map(|v| v.to_string());
        let colour = ["Aman (English (India))", "Rishi", "Daniel"].iter().find(|v| has(v)).map(|v| v.to_string());
        Commentary {
            enabled: true,
            line: String::new(),
            hint: None,
            shown_for: 0.0,
            since_last: 10.0,
            speaking: None,
            voice_main: main,
            voice_colour: colour,
        }
    }

    fn busy(&mut self) -> bool {
        match &mut self.speaking {
            Some(c) => matches!(c.try_wait(), Ok(None)),
            None => false,
        }
    }

    pub fn stop(&mut self) {
        if let Some(mut c) = self.speaking.take() {
            let _ = c.kill();
        }
    }

    /// Opening line during the pre-match presentation.
    pub fn welcome(&mut self, home: &str, away: &str) {
        let pick = |xs: &[&str]| xs[gen_range(0, xs.len())].to_string();
        let l = pick(&[
            "Good afternoon and welcome. {h} against {a}, and the atmosphere here is something else.",
            "Welcome along. {h} take on {a}, and the snacks in the stands are already running low.",
            "Hello and welcome to {h} versus {a}. Grab your chai, this one should be good.",
        ]);
        self.say(&l.replace("{h}", home).replace("{a}", away), false, true);
    }

    fn say(&mut self, text: &str, colour: bool, urgent: bool) {
        self.line = text.to_string();
        self.shown_for = 0.0;
        self.since_last = 0.0;
        if !self.enabled {
            return;
        }
        if urgent {
            self.stop();
        } else if self.busy() {
            return;
        }
        let voice = if colour { &self.voice_colour } else { &self.voice_main };
        let mut cmd = Command::new("say");
        if let Some(v) = voice {
            cmd.args(["-v", v]);
        }
        cmd.args(["-r", if urgent { "215" } else { "195" }, text]).stdout(Stdio::null()).stderr(Stdio::null());
        self.speaking = cmd.spawn().ok();
    }

    pub fn tick(&mut self, dt: f32) {
        self.shown_for += dt;
        self.since_last += dt;
        if self.shown_for > 4.0 {
            self.line.clear();
        }
    }

    /// Names the scorer's celebration, in the colour voice.
    pub fn celebration(&mut self, scorer: &str, routine: &str) {
        let pick = |xs: &[&str]| xs[gen_range(0, xs.len())].to_string();
        let l = pick(&["And {p} busts out the {c}!", "Oh, it's the {c} from {p}!", "{p} goes for the {c}. Magnificent.", "The {c}! {p} has been practising that."]);
        self.say(&l.replace("{p}", scorer).replace("{c}", routine), true, true);
    }

    /// Reacts to one simulation call. `team_name` resolves a team index for goal lines.
    pub fn react(&mut self, call: &Call, team_name: impl Fn(usize) -> String) {
        let pick = |xs: &[&str]| xs[gen_range(0, xs.len())].to_string();
        let fill = |s: String, name: &str| s.replace("{p}", name);
        match call {
            Call::KickOff => {
                let l = pick(&["And we're under way!", "Here we go, the ball is rolling.", "The referee gets us started."]);
                self.say(&l, false, false);
            }
            Call::HalfTime => self.say(&pick(&["That's the half-time whistle.", "Half time. Time for a quick chai."]), false, true),
            Call::FullTime => self.say(&pick(&["There's the final whistle!", "It's all over!", "Full time, and that's the end of that."]), false, true),
            Call::Goal { scorer, team, own_goal } => {
                let l = if *own_goal {
                    fill(pick(&["Oh no, {p} has put it into his own net!", "An own goal from {p}. He'll want to forget that one."]), scorer)
                } else {
                    let t = team_name(*team);
                    let l = pick(&[
                        "GOAL! {p} scores!",
                        "It's in! {p} finds the net!",
                        "{p}! What a finish!",
                        "GOAL for {t}! {p} with the strike!",
                        "Oh, that's lovely from {p}!",
                    ]);
                    fill(l.replace("{t}", &t), scorer)
                };
                self.say(&l, false, true);
                if gen_range(0.0, 1.0) < 0.35 {
                    // A second voice chimes in, sometimes with a food metaphor.
                    let extra = pick(&[
                        "Smoother than ghee on a hot dosa, that.",
                        "Somebody get that man a filter coffee!",
                        "The crowd have gone absolutely biryani.",
                        "That's the finish of a man who skipped breakfast.",
                        "Absolute scenes!",
                    ]);
                    self.line = format!("{}  —  {}", self.line, extra);
                }
            }
            Call::Shot { player } => {
                if self.since_last > 1.2 {
                    self.say(&fill(pick(&["{p} lets fly!", "{p} has a go!", "Shot from {p}!", "{p} pulls the trigger!"]), player), false, false);
                }
            }
            Call::Save { keeper } => self.say(&fill(pick(&["Great save by {p}!", "{p} keeps it out!", "Denied by {p}!", "Strong hands from {p}."]), keeper), false, true),
            Call::Miss { player } => self.say(&fill(pick(&["{p} drags it wide.", "Over the bar from {p}.", "That's off target from {p}."]), player), false, false),
            Call::Woodwork { player } => self.say(&fill(pick(&["Off the woodwork! So close for {p}!", "{p} hits the frame of the goal!"]), player), false, true),
            Call::Tackle { player } => {
                if self.since_last > 3.0 {
                    self.say(&fill(pick(&["Good tackle by {p}.", "{p} wins it back.", "Strong challenge from {p}."]), player), true, false);
                }
            }
            Call::Foul { by } => self.say(&fill(pick(&["Free kick. {p} caught him there.", "The referee blows for a foul by {p}."]), by), false, false),
            Call::Card { player, red } => {
                let l = if *red {
                    pick(&["Red card! {p} is off!", "{p} is sent off! Big moment in this match."])
                } else {
                    pick(&["{p} goes into the book.", "Yellow card for {p}."])
                };
                self.say(&fill(l, player), false, true);
            }
            Call::Offside { player } => self.say(&fill(pick(&["The flag is up. {p} was offside.", "Offside against {p}."]), player), false, false),
            Call::Corner => {
                if self.since_last > 2.0 {
                    self.say(&pick(&["Corner kick.", "It's a corner."]), false, false);
                }
            }
            Call::Penalty => self.say(&pick(&["Penalty! The referee points to the spot!", "It's a penalty!"]), false, true),
            Call::Sub { off, on } => self.say(&format!("A change: {on} comes on for {off}."), false, false),
            Call::Shootout { player, scored } => {
                let l = if *scored { pick(&["{p} scores!", "{p} sends the keeper the wrong way!"]) } else { pick(&["{p} misses!", "Saved! {p} can't believe it."]) };
                self.say(&fill(l, player), false, true);
            }
            Call::ExtraTime => {
                let l = pick(&[
                    "Ninety minutes can't separate them. Thirty more it is.",
                    "Still level! We're going to extra time.",
                    "Extra time, and somebody is going to need a second dinner.",
                ]);
                self.say(&l, false, true);
            }
            Call::Injury { player } => {
                let l = pick(&[
                    "{p} is down, and that looks painful.",
                    "{p} is hobbling. The physio is grabbing the magic sponge.",
                    "That's a knock for {p}. He's moving like a man who ate too much payasam.",
                ]);
                self.say(&fill(l, player), true, false);
            }
            Call::Tactics { team, attacking } => {
                let t = team_name(*team);
                let l = if *attacking {
                    pick(&["{t} are throwing bodies forward now.", "The manager of {t} wants a goal. Everyone up!"])
                } else {
                    pick(&["{t} are shutting up shop.", "{t} drop deep to protect what they have."])
                };
                self.say(&l.replace("{t}", &t), true, false);
            }
        }
    }
}

impl Drop for Commentary {
    fn drop(&mut self) {
        self.stop();
    }
}
