# Changelog

All notable changes to this project will be documented in this file. See [conventional commits](https://www.conventionalcommits.org/) for commit guidelines.

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

