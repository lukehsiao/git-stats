use std::str::FromStr;

use anyhow::{bail, Result};
use clap::Parser;
use clap_verbosity_flag::{Verbosity, WarnLevel};
use rayon::prelude::*;
use tabled::{object::Columns, Alignment, Modify, Style, Table, Tabled};

use xshell::{cmd, Shell};

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[arg(name = "revision-range", default_value = "HEAD")]
    /// Show only commits in the specified revision range.
    ///
    /// When no <revision-range> is specified, it defaults to HEAD (i.e. the whole history leading
    /// to the current commit). origin..HEAD specifies all the commits reachable from the current
    /// commit (i.e. HEAD), but not from origin. For a complete list of ways to spell
    /// [revision-range], see the "Specifying Ranges" section of gitrevisions(7).
    rev_range: String,
    #[command(flatten)]
    verbose: Verbosity<WarnLevel>,
}

#[derive(Tabled)]
struct Stat {
    #[tabled(rename = "Author")]
    author: String,
    #[tabled(rename = "Commits")]
    commits: usize,
    #[tabled(rename = "Changed Files")]
    num_files: usize,
    #[tabled(rename = "Insertions", display_with = "display_add")]
    insertions: usize,
    #[tabled(rename = "Deletions", display_with = "display_del")]
    deletions: usize,
}

fn display_del(o: &usize) -> String {
    match o {
        0 => format!("{}", 0),
        n => format!("-{}", n),
    }
}

fn display_add(o: &usize) -> String {
    match o {
        0 => format!("{}", 0),
        n => format!("+{}", n),
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    env_logger::Builder::new()
        .filter_level(cli.verbose.log_level_filter())
        .format_timestamp(None)
        .init();

    let sh = Shell::new()?;

    let rev_range = cli.rev_range;
    let raw_shortlog = cmd!(sh, "git shortlog -sn {rev_range}").read()?;

    let shortlog: Vec<(usize, &str)> = raw_shortlog
        .lines()
        .map(|line| {
            let chunks = line.trim().split_once('\t').unwrap();
            let commits = usize::from_str(chunks.0).unwrap();
            let author = chunks.1;
            (commits, author)
        })
        .collect::<_>();

    if !shortlog.is_empty() {
        let stats: Vec<Stat> = shortlog
            .par_iter()
            .map(|(commits, author)| {
                let sh = Shell::new()?;
                let raw_stats = cmd!(
                    sh,
                    "git log -F --author={author} --pretty=tformat: --numstat {rev_range}"
                )
                .read()?;
                let mut insertions = 0;
                let mut deletions = 0;
                let mut num_files = 0;
                for line in raw_stats.lines() {
                    let mut chunks = line.split_whitespace();
                    insertions += match chunks.next() {
                        // For binary files
                        Some("-") => 0,
                        Some(n) => usize::from_str(n)?,
                        None => bail!("Invalid shortlog line"),
                    };
                    deletions += match chunks.next() {
                        // For binary files
                        Some("-") => 0,
                        Some(n) => usize::from_str(n)?,
                        None => bail!("Invalid shortlog line"),
                    };
                    num_files += 1;
                }
                Ok(Stat {
                    author: author.to_string(),
                    commits: *commits,
                    insertions,
                    deletions,
                    num_files,
                })
            })
            .filter_map(|r| r.ok())
            .collect::<_>();

        let mut table = Table::new(stats);
        table
            .with(Style::empty())
            .with(Modify::new(Columns::new(1..=4)).with(Alignment::right()));

        println!("{table}");
    }

    Ok(())
}

#[cfg(test)]
mod test {
    use crate::*;
    #[test]
    fn verify_app() {
        use clap::CommandFactory;
        Cli::command().debug_assert()
    }
}
