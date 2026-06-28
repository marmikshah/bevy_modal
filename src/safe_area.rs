//! Safe-area insets so overlays and toasts clear the notch / Dynamic Island /
//! home indicator.
//!
//! This is generic and platform-free: the consumer populates [`SafeAreaInsets`]
//! (on iOS, e.g. from a platform API that reads the window's safe-area), and
//! `bevy_modal` reads it every frame — so rotation and a late-arriving inset are
//! handled. Overlay roots get the insets as padding (centred panels and
//! full-bleed `.content()` both stay inside the safe area), and the toast column
//! offsets its anchored edge by the matching inset.

use bevy::prelude::*;

use crate::stack::Overlay;
use crate::theme::Theme;
use crate::toast::{TOAST_EDGE_MARGIN, ToastLayer, ToastPosition};

/// Device safe-area insets in logical pixels. Defaults to zero; set it from your
/// platform layer (it can change on rotation). All four edges are independent.
#[derive(Resource, Clone, Copy, Default, Debug, PartialEq)]
pub struct SafeAreaInsets {
    pub top: f32,
    pub bottom: f32,
    pub left: f32,
    pub right: f32,
}

/// Keep overlay roots padded and the toast column offset to match the current
/// [`SafeAreaInsets`]. Writes only on change, so a static inset costs nothing.
pub(crate) fn apply_safe_area(
    insets: Res<SafeAreaInsets>,
    theme: Res<Theme>,
    mut roots: Query<&mut Node, (With<Overlay>, Without<ToastLayer>)>,
    mut layers: Query<&mut Node, (With<ToastLayer>, Without<Overlay>)>,
) {
    let pad = UiRect {
        left: Val::Px(insets.left),
        right: Val::Px(insets.right),
        top: Val::Px(insets.top),
        bottom: Val::Px(insets.bottom),
    };
    for mut node in &mut roots {
        if node.padding != pad {
            node.padding = pad;
        }
    }

    for mut node in &mut layers {
        match theme.toast_position {
            ToastPosition::Top => {
                let top = Val::Px(TOAST_EDGE_MARGIN + insets.top);
                if node.top != top {
                    node.top = top;
                }
            }
            ToastPosition::Bottom => {
                let bottom = Val::Px(TOAST_EDGE_MARGIN + insets.bottom);
                if node.bottom != bottom {
                    node.bottom = bottom;
                }
            }
        }
    }
}
