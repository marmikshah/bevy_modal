# Changelog

Notable changes to `bevy_modal`, newest first. Follows [SemVer](https://semver.org);
format loosely [Keep a Changelog](https://keepachangelog.com). Entries begin
from the point this file was added — earlier releases live in the crates.io
version history and the git log.

## Unreleased

### Added

- `Typography`, `TextMetrics`, `TextFace`, and `TextRole` make the font face,
  size, line height, and tracking of every library-emitted text surface
  configurable without changing existing `Theme` or widget method signatures.
- Built-in overlays, toasts, and standalone widget labels follow typography and
  font-handle changes in place; the default resource preserves the 0.3.3 scale.

## 0.3.3 — 2026-08-06

### Added

- Existing overlays now follow runtime `Theme` changes in place through public
  `ThemedPanel`, `ThemedScrim`, `ThemedText`, and `ThemedButton` semantics.
- `OverlayBuilder::body_tagged` attaches caller-owned field identity when a
  built-in body row spawns, without hierarchy or rendered-copy inspection.

### Fixed

- Default overlay and standalone-button accents now follow the active theme;
  explicit accent overrides remain fixed.
- Live scrim recoloring preserves transition alpha and button rest, hover,
  pressed, border, and font styling update without rebuilding the overlay.

## 0.3.2 — 2026-07-17

### Fixed

- The built-in overlay panel now clamps to 90% of the viewport height and
  scrolls vertically past the clamp — tall overlays (long body lists, many
  buttons) no longer grow past the window and clip off both ends. The panel
  answers the existing wheel-scroll system (`Scrollable` + `Interaction` +
  `Pickable`), and short panels look exactly as before.

## 0.3.1 — 2026-07-11

Review follow-ups on the 0.3.0 widget + focus work.

### Fixed
- The `list()` scroll was dead — the bundle had no `Interaction`, so
  `scroll_lists` matched nothing; now carries `Interaction` + `Pickable`.
- `Slider` constructor no longer panics on an inverted range (endpoints ordered).
- `active_scope` picks a stable scope instead of an arbitrary one when more than
  one `FocusScope` exists; `hover_focuses` only focuses widgets in the active
  scope (an inactive-scope hover can't steal focus).
- `slider_drag` only writes on a real value change (no `Changed<Slider>` churn
  every held frame); `react_toggles` dropped its unused query terms.
