# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.4.7] - 2024-05-12
### Details
#### Features
- Parallel sourcemap generation

## [0.4.6] - 2024-05-12
### Details
#### Bug Fixes
- Create package folders by @daimond113
- Colour single tick codeblocks properly by @daimond113

#### Continuous Integration
- Update macos x86_64 image (the default is now arm) by @daimond113

#### Documentation
- Bring back convert index note by @daimond113
- Correct manifest dependency format by @daimond113
- Link documentation in readme by @daimond113

#### Features
- Allow images in readmes by @daimond113
- Add exports to sidebar by @daimond113

#### Miscellaneous Tasks
- Make registry image weigh less by @daimond113

## [0.4.5] - 2024-04-01
### Details
#### Bug Fixes
- Remove manifest requirement by @daimond113

## [0.4.4] - 2024-03-31
### Details
#### Bug Fixes
- Use project indices in specifier by @daimond113
- Correctly update sync tool files by @daimond113

## [0.4.3] - 2024-03-31
### Details
#### Bug Fixes
- Ensure version is root by @daimond113

#### Documentation
- Document exports field of manifest by @daimond113

#### Features
- Support manifest-less repos & running local package bin export by @daimond113

#### Miscellaneous Tasks
- Merge pull request #1 from Foorack/repo-patch

Update repository field in Cargo.toml by @daimond113 in [#1](https://github.com/daimond113/pesde/pull/1)
- Update repository field in Cargo.toml by @Foorack

#### Refactor
- Improve manifest parsing by @daimond113

## New Contributors
* @Foorack made their first contribution
## [0.4.2] - 2024-03-29
### Details
#### Bug Fixes
- Create folder for config by @daimond113

#### Features
- Add documentation meta tags by @daimond113
- Add documentation by @daimond113

## [0.4.1] - 2024-03-28
### Details
#### Bug Fixes
- :bug: correctly insert packages from lockfile by @daimond113

#### Continuous Integration
- :triangular_flag_on_post: remove macos aarch64 due to costs by @daimond113

#### Miscellaneous Tasks
- :alien: fix compilation due to zstd-sys by @daimond113

## [0.4.0] - 2024-03-27
### Details
#### Bug Fixes
- :bug: link root dependencies to their dependents aswell by @daimond113

#### Features
- :sparkles: add dependency names by @daimond113
- :sparkles: add dependency overrides by @daimond113

#### Refactor
- :art: improve lockfile format by @daimond113

#### Styling
- :art: apply clippy & rustfmt by @daimond113

## [0.3.2] - 2024-03-24
### Details
#### Bug Fixes
- :bug: correct linking file paths by @daimond113
- :bug: correctly enable fields with features by @daimond113

## [0.3.1] - 2024-03-24
### Details
#### Features
- :sparkles: automatically find file to use as lib by @daimond113

## [0.3.0] - 2024-03-24
### Details
#### Features
- :sparkles: multi-index + wally support by @daimond113

#### Miscellaneous Tasks
- :pencil2: correct env variable names by @daimond113

## [0.2.0] - 2024-03-17
### Details
#### Continuous Integration
- :white_check_mark: run clippy on all workspace members by @daimond113

#### Features
- :children_crossing: add wally conversion by @daimond113
- :sparkles: add embed metadata by @daimond113

#### Miscellaneous Tasks
- :bug: show logo on all platforms by @daimond113

#### Refactor
- :art: use static variables by @daimond113
- :zap: store index files as btreemaps by @daimond113

## [0.1.4] - 2024-03-16
### Details
#### Features
- :sparkles: add repository field by @daimond113
- :rocket: create website by @daimond113
- :sparkles: add listing newest packages by @daimond113

## [0.1.3] - 2024-03-10
### Details
#### Features
- :sparkles: add init, add, remove, and outdated commands by @daimond113
- :sparkles: package versions endpoint by @daimond113

## [0.1.2] - 2024-03-06
### Details
#### Features
- :sparkles: add ratelimits by @daimond113

#### Miscellaneous Tasks
- :rocket: setup crates.io publishing by @daimond113

## [0.1.1] - 2024-03-04
### Details
#### Bug Fixes
- :passport_control: properly handle missing api token entry by @daimond113

#### Documentation
- :memo: update README by @daimond113

## [0.1.0] - 2024-03-04
### Details
#### Features
- :tada: initial commit by @daimond113

[0.4.7]: https://github.com/daimond113/pesde/compare/v0.4.6..v0.4.7
[0.4.6]: https://github.com/daimond113/pesde/compare/v0.4.5..v0.4.6
[0.4.5]: https://github.com/daimond113/pesde/compare/v0.4.4..v0.4.5
[0.4.4]: https://github.com/daimond113/pesde/compare/v0.4.3..v0.4.4
[0.4.3]: https://github.com/daimond113/pesde/compare/v0.4.2..v0.4.3
[0.4.2]: https://github.com/daimond113/pesde/compare/v0.4.1..v0.4.2
[0.4.1]: https://github.com/daimond113/pesde/compare/v0.4.0..v0.4.1
[0.4.0]: https://github.com/daimond113/pesde/compare/v0.3.2..v0.4.0
[0.3.2]: https://github.com/daimond113/pesde/compare/v0.3.1..v0.3.2
[0.3.1]: https://github.com/daimond113/pesde/compare/v0.3.0..v0.3.1
[0.3.0]: https://github.com/daimond113/pesde/compare/v0.2.0..v0.3.0
[0.2.0]: https://github.com/daimond113/pesde/compare/v0.1.4..v0.2.0
[0.1.4]: https://github.com/daimond113/pesde/compare/v0.1.3..v0.1.4
[0.1.3]: https://github.com/daimond113/pesde/compare/v0.1.2..v0.1.3
[0.1.2]: https://github.com/daimond113/pesde/compare/v0.1.1..v0.1.2
[0.1.1]: https://github.com/daimond113/pesde/compare/v0.1.0..v0.1.1

<!-- generated by git-cliff -->
