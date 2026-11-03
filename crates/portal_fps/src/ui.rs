//! Simple HUD: crosshair, controls hint, portal counter and a frame-time graph.
//!
//! The default Bevy font has no Cyrillic glyphs, so the HUD is in English.

use bevy::diagnostic::{Diagnostic, DiagnosticsStore, FrameTimeDiagnosticsPlugin};
use bevy::prelude::*;
use bevy::text::FontSize;
use bevy::ui::{AlignItems, FlexDirection, Overflow, percent, px};

use crate::portal::TeleportCount;

/// Number of frame-time samples kept in the diagnostics history and drawn in
/// the graph. One bar is drawn per sample, so at 60 FPS the graph shows two
/// seconds of history.
pub const GRAPH_SAMPLES: usize = 120;

/// Height of the graph widget, in logical pixels.
const GRAPH_HEIGHT: f32 = 64.0;
/// Width of one bar, in logical pixels. Bars touch, so the graph reads as a
/// filled area chart.
const BAR_WIDTH: f32 = 2.0;
/// Total width of the graph widget, in logical pixels.
#[allow(clippy::cast_precision_loss)]
const GRAPH_WIDTH: f32 = BAR_WIDTH * GRAPH_SAMPLES as f32;

/// Frame budget for 60 FPS, in milliseconds. Drawn as a reference line.
const BUDGET_MS: f32 = 1000.0 / 60.0;

/// Bar colors: within budget, up to twice the budget, and worse than that.
const GOOD: Color = Color::srgba(0.55, 1.0, 0.55, 0.9);
const WARN: Color = Color::srgba(1.0, 0.85, 0.35, 0.9);
const BAD: Color = Color::srgba(1.0, 0.4, 0.35, 0.9);
/// Used for the empty left part of the widget while the history is short.
const EMPTY: Color = Color::srgba(0.0, 0.0, 0.0, 0.0);

#[derive(Component)]
struct TeleportText;

/// The FPS / frame-time readout in the top right corner.
#[derive(Component)]
struct FpsText;

/// One bar of the frame-time graph. The index is the position inside the
/// widget, left to right.
#[derive(Component)]
struct GraphBar(usize);

/// The horizontal `60 FPS` reference line on the graph.
#[derive(Component)]
struct FrameBudgetLine;

/// Vertical scale of the graph, in milliseconds. Eased towards a high
/// percentile of the recent samples so that a fast game does not render a flat
/// line and a slow one does not clip every bar.
#[derive(Resource)]
struct GraphScale {
    current: f32,
}

impl Default for GraphScale {
    fn default() -> Self {
        Self {
            current: BUDGET_MS * 2.0,
        }
    }
}

pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GraphScale>()
            .add_systems(Startup, setup_ui)
            .add_systems(
                Update,
                (
                    update_teleport_text,
                    update_fps_text,
                    update_frame_graph.after(FrameTimeDiagnosticsPlugin::diagnostic_system),
                ),
            );
    }
}

fn setup_ui(mut commands: Commands) {
    spawn_crosshair(&mut commands);
    spawn_hint(&mut commands);
    spawn_teleport_counter(&mut commands);
    spawn_frame_panel(&mut commands);
}

/// A zero-sized anchor at the screen center with four bars.
fn spawn_crosshair(commands: &mut Commands) {
    let dot = BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.9));
    let bar = |left: f32, top: f32, width: f32, height: f32| Node {
        position_type: PositionType::Absolute,
        left: px(left),
        top: px(top),
        width: px(width),
        height: px(height),
        ..default()
    };

    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: percent(50.0),
                top: percent(50.0),
                ..default()
            },
            Name::new("Crosshair"),
        ))
        .with_children(|parent| {
            parent.spawn((bar(-1.0, -16.0, 2.0, 8.0), dot));
            parent.spawn((bar(-1.0, 8.0, 2.0, 8.0), dot));
            parent.spawn((bar(-16.0, -1.0, 8.0, 2.0), dot));
            parent.spawn((bar(8.0, -1.0, 8.0, 2.0), dot));
        });
}

/// Controls hint, bottom left.
fn spawn_hint(commands: &mut Commands) {
    commands.spawn((
        Text::new(
            "WASD - move   |   Mouse - look   |   Space - jump   |   Shift - sprint\n\
             Walk into a portal to cross to the other room   |   R - respawn   |   Esc - cursor",
        ),
        TextFont {
            font_size: FontSize::Px(16.0),
            ..default()
        },
        TextColor(Color::srgba(1.0, 1.0, 1.0, 0.9)),
        Node {
            position_type: PositionType::Absolute,
            left: px(16.0),
            bottom: px(14.0),
            ..default()
        },
        Name::new("Hint"),
    ));
}

/// Portal counter, top left.
fn spawn_teleport_counter(commands: &mut Commands) {
    commands.spawn((
        Text::new("Portal crossings: 0"),
        TextFont {
            font_size: FontSize::Px(22.0),
            ..default()
        },
        TextColor(Color::srgba(1.0, 0.85, 0.4, 1.0)),
        Node {
            position_type: PositionType::Absolute,
            left: px(16.0),
            top: px(14.0),
            ..default()
        },
        TeleportText,
        Name::new("TeleportCount"),
    ));
}

/// Frame-time readout and graph, top right.
fn spawn_frame_panel(commands: &mut Commands) {
    // Readout.
    commands.spawn((
        Text::new("-- FPS   --.- ms"),
        TextFont {
            font_size: FontSize::Px(20.0),
            ..default()
        },
        TextColor(Color::srgba(0.7, 1.0, 0.7, 1.0)),
        Node {
            position_type: PositionType::Absolute,
            right: px(16.0),
            top: px(14.0),
            ..default()
        },
        FpsText,
        Name::new("FrameTimeText"),
    ));

    // Frame-time graph, right under the readout. Bars grow from the bottom and
    // scroll right to left, newest on the right.
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                right: px(16.0),
                top: px(46.0),
                width: px(GRAPH_WIDTH),
                height: px(GRAPH_HEIGHT),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::FlexEnd,
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.45)),
            Name::new("FrameTimeGraph"),
        ))
        .with_children(|parent| {
            // Reference line for the 60 FPS budget. It is absolutely
            // positioned inside the widget and follows the adaptive scale.
            parent.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: px(0.0),
                    bottom: percent(50.0),
                    width: percent(100.0),
                    height: px(1.0),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.7, 1.0, 0.7, 0.35)),
                FrameBudgetLine,
            ));

            for index in 0..GRAPH_SAMPLES {
                parent.spawn((
                    Node {
                        width: px(BAR_WIDTH),
                        height: px(0.0),
                        ..default()
                    },
                    BackgroundColor(EMPTY),
                    GraphBar(index),
                ));
            }
        });
}

fn update_teleport_text(count: Res<TeleportCount>, mut text: Query<&mut Text, With<TeleportText>>) {
    if !count.is_changed() {
        return;
    }
    for mut text in &mut text {
        **text = format!("Portal crossings: {}", count.0);
    }
}

fn frame_time_ms(diagnostics: &DiagnosticsStore) -> Option<f64> {
    diagnostics
        .get(&FrameTimeDiagnosticsPlugin::FRAME_TIME)
        .and_then(Diagnostic::smoothed)
}

fn frame_time_fps(diagnostics: &DiagnosticsStore) -> Option<f64> {
    diagnostics
        .get(&FrameTimeDiagnosticsPlugin::FPS)
        .and_then(Diagnostic::smoothed)
}

/// Shows the smoothed FPS and frame time from Bevy's frame-time diagnostics.
fn update_fps_text(diagnostics: Res<DiagnosticsStore>, mut text: Query<&mut Text, With<FpsText>>) {
    let (Some(fps), Some(ms)) = (frame_time_fps(&diagnostics), frame_time_ms(&diagnostics)) else {
        return;
    };
    for mut text in &mut text {
        **text = format!("{fps:.0} FPS   {ms:.1} ms");
    }
}

/// Draws the last `GRAPH_SAMPLES` frame times as bars.
///
/// The vertical scale is adaptive: it follows a high percentile of the recent
/// samples instead of the absolute maximum, so a single hitch does not squash
/// the whole graph flat.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn update_frame_graph(
    diagnostics: Res<DiagnosticsStore>,
    mut scale: ResMut<GraphScale>,
    mut bars: Query<(&GraphBar, &mut Node, &mut BackgroundColor)>,
    mut budget_line: Query<&mut Node, (With<FrameBudgetLine>, Without<GraphBar>)>,
) {
    let Some(diagnostic) = diagnostics.get(&FrameTimeDiagnosticsPlugin::FRAME_TIME) else {
        return;
    };

    // Oldest first, so the newest sample is drawn on the right.
    let samples: Vec<f32> = diagnostic.values().map(|value| *value as f32).collect();
    if samples.is_empty() {
        return;
    }

    // Use the 95th percentile as the target scale: spikes stay visible as tall
    // bars but do not redefine the whole vertical range.
    let mut sorted = samples.clone();
    sorted.sort_unstable_by(f32::total_cmp);
    let percentile = sorted[sorted.len() * 95 / 100].max(BUDGET_MS);
    let target = (percentile * 1.2).clamp(BUDGET_MS * 1.5, 250.0);
    scale.current += (target - scale.current) * 0.08;

    let scale_ms = scale.current.max(1.0);

    // Right-align short histories: the newest sample always sits at the right
    // edge of the widget and the left part stays empty.
    let offset = GRAPH_SAMPLES.saturating_sub(samples.len());

    for (bar, mut node, mut color) in &mut bars {
        let value = bar
            .0
            .checked_sub(offset)
            .and_then(|index| samples.get(index))
            .copied();

        let (height, tint) = match value {
            None => (0.0, EMPTY),
            Some(ms) => {
                let fraction = (ms / scale_ms).clamp(0.0, 1.0);
                let tint = if ms <= BUDGET_MS * 1.05 {
                    GOOD
                } else if ms <= BUDGET_MS * 2.0 {
                    WARN
                } else {
                    BAD
                };
                (fraction * GRAPH_HEIGHT, tint)
            }
        };

        node.height = px(height);
        *color = BackgroundColor(tint);
    }

    // Keep the 60 FPS marker glued to its real position on the current scale.
    let budget_fraction = (BUDGET_MS / scale_ms).clamp(0.0, 1.0);
    for mut node in &mut budget_line {
        node.bottom = percent(budget_fraction * 100.0);
    }
}
