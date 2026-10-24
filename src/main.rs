use std::io::Write;

use anyhow::Result;
use clap::Parser;
use yansi::Condition;

use git_stats::{app, cli::Cli, logic::render, repo::Repo};

fn main() -> Result<()> {
    enable_color_when_appropriate();

    let cli = Cli::parse();
    let repo = Repo::open(".", cli.vcs)?;
    if repo.is_shallow() {
        eprintln!(
            "warning: this is a shallow clone; commits beyond the shallow boundary are not counted"
        );
    }
    let report = app::run(&repo, &cli, &gix::date::Zoned::now())?;
    write_report(&render::report(&report))
}

/// Write the report to stdout, dying quietly when the pipe closes early (`git stats | head`). Rust
/// ignores SIGPIPE, so the closed pipe surfaces as an EPIPE write error, and `print!` would panic
/// on it.
fn write_report(output: &str) -> Result<()> {
    let mut stdout = std::io::stdout().lock();
    let result = stdout
        .write_all(output.as_bytes())
        .and_then(|()| stdout.flush());
    match result {
        Err(e) if e.kind() == std::io::ErrorKind::BrokenPipe => {
            // 128 + SIGPIPE: the status a shell reports when git itself is killed by the signal.
            std::process::exit(141);
        }
        other => Ok(other?),
    }
}

/// Emit ANSI styling only when stdout is a TTY, `CLICOLOR` permits it, and `NO_COLOR` is unset.
/// yansi's global condition then gates every `Paint` call in the render layer, so the logic itself
/// stays oblivious to color.
fn enable_color_when_appropriate() {
    static HAVE_COLOR: Condition = Condition::from(|| {
        Condition::stdout_is_tty() && Condition::clicolor() && Condition::no_color()
    });
    yansi::whenever(Condition::cached((HAVE_COLOR)()));
}
