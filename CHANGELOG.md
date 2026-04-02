## [0.3.6] - 2026-04-02

### Changed

- Fix LCFS income columns and weights; add --uprate-to flag; generate 2026/27 clean data for FRS, LCFS, SPI, and WAS.

  LCFS loader: switch employment income to wkgrossp (weekly gross pay, well-populated), add p047p for main SE income, add p048p for investment income, and rescale weighta to UK household population (~28.3m) so weighted aggregates are correct.

  Add --uprate-to flag to --extract mode, allowing raw survey data to be extracted and uprated to a target fiscal year in one step (e.g. --frs raw/ --year 2023 --uprate-to 2026 --extract data/frs/2026/).

  Update SKILL.md to document --uprate-to and the UKDS project ID for LCFS/WAS/SPI downloads.


## [0.3.5] - 2026-03-30

### Changed

- Use variable-specific uprating indices (earnings, CPI, GDP/capita, etc.) matching policyengine-uk; fix Scottish brackets for 2025/26+; repeal two-child limit from 2026/27; upload uprated FRS data for 2024-2029 to GCS


## [0.3.4] - 2026-03-30

### Fixed

- Fix aarch64-linux wheel: build natively in manylinux container instead of cross-compiling (fixes glibc 2.39 dependency)


## [0.3.3] - 2026-03-30

### Fixed

- Fix CI: use manylinux container's bundled Python for wheel builds


## [0.3.2] - 2026-03-30

### Fixed

- Fix CI: resolve rustup/cargo toolchain detection in manylinux container builds


## [0.3.1] - 2026-03-30

### Fixed

- Fix Linux wheel builds: manylinux container for glibc compat, aarch64-linux support


## [0.3.0] - 2026-03-30

### Added

- Add aarch64-linux wheel and fix x86_64-linux glibc compatibility (build in manylinux container)


## [0.2.1] - 2026-03-30

### Fixed

- Fixed PyPI publishing pipeline: manylinux wheel tags, automated versioning trigger.


## [0.2.0] - 2026-03-30

### Added

- Initial release: compiled UK microsimulation engine with Python interface, PyPI packaging, and Modal API deployment.


# Changelog
