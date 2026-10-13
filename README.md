# Bigileee 22

A parody football game for macOS in the spirit of FIFA 22, written in Rust on
[Bevy](https://bevyengine.org). 3D stadium, broadcast camera, 11 leagues and
209 clubs with pun-named players. It's a meme game and isn't affiliated with
EA, FIFA or any club or league.

## Run

```sh
cargo run --release
```

Requires a Rust toolchain (`rustup`). The first build compiles Bevy and takes
a few minutes.

## Leagues

| Real league | In game |
| --- | --- |
| Premier League | Spud League |
| La Liga | Paella League |
| Serie A | Pizza League |
| Bundesliga | Burger League |
| Ligue 1 | Croissant League |
| Indian Super League | Dosa League |
| Saudi Pro League | Camel League |
| MLS | Obesity League |
| Brasileirao | Samba League |
| Eredivisie | Cheese League |
| Primeira Liga | Pastel de Nata League |

Squads are 2021-22 (2022 for MLS and Brazil). Ratings are hand estimates.
Where a club's real squad wasn't known well enough, the game fills the gaps
with generated players, marked `*` on the team sheet. Crests are original
geometric designs in club colours.

## Modes

Kick Off (vs CPU, five difficulty levels), 2 Players on one Mac, and CPU vs
CPU. Half length from 2 to 10 minutes.

## Controls

Xbox and PlayStation controllers work, with a FIFA-style layout:

| Action | Pad | Keyboard (1P) | Keyboard P2 |
| --- | --- | --- | --- |
| Move | Left stick / D-pad | WASD or arrows | Arrows |
| Sprint | RT / R2 | Shift | Right Shift |
| Pass | A / Cross | J | , |
| Through ball | Y / Triangle | I | ; |
| Lob / cross | X / Square | L | / |
| Shoot (hold for power) | B / Circle | K or Space | . |
| Tackle (defending) | B / Circle | K | . |
| Slide (defending) | X / Square | L | / |
| Jockey (hold) | LT / L2 | E | N |
| Switch player | LB / L1 | Q | M |
| Camera | | C | |
| Pause | Start | Esc | |

Pressing pass or shoot just before the ball arrives plays it first time.
Overcharging a shot sends it over the bar. Goals get a replay; skip it with
A or Enter.

## Code

| Path | What it does |
| --- | --- |
| `src/sim/` | The match itself, independent of rendering: rules (offside, fouls, penalties, every restart), ball physics, AI, formations and squad selection |
| `src/front/` | Bevy front end: menus, stadium, player rigs and animation, cameras, replays, HUD, controls, synthesised audio |
| `src/data/` | Leagues, clubs and player cards |

## Tests

```sh
cargo test --release -- --nocapture
```

Plays six CPU-vs-CPU matches headlessly across all formations, and checks
every club builds a valid squad.

`BIGILEEE_SNAPSHOT=/some/dir cargo run --release` walks through the menus,
plays a CPU match and saves screenshots, then quits.
