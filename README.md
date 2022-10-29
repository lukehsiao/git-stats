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

    $ git stats origin..HEAD
     Author      Commits  Changed Files  Insertions  Deletions
     Luke Hsiao       19             42       +2816       -252


## Install

This is a glorified shell script. As such, it expects that you have `git` installed on your machine
and in your `$PATH`.

### From crates.io

```
cargo install git-stats --locked
```

## Usage

```
$ git stats -h
A script for grabbing more thorough shortlog stats

Usage: git-stats [OPTIONS] [revision-range]

Arguments:
  [revision-range]  Show only commits in the specified revision range [default: HEAD]

Options:
  -e, --email       Show the email address of each author
  -v, --verbose...  More output per occurrence
  -q, --quiet...    Less output per occurrence
  -h, --help        Print help information (use `--help` for more detail)
  -V, --version     Print version information
```
