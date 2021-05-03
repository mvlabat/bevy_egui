# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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

- Implement Egui 0.11.0 support ([#12](https://github.com/mvlabat/bevy_egui/pull/12) by @Weasy666 and @jakobhellermann).
- Implement multiple windows support ([#14](https://github.com/mvlabat/bevy_egui/pull/14) by @jakobhellermann).

## [0.3.0] - 02-Mar-2021

### Added

- Upgrade Egui to 0.10.0.

## [0.2.0] - 08-Feb-2021

### Added

- Implement Egui 0.9.0 support.

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
