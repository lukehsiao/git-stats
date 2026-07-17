<h1 align="center">
    📊<br>
    git stats
</h1>
<div align="center">
    <strong>A tool for getting aggregated commit stats.</strong>
</div>
<br>
<div align="center">
  <a href="https://github.com/lukehsiao/git-stats/actions/workflows/general.yml">
    <img src="https://img.shields.io/github/actions/workflow/status/lukehsiao/git-stats/general.yml" alt="Build Status">
  </a>
  <a href="https://crates.io/crates/git-stats">
    <img src="https://img.shields.io/crates/v/git-stats" alt="Version">
  </a>
  <a href="https://github.com/lukehsiao/git-stats/blob/main/LICENSE.md">
    <img src="https://img.shields.io/crates/l/git-stats" alt="License">
  </a>
</div>
<br>

`git-stats` parses [log](https://git-scm.com/docs/git-log) information to get stats about the files changed, additions, and deletions.
For example:

```
$ git stats -r origin..HEAD
Author           Commits  Changed Files  Insertions  Deletions  Net Δ
Luke Hsiao            67            117       +2616      -1126  +1490
dependabot[bot]       31             62        +203       -267    -64
Total                 98            179       +2819      -1393  +1426

Reviewer/Tester  Commits
Luke Hsiao             1
```

## Install

Git Stats reads your repository directly with [gitoxide](https://github.com/GitoxideLabs/gitoxide),
a pure-Rust implementation of `git`.

### From crates.io

```
cargo install git-stats --locked
```

Since you are compiling from source anyway, you can opt into CPU-specific
optimizations that the prebuilt binaries cannot use:

```
RUSTFLAGS="-C target-cpu=native" cargo install git-stats --locked
```

Or, if you use [`cargo-binstall`](https://github.com/cargo-bins/cargo-binstall):

```
cargo binstall git-stats
```

### From the AUR

On Arch Linux, install from the [AUR](https://aur.archlinux.org/) using your preferred helper (e.g. [`paru`](https://github.com/Morganamilo/paru) or [`yay`](https://github.com/Jguer/yay)):

```
paru -S git-stats       # builds from source
paru -S git-stats-bin   # prebuilt binary
```

The two packages conflict with each other, so only one may be installed at a time.

## Usage

```
A tool for getting aggregated commit stats

Usage: git-stats [OPTIONS] [revision-range]

Arguments:
  [revision-range]  Show only commits in the specified revision range [default: HEAD]

Options:
  -e, --email            Show the email address of each author
  -r, --reviews          Show who reviewed/tested commits based on `Acked-by`, `Tested-by`, and `Reviewed-by` git trailers
  -s, --sort <SORT>      What column to sort by [default: commits] [possible values: author, commits, files, insertions, deletions, net]
      --reverse          Whether to reverse the sorting from descending to ascending
  -a, --author <AUTHOR>  Limit the commits output to ones with author header lines that match the specified pattern (regular expression)
      --since <SINCE>    Limit the commits output to ones more recent than a specific date
      --until <UNTIL>    Limit the commits output to ones older than a specific date
  -h, --help             Print help (see more with '--help')
  -V, --version          Print version
```


## Notes

A few behaviors are worth knowing, mostly where reading git natively differs slightly from the
porcelain:

- **Revision ranges.** Single revisions, `A..B`, and `A...B` are supported, and each endpoint can be spelled any way `gitoxide` understands (refs, short hashes, `@{n}`, and so on).
  More exotic gitrevisions(7) forms are not interpreted.
- **`--since` / `--until`.** Accepts ISO 8601, RFC 2822, unix timestamps, and relative dates like `"2 weeks ago"`.
  This is a subset of git's full approxidate, and filtering is on the committer date.
- **`--author`.** Matches against the mailmap-resolved author (`Name <email>`), whereas `git log --author` matches the raw author header.
  These differ only for repositories that carry a `.mailmap`.
- **Merge commits** count as commits but contribute no line or file changes, matching the default of `git log --numstat`.
- **Shallow clones** work: commits at the shallow boundary diff against the empty tree, exactly as `git log --numstat` shows them.
  A warning on stderr notes that the truncated history makes the stats differ from a full clone's.
- **Submodules** count like `git log --numstat`: adding or repointing one is a single changed file whose pointer line is the diff.
  Two corners diverge from git: a renamed submodule counts as an addition plus a deletion rather than git's paired rename, and a same-path swap between a file and a submodule is not counted.
- **Line counts** come from gitoxide's diff engine, which can differ from git's by a tiny margin on some hunks (a different but equally valid diff).
  File and net counts match git's rename-aware output.
- **SHA-256 repositories** (`git init --object-format=sha256`) are not supported, as gitoxide cannot read them yet.
- **Commit encodings** other than UTF-8 are not converted: the `encoding` header is ignored and non-UTF-8 author names render with replacement characters, where `git log` would re-encode them.
  Grouping and counts are unaffected, since authors are grouped by their raw bytes.
