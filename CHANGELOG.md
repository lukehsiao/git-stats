# Changelog

## 0.2.4

### Patch Changes

- [`e90511a`](https://github.com/lukehsiao/git-stats/commit/e90511a4dedbd83970f3e2ccb1a8ef457c0b8165) - **style**: we now color the header and total rows, rather than just bold (or bold+underline).

  The style when NO_COLOR is set, or being piped to another process remains the same: plain text.

<pre>
$ git-stats v0.2.3..v0.2.4
Author      Commits  Changed Files  Insertions  Deletions  Net Δ
Luke Hsiao        2              4         +32        -20    +12
Total             2              4         +32        -20    +12
</pre>

## 0.2.3

### Patch Changes

- [`1a3e321`](https://github.com/lukehsiao/git-stats/commit/1a3e3213a615374136f36cf25d689d0dd1ec2e04) - **fix**: exit quietly when stdout closes early instead of panicking.

  Piping into a pager or `head` closes stdout before the report is fully written.
  Rust ignores SIGPIPE, so `git-stats | head` could die with a noisy "failed printing to stdout: Broken pipe" panic.
  It now exits silently with status 141, the same status a shell reports for git itself.

- [`0193da9`](https://github.com/lukehsiao/git-stats/commit/0193da983104928b88bc02ea65b4004cb8a91beb) - **fix**: count binary file changes in the Changed Files column.

  Since the v0.2.0 rewrite, a binary file change was dropped entirely from the stats because it has no line counts, under-reporting the Changed Files column for commits touching images or other binary blobs.
  Binary changes now count as changed files contributing no lines, matching `git diff --shortstat`.

- [`ba65955`](https://github.com/lukehsiao/git-stats/commit/ba65955f7f6517ef7d5204e95e7ffc2699b8fe19) - **fix**: count submodule pointer changes the way `git log --numstat` does.

  A gitlink has no blob to diff, so submodule additions and pointer bumps were dropped from the stats entirely.
  git renders the pointer as a one-line `Subproject commit <hash>` pseudo-file; `git-stats` now matches it: an added submodule is one file and +1, a repointed one is one file with +1/-1.
  A renamed submodule still diverges from git, counting as an addition plus a deletion rather than a paired rename.

- [`c580553`](https://github.com/lukehsiao/git-stats/commit/c5805533162153c0eb9713c6984d7233f8106a99) - **perf**: decode each commit header once during the history walk.

  The walk used to rescan the raw header separately for the parents, author, committer, and message.
  Walk-dominated invocations (heavy filtering, `--reviews`) are about 10% faster; diff-heavy runs are unchanged.

- [`cbd4379`](https://github.com/lukehsiao/git-stats/commit/cbd4379e5a3d221e2aaed34f1cdfcdad4f8536a1) - **fix**: hide every merge base in symmetric difference (`A...B`) ranges.

  `A...B` hid only the single "best" merge base, but criss-cross histories have more than one.
  Commits reachable from the unhidden bases leaked into the walk, inflating the counts relative to `git rev-list --count A...B`.
  All merge bases are now hidden, matching git.

- [`bf3e00c`](https://github.com/lukehsiao/git-stats/commit/bf3e00c141d35cea3b421a1b67972e58918b7fe7) - **fix**: honor `GIT_DIR` and related environment variables when locating the repository.

  Repository discovery only searched upward from the current directory, so `git-stats` failed inside git hooks (git sets `GIT_DIR` for them, and the working directory need not be in the repository) and under `git --git-dir=... stats`.
  Discovery now checks the environment first, exactly like git.

- [`f058233`](https://github.com/lukehsiao/git-stats/commit/f058233e5f557bf512685f8587efd4e3814361af) - **fix**: name the offending commit when one fails to decode.

  A corrupt commit header used to kill the report with an anonymous "could not read commit data: object parsing failed", leaving no way to locate the bad object in a large history.
  When the repository has a commit-graph (git writes one during `gc` by default), the error now reads "could not decode commit <id>".

- [`a7c967b`](https://github.com/lukehsiao/git-stats/commit/a7c967b3afcdd6d762c4f37e69f1a5105e9e2ba7) - **fix**: drop the stray leading blank line when only the reviews table renders.

  Reviews ignore the author and date filters by design, so filters that match no commits leave the stats table empty while `--reviews` still prints.
  The output began with a blank line meant to separate the two tables; it now appears only when both tables are present.

- [`35107c7`](https://github.com/lukehsiao/git-stats/commit/35107c723dba33510945741b8f624609e3b50a57) - **fix**: support shallow clones.

  Since the v0.2.0 rewrite, running `git-stats` in a shallow clone failed with "could not read commit data": computing stats for a commit at the shallow boundary tried to load its parent, which a shallow clone does not have.
  Boundary commits now diff against the empty tree, exactly like root commits, matching `git log --numstat`.
  Because the truncated history makes every count differ from the full clone's, `git-stats` also prints a warning to stderr when it detects a shallow clone.

- [`dfdac00`](https://github.com/lukehsiao/git-stats/commit/dfdac00894f50d289b1122cf1ee3fba2c1d860e5) - **perf**: validate `--author`, `--since`, and `--until` before walking the range.

  A typo'd pattern or date now errors immediately instead of after a walk that can take minutes on a large history.

<pre>
$ git-stats v0.2.2..v0.2.3
Author      Commits  Changed Files  Insertions  Deletions  Net Δ
Luke Hsiao       16             41       +1029        -75   +954
Total            16             41       +1029        -75   +954
</pre>

## 0.2.2

### Patch Changes

- [`9918e04`](https://github.com/lukehsiao/git-stats/commit/9918e04f53ba0d6bff854f8795e4f5d79aa596a2) - **fix**: peel annotated tags to commits in revision ranges.

  Since the v0.2.0 rewrite, range endpoints naming annotated tags were not peeled
  the way git peels them at rev-list endpoints. An annotated tag on the excluded
  side (`tag..HEAD`) silently returned whole-repo history with exit 0, while a
  tag on the included side errored with "Expected object of kind commit but got
  tag". Lightweight tags were unaffected. This bit our own release notes: the
  v0.2.0 and v0.2.1 stats tables claimed 181 and 184 commits for 9- and 3-commit
  releases.

<pre>
$ git-stats v0.2.1..v0.2.2
Author      Commits  Changed Files  Insertions  Deletions  Net Δ
Luke Hsiao        4             13        +272        -96   +176
Total             4             13        +272        -96   +176
</pre>

## 0.2.1

### Patch Changes

- [`27a2bd7`](https://github.com/lukehsiao/git-stats/commit/27a2bd7d17073e9a4bffd238d5e12ccf7e3c80bc) Thanks [@lukehsiao](https://github.com/lukehsiao)! - **perf**: dramatically speed up stats on large repositories.

  Per-commit diffing did redundant work that cancelled out parallelism. On a full
  clone of `torvalds/linux`, `HEAD~2000..HEAD` (61,860 commits) now runs about 3x
  faster, and a full-history run completes in minutes where it was previously
  impractical. Output is unchanged.

<pre>
$ git-stats v0.2.0..v0.2.1
Author               Commits  Changed Files  Insertions  Deletions  Net Δ
Luke Hsiao                 2              4        +138        -39    +99
github-actions[bot]        1              5         +29        -17    +12
Total                      3              9        +167        -56   +111
</pre>

## 0.2.0

### Minor Changes

- [`1af1932`](https://github.com/lukehsiao/git-stats/commit/1af1932bc1ddbe9e403caeafb15da1d4d107aa34) Thanks [@lukehsiao](https://github.com/lukehsiao)! - **refactor**: This is a full rewrite from being a glorified shell script to doing all of the computation natively in Rust.

  There are no feature changes.

  This also dramatically improves performance.
  For example, with this refactor, we can run on the `git/git` repository in

  ```
  Benchmark 1: git-stats
    Time (mean ± σ):     19.520 s ±  0.622 s    [User: 185.828 s, System: 4.572 s]
    Range (min … max):   18.460 s … 20.565 s    10 runs
  ```

  on a laptop with an Intel i7-1355U.
  With the previous version of `git-stats` this takes about 6 minutes!

<pre>
$ git-stats v0.1.23..v0.2.0
Author               Commits  Changed Files  Insertions  Deletions  Net Δ
Luke Hsiao                 6             29       +3585       -494  +3091
dependabot[bot]            2              2          +2         -2      0
github-actions[bot]        1              5         +20        -10    +10
Total                      9             36       +3607       -506  +3101
</pre>

## 0.1.23

### Patch Changes

- [`197ccc9`](https://github.com/lukehsiao/git-stats/commit/197ccc9a25d138f573e9f73a35df668c70c5922f) Thanks [@lukehsiao](https://github.com/lukehsiao)! - **style**: remove leading and trailing spaces on output.

  Before:

  ```
   Author           Commits  Changed Files  Insertions  Deletions  Net Δ
   Luke Hsiao           120            214       +4863      -2032  +2831
   dependabot[bot]       50            100        +326       -456   -130
   Total                170            314       +5189      -2488  +2701
  ```

  After:

  ```
  Author           Commits  Changed Files  Insertions  Deletions  Net Δ
  Luke Hsiao           121            215       +4868      -2033  +2835
  dependabot[bot]       50            100        +326       -456   -130
  Total                171            315       +5194      -2489  +2705
  ```

<pre>
$ git-stats v0.1.22..v0.1.23
Author      Commits  Changed Files  Insertions  Deletions  Net Δ
Luke Hsiao        1              3         +39        -10    +29
Total             1              3         +39        -10    +29
</pre>

## 0.1.22

### Patch Changes

- [`7a1196e`](https://github.com/lukehsiao/git-stats/commit/7a1196ec630e9e6dbac6057975991333e5e19fa2) Thanks [@lukehsiao](https://github.com/lukehsiao)! - Add metadata and binaries for [`cargo-binstall`](https://github.com/cargo-bins/cargo-binstall) compatibility.

<pre>
$ git-stats v0.1.21..v0.1.22
 Author      Commits  Changed Files  Insertions  Deletions  Net Δ
 Luke Hsiao        3             17       +1249       -214  +1035
 Total             3             17       +1249       -214  +1035
</pre>

## [0.1.21](https://github.com/lukehsiao/git-stats/compare/v0.1.20..v0.1.21) - 2026-01-07

### Bug Fixes

- escape author in cli arguments - ([fcd700f](https://github.com/lukehsiao/git-stats/commit/fcd700fe96784e2b315aece5e91421d69c6ead21)) - Luke Hsiao

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
