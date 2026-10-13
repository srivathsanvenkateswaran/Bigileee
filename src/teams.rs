//! Clubs (parody names, original crests) and squad generation.

use macroquad::prelude::*;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum CrestStyle {
    Stripes,
    Chevron,
    Roundel,
    Halves,
    Sash,
}

#[derive(Clone)]
pub struct TeamDef {
    pub name: &'static str,
    pub short: &'static str,
    pub initials: &'static str,
    pub crest: CrestStyle,
    pub shirt: Color,
    pub trim: Color,
    pub keeper: Color,
    pub attack: f32,
    pub defense: f32,
    pub pace: f32,
    pub passing: f32,
    pub keeping: f32,
}

fn hex(v: u32) -> Color {
    Color::from_rgba((v >> 16) as u8, (v >> 8) as u8, v as u8, 255)
}

pub fn all() -> Vec<TeamDef> {
    use CrestStyle::*;
    #[rustfmt::skip]
    let raw: [(&str, &str, &str, CrestStyle, u32, u32, u32, [f32; 5]); 16] = [
        // name, short, initials, crest, shirt, trim, keeper, [attack, defense, pace, passing, keeping]
        ("Manchester Town",    "MNT", "MT", Roundel, 0x6cabdd, 0xffffff, 0xf2c94c, [0.88, 0.84, 0.83, 0.90, 0.84]),
        ("Manchester Divided", "MND", "MD", Chevron, 0xda291c, 0x1a1a1a, 0x2e7d32, [0.82, 0.78, 0.82, 0.80, 0.82]),
        ("London Barracks",    "LDB", "LB", Sash,    0xef0107, 0xffffff, 0x8e24aa, [0.84, 0.83, 0.82, 0.85, 0.83]),
        ("West London Blues",  "WLB", "WL", Roundel, 0x034694, 0xffffff, 0xf4d03f, [0.81, 0.80, 0.81, 0.82, 0.81]),
        ("Merseyside Reds",    "MSR", "MR", Halves,  0xc8102e, 0xf6eb61, 0x00a39b, [0.87, 0.82, 0.86, 0.84, 0.85]),
        ("Madrid Regal",       "MRG", "RG", Roundel, 0xf5f5f5, 0x5b2d8e, 0x1a1a1a, [0.88, 0.83, 0.84, 0.86, 0.86]),
        ("Catalunya Club",     "CAT", "CC", Stripes, 0x004d98, 0xa50044, 0xf2c94c, [0.86, 0.80, 0.80, 0.90, 0.82]),
        ("Munich Bavarians",   "MUB", "MB", Halves,  0xdc052d, 0x0066b2, 0x2e7d32, [0.87, 0.82, 0.83, 0.86, 0.87]),
        ("Turin Zebras",       "TUZ", "TZ", Stripes, 0x111111, 0xf5f5f5, 0xf28c28, [0.80, 0.85, 0.77, 0.81, 0.84]),
        ("Paris Royals",       "PAR", "PR", Sash,    0x004170, 0xda291c, 0x2e7d32, [0.87, 0.79, 0.85, 0.84, 0.83]),
        ("Madrasin FC",        "MAD", "MF", Chevron, 0x0d3d91, 0xf2b705, 0xd84315, [0.76, 0.74, 0.79, 0.75, 0.75]),
        ("Bangalore FT",       "BFT", "BT", Roundel, 0x0a2e8a, 0xd62828, 0xf2c94c, [0.75, 0.77, 0.76, 0.76, 0.77]),
        ("Kolkata Baddies",    "KOB", "KB", Halves,  0x0b6b3a, 0x8a1c2b, 0xf28c28, [0.78, 0.76, 0.77, 0.77, 0.76]),
        ("Kerala Tuskers",     "KET", "KT", Chevron, 0xf6c300, 0x1546a0, 0x2e7d32, [0.75, 0.73, 0.78, 0.74, 0.73]),
        ("Bombay Town",        "BMT", "BT", Stripes, 0x5aa7de, 0x0b1f4b, 0xf2c94c, [0.77, 0.76, 0.77, 0.78, 0.76]),
        ("Goa Islanders",      "GOI", "GI", Sash,    0xf47b20, 0x0b2f6b, 0x2e7d32, [0.76, 0.72, 0.78, 0.77, 0.74]),
    ];
    raw.iter()
        .map(|&(name, short, initials, crest, shirt, trim, keeper, r)| TeamDef {
            name,
            short,
            initials,
            crest,
            shirt: hex(shirt),
            trim: hex(trim),
            keeper: hex(keeper),
            attack: r[0],
            defense: r[1],
            pace: r[2],
            passing: r[3],
            keeping: r[4],
        })
        .collect()
}

const FIRST: [&str; 30] = [
    "Arun", "Karthik", "Vijay", "Ravi", "Sanjay", "Mateo", "Lucas", "Noah", "Leo", "Omar",
    "Yusuf", "Kenji", "Diego", "Tomas", "Felix", "Ivan", "Hugo", "Rafael", "Ade", "Kwame",
    "Elias", "Nikhil", "Pranav", "Suresh", "Andre", "Marco", "Jonas", "Emil", "Kai", "Theo",
];

const LAST: [&str; 40] = [
    "Raman", "Iyer", "Pillai", "Menon", "Natarajan", "Okoro", "Mensah", "Albuquerque", "Moreau",
    "Novak", "Larsen", "Kovac", "Haddad", "Tanaka", "Azevedo", "Reyes", "Fischer", "Brandt",
    "Petrov", "Mbeki", "Duarte", "Lindqvist", "Ricci", "Varga", "Kaur", "Sethi", "Banerjee",
    "Rahman", "Osei", "Navarro", "Ferreira", "Holm", "Weber", "Castillo", "Aziz", "Krishnan",
    "Subramanian", "Gomez", "Nakamura", "Valdez",
];

/// Deterministic fictional squad names for a club, so players keep their names between matches.
pub fn squad_names(team_index: usize) -> Vec<String> {
    let mut seed = (team_index as u64 + 1).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    let mut next = move || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed
    };
    let mut used = Vec::new();
    (0..11)
        .map(|_| loop {
            let first = FIRST[(next() % FIRST.len() as u64) as usize];
            let last = LAST[(next() % LAST.len() as u64) as usize];
            if !used.contains(&last) {
                used.push(last);
                break format!("{} {}", &first[..1], last);
            }
        })
        .collect()
}
