//! In-match overlay: scorebug, banners, player tag with power bar, minimap, prompts and the
//! pause / half-time / full-time panels.

use super::camera::MainCam;
use super::players::HUMAN_COLOURS;
use super::stadium::sim_to_world;
use super::textures::{self, color};
use super::view::View;
use super::{AppState, Fonts, MatchRes};
use crate::sim::consts::*;
use crate::sim::game::{Phase, Restart, Role};
use bevy::prelude::*;

#[derive(Component)]
pub struct ScoreText;
#[derive(Component)]
pub struct ClockText;
#[derive(Component)]
pub struct HalfText;
#[derive(Component)]
pub struct BannerRoot;
#[derive(Component)]
pub struct BannerText;
#[derive(Component)]
pub struct Tag(pub usize);
#[derive(Component)]
pub struct TagName(pub usize);
#[derive(Component)]
pub struct StaminaBar(pub usize);
#[derive(Component)]
pub struct PowerBar(pub usize);
#[derive(Component)]
pub struct MiniDot(pub usize);
#[derive(Component)]
pub struct PromptText;
#[derive(Component)]
pub struct ReplayTag;
#[derive(Component)]
pub struct CommentaryText;
#[derive(Component)]
pub struct OverlayRoot;
#[derive(Component)]
pub struct OverlayTitle;
#[derive(Component)]
pub struct OverlayBody;

const MAP_W: f32 = 252.0;
const MAP_H: f32 = MAP_W * HALF_W / HALF_L;

pub fn text(fonts: &Fonts, s: impl Into<String>, size: f32, c: Color) -> (Text, TextFont, TextColor) {
    (Text::new(s), TextFont { font: fonts.bold.clone(), font_size: size, ..default() }, TextColor(c))
}

pub fn spawn_hud(commands: &mut Commands, fonts: &Fonts, images: &mut Assets<Image>, res: &MatchRes) {
    let m = &res.m;
    let scoped = StateScoped(AppState::Match);
    let dark = Color::srgba(0.03, 0.04, 0.08, 0.88);
    let crests: Vec<Handle<Image>> =
        (0..2).map(|t| images.add(textures::crest(m.teams[t].crest, m.teams[t].home, m.teams[t].trim, 64))).collect();

    // Scorebug.
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(24.0),
                top: Val::Px(20.0),
                height: Val::Px(40.0),
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(dark),
            scoped.clone(),
        ))
        .with_children(|bug| {
            for t in 0..2 {
                if t == 1 {
                    bug.spawn((
                        Node { height: Val::Percent(100.0), padding: UiRect::horizontal(Val::Px(14.0)), align_items: AlignItems::Center, ..default() },
                        BackgroundColor(Color::srgb(0.96, 0.96, 0.96)),
                    ))
                    .with_child((text(fonts, "0 - 0", 26.0, Color::BLACK), ScoreText));
                }
                bug.spawn((Node { width: Val::Px(6.0), height: Val::Percent(100.0), ..default() }, BackgroundColor(color(m.kit[t]))));
                bug.spawn((Node { width: Val::Px(30.0), height: Val::Px(30.0), margin: UiRect::horizontal(Val::Px(8.0)), ..default() }, ImageNode::new(crests[t].clone())));
                bug.spawn((Node { margin: UiRect::right(Val::Px(12.0)), ..default() }, text(fonts, m.teams[t].short.clone(), 24.0, Color::WHITE)));
            }
            bug.spawn((
                Node { height: Val::Percent(100.0), padding: UiRect::horizontal(Val::Px(12.0)), align_items: AlignItems::Center, ..default() },
                BackgroundColor(Color::srgb(0.85, 0.16, 0.22)),
            ))
            .with_child((text(fonts, "00:00", 22.0, Color::WHITE), ClockText));
        });
    commands.spawn((
        Node { position_type: PositionType::Absolute, left: Val::Px(26.0), top: Val::Px(64.0), ..default() },
        text(fonts, "1st half", 15.0, Color::srgba(1.0, 1.0, 1.0, 0.75)),
        HalfText,
        scoped.clone(),
    ));

    // Replay tag.
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            right: Val::Px(28.0),
            top: Val::Px(22.0),
            padding: UiRect::axes(Val::Px(14.0), Val::Px(6.0)),
            ..default()
        },
        BackgroundColor(Color::srgb(0.85, 0.16, 0.22)),
        Visibility::Hidden,
        ReplayTag,
        scoped.clone(),
    ))
    .with_child(text(fonts, "REPLAY", 22.0, Color::WHITE));

    // Banner.
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Percent(16.0),
                width: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                ..default()
            },
            Visibility::Hidden,
            BannerRoot,
            scoped.clone(),
        ))
        .with_children(|b| {
            b.spawn((Node { padding: UiRect::axes(Val::Px(36.0), Val::Px(10.0)), ..default() }, BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.55))))
                .with_child((text(fonts, "", 52.0, Color::WHITE), BannerText));
        });

    // Player tags (name, stamina, shot power) for each human.
    for h in 0..m.humans.len() {
        commands
            .spawn((
                Node {
                    position_type: PositionType::Absolute,
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    width: Val::Px(160.0),
                    ..default()
                },
                Visibility::Hidden,
                Tag(h),
                scoped.clone(),
            ))
            .with_children(|tag| {
                tag.spawn((text(fonts, "", 15.0, Color::WHITE), TagName(h), TextShadow::default()));
                tag.spawn((Node { width: Val::Px(60.0), height: Val::Px(4.0), margin: UiRect::top(Val::Px(2.0)), ..default() }, BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.5))))
                    .with_child((Node { width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() }, BackgroundColor(Color::srgb(0.3, 0.9, 0.45)), StaminaBar(h)));
                tag.spawn((Node { width: Val::Px(90.0), height: Val::Px(8.0), margin: UiRect::top(Val::Px(3.0)), ..default() }, BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6))))
                    .with_child((Node { width: Val::Percent(0.0), height: Val::Percent(100.0), ..default() }, BackgroundColor(Color::srgb(1.0, 0.85, 0.1)), PowerBar(h)));
            });
    }

    // Minimap.
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                bottom: Val::Px(26.0),
                left: Val::Percent(50.0),
                margin: UiRect::left(Val::Px(-MAP_W / 2.0)),
                width: Val::Px(MAP_W),
                height: Val::Px(MAP_H),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.1, 0.35, 0.15, 0.6)),
            BorderColor(Color::srgba(1.0, 1.0, 1.0, 0.5)),
            scoped.clone(),
        ))
        .with_children(|map| {
            map.spawn((
                Node { position_type: PositionType::Absolute, left: Val::Percent(50.0), width: Val::Px(1.0), height: Val::Percent(100.0), ..default() },
                BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.45)),
            ));
            for i in 0..=PLAYERS {
                let (c, size) = if i == PLAYERS {
                    (Color::WHITE, 6.0)
                } else {
                    let p = &m.players[i];
                    (color(if p.role == Role::Gk { m.teams[p.team].keeper } else { m.kit[p.team] }), 8.0)
                };
                map.spawn((
                    Node { position_type: PositionType::Absolute, width: Val::Px(size), height: Val::Px(size), border: UiRect::all(Val::Px(1.0)), ..default() },
                    BackgroundColor(c),
                    BorderColor(Color::srgba(0.0, 0.0, 0.0, 0.6)),
                    BorderRadius::MAX,
                    MiniDot(i),
                ));
            }
        });

    // Controls hint and restart prompt.
    let hint = if m.humans.len() == 2 {
        "P1: WASD  J pass  I through  L lob  K shoot  LShift sprint  Q switch  E jockey     P2: arrows  , ; / .  RShift  M  N     Esc pause"
    } else {
        "WASD move  Shift sprint  J pass  I through  L lob/cross  K shoot (hold)  |  defending: K tackle  L slide  E jockey  Q switch  |  C camera  Esc pause"
    };
    commands.spawn((
        Node { position_type: PositionType::Absolute, left: Val::Px(20.0), bottom: Val::Px(6.0), ..default() },
        text(fonts, hint, 13.0, Color::srgba(1.0, 1.0, 1.0, 0.55)),
        scoped.clone(),
    ));
    commands.spawn((
        Node { position_type: PositionType::Absolute, bottom: Val::Px(250.0), width: Val::Percent(100.0), justify_content: JustifyContent::Center, ..default() },
        scoped.clone(),
    ))
    .with_child((text(fonts, "", 22.0, HUMAN_COLOURS[0]), PromptText, TextShadow::default()));
    commands.spawn((
        Node { position_type: PositionType::Absolute, bottom: Val::Px(MAP_H + 40.0), width: Val::Percent(100.0), justify_content: JustifyContent::Center, ..default() },
        scoped.clone(),
    ))
    .with_child((
        Text::new(""),
        TextFont { font: fonts.regular.clone(), font_size: 21.0, ..default() },
        TextColor(Color::WHITE),
        TextShadow::default(),
        CommentaryText,
    ));

    // Pause and match-report panel.
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.55)),
            Visibility::Hidden,
            OverlayRoot,
            scoped,
        ))
        .with_children(|o| {
            o.spawn((
                Node {
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    padding: UiRect::all(Val::Px(32.0)),
                    row_gap: Val::Px(14.0),
                    min_width: Val::Px(560.0),
                    border: UiRect::top(Val::Px(4.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.03, 0.04, 0.09, 0.95)),
                BorderColor(Color::srgb(0.97, 0.78, 0.25)),
            ))
            .with_children(|panel| {
                panel.spawn((text(fonts, "", 40.0, Color::WHITE), OverlayTitle));
                panel.spawn((
                    Text::new(""),
                    TextFont { font: fonts.regular.clone(), font_size: 21.0, ..default() },
                    TextColor(Color::srgb(0.9, 0.9, 0.92)),
                    TextLayout::new_with_justify(JustifyText::Center),
                    OverlayBody,
                ));
            });
        });
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn update_hud(
    res: Res<MatchRes>,
    talk: Option<Res<super::commentary::Commentary>>,
    mut subtitle: Query<&mut Text, (With<CommentaryText>, Without<ScoreText>, Without<ClockText>, Without<HalfText>, Without<BannerText>, Without<TagName>, Without<PromptText>, Without<OverlayTitle>, Without<OverlayBody>)>,
    view: Res<View>,
    cam: Query<(&Camera, &GlobalTransform), With<MainCam>>,
    mut texts: ParamSet<(
        Query<&mut Text, With<ScoreText>>,
        Query<&mut Text, With<ClockText>>,
        Query<&mut Text, With<HalfText>>,
        Query<&mut Text, With<BannerText>>,
        Query<(&mut Text, &TagName)>,
        Query<(&mut Text, &mut TextColor), With<PromptText>>,
        Query<&mut Text, With<OverlayTitle>>,
        Query<&mut Text, With<OverlayBody>>,
    )>,
    mut vis: ParamSet<(
        Query<&mut Visibility, With<BannerRoot>>,
        Query<&mut Visibility, With<ReplayTag>>,
        Query<&mut Visibility, With<OverlayRoot>>,
    )>,
    mut tags: Query<(&Tag, &mut Node, &mut Visibility), (Without<BannerRoot>, Without<ReplayTag>, Without<OverlayRoot>)>,
    mut bars: ParamSet<(Query<(&StaminaBar, &mut Node), Without<Tag>>, Query<(&PowerBar, &mut Node, &mut BackgroundColor), Without<Tag>>)>,
    mut dots: Query<(&MiniDot, &mut Node), (Without<Tag>, Without<StaminaBar>, Without<PowerBar>)>,
) {
    let m = &res.m;
    let set = |t: &mut Text, s: String| {
        if t.0 != s {
            t.0 = s;
        }
    };
    if let Ok(mut t) = texts.p0().single_mut() {
        let pens = m.shootout.as_ref().map(|s| format!("  ({}-{} P)", s.goals(0), s.goals(1))).unwrap_or_default();
        set(&mut t, format!("{} - {}{}", m.score[0], m.score[1], pens));
    }
    let talk_on = talk.as_ref().is_some_and(|t| t.enabled);
    if let (Some(talk), Ok(mut t)) = (talk.as_ref(), subtitle.single_mut()) {
        let line = if res.paused || view.replay { String::new() } else { talk.hint.clone().unwrap_or_else(|| talk.line.clone()) };
        set(&mut t, line);
    }
    let (min, sec) = m.display_minute();
    if let Ok(mut t) = texts.p1().single_mut() {
        set(&mut t, format!("{:02}:{:02}", min, sec));
    }
    if let Ok(mut t) = texts.p2().single_mut() {
        set(&mut t, m.period_label().into());
    }

    // Banner.
    let banner = m.banner.as_ref().filter(|_| !view.replay && m.phase != Phase::FullTime && !res.paused);
    if let Ok(mut t) = texts.p3().single_mut() {
        set(&mut t, banner.map(|b| b.0.clone()).unwrap_or_default());
    }
    if let Ok(mut v) = vis.p0().single_mut() {
        *v = if banner.is_some() { Visibility::Inherited } else { Visibility::Hidden };
    }
    if let Ok(mut v) = vis.p1().single_mut() {
        *v = if view.replay { Visibility::Inherited } else { Visibility::Hidden };
    }

    // Player tags follow the controlled player on screen.
    let cam = cam.single().ok();
    for (tag, mut node, mut v) in &mut tags {
        let shown = view.controlled.get(tag.0).copied().flatten().zip(cam).and_then(|(i, (c, gt))| {
            let at = sim_to_world(view.players[i].pos, 2.75);
            c.world_to_viewport(gt, at).ok().map(|s| (i, s))
        });
        match shown {
            Some((_, s)) if !res.paused => {
                node.left = Val::Px(s.x - 80.0);
                node.top = Val::Px(s.y - 34.0);
                *v = Visibility::Inherited;
            }
            _ => *v = Visibility::Hidden,
        }
    }
    for (mut t, name) in &mut texts.p4() {
        if let Some(h) = m.humans.get(name.0) {
            let p = &m.players[h.player];
            set(&mut t, format!("{}  {}{}", p.num, p.name, if p.injured { "  [INJ]" } else { "" }));
        }
    }
    for (bar, mut node) in &mut bars.p0() {
        if let Some(h) = m.humans.get(bar.0) {
            node.width = Val::Percent(m.players[h.player].stamina * 100.0);
        }
    }
    for (bar, mut node, mut bg) in &mut bars.p1() {
        if let Some(h) = m.humans.get(bar.0) {
            node.width = Val::Percent(if h.charging { h.charge * 100.0 } else { 0.0 });
            bg.0 = if h.charge > 0.88 {
                Color::srgb(0.95, 0.15, 0.15)
            } else if h.charge > 0.6 {
                Color::srgb(1.0, 0.55, 0.1)
            } else {
                Color::srgb(1.0, 0.85, 0.1)
            };
        }
    }

    // Minimap.
    for (dot, mut node) in &mut dots {
        node.display = if dot.0 < PLAYERS && view.players[dot.0].sent_off { Display::None } else { Display::Flex };
        let (p, size) = if dot.0 == PLAYERS { (view.ball.pos, 6.0) } else { (view.players[dot.0].pos, 8.0) };
        node.left = Val::Px((p.x + HALF_L) / (2.0 * HALF_L) * MAP_W - size / 2.0);
        node.top = Val::Px((p.y + HALF_W) / (2.0 * HALF_W) * MAP_H - size / 2.0);
    }

    // Set-piece prompt for a human taker, otherwise a nudge to replace an injured player.
    if let Ok((mut t, mut c)) = texts.p5().single_mut() {
        let msg = m.lock.and_then(|l| m.humans.iter().position(|h| h.player == l.taker).map(|h| (l, h)));
        let injury = m.injury_alert.filter(|_| !view.replay && !res.paused).map(|(i, _)| {
            let p = &m.players[i];
            let h = m.human_of_team(p.team).unwrap_or(0);
            let line = if m.subs_left[p.team] > 0 {
                format!("{} is injured  ·  pause (Esc / Start) and pick Substitutions to replace him", p.name)
            } else {
                format!("{} is injured, but there are no subs left  ·  he'll have to limp on", p.name)
            };
            (line, h)
        });
        match msg {
            Some((l, h)) if !view.replay => {
                c.0 = HUMAN_COLOURS[h.min(1)];
                set(
                    &mut t,
                    match l.kind {
                        Restart::Penalty => "Aim with direction, hold SHOOT for power".into(),
                        Restart::Corner => "LOB to cross it into the box".into(),
                        Restart::FreeKick => "PASS, LOB, or hold SHOOT to go for goal".into(),
                        _ => "PASS / THROUGH / LOB to restart".into(),
                    },
                );
            }
            _ => match injury {
                Some((line, h)) => {
                    c.0 = HUMAN_COLOURS[h.min(1)];
                    set(&mut t, line);
                }
                None => set(&mut t, String::new()),
            },
        }
    }

    // Pause, half-time and full-time panel.
    let panel = if let (true, Some(ui)) = (res.paused, res.subs) {
        Some((format!("SUBSTITUTIONS  ·  {}", m.teams[ui.team].short), subs_body(&res, ui)))
    } else if let (true, Some(ui)) = (res.paused, res.tactics) {
        Some((format!("TACTICS  ·  {}", m.teams[ui.team].short), tactics_body(&res, ui)))
    } else if res.paused {
        let body = super::PAUSE_ITEMS
            .iter()
            .enumerate()
            .map(|(k, s)| {
                let s = if *s == "Commentary" { format!("Commentary: {}", if talk_on { "On" } else { "Off" }) } else { s.to_string() };
                if k == res.pause_sel { format!("> {s} <") } else { s }
            })
            .collect::<Vec<_>>()
            .join("\n");
        Some(("PAUSED".to_string(), body))
    } else {
        match m.phase {
            Phase::HalfTime { .. } => {
                let title = match m.half {
                    1 => "HALF TIME",
                    2 => "EXTRA TIME",
                    _ => "EXTRA TIME  ·  HALF TIME",
                };
                Some((title.into(), report(&res)))
            }
            Phase::FullTime => Some(("FULL TIME".into(), format!("{}\n\nPress Enter / A to continue", report(&res)))),
            _ => None,
        }
    };
    if let Ok(mut v) = vis.p2().single_mut() {
        *v = if panel.is_some() { Visibility::Inherited } else { Visibility::Hidden };
    }
    if let Some((title, body)) = panel {
        if let Ok(mut t) = texts.p6().single_mut() {
            set(&mut t, title);
        }
        if let Ok(mut t) = texts.p7().single_mut() {
            set(&mut t, body);
        }
    }
}

fn report(res: &MatchRes) -> String {
    let m = &res.m;
    let total = (m.possession[0] + m.possession[1]).max(0.01);
    let pos = (m.possession[0] / total * 100.0).round();
    let mut s = format!(
        "{}  {} - {}  {}\n\nPossession   {}%  -  {}%\nShots   {}  -  {}",
        m.teams[0].name,
        m.score[0],
        m.score[1],
        m.teams[1].name,
        pos,
        100.0 - pos,
        m.shots[0],
        m.shots[1]
    );
    if !m.scorers.is_empty() {
        s.push_str("\n\nGoals");
        for (team, name, minute) in &m.scorers {
            s.push_str(&format!("\n{}'  {}  ({})", minute, name, m.teams[*team].short));
        }
    }
    if !m.injuries.is_empty() {
        s.push_str("\n\nInjuries");
        for (team, name, minute) in &m.injuries {
            s.push_str(&format!("\n{}'  {}  ({})", minute, name, m.teams[*team].short));
        }
    }
    s
}

fn subs_body(res: &MatchRes, ui: super::SubsUi) -> String {
    let m = &res.m;
    let t = ui.team;
    let slots = m.teams[t].slots();
    let mut lines = vec![format!("Subs left: {}", m.subs_left[t]), String::new()];
    let mark = |col: usize, row: usize| if ui.col == col && ui.row == row { "> " } else { "   " };
    lines.push("ON THE PITCH".into());
    for k in 0..11 {
        let i = t * 11 + k;
        let p = &m.players[i];
        let status = if p.sent_off {
            "  sent off".to_string()
        } else {
            let card = if p.yellows > 0 { "  [Y]" } else { "" };
            let hurt = if p.injured { "  INJURED" } else { "" };
            format!("  {:>3}% fit{}{}", (p.fitness.min(p.stamina) * 100.0) as u32, card, hurt)
        };
        let chosen = if ui.out == Some(i) { "  <- OFF" } else { "" };
        lines.push(format!("{}{:<4} {:>2} {}  {}{}{}", mark(0, k), format!("{:?}", slots[k].pos), p.num, p.name, p.ovr, status, chosen));
    }
    lines.push(String::new());
    lines.push("BENCH".into());
    let bench = m.bench(t);
    if bench.is_empty() {
        lines.push("   (nobody left)".into());
    }
    for (r, &k) in bench.iter().enumerate() {
        let c = &m.teams[t].squad[k];
        lines.push(format!("{}{:<4} {:>2} {}  {}", mark(1, r), format!("{:?}", c.pos), c.num, c.name, c.ovr));
    }
    lines.push(String::new());
    let hint = if ui.out.is_some() { "Pick who comes on  ·  Esc cancel" } else { "Pick who comes off  ·  Esc back" };
    lines.push(if m.humans.len() == 1 { hint.to_string() } else { format!("{hint}  ·  Tab / RB other team") });
    lines.join("\n")
}

fn tactics_body(res: &MatchRes, ui: super::TacticsUi) -> String {
    use crate::sim::formation::{slots, NAMES};
    let m = &res.m;
    let t = ui.team;
    let name = NAMES[ui.formation];
    let pick = |row: usize, label: &str, value: &str| {
        if ui.row == row { format!("{label}    <  {value}  >") } else { format!("{label}    {value}") }
    };
    let mut lines = vec![
        format!("Now: {}  ·  {}", m.teams[t].formation, m.mentality[t].label()),
        String::new(),
        pick(0, "Formation", name),
        pick(1, "Mentality", ui.mentality.label()),
        if ui.row == 2 { "> Apply <".to_string() } else { "Apply".to_string() },
        String::new(),
        format!("NEW SHAPE  ·  {name}"),
    ];
    let fit = m.formation_fit(t, name);
    let new_slots = slots(name);
    for (k, &old) in fit.iter().enumerate() {
        let p = &m.players[t * 11 + old];
        let natural = m.teams[t].squad[p.card].pos;
        let note = if p.sent_off {
            "  sent off".to_string()
        } else if crate::sim::squad::fit_penalty(new_slots[k].pos, natural) > 2 {
            format!("  (plays {natural:?})")
        } else {
            String::new()
        };
        lines.push(format!("{:<4} {:>2} {}{}", format!("{:?}", new_slots[k].pos), p.num, p.name, note));
    }
    lines.push(String::new());
    let hint = "Up/Down choose  ·  Left/Right change  ·  Enter apply  ·  Esc back";
    lines.push(if m.humans.len() == 1 { hint.to_string() } else { format!("{hint}  ·  Tab / RB other team") });
    lines.join("\n")
}
