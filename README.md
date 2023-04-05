<h1 align="center">
    📊<br>
    git stats
</h1>

<div align="center">
    <strong>A small script to get more thorough shortlog stats.</strong>
</div>
<br>
<br>

Git Stats parses shortlog information to get stats about the files changed, additions, and
deletions. For example:

    $ git stats -r origin..HEAD
     Author      Commits  Changed Files  Insertions  Deletions  Net Δ
     Luke Hsiao       30             50       +1324       -166  +1158

     Reviewer/Tester  Commits
     Luke Hsiao             1


## Install

This is a glorified shell script. As such, it expects that you have `git` installed on your machine
and in your `$PATH`.

### From crates.io

```
cargo install git-stats --locked
```

## Usage

```
A script for grabbing more thorough shortlog stats

Usage: git-stats [OPTIONS] [revision-range]

Arguments:
  [revision-range]  Show only commits in the specified revision range [default: HEAD]

Options:
  -e, --email    Show the email address of each author
  -r, --reviews  Show who reviewed/tested commits based on `Acked-by`, `Tested-by`, and `Reviewed-by` git trailers
  -h, --help     Print help (see more with '--help')
  -V, --version  Print version
```
