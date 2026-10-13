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

/// Collects sound events during a step; the front end drains them each frame.
#[derive(Default)]
pub struct Sfx {
    events: Mutex<Vec<SoundEvent>>,
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
    pub fn drain(&self) -> Vec<SoundEvent> {
        std::mem::take(&mut *self.events.lock().unwrap())
    }
}
