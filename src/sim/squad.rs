//! Turns a club's player cards into a match squad and starting XI.

use super::formation::{self, Slot};
use super::rng::gen_range;
use crate::data::types::{Club, Crest, Pos, P};

#[derive(Clone, Debug)]
pub struct Card {
    pub name: String,
    pub pos: Pos,
    pub num: u8,
    pub ovr: u8,
    pub pac: u8,
    pub sho: u8,
    pub pas: u8,
    pub dri: u8,
    pub def: u8,
    pub phy: u8,
    /// True for fictional fillers created because the club's card list was short.
    pub generated: bool,
}

impl From<&P> for Card {
    fn from(p: &P) -> Card {
        Card {
            name: p.name.to_string(),
            pos: p.pos,
            num: p.num,
            ovr: p.ovr,
            pac: p.pac,
            sho: p.sho,
            pas: p.pas,
            dri: p.dri,
            def: p.def,
            phy: p.phy,
            generated: false,
        }
    }
}

#[derive(Clone)]
pub struct TeamSetup {
    pub name: String,
    pub short: String,
    pub home: u32,
    pub trim: u32,
    pub away: u32,
    pub keeper: u32,
    pub crest: Crest,
    pub formation: String,
    pub rating: u8,
    pub squad: Vec<Card>,
    /// Squad indices of the starting XI, in formation-slot order.
    pub xi: [usize; 11],
}

const FIRST: [&str; 30] = [
    "Arun", "Karthik", "Vijay", "Ravi", "Sanjay", "Mateo", "Lucas", "Noah", "Leo", "Omar", "Yusuf", "Kenji",
    "Diego", "Tomas", "Felix", "Ivan", "Hugo", "Rafael", "Ade", "Kwame", "Elias", "Nikhil", "Pranav", "Suresh",
    "Andre", "Marco", "Jonas", "Emil", "Kai", "Theo",
];
const LAST: [&str; 40] = [
    "Benchwarmer", "Iyer", "Pillai", "Menon", "Natarajan", "Okoro", "Mensah", "Albuquerque", "Moreau", "Novak",
    "Larsen", "Kovac", "Haddad", "Tanaka", "Azevedo", "Reyes", "Fischer", "Brandt", "Petrov", "Mbeki", "Duarte",
    "Lindqvist", "Ricci", "Varga", "Kaur", "Sethi", "Banerjee", "Rahman", "Osei", "Navarro", "Ferreira", "Holm",
    "Weber", "Castillo", "Aziz", "Krishnan", "Subramanian", "Gomez", "Nakamura", "Valdez",
];

/// How well a player at `has` covers a slot asking for `want`: 0 is natural, higher is worse.
pub fn fit_penalty(want: Pos, has: Pos) -> i32 {
    use Pos::*;
    if want == has {
        return 0;
    }
    let group = |p: Pos| match p {
        GK => 0,
        CB => 1,
        LB | LWB => 2,
        RB | RWB => 3,
        CDM | CM => 4,
        CAM | CF => 5,
        LM | LW => 6,
        RM | RW => 7,
        ST => 8,
    };
    let (a, b) = (group(want), group(has));
    if a == b {
        return 2;
    }
    if a == 0 || b == 0 {
        return 60;
    }
    let near = [(1, 4), (2, 3), (2, 6), (3, 7), (4, 5), (5, 8), (6, 7), (6, 8), (7, 8), (5, 6), (5, 7), (1, 2), (1, 3)];
    if near.contains(&(a.min(b), a.max(b))) {
        6
    } else {
        14
    }
}

fn filler(pos: Pos, num: u8, rating: u8) -> Card {
    let r = |base: i32, spread: i32| (base + gen_range(-spread, spread + 1)).clamp(35, 95) as u8;
    let ovr = r(rating as i32 - 5, 3);
    let o = ovr as i32;
    let (pac, sho, pas, dri, def, phy) = match pos {
        Pos::GK => (r(o, 3), r(o, 3), r(o - 6, 4), r(o + 1, 3), r(o - 30, 6), r(o, 3)),
        Pos::CB => (r(o - 8, 6), r(o - 30, 6), r(o - 12, 6), r(o - 14, 6), r(o + 2, 3), r(o + 3, 4)),
        Pos::LB | Pos::RB | Pos::LWB | Pos::RWB => (r(o + 3, 4), r(o - 22, 6), r(o - 5, 4), r(o - 3, 4), r(o - 2, 4), r(o - 3, 4)),
        Pos::CDM | Pos::CM => (r(o - 6, 5), r(o - 8, 6), r(o + 2, 3), r(o, 3), r(o - 4, 6), r(o, 4)),
        Pos::CAM | Pos::CF => (r(o, 4), r(o - 1, 4), r(o + 1, 3), r(o + 3, 3), r(o - 28, 6), r(o - 8, 5)),
        Pos::LM | Pos::RM | Pos::LW | Pos::RW => (r(o + 6, 4), r(o - 3, 4), r(o - 2, 4), r(o + 3, 3), r(o - 30, 6), r(o - 10, 5)),
        Pos::ST => (r(o + 2, 5), r(o + 3, 3), r(o - 8, 5), r(o, 4), r(o - 35, 6), r(o, 5)),
    };
    let name = format!("{} {}", FIRST[gen_range(0, FIRST.len())], LAST[gen_range(0, LAST.len())]);
    Card { name, pos, num, ovr, pac, sho, pas, dri, def, phy, generated: true }
}

impl TeamSetup {
    pub fn from_club(club: &Club) -> TeamSetup {
        let mut squad: Vec<Card> = club.players.iter().map(Card::from).collect();
        let slots = formation::slots(club.formation);

        let free_num = |squad: &Vec<Card>| (2..99u8).find(|n| !squad.iter().any(|c| c.num == *n)).unwrap_or(99);
        // Make sure every slot has a natural fit and there are two keepers, then pad to 18.
        let gks = squad.iter().filter(|c| c.pos == Pos::GK).count();
        for _ in gks..2 {
            let n = if squad.iter().any(|c| c.num == 1) { free_num(&squad) } else { 1 };
            squad.push(filler(Pos::GK, n, club.rating));
        }
        for slot in slots.iter().skip(1) {
            let covered = squad.iter().filter(|c| fit_penalty(slot.pos, c.pos) <= 2).count();
            let needed = slots.iter().filter(|s| fit_penalty(slot.pos, s.pos) <= 2).count();
            if covered < needed {
                let n = free_num(&squad);
                squad.push(filler(slot.pos, n, club.rating));
            }
        }
        let pad = [Pos::CB, Pos::CM, Pos::ST, Pos::RB, Pos::LB, Pos::LW, Pos::CDM];
        let mut k = 0;
        while squad.len() < 18 {
            let n = free_num(&squad);
            squad.push(filler(pad[k % pad.len()], n, club.rating));
            k += 1;
        }

        let xi = pick_xi(&squad, &slots);
        TeamSetup {
            name: club.name.to_string(),
            short: club.short.to_string(),
            home: club.home,
            trim: club.trim,
            away: club.away,
            keeper: club.keeper,
            crest: club.crest,
            formation: club.formation.to_string(),
            rating: club.rating,
            squad,
            xi,
        }
    }

    pub fn slots(&self) -> [Slot; 11] {
        formation::slots(&self.formation)
    }

    /// Re-picks the XI after the formation changes.
    pub fn set_formation(&mut self, name: &str) {
        self.formation = name.to_string();
        self.xi = pick_xi(&self.squad, &self.slots());
    }
}

/// Greedy XI: goalkeeper first, then each slot takes the best remaining player, penalised for playing out of position.
pub fn pick_xi(squad: &[Card], slots: &[Slot; 11]) -> [usize; 11] {
    let mut used = vec![false; squad.len()];
    let mut xi = [0usize; 11];
    let mut order: Vec<usize> = (0..11).collect();
    // Fill the most specialised slots first so wide players aren't spent in the middle.
    order.sort_by_key(|&k| squad.iter().filter(|c| fit_penalty(slots[k].pos, c.pos) == 0).count());
    order.retain(|&k| k != 0);
    order.insert(0, 0);
    for k in order {
        let best = (0..squad.len())
            .filter(|&i| !used[i])
            .max_by_key(|&i| squad[i].ovr as i32 - fit_penalty(slots[k].pos, squad[i].pos))
            .unwrap();
        used[best] = true;
        xi[k] = best;
    }
    xi
}
