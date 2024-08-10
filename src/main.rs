use std::str::FromStr;

use anyhow::{bail, Result};
use clap::{Parser, ValueEnum};
use rayon::prelude::*;
use tabled::{
    settings::{
        format::Format,
        object::{Columns, Rows},
        Alignment, Modify, Style,
    },
    Table, Tabled,
};
use yansi::{Condition, Paint};

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
    /// What column to sort by
    #[arg(short, long, value_enum, default_value_t = SortBy::Commits)]
    sort: SortBy,
    /// Whether to reverse the sorting from descending to ascending
    #[arg(long)]
    reverse: bool,
    /// Limit the commits output to ones with author header lines that match the specified pattern (regular expression).
    ///
    /// With more than one --author=<pattern>, commits whose author matches any of the given patterns are chosen.
    /// This is pased through as `--author` to `git log`.
    #[arg(short, long)]
    author: Option<Vec<String>>,
}

#[derive(Clone, ValueEnum)]
enum SortBy {
    /// Sort by author alphabetic order
    Author,
    /// Sort by number of commits
    Commits,
    /// Sort by number of files touched
    Files,
    /// Sort by number of insertions
    Insertions,
    /// Sort by number of deletions
    Deletions,
    /// Sort by net lines of change
    Net,
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

    // Check `NO_COLOR`, `CLICOLOR`, and if we have TTYs.
    static HAVE_COLOR: Condition = Condition::from(|| {
        Condition::stdout_is_tty() && Condition::clicolor() && Condition::no_color()
    });
    yansi::whenever(Condition::cached((HAVE_COLOR)()));

    // Build up the command based on flags
    let rev_range = cli.rev_range;
    let author = cli.author.map(|authors| {
        authors
            .iter()
            .map(|a| format!("--author={}", a))
            .collect::<Vec<String>>()
    });
    let mut log_cmd = if cli.email {
        "git log --format='%aN <%aE>' ".to_string()
    } else {
        "git log --format='%aN' ".to_string()
    };
    if let Some(a) = author {
        log_cmd.push_str(&a.join(" "));
    }
    log_cmd.push(' ');
    log_cmd.push_str(&rev_range);
    log_cmd.push_str(" | sort | uniq -c | sort -nr");

    let cmd = cmd!(sh, "bash -c {log_cmd}");
    let raw_shortlog = cmd.read()?;

    let shortlog: Vec<(usize, &str)> = raw_shortlog
        .lines()
        .map(|line| {
            let chunks = line.trim().split_once(' ').unwrap();
            let commits = usize::from_str(chunks.0).unwrap();
            let author = chunks.1;
            (commits, author)
        })
        .collect::<_>();

    if !shortlog.is_empty() {
        let mut stats: Vec<Stat> = shortlog
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

        match cli.sort {
            SortBy::Author => stats.sort_unstable_by(|a, b| b.author.cmp(&a.author)),
            SortBy::Commits => (), // It's already sorted by commits
            SortBy::Files => stats.sort_unstable_by(|a, b| b.num_files.cmp(&a.num_files)),
            SortBy::Insertions => stats.sort_unstable_by(|a, b| b.insertions.cmp(&a.insertions)),
            SortBy::Deletions => stats.sort_unstable_by(|a, b| b.deletions.cmp(&a.deletions)),
            SortBy::Net => stats.sort_unstable_by(|a, b| b.net.cmp(&a.net)),
        }
        if cli.reverse {
            stats.reverse()
        }

        // Collect totals
        let totals = stats.iter().fold(
            Stat {
                author: "Total".to_string(),
                commits: 0,
                num_files: 0,
                insertions: 0,
                deletions: 0,
                net: 0,
            },
            |acc, s| Stat {
                author: acc.author,
                commits: acc.commits + s.commits,
                num_files: acc.num_files + s.num_files,
                insertions: acc.insertions + s.insertions,
                deletions: acc.deletions + s.deletions,
                net: acc.net + s.net,
            },
        );
        stats.push(totals);

        let mut table = Table::new(stats);
        table
            .with(Style::empty())
            .modify(Columns::new(1..=5), Alignment::right())
            .modify(
                Rows::first(),
                Format::content(|s| s.bold().underline().to_string()),
            )
            .modify(Rows::last(), Format::content(|s| s.bold().to_string()));

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
