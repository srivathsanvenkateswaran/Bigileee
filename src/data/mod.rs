//! Leagues and clubs (parody names) with their player cards.

pub mod types;

mod burger;
mod cheese;
mod croissant;
mod paella;
mod pastel;
mod pizza;
mod spud;

use types::League;

/// Every league in menu order.
pub fn leagues() -> [&'static League; 7] {
    [
        &spud::LEAGUE,
        &paella::LEAGUE,
        &pizza::LEAGUE,
        &burger::LEAGUE,
        &croissant::LEAGUE,
        &cheese::LEAGUE,
        &pastel::LEAGUE,
    ]
}
