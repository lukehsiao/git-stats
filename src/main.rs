use std::str::FromStr;

use anyhow::{bail, Result};
use clap::Parser;
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
    #[arg(short, long)]
    /// Show the email address of each author.
    email: bool,
    #[arg(short, long)]
    /// Show who reviewed/tested commits based on `Acked-by`, `Tested-by`, and
    /// `Reviewed-by` git trailers.
    reviews: bool,
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
    #[tabled(rename = "Net Δ", display_with = "display_net")]
    net: i64,
}

#[derive(Tabled)]
struct Review {
    #[tabled(rename = "Reviewer/Tester")]
    author: String,
    #[tabled(rename = "Commits")]
    commits: usize,
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

fn display_net(o: &i64) -> String {
    match o {
        n if *n > 0 => format!("+{}", n),
        n if *n <= 0 => format!("{}", n),
        _ => todo!(),
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let sh = Shell::new()?;

    let rev_range = cli.rev_range;
    let raw_shortlog = if cli.email {
        cmd!(sh, "git shortlog -sen {rev_range}").read()?
    } else {
        cmd!(sh, "git shortlog -sn {rev_range}").read()?
    };

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
                    num_files,
                    insertions,
                    deletions,
                    net: insertions as i64 - deletions as i64,
                })
            })
            .filter_map(|r| r.ok())
            .collect::<_>();

        let mut table = Table::new(stats);
        table
            .with(Style::empty())
            .with(Modify::new(Columns::new(1..=5)).with(Alignment::right()));

        println!("{table}");
    }

    if cli.reviews {
        let raw_reviewers = if cli.email {
            cmd!(sh, "git shortlog -sen --group=trailer:acked-by --group=trailer:tested-by --group=trailer:reviewed-by {rev_range}").read()?
        } else {
            cmd!(sh, "git shortlog -sn --group=trailer:acked-by --group=trailer:tested-by --group=trailer:reviewed-by {rev_range}").read()?
        };
        let reviewers: Vec<(usize, &str)> = raw_reviewers
            .lines()
            .map(|line| {
                let chunks = line.trim().split_once('\t').unwrap();
                let commits = usize::from_str(chunks.0).unwrap();
                let author = chunks.1;
                (commits, author)
            })
            .collect::<_>();

        if !reviewers.is_empty() {
            let reviews: Vec<Review> = reviewers
                .par_iter()
                .map(|(commits, author)| Review {
                    author: author.to_string(),
                    commits: *commits,
                })
                .collect::<_>();

            let mut table = Table::new(reviews);
            table
                .with(Style::empty())
                .with(Modify::new(Columns::new(1..=1)).with(Alignment::right()));

            println!("\n{table}");
        }
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
