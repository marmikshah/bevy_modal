//! A confirm dialog — a thin convenience over the [`overlay`] builder. It's a
//! titled modal with an optional body and exactly two buttons (confirm /
//! cancel), each running a caller callback and then dismissing the dialog. It
//! reuses the full overlay/scrim/stack/gate machinery, so it blocks input like
//! any other overlay.
//!
//! ```no_run
//! use bevy::prelude::*;
//! use bevy_modal::prelude::*;
//!
//! fn ask(mut commands: Commands) {
//!     confirm(&mut commands, "delete-save", "Delete save?")
//!         .message("This can't be undone.")
//!         .confirm_label("Delete")
//!         .cancel_label("Keep")
//!         .on_confirm(|c| { c.queue(|_w: &mut World| { /* delete */ }); })
//!         .push();
//! }
//! ```

use bevy::prelude::*;

use crate::build::overlay;
use crate::stack::OverlayCommandsExt;

type ConfirmCb = Box<dyn FnMut(&mut Commands) + Send + Sync>;

/// Start a confirm dialog with the given id and title. `.push()` queues it.
pub fn confirm<'a, 'w, 's>(
    commands: &'a mut Commands<'w, 's>,
    id: impl Into<String>,
    title: impl Into<String>,
) -> ConfirmBuilder<'a, 'w, 's> {
    ConfirmBuilder {
        commands,
        id: id.into(),
        title: title.into(),
        message: Vec::new(),
        confirm_label: "Confirm".into(),
        cancel_label: "Cancel".into(),
        accent: None,
        on_confirm: None,
        on_cancel: None,
    }
}

/// Chained confirm builder. Borrows `Commands` for one statement; `.push()`
/// enqueues the overlay spawn.
pub struct ConfirmBuilder<'a, 'w, 's> {
    commands: &'a mut Commands<'w, 's>,
    id: String,
    title: String,
    message: Vec<String>,
    confirm_label: String,
    cancel_label: String,
    accent: Option<Color>,
    on_confirm: Option<ConfirmCb>,
    on_cancel: Option<ConfirmCb>,
}

impl ConfirmBuilder<'_, '_, '_> {
    /// A line of body text. Call repeatedly for multiple lines.
    pub fn message(mut self, text: impl Into<String>) -> Self {
        self.message.push(text.into());
        self
    }

    /// Label for the confirm button. Defaults to `"Confirm"`.
    pub fn confirm_label(mut self, text: impl Into<String>) -> Self {
        self.confirm_label = text.into();
        self
    }

    /// Label for the cancel button. Defaults to `"Cancel"`.
    pub fn cancel_label(mut self, text: impl Into<String>) -> Self {
        self.cancel_label = text.into();
        self
    }

    /// Override the accent (title + button tint). Defaults to `Theme::accent`.
    pub fn accent(mut self, color: Color) -> Self {
        self.accent = Some(color);
        self
    }

    /// Runs on confirm with `&mut Commands`; the dialog dismisses itself after.
    pub fn on_confirm(mut self, cb: impl FnMut(&mut Commands) + Send + Sync + 'static) -> Self {
        self.on_confirm = Some(Box::new(cb));
        self
    }

    /// Runs on cancel with `&mut Commands`; the dialog dismisses itself after.
    pub fn on_cancel(mut self, cb: impl FnMut(&mut Commands) + Send + Sync + 'static) -> Self {
        self.on_cancel = Some(Box::new(cb));
        self
    }

    /// Queue the dialog. Builds an `overlay()` with the two wired buttons.
    pub fn push(self) {
        let id_confirm = self.id.clone();
        let id_cancel = self.id.clone();
        let mut on_confirm = self.on_confirm;
        let mut on_cancel = self.on_cancel;

        let mut builder = overlay(self.commands, self.id).title(self.title);
        if let Some(accent) = self.accent {
            builder = builder.accent(accent);
        }
        for line in self.message {
            builder = builder.body(line);
        }
        builder
            .button(self.confirm_label, move |commands| {
                if let Some(cb) = on_confirm.as_mut() {
                    cb(commands);
                }
                commands.dismiss_overlay(id_confirm.clone());
            })
            .button(self.cancel_label, move |commands| {
                if let Some(cb) = on_cancel.as_mut() {
                    cb(commands);
                }
                commands.dismiss_overlay(id_cancel.clone());
            })
            .dismissable(true)
            .escape(true)
            .push();
    }
}
