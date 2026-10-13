//! Leagues and clubs (parody names) with their player cards.

pub mod bio;
pub mod types;

mod burger;
mod camel;
mod cheese;
mod croissant;
mod dosa;
mod obesity;
mod paella;
mod pastel;
mod pizza;
mod samba;
mod spud;

use types::League;

/// Every league in menu order.
pub fn leagues() -> [&'static League; 11] {
    [
        &spud::LEAGUE,
        &paella::LEAGUE,
        &pizza::LEAGUE,
        &burger::LEAGUE,
        &croissant::LEAGUE,
        &dosa::LEAGUE,
        &camel::LEAGUE,
        &obesity::LEAGUE,
        &samba::LEAGUE,
        &cheese::LEAGUE,
        &pastel::LEAGUE,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::squad::TeamSetup;

    /// Every club builds a full squad with a goalkeeper in goal and unique shirt numbers.
    #[test]
    fn every_club_builds_a_squad() {
        let mut clubs = 0;
        for l in leagues() {
            for c in l.clubs {
                let t = TeamSetup::from_club(c);
                assert!(t.squad.len() >= 18, "{} has {}", c.name, t.squad.len());
                assert_eq!(t.squad[t.xi[0]].pos, types::Pos::GK, "{} has no keeper in goal", c.name);
                let mut nums: Vec<u8> = t.squad.iter().map(|p| p.num).collect();
                nums.sort();
                nums.dedup();
                assert_eq!(nums.len(), t.squad.len(), "{} has duplicate numbers", c.name);
                clubs += 1;
            }
        }
        println!("{clubs} clubs");
        assert_eq!(clubs, 209);
    }
}
