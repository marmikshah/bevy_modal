# Changelog

Notable changes to `bevy_modal`, newest first. Follows [SemVer](https://semver.org);
format loosely [Keep a Changelog](https://keepachangelog.com). Entries begin
from the point this file was added — earlier releases live in the crates.io
version history and the git log.

## Unreleased

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
