# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.20.3] - 21-Apr-2023

### Fixed

- Accept NumpadEnter as Enter ([#171](https://github.com/mvlabat/bevy_egui/pull/171) by @dimvoly)

## [0.20.2] - 27-Mar-2023

### Fixed

- Fix incorrect bounds check for set_scissor_rect ([#167](https://github.com/mvlabat/bevy_egui/pull/167) by @Gorialis)
- Fix panic messages for uninitialised contexts

### Changed

- Move `bevy_core_pipeline` to dev-dependencies ([#166](https://github.com/mvlabat/bevy_egui/pull/166) by @jakobhellermann)

## [0.20.1] - 12-Mar-2023

### Fixed

- Fix recreation of `EguiContext` on startup ([#162](https://github.com/mvlabat/bevy_egui/pull/162) by @encounter)
- Set image sampler address modes to `ClampToEdge` ([#158](https://github.com/mvlabat/bevy_egui/pull/158) by @galop1n)

## [0.20.0] - 8-Mar-2023

### Added

- Add `altgr` support for Windows ([#149](https://github.com/mvlabat/bevy_egui/pull/149) by @Vrixyz)
- Add `serde` feature ([#154](https://github.com/mvlabat/bevy_egui/pull/154) by @AlanRace)

### Changed

- Update Bevy to 0.10 ([#159](https://github.com/mvlabat/bevy_egui/pull/159), thanks to @DGriffin91)
- Update Egui to 0.21 ([#152](https://github.com/mvlabat/bevy_egui/pull/152) by @paul-hansen)
- Implement better multi-window support ([#147](https://github.com/mvlabat/bevy_egui/pull/147) by @TheRawMeatball)

### Fixed

- Pass raw Bevy time to Egui to fix UI animations ([#155](https://github.com/mvlabat/bevy_egui/pull/155) by @jakobhellermann)

## [0.19.0] - 15-Jan-2023

### Changed

- Update the `arboard` dependency ([#142](https://github.com/mvlabat/bevy_egui/pull/142) by @jakobhellermann)

### Fixed

- Fix panics due to missing swapchain textures ([#141](https://github.com/mvlabat/bevy_egui/pull/141) by @connerebbinghaus)

## [0.18.0] - 11-Dec-2022

### Changed

- Update Egui to 0.20 ([#139](https://github.com/mvlabat/bevy_egui/pull/139) by @no-materials)

## [0.17.1] - 14-Nov-2022

### Fixed

- Fix clearing event readers (missed events warnings)

## [0.17.0] - 13-Nov-2022

### Changed

- Update to Bevy 0.9 ([#127](https://github.com/mvlabat/bevy_egui/pull/127), [#133](https://github.com/mvlabat/bevy_egui/pull/133), thanks to @terhechte and @jakobhellermann)

### Fixed

- Fix window resizing on Windows ([#128](https://github.com/mvlabat/bevy_egui/pull/128) by @chronicl) 

## [0.16.1] - 18-Sep-2022

### Fixed

- Fix releasing buttons outside a window ([#123](https://github.com/mvlabat/bevy_egui/pull/123), thanks to @TheRawMeatball for flagging the issue in [#121](https://github.com/mvlabat/bevy_egui/pull/121))

## [0.16.0] - 24-Aug-2022

### Changed

- Update Egui to 0.19

## [0.15.1] - 13-Aug-2022

### Fixed

- Store image handles instead of ids to persist strong handles.

## [0.15.0] - 30-Jul-2022

### Added

- Add a feature that can be disabled to replace default Egui fonts ([#110](https://github.com/mvlabat/bevy_egui/pull/110) by @iTitus)

### Changed
 
- Update Bevy to 0.8 ([#111](https://github.com/mvlabat/bevy_egui/pull/111) by @DGriffin91)

## [0.14.0] - 1-May-2022

### Added

- Add new_tab support for open_url ([#96](https://github.com/mvlabat/bevy_egui/pull/96) by @Azorlogh).
  - `EguiSettings` has also got the `default_open_url_target` parameter to make the default behaviour on left mouse click configurable.
- Update Egui to 0.18 ([#99](https://github.com/mvlabat/bevy_egui/pull/99)).

### Changed

- The `multi_threaded` feature was renamed to `immutable_ctx`.

### Fixed

- Improve wgsl readability and introduce minor optimisations ([#95](https://github.com/mvlabat/bevy_egui/pull/95) by @lain-dono).
- Remove duplicate EguiPipeline resource initialization ([#98](https://github.com/mvlabat/bevy_egui/pull/98) by @lain-dono).
- Fix color blending for user textures ([#100](https://github.com/mvlabat/bevy_egui/pull/100)).

## [0.13.0] - 16-Apr-2022

### Changed

- Update Bevy to 0.7 ([#79](https://github.com/mvlabat/bevy_egui/pull/79) by @aevyrie and @forbjok).
- Return egui::TextureId on removal ([#81](https://github.com/mvlabat/bevy_egui/pull/81) by @Shatur).
- Add `must_use` attributes to methods ([#82](https://github.com/mvlabat/bevy_egui/pull/82)).

### Fixed

- Remove unnecessary image clone allocation ([#84](https://github.com/mvlabat/bevy_egui/pull/84) by @frewsxcv).
- Avoid allocations by utilizing `HashMap::iter_mut` ([#83](https://github.com/mvlabat/bevy_egui/pull/83) by @frewsxcv).
- Remove unnecessary swap texture clone ([#85](https://github.com/mvlabat/bevy_egui/pull/85) by @frewsxcv).

## [0.12.1] - 13-Mar-2022

### Added

- Add a function to get image id ([#80](https://github.com/mvlabat/bevy_egui/pull/80) by @Shatur).

## [0.12.0] - 12-Mar-2022

### Added

- Add side panel example ([#73](https://github.com/mvlabat/bevy_egui/pull/73)).

### Changed

- Update Egui to 0.17 ([#78](https://github.com/mvlabat/bevy_egui/pull/78) by @emilk).

### Changed

- User texture ids are now tracked internally ([#71](https://github.com/mvlabat/bevy_egui/pull/71)).
  - Instead of using `set_egui_texture`, you can now use `add_image` which returns a texture id itself
    (see the updated [ui](https://github.com/mvlabat/bevy_egui/blob/c611671603a70e5956ba06f77bb94851c7ced659/examples/ui.rs) example).
- Switch to `arboard` for managing clipboard ([#72](https://github.com/mvlabat/bevy_egui/pull/72)).

## [0.11.1] - 4-Feb-2022

### Added

- Add `ctx_for_windows_mut` and `try_ctx_for_windows_mut` for accessing multiple contexts without the `multi_threaded` feature.

## [0.11.0] - 4-Feb-2022

### Changed

- Introduce mutable getters for EguiContext, feature gate immutable ones ([#64](https://github.com/mvlabat/bevy_egui/pull/63)).
  - If you used `bevy_egui` without the `multi_threaded` feature, you'll need to change every `ctx` call to `ctx_mut`.

## [0.10.3] - 29-Jan-2022

### Added

- Feature `multi_threaded`, to avoid using `egui/multi_threaded` ([#63](https://github.com/mvlabat/bevy_egui/pull/63) by @ndarilek).

### Fixed

- WGPU crash on minimizing a window ([#62](https://github.com/mvlabat/bevy_egui/pull/62) by @aevyrie).

## [0.10.2] - 23-Jan-2022

### Added

- Horizontal scroll support (Shift + Mouse Wheel).
- Zoom support (Ctrl/Cmd + Mouse Wheel).

### Fixed

- Change points delta from 24 to 50 for `MouseScrollUnit::Line` event.
- Fix handling of mouse button events for Safari (inputs are no longer ignored).
- Scroll is no longer applied to every Bevy window.

## [0.10.1] - 16-Jan-2022

### Added

- Headless mode support ([#51](https://github.com/mvlabat/bevy_egui/pull/51) by @Shatur).

### Fixed

- Egui pass now runs after `bevy_ui` ([#53](https://github.com/mvlabat/bevy_egui/pull/53) by @jakobhellermann).

## [0.10.0] - 8-Jan-2022

### Changed

- Update Bevy to 0.6 ([#25](https://github.com/mvlabat/bevy_egui/pull/25) by @jakobhellermann).

## [0.9.0] - 1-Jan-2022

### Changed

- Update Egui to 0.16 ([#49](https://github.com/mvlabat/bevy_egui/pull/49) by @Meshiest).

## [0.8.0] - 27-Nov-2021

### Changed

- Update Egui to 0.15.0 ([#45](https://github.com/mvlabat/bevy_egui/pull/45)).

## [0.7.1] - 06-Oct-2021

### Added

- Add `EguiStartupSystem` system labels.

### Fixed

- Initialize egui contexts during startup (fixes [#41](https://github.com/mvlabat/bevy_egui/issues/41)).

## [0.7.0] - 05-Sep-2021

### Changed

- Update Egui to 0.14.0 ([#38](https://github.com/mvlabat/bevy_egui/pull/38)).

## [0.6.2] - 15-Aug-2021

### Fixed

- Fix receiving input when holding a button ([#37](https://github.com/mvlabat/bevy_egui/pull/37)).

## [0.6.1] - 20-Jul-2021

### Fixed

- Fix more edge-cases related to invalid scissors.

## [0.6.0] - 29-Jun-2021

### Changed

- Update Egui to 0.13.0.

## [0.5.0] - 22-May-2021

### Changed

- Update Egui to 0.12.0.

## [0.4.2] - 03-May-2021

### Added

- Better error message for a missing Egui context ([#24](https://github.com/mvlabat/bevy_egui/pull/24) by @jakobhellermann)
- Add `try_ctx_for_window` function ([#20](https://github.com/mvlabat/bevy_egui/pull/20) by @jakobhellermann)

## [0.4.1] - 24-Apr-2021

### Fixed

- Fix crashes related to invalid scissor or window size ([#18](https://github.com/mvlabat/bevy_egui/pull/18))

## [0.4.0] - 10-Apr-2021

Huge thanks to @jakobhellermann and @Weasy666 for contributing to this release!

### Added

- Implement multiple windows support ([#14](https://github.com/mvlabat/bevy_egui/pull/14) by @jakobhellermann).

### Changed

- Update Egui to 0.11.0 ([#12](https://github.com/mvlabat/bevy_egui/pull/12) by @Weasy666 and @jakobhellermann).

## [0.3.0] - 02-Mar-2021

### Changed

- Update Egui to 0.10.0.

## [0.2.0] - 08-Feb-2021

### Changed

- Update Egui to 0.9.0.

## [0.1.3] - 20-Jan-2021

### Fixed

- Fix copying textures to take alignment into account.
- Disable a documentation test.

## [0.1.2] - 18-Jan-2021

### Fixed

- Disable default features for docs.rs to fix the build.

## [0.1.1] - 18-Jan-2021

### Fixed

- Fix compilation errors when no features are set.
