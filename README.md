<h1 align="center">
    📊<br>
    git stats
</h1>
<div align="center">
    <strong>A tool for getting aggregated shortlog stats.</strong>
</div>
<br>
<div align="center">
  <a href="https://github.com/lukehsiao/git-stats/actions/workflows/general.yml">
    <img src="https://img.shields.io/github/actions/workflow/status/lukehsiao/git-stats/general.yml" alt="Build Status">
  </a>
  <a href="https://crates.io/crates/git-stats">
    <img src="https://img.shields.io/crates/v/git-stats" alt="Version">
  </a>
  <a href="https://github.com/lukehsiao/git-stats/blob/main/LICENSE">
    <img src="https://img.shields.io/crates/l/git-stats" alt="License">
  </a>
</div>
<br>

Git Stats parses [shortlog](https://git-scm.com/docs/git-shortlog) information to get stats about the files changed, additions, and deletions.
For example:

    $ git stats -r origin..HEAD
     Author           Commits  Changed Files  Insertions  Deletions  Net Δ
     Luke Hsiao            67            117       +2616      -1126  +1490
     dependabot[bot]       31             62        +203       -267    -64
     Total                 98            179       +2819      -1393  +1426

     Reviewer/Tester  Commits
     Luke Hsiao             1

## Install

This is a glorified shell script.
As such, it expects that you have `git` installed on your machine and in your `$PATH`.

### From crates.io

```
cargo install git-stats --locked
```

## Usage

```
A tool for getting aggregated shortlog stats

Usage: git-stats [OPTIONS] [revision-range]

Arguments:
  [revision-range]  Show only commits in the specified revision range [default: HEAD]

Options:
  -e, --email            Show the email address of each author
  -r, --reviews          Show who reviewed/tested commits based on `Acked-by`, `Tested-by`, and `Reviewed-by` git trailers
  -s, --sort <SORT>      What column to sort by [default: commits] [possible values: author, commits, files, insertions, deletions, net]
      --reverse          Whether to reverse the sorting from descending to ascending
  -a, --author <AUTHOR>  Limit the commits output to ones with author header lines that match the specified pattern (regular expression)
  -h, --help             Print help (see more with '--help')
  -V, --version          Print version
```
