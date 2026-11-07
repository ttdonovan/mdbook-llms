# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.4.0](https://github.com/ttdonovan/mdbook-llms/compare/v0.3.0...v0.4.0) - 2026-11-07

### Fixed

- name the optional files key optional-files, since mdBook reads optional under every output as a bool of its own.

## [0.3.0](https://github.com/ttdonovan/mdbook-llms/compare/v0.2.0...v0.3.0) - 2026-11-07

### Added

- add entry sizes, a preamble, and optional files to llms.txt.

## [0.2.0](https://github.com/ttdonovan/mdbook-llms/compare/v0.1.1...v0.2.0) - 2026-11-06

### Added

- warn about broken links in the llms output.
- keep hidden code lines in llms output without their marker.
- add llms.txt v2 discovery links to merged HTML pages.

### Fixed

- read folded, literal and quoted frontmatter descriptions.

### Other

- enforce strict clippy lints and drop process::exit.
- split lib.rs into modules and parse chapters once.
- add crates.io and docs.rs badges to the README.

## [0.1.1](https://github.com/ttdonovan/mdbook-llms/compare/v0.1.0...v0.1.1) - 2026-11-06

### Other

- note the Config source in the skill's llms-full.txt link.
- point the skill at the published llms.txt and llms-full.txt.
- include Config source in the example via anchors.

## [0.1.0](https://github.com/ttdonovan/mdbook-llms/releases/tag/v0.1.0) - 2026-11-06

### Added

- initial mdbook-llms renderer, docs book and CI.

### Other

- link llms.txt and llms-full.txt on getting started.
