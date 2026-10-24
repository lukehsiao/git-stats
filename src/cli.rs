use clap::Parser;

use crate::logic::table::Column;
use crate::repo::Vcs;

/// The parsed command line. [`app::run`](crate::app::run) reads it directly, so the defaults here
/// are the only ones.
#[derive(Parser)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    #[arg(name = "revision-range")]
    /// Show only commits in the specified revision range [default: HEAD, or ::@ with jj]
    ///
    /// When no <revision-range> is specified, it defaults to HEAD (i.e. the whole history leading
    /// to the current commit). origin..HEAD specifies all the commits reachable from the current
    /// commit (i.e. HEAD), but not from origin. For a complete list of ways to spell
    /// [revision-range], see the "Specifying Ranges" section of gitrevisions(7).
    ///
    /// In a jj workspace, [revision-range] is a revset instead, such as `trunk()..@` or a change
    /// ID. It defaults to ::@ (the working-copy commit and its ancestors). See `jj help -k
    /// revsets`.
    pub(crate) rev_range: Option<String>,
    #[arg(short, long)]
    /// Show the email address of each author.
    pub(crate) email: bool,
    #[arg(short, long)]
    /// Show who reviewed/tested commits based on `Acked-by`, `Tested-by`, and `Reviewed-by` git
    /// trailers.
    pub(crate) reviews: bool,
    /// What column to sort the stats table by. Names sort by byte value, so uppercase comes before
    /// lowercase. The reviewers table always sorts by commits.
    #[arg(short, long, value_enum, default_value_t = Column::Commits)]
    pub(crate) sort: Column,
    /// Reverse the sort order of the stats table: counts ascending, authors Z to A
    #[arg(long)]
    pub(crate) reverse: bool,
    /// Limit the commits output to ones with author header lines that match the specified pattern
    /// (regular expression).
    ///
    /// With more than one --author=<pattern>, commits whose author matches any of the given
    /// patterns are chosen.
    #[arg(short, long)]
    pub(crate) author: Vec<String>,
    /// Limit the commits output to ones more recent than a specific date.
    #[arg(long)]
    pub(crate) since: Option<String>,
    /// Limit the commits output to ones older than a specific date.
    #[arg(long)]
    pub(crate) until: Option<String>,
    /// Which version control system reads the repository and interprets [revision-range].
    ///
    /// `auto` picks jj when the nearest repository is a jj workspace, colocated or not, and git
    /// otherwise. It also picks git when `GIT_DIR` is set, as it is inside git hooks.
    #[arg(long, value_enum, default_value_t = Vcs::Auto)]
    pub vcs: Vcs,
}

#[cfg(test)]
mod test {
    use super::Cli;

    #[test]
    fn verify_app() {
        use clap::CommandFactory;
        Cli::command().debug_assert();
    }
}
