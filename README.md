<h1 align="center">
    📊<br>
    git stats
</h1>

<div align="center">
    <strong>A small script to get more thorough shortlog stats.</strong>
<br>
</div>

Git Stats parses shortlog information to get stats about the files changed, additions, and
deletions. For example:

    $ git stats origin..HEAD
     Author      Commits  Changed Files  Insertions  Deletions
     Luke Hsiao       19             42       +2816       -252


## Install

### From crates.io

```
cargo install git-stats --locked
```

## Usage

```
Usage: git-stats [OPTIONS] <REV_RANGE>

Arguments:
  <REV_RANGE>  The revision range to consider

Options:
  -v, --verbose...  More output per occurrence
  -q, --quiet...    Less output per occurrence
  -h, --help        Print help information
  -V, --version     Print version information

```
