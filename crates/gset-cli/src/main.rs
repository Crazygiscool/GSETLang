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

use std::path::PathBuf;

use clap::{Parser, Subcommand};
use gset_cli::{Output, PipelineError, transpile};

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
        path: PathBuf,
        /// Target language to emit
        #[arg(long, short = 't', default_value = "go")]
        to: String,
        /// Source language, overriding the one inferred from the extension
        #[arg(long, short = 'f')]
        from: Option<String>,
        /// Print diagnostics even when there are none
        #[arg(long)]
        verbose: bool,
    },
    /// Transpile a source file, then execute it on the target runtime
    Run {
        /// Source file to transpile and run
        path: PathBuf,
        /// Target language to emit before running
        #[arg(long, short = 't', default_value = "go")]
        to: String,
        /// Source language, overriding the one inferred from the extension
        #[arg(long, short = 'f')]
        from: Option<String>,
    },
}

fn main() -> std::process::ExitCode {
    let cli = Cli::parse();

    match cli.command {
        Command::Transpile {
            path,
            to,
            from,
            verbose,
        } => transpile_command(&path, &to, from.as_deref(), verbose),
        Command::Run { path, to, from } => {
            eprintln!("gset: run is not implemented yet (milestone 1).");
            let _ = (path, to, from);
            std::process::ExitCode::from(1)
        }
    }
}

fn transpile_command(
    path: &PathBuf,
    target: &str,
    from: Option<&str>,
    verbose: bool,
) -> std::process::ExitCode {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) => {
            eprintln!("gset: cannot read {}: {error}", path.display());
            return std::process::ExitCode::from(1);
        }
    };
    let name = path.to_string_lossy();

    let output = match transpile(&name, &text, from, target) {
        Ok(output) => output,
        Err(error) => {
            print_pipeline_error(&error);
            return std::process::ExitCode::from(2);
        }
    };

    report(&output, verbose);

    // Generated code is printed even on failure, because partial output is how
    // a user sees what did translate; the exit code still reflects the errors.
    print!("{}", output.text);

    if output.failed {
        std::process::ExitCode::from(1)
    } else {
        std::process::ExitCode::SUCCESS
    }
}

fn print_pipeline_error(error: &PipelineError) {
    eprintln!("gset: {error}");
}

fn report(output: &Output, verbose: bool) {
    for diagnostic in output.diagnostics.iter() {
        eprintln!("{}", diagnostic.render_one_line(&output.source_map));
    }
    if verbose && output.diagnostics.is_empty() {
        eprintln!("gset: no diagnostics");
    }
}
