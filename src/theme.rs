//! The injected look. The library bakes nothing game-specific: a consumer drops
//! their own [`Theme`] in (`app.insert_resource(Theme { .. })`) and every
//! overlay, scrim, toast and button picks it up. The [`Default`] is a plain
//! dark chrome so examples and tests render without any setup.

use bevy::prelude::*;

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
        }
    }
}
