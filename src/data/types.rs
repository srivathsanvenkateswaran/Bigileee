//! Data types for leagues, clubs and players. Kept dependency-free so league files can be
//! type-checked on their own.

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[allow(clippy::upper_case_acronyms)]
pub enum Pos {
    GK,
    CB,
    LB,
    RB,
    LWB,
    RWB,
    CDM,
    CM,
    CAM,
    LM,
    RM,
    LW,
    RW,
    CF,
    ST,
}

/// A player card. Ratings are 1-99: overall, pace, shooting, passing, dribbling, defending, physical.
#[derive(Clone, Copy, Debug)]
pub struct P {
    pub name: &'static str,
    pub pos: Pos,
    pub num: u8,
    pub ovr: u8,
    pub pac: u8,
    pub sho: u8,
    pub pas: u8,
    pub dri: u8,
    pub def: u8,
    pub phy: u8,
}

/// `p("Name", Pos::ST, 9, [ovr, pac, sho, pas, dri, def, phy])`
pub const fn p(name: &'static str, pos: Pos, num: u8, r: [u8; 7]) -> P {
    P { name, pos, num, ovr: r[0], pac: r[1], sho: r[2], pas: r[3], dri: r[4], def: r[5], phy: r[6] }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Crest {
    Shield,
    Roundel,
    Stripes,
    Halves,
    Chevron,
    Sash,
    Hoops,
}

pub struct Club {
    pub name: &'static str,
    pub short: &'static str,
    /// Colours as 0xRRGGBB.
    pub home: u32,
    pub trim: u32,
    pub away: u32,
    pub keeper: u32,
    pub crest: Crest,
    /// One of "4-4-2", "4-3-3", "4-2-3-1", "3-5-2", "5-3-2", "4-1-2-1-2".
    pub formation: &'static str,
    /// Team strength 1-99, used to fill generated squad members and for menus.
    pub rating: u8,
    pub players: &'static [P],
}

pub struct League {
    pub name: &'static str,
    pub country: &'static str,
    pub clubs: &'static [Club],
}
