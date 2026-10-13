//! Turns a club's player cards into a match squad and starting XI.

use super::formation::{self, Slot};
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
    /// Real height, weight, strong foot and age, when known.
    pub bio: Option<crate::data::bio::Bio>,
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
            bio: None,
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

const FIRST: [&str; 32] = [
    "Arun", "Karthik", "Vijay", "Ravi", "Sanjay", "Mateo", "Lucas", "Noah", "Leo", "Omar", "Yusuf", "Kenji",
    "Diego", "Tomas", "Felix", "Ivan", "Hugo", "Rafael", "Ade", "Kwame", "Elias", "Nikhil", "Pranav", "Suresh",
    "Andre", "Marco", "Jonas", "Emil", "Kai", "Theo", "Bruno", "Sami",
];
/// Fictional fillers get food surnames, in keeping with the league names.
const LAST: [&str; 48] = [
    "Benchwarmer", "Sambar", "Chutney", "Pappadum", "Rasam", "Kurma", "Idli", "Vadai", "Halwa", "Payasam",
    "Bajji", "Kesari", "Upma", "Pongal", "Murukku", "Laddu", "Jalebi", "Kulfi", "Samosa", "Pakora", "Naan",
    "Kebab", "Hummus", "Falafel", "Taco", "Burrito", "Nacho", "Pretzel", "Strudel", "Baguette", "Croquette",
    "Gelato", "Risotto", "Ravioli", "Churro", "Empanada", "Feijoada", "Brigadeiro", "Poutine", "Waffle",
    "Bratwurst", "Pierogi", "Gnocchi", "Tiramisu", "Shawarma", "Kunafa", "Dumpling", "Biryani",
];

/// Deterministic per-club randomness, so fillers keep their names and ratings between matches.
struct Seeded(u64);

impl Seeded {
    fn new(name: &str) -> Seeded {
        Seeded(name.bytes().fold(0xcbf2_9ce4_8422_2325u64, |h, b| (h ^ b as u64).wrapping_mul(0x100_0000_01b3)) | 1)
    }
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn range(&mut self, lo: i32, hi: i32) -> i32 {
        lo + (self.next() % (hi - lo) as u64) as i32
    }
    fn pick<'a>(&mut self, xs: &[&'a str]) -> &'a str {
        xs[(self.next() % xs.len() as u64) as usize]
    }
}

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

fn filler(rng: &mut Seeded, pos: Pos, num: u8, rating: u8, taken: &[Card]) -> Card {
    let mut r = |base: i32, spread: i32| (base + rng.range(-spread, spread + 1)).clamp(35, 95) as u8;
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
    let name = loop {
        let n = format!("{} {}", rng.pick(&FIRST), rng.pick(&LAST));
        if !taken.iter().any(|c| c.name == n) {
            break n;
        }
    };
    Card { name, pos, num, ovr, pac, sho, pas, dri, def, phy, generated: true, bio: None }
}

impl TeamSetup {
    pub fn from_club(club: &Club) -> TeamSetup {
        let mut squad: Vec<Card> = club.players.iter().map(Card::from).collect();
        for c in &mut squad {
            c.bio = crate::data::bio::lookup(club.name, c.num);
        }
        let mut rng = Seeded::new(club.name);
        let slots = formation::slots(club.formation);

        let free_num = |squad: &Vec<Card>| (2..99u8).find(|n| !squad.iter().any(|c| c.num == *n)).unwrap_or(99);
        // Make sure every slot has a natural fit and there are two keepers, then pad to 18.
        let gks = squad.iter().filter(|c| c.pos == Pos::GK).count();
        for _ in gks..2 {
            let n = if squad.iter().any(|c| c.num == 1) { free_num(&squad) } else { 1 };
            squad.push(filler(&mut rng, Pos::GK, n, club.rating, &squad));
        }
        for slot in slots.iter().skip(1) {
            let covered = squad.iter().filter(|c| fit_penalty(slot.pos, c.pos) <= 2).count();
            let needed = slots.iter().filter(|s| fit_penalty(slot.pos, s.pos) <= 2).count();
            if covered < needed {
                let n = free_num(&squad);
                squad.push(filler(&mut rng, slot.pos, n, club.rating, &squad));
            }
        }
        let pad = [Pos::CB, Pos::CM, Pos::ST, Pos::RB, Pos::LB, Pos::LW, Pos::CDM];
        let mut k = 0;
        while squad.len() < 18 {
            let n = free_num(&squad);
            squad.push(filler(&mut rng, pad[k % pad.len()], n, club.rating, &squad));
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

    /// Removes squad members (they've been sold), promotes youth fillers so the club still has two
    /// keepers and sixteen players, and re-picks the XI.
    pub fn drop_players(&mut self, gone: &[usize]) {
        if gone.is_empty() {
            return;
        }
        let mut k = 0;
        self.squad.retain(|_| {
            k += 1;
            !gone.contains(&(k - 1))
        });
        let mut rng = Seeded::new(&format!("{} youth", self.name));
        let free_num = |squad: &Vec<Card>| (2..99u8).find(|n| !squad.iter().any(|c| c.num == *n)).unwrap_or(99);
        while self.squad.iter().filter(|c| c.pos == Pos::GK).count() < 2 {
            let n = free_num(&self.squad);
            let c = filler(&mut rng, Pos::GK, n, self.rating, &self.squad);
            self.squad.push(c);
        }
        let pad = [Pos::CB, Pos::CM, Pos::ST, Pos::RB, Pos::LB, Pos::LW, Pos::CDM];
        let mut k = 0;
        while self.squad.len() < 16 {
            let n = free_num(&self.squad);
            let c = filler(&mut rng, pad[k % pad.len()], n, self.rating, &self.squad);
            self.squad.push(c);
            k += 1;
        }
        self.xi = pick_xi(&self.squad, &self.slots());
    }
}

/// Seats the same eleven players in a new formation. `pos[k]` is the natural position of whoever
/// stands in old slot k and `active[k]` is false for anyone sent off. Returns, for each new slot,
/// the old slot that fills it. The keeper (slot 0) never moves; players who are off the pitch
/// take whatever slots are left once everyone else is placed.
pub fn refit(pos: &[Pos; 11], active: &[bool; 11], old: &[Slot; 11], new: &[Slot; 11]) -> [usize; 11] {
    let mut out = [0usize; 11];
    let mut used = [false; 11];
    used[0] = true;
    let cost = |k: usize, j: usize| {
        let same_role = old[j].role == new[k].role;
        fit_penalty(new[k].pos, pos[j]) * 4 - same_role as i32 * 3
    };
    let mut order: Vec<usize> = (1..11).collect();
    order.sort_by_key(|&k| (1..11).filter(|&j| active[j] && fit_penalty(new[k].pos, pos[j]) <= 2).count());
    for k in order {
        let best = (1..11).filter(|&j| !used[j]).min_by_key(|&j| (!active[j], cost(k, j), j)).unwrap();
        used[best] = true;
        out[k] = best;
    }
    // Greedy can strand players; swap any pair of active players whose exchange fits better.
    let mut improved = true;
    while improved {
        improved = false;
        for a in 1..11 {
            for b in a + 1..11 {
                let (ja, jb) = (out[a], out[b]);
                if active[ja] && active[jb] && cost(a, jb) + cost(b, ja) < cost(a, ja) + cost(b, jb) {
                    out.swap(a, b);
                    improved = true;
                }
            }
        }
    }
    out
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
