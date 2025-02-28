# Changelog

All notable changes to this project will be documented in this file. See [conventional commits](https://www.conventionalcommits.org/) for commit guidelines.

---
## [0.1.20](https://github.com/lukehsiao/git-stats/compare/v0.1.19..v0.1.20) - 2025-02-28

### Bug Fixes

- add proper spacing when using --author - ([30fb6d0](https://github.com/lukehsiao/git-stats/commit/30fb6d069b91c207959c9395ca5f37f4f5dfb7b6)) - Luke Hsiao

### Build and Dependencies

- **(deps)** bump clap from 4.5.23 to 4.5.27 - ([f0d95ea](https://github.com/lukehsiao/git-stats/commit/f0d95ea69a19965c7a5a0712e06590de0058fbe6)) - dependabot[bot]
- **(deps)** bump anyhow from 1.0.94 to 1.0.96 - ([c81ca02](https://github.com/lukehsiao/git-stats/commit/c81ca022ba11a14daed50bbd2f13b270e6c778ed)) - dependabot[bot]
- update all dependencies - ([d581c5c](https://github.com/lukehsiao/git-stats/commit/d581c5ce1fd1b0d7324e094bc1546ccf0bbfc803)) - Luke Hsiao

---
## [0.1.19](https://github.com/lukehsiao/git-stats/compare/v0.1.18..v0.1.19) - 2024-12-19

### Features

- support `--since` and `--until` filters for git log - ([c0c1361](https://github.com/lukehsiao/git-stats/commit/c0c1361b46e8ad8bd37a70263b4a4b9ed0bb17dc)) - Luke Hsiao

### Documentation

- **(LICENSE)** use markdown for nicer rendering - ([de2f3fb](https://github.com/lukehsiao/git-stats/commit/de2f3fbd08808bbf4095ce7682cc79e73728aa2b)) - Luke Hsiao

---
## [0.1.18](https://github.com/lukehsiao/git-stats/compare/v0.1.17..v0.1.18) - 2024-10-01

This is a minor refactor.
As part of fixing clippy's pedantic lints, there should be slight performance improvments.

### Refactor

- fix clippy pedantic lints - ([ec15965](https://github.com/lukehsiao/git-stats/commit/ec15965c6b87df25cfd2496944a9e3cd7b676bab)) - Luke Hsiao

### Build and Dependencies

- **(deps)** bump clap from 4.5.15 to 4.5.16 - ([3c03f20](https://github.com/lukehsiao/git-stats/commit/3c03f20b5336aa592773087e3fc2a156d8f6cd1d)) - dependabot[bot]
- **(deps)** bump tabled from 0.15.0 to 0.16.0 - ([8645b3b](https://github.com/lukehsiao/git-stats/commit/8645b3be6930684ca7298b6dacc567b939fe087a)) - dependabot[bot]
- **(deps)** bump clap from 4.5.16 to 4.5.18 - ([641dce5](https://github.com/lukehsiao/git-stats/commit/641dce557c8ec4be2c250a332b4442bf8a7b8b09)) - dependabot[bot]
- **(deps)** bump anyhow from 1.0.86 to 1.0.89 - ([8f21b1c](https://github.com/lukehsiao/git-stats/commit/8f21b1c82ccff8ec0f3ad18d18d2554f27143e16)) - dependabot[bot]

---
## [0.1.17](https://github.com/lukehsiao/git-stats/compare/v0.1.16..v0.1.17) - 2024-08-10

This release fixes a significant flaw with how `--author` behaved, which caused authors and commits to be lower than expected, or missed entirely.
**I strongly recommend you upgrade.**

This patch switches from using `git shortlog` to `git log` when collecting the set of authors to compute stats on.

There is a bug with `git` itself when providing the `--author` flag.
Specifically, the behavior of `git log` and `git shortlog` behave differently when dealing with author filters that also are affected by `.mailmap`.
`git log` appears to apply the filter AFTER applying `.mailmap`, whereas
`git shortlog` appears to do it BEFORE.

### Bug Fixes

- use `log`, not `shortlog` for proper `--author` support - ([654a7b4](https://github.com/lukehsiao/git-stats/commit/654a7b46669ce5e17614bb42e7335a7a099ea94b)) - Luke Hsiao

---
## [0.1.16](https://github.com/lukehsiao/git-stats/compare/v0.1.15..v0.1.16) - 2024-05-03

### Features

- support `--author` flag for git-shortlog - ([ed80049](https://github.com/lukehsiao/git-stats/commit/ed800491c5bcc52fc354445dab9634ca0ca641b3)) - Luke Hsiao

---
## [0.1.15](https://github.com/lukehsiao/git-stats/compare/v0.1.14..v0.1.15) - 2024-04-26

### Documentation

- **(README)** fix indentation of example output - ([bdc8dac](https://github.com/lukehsiao/git-stats/commit/bdc8dac79963e9f773b9e4f2d1c63094b0159b5f)) - Luke Hsiao

### Performance

- drop unneeded derives and sort - ([9766ad4](https://github.com/lukehsiao/git-stats/commit/9766ad42503474ffe80f9f66743bc80232eabb34)) - Luke Hsiao

---
## [0.1.14](https://github.com/lukehsiao/git-stats/compare/v0.1.13..v0.1.14) - 2024-04-26

### Documentation

- **(README)** update alignment of output - ([960f18e](https://github.com/lukehsiao/git-stats/commit/960f18e1f68af5b4b4b3096b13f4e98913a895c6)) - Luke Hsiao

### Features

- allow sorting by other columns - ([627d3f0](https://github.com/lukehsiao/git-stats/commit/627d3f09dd596ae862fe3a4de57bcd74d3b94847)) - Luke Hsiao

---
## [0.1.13](https://github.com/lukehsiao/git-stats/compare/v0.1.12..v0.1.13) - 2024-04-19

### Refactor

- don't right-align "Total" - ([1f80584](https://github.com/lukehsiao/git-stats/commit/1f805843d60da3da8429229188b331ca54f385d8)) - Luke Hsiao

---
## [0.1.12](https://github.com/lukehsiao/git-stats/compare/v0.1.11..v0.1.12) - 2024-04-19

### Refactor

- remove redundant logic - ([9db509e](https://github.com/lukehsiao/git-stats/commit/9db509ece52584c8459d91131b1db2f79c63ffa2)) - Luke Hsiao

---
## [0.1.11](https://github.com/lukehsiao/git-stats/compare/v0.1.10..v0.1.11) - 2024-04-19

### Features

- improve styling, and disable if not interactive - ([d5a6209](https://github.com/lukehsiao/git-stats/commit/d5a6209838bdf7e3def54c0919c5e0105aa73bc6)) - Luke Hsiao

---
## [0.1.10](https://github.com/lukehsiao/git-stats/compare/v0.1.9..v0.1.10) - 2024-04-19

### Features

- include totals of all statistics - ([8c1895f](https://github.com/lukehsiao/git-stats/commit/8c1895ffaaa5df998dd2b6635b8c9f221c11747a)) - Luke Hsiao

---
## [0.1.9](https://github.com/lukehsiao/git-stats/compare/v0.1.8..v0.1.9) - 2024-01-18

### Documentation

- **(CHANGELOG)** add entry for v0.1.9 - ([cef37bf](https://github.com/lukehsiao/git-stats/commit/cef37bf01f3e7fccd659f00a897f0167ec93baee)) - Luke Hsiao
- **(README)** link license badge to license - ([57d4d74](https://github.com/lukehsiao/git-stats/commit/57d4d744e9d21c1c05d30558203b71eed969d01d)) - Luke Hsiao

---
## [0.1.8](https://github.com/lukehsiao/git-stats/compare/v0.1.7..v0.1.8) - 2023-06-26

### Documentation

- **(CHANGELOG)** add entry for v0.1.8 - ([b298900](https://github.com/lukehsiao/git-stats/commit/b2989006db4f8c3dc693b87482ed5af9986ffa10)) - Luke Hsiao
- **(README)** add badges - ([ab759f7](https://github.com/lukehsiao/git-stats/commit/ab759f794b4c9be75f08b8b927c37e2f00f8d3e2)) - Luke Hsiao

---
## [0.1.7](https://github.com/lukehsiao/git-stats/compare/v0.1.6..v0.1.7) - 2023-04-05

### Documentation

- **(CHANGELOG)** add entry for v0.1.7 - ([6b22fac](https://github.com/lukehsiao/git-stats/commit/6b22fac18520ebaa087c5b5457bce12d9ac83bdd)) - Luke Hsiao
- **(README)** update readme to include reviewed/tested - ([4f5d386](https://github.com/lukehsiao/git-stats/commit/4f5d386b1b60709b85e8324fae3fcc797afcb3a0)) - Luke Hsiao

### Refactor

- gate reviewer/tester stats behind flag - ([328a600](https://github.com/lukehsiao/git-stats/commit/328a60091795562e4624362de3a1f5eec2bea3b4)) - Luke Hsiao

---
## [0.1.6](https://github.com/lukehsiao/git-stats/compare/v0.1.5..v0.1.6) - 2023-01-21

### Documentation

- **(CHANGELOG)** add entry for v0.1.6 - ([d367168](https://github.com/lukehsiao/git-stats/commit/d3671684dcb813c5f02505eb78533b170136acb7)) - Luke Hsiao

### Features

- output reviewer/tests and commit counts - ([885334d](https://github.com/lukehsiao/git-stats/commit/885334d46c2c5686fcc0f81d4c1265f884eca68b)) - Luke Hsiao

### Refactor

- add `Δ` in the `Net` column header - ([6369d04](https://github.com/lukehsiao/git-stats/commit/6369d04eb0737aa063b15fb461b6885c6d980591)) - Luke Hsiao

---
## [0.1.5](https://github.com/lukehsiao/git-stats/compare/v0.1.4..v0.1.5) - 2022-11-23

### Documentation

- **(CHANGELOG)** add entry for v0.1.5 - ([c4dca25](https://github.com/lukehsiao/git-stats/commit/c4dca255a8f1b0ff1f40352b7851bd9787edbc90)) - Luke Hsiao
- **(README)** update usage example - ([74b148a](https://github.com/lukehsiao/git-stats/commit/74b148af9bd3b2029bf009e794bc575dcb502371)) - Luke Hsiao

### Features

- add net change column to output - ([a90377a](https://github.com/lukehsiao/git-stats/commit/a90377a537644b07ba689e7d7f9579d8120f7916)) - Luke Hsiao

---
## [0.1.4](https://github.com/lukehsiao/git-stats/compare/v0.1.3..v0.1.4) - 2022-10-29

### Documentation

- **(CHANGELOG)** add entry for v0.1.4 - ([c887a0c](https://github.com/lukehsiao/git-stats/commit/c887a0c04db89d75c197a64f02b784f8939d2bcb)) - Luke Hsiao
- **(README)** note the `git` dependency - ([7fc2765](https://github.com/lukehsiao/git-stats/commit/7fc27656a9cb618fca6580d460c2c23d9eb25a73)) - Luke Hsiao

### Refactor

- remove unused verbosity flag and deps - ([b5de2e3](https://github.com/lukehsiao/git-stats/commit/b5de2e354c3de6836fdecbb40839bd5be61ccff1)) - Luke Hsiao

---
## [0.1.3](https://github.com/lukehsiao/git-stats/compare/v0.1.2..v0.1.3) - 2022-10-29

### Documentation

- **(CHANGELOG)** add entry for v0.1.3 - ([11879c9](https://github.com/lukehsiao/git-stats/commit/11879c9e75a3e4bd07954386aab270119da97f19)) - Luke Hsiao

### Features

- add email option for including author email addresses - ([efe5c79](https://github.com/lukehsiao/git-stats/commit/efe5c7942797c62aa518aef7f56d6a9f48c817dd)) - Luke Hsiao

### Refactor

- drop raw stats from verbose logs - ([267a28f](https://github.com/lukehsiao/git-stats/commit/267a28fe590549069c1b3f396fdbde9b243d84da)) - Luke Hsiao
- improve help text for revision-range, default to HEAD - ([6bf71a0](https://github.com/lukehsiao/git-stats/commit/6bf71a03099a489e71acc27e60b918087f6a6ab4)) - Luke Hsiao

---
## [0.1.2](https://github.com/lukehsiao/git-stats/compare/v0.1.1..v0.1.2) - 2022-10-27

### Documentation

- **(CHANGELOG)** add entry for v0.1.2 - ([079fd69](https://github.com/lukehsiao/git-stats/commit/079fd6984e487f94f1a295eeed61c77ec9ed3064)) - Luke Hsiao

### Performance

- parallelize stat collection with rayon - ([e6783b4](https://github.com/lukehsiao/git-stats/commit/e6783b458920d64fd18a5062fe545970f76ae765)) - Luke Hsiao

---
## [0.1.1](https://github.com/lukehsiao/git-stats/compare/v0.1.0..v0.1.1) - 2022-10-26

### Bug Fixes

- interpret author literally, not as regex - ([c03d158](https://github.com/lukehsiao/git-stats/commit/c03d1589bd7901091b90d2854256e37fd0578f05)) - Luke Hsiao

### Documentation

- **(CHANGELOG)** add entry for v0.1.1 - ([a204b5e](https://github.com/lukehsiao/git-stats/commit/a204b5e4e8c80dbafb03c462a34dc34d13c21baf)) - Luke Hsiao

---
## [0.1.0] - 2022-10-26

### Features

- initial implementation - ([68b516c](https://github.com/lukehsiao/git-stats/commit/68b516cd46b011af8cbba2c63a5b0c50b60bdaa8)) - Luke Hsiao

