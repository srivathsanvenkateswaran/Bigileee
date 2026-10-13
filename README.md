# KaalPanthu

An 11-a-side football game for macOS, written in Rust with
[macroquad](https://github.com/not-fl3/macroquad). Top-down camera, a
4-4-2 for both sides, and all the restarts: kick-offs, throw-ins, goal kicks,
corners, free kicks and penalties.

The clubs are parodies named after real cities (Manchester Town, Madrasin FC,
Kolkata Baddies and so on). Their crests are original geometric designs in the
club colours, and the players are generated fictional names. Nothing here is
affiliated with any real club, league or game publisher.

## Run

```sh
cargo run --release
```

Requires a Rust toolchain (`rustup`). The first build downloads and compiles
dependencies; after that it starts in a couple of seconds.

## Modes

- **1 Player vs CPU** with four CPU levels
- **2 Players** on one keyboard
- **Watch** CPU vs CPU

Half length is 2, 3, 5 or 8 real minutes; the match clock runs to 90.

## Controls

| Action | 1 player / P1 | P2 |
| --- | --- | --- |
| Move | WASD (or arrows in 1P) | Arrows |
| Sprint | Shift | Right Shift |
| Pass | J | , |
| Through ball | I | ; |
| Lob / cross | L | / |
| Shoot (hold for power, aim with direction) | K or Space | . |
| Tackle (defending) | K | . |
| Slide tackle (defending) | L | / |
| Switch player | Q, or J while defending | M, or , while defending |
| Pause | Esc | Esc |

Pressing pass or shoot just before the ball reaches you plays it first time.
Overcharging a shot past the red zone of the power bar sends it high.

## How it works

| File | What it does |
| --- | --- |
| `src/game.rs` | Match state, ball physics (rolling and air drag, bounces, spin), possession, tackles and fouls, goal-line and touchline rules, restarts |
| `src/ai.rs` | Formation shifting, pressing and marking, the ball carrier's shoot/pass/dribble choice, set-piece takers, goalkeeper positioning and dives |
| `src/render.rs` | Pitch, stands, players, ball with height, goal frames, scorebug, minimap, crests |
| `src/audio.rs` | Kick, whistle, crowd and post sounds, synthesised at startup (no audio files) |
| `src/teams.rs` | Clubs, ratings and squad name generation |
| `src/input.rs` | Keyboard bindings |

## Tests

```sh
cargo test --release -- --nocapture
```

Plays six full CPU-vs-CPU matches headlessly and checks they finish cleanly.

`KAALPANTHU_SNAPSHOT=/some/dir cargo run --release` saves PNGs of the menu and
a CPU match, then quits.
