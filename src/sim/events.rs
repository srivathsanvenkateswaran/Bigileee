//! Sounds the simulation asks the front end to play.

use std::sync::Mutex;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SoundEvent {
    Kick(f32),
    Whistle,
    WhistleLong,
    Cheer,
    Tackle,
    Post,
    Ooh,
}

/// Things worth a line of commentary.
#[derive(Clone, Debug, PartialEq)]
pub enum Call {
    KickOff,
    HalfTime,
    FullTime,
    Goal { scorer: String, team: usize, own_goal: bool },
    Shot { player: String },
    Save { keeper: String },
    Miss { player: String },
    Woodwork { player: String },
    Tackle { player: String },
    Foul { by: String },
    Card { player: String, red: bool },
    Offside { player: String },
    Corner,
    Penalty,
    Sub { off: String, on: String },
    Shootout { player: String, scored: bool },
    /// Level after 90 minutes of a knockout tie.
    ExtraTime,
    Injury { player: String },
    /// A CPU side changes approach: `attacking` true when chasing the game, false when sitting on a lead.
    Tactics { team: usize, attacking: bool },
}

/// Collects sound and commentary events during a step; the front end drains them each frame.
#[derive(Default)]
pub struct Sfx {
    events: Mutex<Vec<SoundEvent>>,
    calls: Mutex<Vec<Call>>,
}

impl Sfx {
    fn push(&self, e: SoundEvent) {
        self.events.lock().unwrap().push(e);
    }
    pub fn kick(&self, power: f32) {
        self.push(SoundEvent::Kick(power));
    }
    pub fn whistle(&self) {
        self.push(SoundEvent::Whistle);
    }
    pub fn whistle_long(&self) {
        self.push(SoundEvent::WhistleLong);
    }
    pub fn cheer(&self) {
        self.push(SoundEvent::Cheer);
    }
    pub fn tackle(&self) {
        self.push(SoundEvent::Tackle);
    }
    pub fn post(&self) {
        self.push(SoundEvent::Post);
    }
    pub fn ooh(&self) {
        self.push(SoundEvent::Ooh);
    }
    pub fn call(&self, c: Call) {
        self.calls.lock().unwrap().push(c);
    }
    pub fn drain_calls(&self) -> Vec<Call> {
        std::mem::take(&mut *self.calls.lock().unwrap())
    }
    pub fn drain(&self) -> Vec<SoundEvent> {
        std::mem::take(&mut *self.events.lock().unwrap())
    }
}
