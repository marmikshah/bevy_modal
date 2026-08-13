//! The injected look. The library bakes nothing game-specific: a consumer drops
//! their own [`Theme`] in (`app.insert_resource(Theme { .. })`) and every
//! overlay, scrim, toast and button picks it up. The [`Default`] is a plain
//! dark chrome so examples and tests render without any setup.

use bevy::prelude::*;
use bevy::text::{LetterSpacing, LineHeight};

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

/// Which font handle a semantic text role uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextFace {
    Display,
    Body,
}

/// The complete metrics for one semantic text role.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextMetrics {
    pub face: TextFace,
    pub font_size: f32,
    /// Line height as a multiple of the font size.
    pub line_height: f32,
    /// Additional letter spacing in logical pixels.
    pub letter_spacing: f32,
}

impl TextMetrics {
    pub const fn new(
        face: TextFace,
        font_size: f32,
        line_height: f32,
        letter_spacing: f32,
    ) -> Self {
        Self {
            face,
            font_size,
            line_height,
            letter_spacing,
        }
    }
}

/// Semantic metrics for every text surface emitted by the crate.
///
/// The defaults preserve the rendering from `bevy_modal` 0.3.3. [`ModalPlugin`](crate::ModalPlugin)
/// initializes this resource, and applications can replace it with one bounded
/// scale without changing [`Theme`] or widget method signatures.
#[derive(Resource, Debug, Clone, Copy, PartialEq)]
pub struct Typography {
    pub overlay_title: TextMetrics,
    pub overlay_body: TextMetrics,
    pub overlay_button: TextMetrics,
    pub toast: TextMetrics,
    pub toast_action: TextMetrics,
    pub widget_button: TextMetrics,
    pub widget_label: TextMetrics,
}

impl Default for Typography {
    fn default() -> Self {
        let body = |font_size| TextMetrics::new(TextFace::Body, font_size, 1.2, 0.0);
        Self {
            overlay_title: TextMetrics::new(TextFace::Display, 28.0, 1.2, 0.0),
            overlay_body: body(22.0),
            overlay_button: body(24.0),
            toast: body(20.0),
            toast_action: body(16.0),
            widget_button: body(22.0),
            widget_label: body(20.0),
        }
    }
}

/// Identifies which [`Typography`] entry drives a text entity.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextRole {
    OverlayTitle,
    OverlayBody,
    OverlayButton,
    Toast,
    ToastAction,
    WidgetButton,
    WidgetLabel,
}

impl Typography {
    pub const fn metrics(&self, role: TextRole) -> TextMetrics {
        match role {
            TextRole::OverlayTitle => self.overlay_title,
            TextRole::OverlayBody => self.overlay_body,
            TextRole::OverlayButton => self.overlay_button,
            TextRole::Toast => self.toast,
            TextRole::ToastAction => self.toast_action,
            TextRole::WidgetButton => self.widget_button,
            TextRole::WidgetLabel => self.widget_label,
        }
    }

    /// Resolve a role against the two font handles owned by [`Theme`].
    pub fn text_style(&self, theme: &Theme, role: TextRole) -> ThemedTextStyle {
        let metrics = self.metrics(role);
        let font = match metrics.face {
            TextFace::Display => theme.display.clone(),
            TextFace::Body => theme.body.clone(),
        };
        ThemedTextStyle {
            font: TextFont {
                font: font.into(),
                font_size: FontSize::Px(metrics.font_size),
                ..default()
            },
            line_height: LineHeight::RelativeToFont(metrics.line_height),
            letter_spacing: LetterSpacing::Px(metrics.letter_spacing),
            role,
        }
    }
}

/// Font and metrics for one [`TextRole`]. Use [`Typography::text_style`] when
/// bespoke content should follow the same typography as built-in surfaces.
#[derive(Bundle)]
pub struct ThemedTextStyle {
    font: TextFont,
    line_height: LineHeight,
    letter_spacing: LetterSpacing,
    role: TextRole,
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

type ThemedTextQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static ThemedText,
        Option<&'static TextRole>,
        &'static mut TextColor,
        &'static mut TextFont,
    ),
>;

type TypographyQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static TextRole,
        &'static mut TextFont,
        &'static mut LineHeight,
        &'static mut LetterSpacing,
    ),
>;

pub(crate) fn apply_theme(
    theme: Res<Theme>,
    mut panels: ThemedPanelQuery,
    mut scrims: Query<&mut BackgroundColor, (With<ThemedScrim>, Without<ThemedPanel>)>,
    mut text: ThemedTextQuery,
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
    for (tone, role, mut color, mut font) in &mut text {
        let next_color = match tone {
            ThemedText::Primary => theme.text,
            ThemedText::Secondary => theme.text_dim,
            ThemedText::Accent(accent) => accent.unwrap_or(theme.accent),
        };
        if color.0 != next_color {
            color.0 = next_color;
        }

        if role.is_some() {
            // Role-owned text is resolved once in PostUpdate. Skipping its font
            // here avoids two systems alternately projecting different faces.
            continue;
        }

        // Compatibility for caller-owned text that adopted ThemedText before
        // semantic metrics existed: retain its size and project the same face
        // selection as before.
        let next_font: FontSource = match tone {
            ThemedText::Accent(_) => theme.display.clone(),
            ThemedText::Primary | ThemedText::Secondary => theme.body.clone(),
        }
        .into();
        if font.font != next_font {
            font.font = next_font;
        }
    }
    for mut transition in &mut transitions {
        transition.apply_theme(&theme);
    }
}

/// Resolve semantic metrics after Update's deferred UI spawns and before Bevy
/// measures text for layout. This keeps existing builder signatures additive
/// and prevents a newly spawned widget from presenting the default font for one
/// frame.
pub(crate) fn apply_typography(
    theme: Res<Theme>,
    typography: Res<Typography>,
    mut text: TypographyQuery,
) {
    for (role, mut font, mut line_height, mut letter_spacing) in &mut text {
        let next = typography.text_style(&theme, *role);
        if font.font != next.font.font {
            font.font = next.font.font;
        }
        if font.font_size != next.font.font_size {
            font.font_size = next.font.font_size;
        }
        if *line_height != next.line_height {
            *line_height = next.line_height;
        }
        if *letter_spacing != next.letter_spacing {
            *letter_spacing = next.letter_spacing;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_typography_preserves_the_0_3_3_scale() {
        let typography = Typography::default();
        assert_eq!(typography.overlay_title.font_size, 28.0);
        assert_eq!(typography.overlay_body.font_size, 22.0);
        assert_eq!(typography.overlay_button.font_size, 24.0);
        assert_eq!(typography.toast.font_size, 20.0);
        assert_eq!(typography.toast_action.font_size, 16.0);
        assert_eq!(typography.widget_button.font_size, 22.0);
        assert_eq!(typography.widget_label.font_size, 20.0);
        assert_eq!(typography.overlay_title.face, TextFace::Display);
        assert!(
            [
                typography.overlay_body,
                typography.overlay_button,
                typography.toast,
                typography.toast_action,
                typography.widget_button,
                typography.widget_label,
            ]
            .into_iter()
            .all(|metrics| metrics.face == TextFace::Body)
        );
    }
}
