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

- **Kick Off**: one match against the CPU, five difficulty levels
- **Season**: pick a club and play its league's full fixture list; other
  results are simulated and the table updates as you go
- **Cup**: the Bigileee Champions Cup, a 16-club knockout drawn from the
  strongest clubs in every league plus yours. Ties level after 90 minutes go
  to extra time, then penalties
- **2 Players** on one Mac, and **CPU vs CPU**

Season and Cup progress saves to `~/Library/Application Support/Bigileee22`
and resumes from **Continue Career** on the title screen. Between matches the
career hub has:

- **Squad & formation**: pick from the six formations and swap players
  between the starting XI and the bench
- **Transfers**: a budget set by your club's rating, and every player in
  every league on the market, priced by overall rating and position. Filter
  by league, club and position, sort by overall or value, buy what you can
  afford and sell your own players. Clubs you buy from lose the player for
  the rest of that career

Matches have offside, fouls, yellow and red cards, penalties, injuries, five
substitutions (plus one in extra time), fitness that drops over 90 minutes,
goal replays, and spoken commentary using the Mac's built-in voices (toggle in
setup or the pause menu).

The pause menu has **Substitutions** and **Tactics**. Tactics changes
formation (the same eleven are re-seated by position) and mentality, from
Ultra Defensive to Ultra Attacking. Mentality moves the defensive line and
the team's shape up or down the pitch, sets how hard it presses, and how
readily it shoots and plays risky forward passes; your CPU teammates follow
it too. CPU managers go for it when behind after 70 minutes and sit deeper
to protect a lead after 80.

Injured players are slower. The CPU replaces them at the next stoppage; on
your side a prompt suggests a substitution, and with no subs left he plays on.

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

Plays six CPU-vs-CPU knockout matches headlessly across all formations,
checks a level knockout tie goes through extra time to penalties, that
mid-match formation changes keep eleven players with one keeper, that the CPU
replaces injured players, that every club builds a valid squad, that transfers
move players and money and survive a save/load (old saves included), and runs
full seasons and a cup to a champion.

`BIGILEEE_SNAPSHOT=/some/dir cargo run --release` walks through the menus,
plays a CPU match (opening the tactics screen part way through), visits the
career hub, squad and transfer screens, saves screenshots, then quits.
