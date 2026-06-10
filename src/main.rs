use anyhow::Result;
use clap::Parser;
use yansi::Condition;

use git_stats::{app, cli::Cli, repo::Repo};

fn main() -> Result<()> {
    enable_color_when_appropriate();

    let opts = Cli::parse().into_options();
    let repo = Repo::open(".")?;
    if repo.is_shallow() {
        eprintln!(
            "warning: this is a shallow clone; commits beyond the shallow boundary are not counted"
        );
    }
    let output = app::run(&repo, &opts)?;
    print!("{output}");
    Ok(())
}

/// Emit ANSI styling only when stdout is a TTY, `CLICOLOR` permits it, and
/// `NO_COLOR` is unset. yansi's global condition then gates every `Paint` call
/// in the render layer, so the logic itself stays oblivious to color.
fn enable_color_when_appropriate() {
    static HAVE_COLOR: Condition = Condition::from(|| {
        Condition::stdout_is_tty() && Condition::clicolor() && Condition::no_color()
    });
    yansi::whenever(Condition::cached((HAVE_COLOR)()));
}
