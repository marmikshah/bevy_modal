//! The injected look. The library bakes nothing game-specific: a consumer drops
//! their own [`Theme`] in (`app.insert_resource(Theme { .. })`) and every
//! overlay, scrim, toast and button picks it up. The [`Default`] is a plain
//! dark chrome so examples and tests render without any setup.

use bevy::prelude::*;

use crate::toast::ToastPosition;

/// Semantic styling for text emitted by an overlay.
///
/// The component is public so bespoke overlay content can opt into the same
/// live [`Theme`] projection as the built-in title, body, and button labels.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub enum ThemedText {
    Primary,
    Secondary,
    Accent(Option<Color>),
}

/// Marks a panel whose fill, border, and width follow the live [`Theme`].
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThemedPanel;

/// Marks a scrim whose RGB follows the live [`Theme`] while retaining its
/// transition-controlled alpha.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThemedScrim;

#[derive(Resource, Clone)]
pub struct Theme {
    /// Panel fill (and toast fill).
    pub ink: Color,
    /// Hairline borders / dividers.
    pub line: Color,
    /// Primary text.
    pub text: Color,
    /// Secondary text.
    pub text_dim: Color,
    /// Full-screen dim behind an overlay (the visible part of the scrim).
    pub scrim: Color,
    /// Default accent when a builder call doesn't specify one.
    pub accent: Color,
    /// Semantic accents, selected by [`ToastLevel`](crate::ToastLevel) (and free
    /// for your own UI). `Info` uses [`accent`](Self::accent).
    pub success: Color,
    pub warning: Color,
    pub danger: Color,
    /// Display face (titles).
    pub display: Handle<Font>,
    /// Body face (labels, buttons, toasts).
    pub body: Handle<Font>,
    pub panel_border: f32,
    pub button_border: f32,
    pub btn_fill_rest: f32,
    pub btn_fill_hover: f32,
    pub btn_fill_press: f32,
    pub btn_border_rest: f32,
    pub btn_border_hover: f32,
    /// Seconds to ease an overlay in (scrim fade + panel scale).
    pub open_secs: f32,
    /// Seconds to ease an overlay out before it despawns.
    pub close_secs: f32,
    /// Panel/content scale at the start of the open (and end of the close); eases
    /// to 1.0. Slightly under 1 gives a subtle "pop". Set to 1.0 for fade-only.
    pub panel_scale_from: f32,
    /// Which edge toasts stack against.
    pub toast_position: ToastPosition,
    /// Cap on the built-in panel's width, so it doesn't stretch absurdly wide on
    /// tablets/desktop. The panel is `82%` of the screen, clamped to this.
    pub panel_max_width: f32,
    /// Most toasts shown at once; the oldest are dismissed past this. `0` = no cap.
    pub max_toasts: usize,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            ink: Color::srgba(0.03, 0.04, 0.09, 0.94),
            line: Color::srgba(0.45, 0.55, 0.95, 0.45),
            text: Color::srgb(0.92, 0.94, 1.0),
            text_dim: Color::srgb(0.55, 0.60, 0.78),
            scrim: Color::srgba(0.0, 0.0, 0.0, 0.6),
            accent: Color::srgb(0.45, 0.70, 1.0),
            success: Color::srgb(0.40, 0.80, 0.55),
            warning: Color::srgb(0.95, 0.75, 0.35),
            danger: Color::srgb(0.95, 0.45, 0.45),
            display: Handle::default(),
            body: Handle::default(),
            panel_border: 3.0,
            button_border: 3.0,
            btn_fill_rest: 0.10,
            btn_fill_hover: 0.22,
            btn_fill_press: 0.34,
            btn_border_rest: 0.65,
            btn_border_hover: 1.0,
            open_secs: 0.18,
            close_secs: 0.12,
            panel_scale_from: 0.92,
            toast_position: ToastPosition::Top,
            panel_max_width: 420.0,
            max_toasts: 4,
        }
    }
}

type ThemedPanelQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static mut Node,
        &'static mut BackgroundColor,
        &'static mut BorderColor,
    ),
    (With<ThemedPanel>, Without<ThemedScrim>),
>;

pub(crate) fn apply_theme(
    theme: Res<Theme>,
    mut panels: ThemedPanelQuery,
    mut scrims: Query<&mut BackgroundColor, (With<ThemedScrim>, Without<ThemedPanel>)>,
    mut text: Query<(&ThemedText, &mut TextColor, &mut TextFont)>,
    mut transitions: Query<&mut crate::transition::Transition>,
) {
    for (mut node, mut background, mut border) in &mut panels {
        let max_width = Val::Px(theme.panel_max_width);
        let panel_border = UiRect::all(Val::Px(theme.panel_border));
        let line = BorderColor::all(theme.line);
        if node.max_width != max_width {
            node.max_width = max_width;
        }
        if node.border != panel_border {
            node.border = panel_border;
        }
        if background.0 != theme.ink {
            background.0 = theme.ink;
        }
        if *border != line {
            *border = line;
        }
    }
    for mut background in &mut scrims {
        let next = theme.scrim.with_alpha(background.0.alpha());
        if background.0 != next {
            background.0 = next;
        }
    }
    for (style, mut color, mut font) in &mut text {
        let (next_color, next_font) = match style {
            ThemedText::Primary => (theme.text, theme.body.clone()),
            ThemedText::Secondary => (theme.text_dim, theme.body.clone()),
            ThemedText::Accent(accent) => (accent.unwrap_or(theme.accent), theme.display.clone()),
        };
        if color.0 != next_color {
            color.0 = next_color;
        }
        let next_font: FontSource = next_font.into();
        if font.font != next_font {
            font.font = next_font;
        }
    }
    for mut transition in &mut transitions {
        transition.apply_theme(&theme);
    }
}
