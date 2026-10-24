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

### With mise

[mise](https://mise.jdx.dev/) can install a prebuilt binary directly from the GitHub release, verifying its checksum and SLSA provenance:

```
mise use -g github:lukehsiao/git-stats
```

## Usage

```
A tool for getting aggregated commit stats

Usage: git-stats [OPTIONS] [revision-range]

Arguments:
  [revision-range]  Show only commits in the specified revision range [default: HEAD, or ::@ with jj]

Options:
  -e, --email            Show the email address of each author
  -r, --reviews          Show who reviewed/tested commits based on `Acked-by`, `Tested-by`, and `Reviewed-by` git trailers
  -s, --sort <SORT>      What column to sort the stats table by. Names sort by byte value, so uppercase comes before lowercase. The reviewers table always sorts by commits [default: commits] [possible values: author, commits, files, insertions, deletions, net]
      --reverse          Reverse the sort order of the stats table: counts ascending, authors Z to A
  -a, --author <AUTHOR>  Limit the commits output to ones with author header lines that match the specified pattern (regular expression)
      --since <SINCE>    Limit the commits output to ones more recent than a specific date
      --until <UNTIL>    Limit the commits output to ones older than a specific date
      --vcs <VCS>        Which version control system reads the repository and interprets [revision-range] [default: auto] [possible values: auto, git, jj]
  -h, --help             Print help (see more with '--help')
  -V, --version          Print version
```

## Jujutsu

In a [jj](https://docs.jj-vcs.dev/latest/) workspace, colocated or not, `git stats` takes a revset instead of a git revision range.
Change IDs, bookmarks, and your `revset-aliases` all work:

```
$ git stats 'trunk()..@'
```

- **Default range.** Without a range, it reports `::@`, the working-copy commit and its ancestors.
  Like any jj command, it snapshots the working copy first, so uncommitted edits count.
  An empty working-copy commit counts as one commit with no changes; use `::@-` to leave it out.
- **Requirements.** jj 0.36 or newer must be on your `PATH`. jj selects the commits, and gitoxide reads them from jj's git store.
- **Conflicts.** A conflicted commit, and a commit with a conflicted parent, take their counts from jj's own diff, as `jj diff --stat` shows them.
  jj stores a conflict as a git tree of `.jjconflict-*` directories, which a git diff would count as files.
- **Forcing git.** `--vcs git` reads a colocated workspace as a git repository with git revision ranges, for scripts that pass ranges like `origin..HEAD`.
  `git stats` also uses git when `GIT_DIR` is set, as it is inside git hooks.


## Notes

A few behaviors are worth knowing, mostly where reading git natively differs slightly from the
porcelain:

- **Revision ranges.** Single revisions, `A..B`, and `A...B` are supported, and each endpoint can be spelled any way `gitoxide` understands (refs, short hashes, `@{n}`, and so on).
  More exotic gitrevisions(7) forms are not interpreted.
- **`--since` / `--until`.** Accepts ISO 8601, RFC 2822, unix timestamps, and relative dates like `"2 weeks ago"`.
  This is a subset of git's full approxidate, and filtering is on the committer date.
- **`--author`.** Matches against the mailmap-resolved author (`Name <email>`), whereas `git log --author` matches the raw author header.
  These differ only for repositories that carry a `.mailmap`.
- **Empty names and emails.** An author or reviewer with an empty name shows as their email, and each email gets its own row.
  Without `--email`, `git shortlog` instead groups them all under one blank name; with it, git shows ` <email>`.
  With `--email`, a contributor without an email shows as just the name, where `git shortlog -e` shows `Name <>`.
- **Merge commits** count as commits but contribute no line or file changes, matching the default of `git log --numstat`.
- **Shallow clones** work: commits at the shallow boundary diff against the empty tree, exactly as `git log --numstat` shows them.
  A warning on stderr notes that the truncated history makes the stats differ from a full clone's.
- **Submodules** count like `git log --numstat`: adding or repointing one is a single changed file whose pointer line is the diff, and a file swapped with a submodule is one changed file too.
  Renaming one without repointing it is one changed file with no line changes, as git pairs the rename.
- **Partial clones** (`git clone --filter=blob:none`) fail with an error that says the clone lacks objects, because gitoxide does not fetch missing objects the way git does.
  Running `git log --numstat` over the same range first fetches them.
- **Line counts** come from gitoxide's diff engine, which can differ from git's by a tiny margin on some hunks (a different but equally valid diff).
  File and net counts match git's rename-aware output.
- **SHA-256 repositories** (`git init --object-format=sha256`) are not supported, as gitoxide cannot read them yet.
- **Commit encodings** other than UTF-8 are not converted: the `encoding` header is ignored and non-UTF-8 author names render with replacement characters, where `git log` would re-encode them.
  Grouping and counts are unaffected, since authors are grouped by their raw bytes.
