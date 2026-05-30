use clap::Parser;

use crate::model::{Options, SortBy};

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
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
    #[arg(short, long)]
    author: Option<Vec<String>>,
    /// Limit the commits output to ones more recent than a specific date.
    #[arg(long)]
    since: Option<String>,
    /// Limit the commits output to ones older than a specific date.
    #[arg(long)]
    until: Option<String>,
}

impl Cli {
    /// Lower the parsed CLI into the application's [`Options`].
    #[must_use]
    pub fn into_options(self) -> Options {
        Options {
            range: self.rev_range,
            email: self.email,
            reviews: self.reviews,
            sort: self.sort,
            reverse: self.reverse,
            authors: self.author.unwrap_or_default(),
            since: self.since,
            until: self.until,
        }
    }
}

#[cfg(test)]
mod test {
    use super::Cli;
    use crate::model::{Options, SortBy};
    use clap::Parser;

    #[test]
    fn verify_app() {
        use clap::CommandFactory;
        Cli::command().debug_assert();
    }

    #[test]
    fn into_options_maps_parsed_flags() {
        let cli = Cli::try_parse_from([
            "git-stats",
            "--email",
            "--reviews",
            "--reverse",
            "--sort",
            "net",
            "--author",
            "alice",
            "--since",
            "2020-01-01",
            "--until",
            "2021-01-01",
            "origin..HEAD",
        ])
        .unwrap();

        let expected = Options {
            range: "origin..HEAD".to_string(),
            email: true,
            reviews: true,
            sort: SortBy::Net,
            reverse: true,
            authors: vec!["alice".to_string()],
            since: Some("2020-01-01".to_string()),
            until: Some("2021-01-01".to_string()),
        };
        assert_eq!(cli.into_options(), expected);
    }

    #[test]
    fn into_options_uses_defaults_without_flags() {
        let opts = Cli::try_parse_from(["git-stats"]).unwrap().into_options();

        let expected = Options {
            range: "HEAD".to_string(),
            email: false,
            reviews: false,
            sort: SortBy::Commits,
            reverse: false,
            authors: Vec::new(),
            since: None,
            until: None,
        };
        assert_eq!(opts, expected);
    }
}
