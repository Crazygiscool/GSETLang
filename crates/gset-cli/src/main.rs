//! The `gset` command-line interface.
//!
//! Verb structure is preserved from the Go CLI so existing invocations and docs
//! keep working: `transpile`, `run`, `version`, `help`.
//!
//! # Startup cost
//!
//! Invocation latency is the dominant cost in a per-invocation transpiler, so
//! the binary must not touch anything expensive before it has parsed its
//! arguments. In particular no grammar may be initialised, and no language
//! registry may be populated, until `transpile` or `run` has established which
//! language is actually needed. Grammars are feature-gated for the same
//! reason: linking every grammar into every binary costs startup even if none
//! is used.
//!
//! The Go binary had no equivalent structure. It called `config.LoadConfig("")`,
//! which resolved against `os.Getwd()` and read the first `gset.conf` found on
//! disk, before it had even looked at the arguments.

use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(
    name = "gset",
    version,
    about = "Transpile any language to any language",
    long_about = None
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Transpile a source file and print the result to stdout
    Transpile {
        /// Source file to transpile
        path: std::path::PathBuf,
    },
    /// Transpile a source file, then execute it on the target runtime
    Run {
        /// Source file to transpile and run
        path: std::path::PathBuf,
    },
}

fn main() -> std::process::ExitCode {
    let cli = Cli::parse();

    match cli.command {
        Command::Transpile { path } => {
            eprintln!("gset: transpile is not implemented yet (milestone 1)");
            eprintln!("gset: cannot yet transpile {}", path.display());
            std::process::ExitCode::from(1)
        }
        Command::Run { path } => {
            eprintln!("gset: run is not implemented yet (milestone 1)");
            eprintln!("gset: cannot yet run {}", path.display());
            std::process::ExitCode::from(1)
        }
    }
}