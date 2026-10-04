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

use std::path::{Path, PathBuf};
use std::process::{Command as ProcessCommand, ExitCode};
use std::time::{SystemTime, UNIX_EPOCH};

use clap::{Parser, Subcommand};
use gset_backend::TargetId;
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
        #[arg(long, short = 't', visible_alias = "target", default_value = "go")]
        to: String,
        /// Source language, overriding the one inferred from the extension
        #[arg(long, short = 'f')]
        from: Option<String>,
        /// Write the result here instead of to stdout
        #[arg(long, short = 'o')]
        output: Option<PathBuf>,
        /// Print diagnostics even when there are none
        #[arg(long)]
        verbose: bool,
    },
    /// Transpile a source file, then execute it on the target runtime
    Run {
        /// Source file to transpile and run
        path: PathBuf,
        /// Target language to emit before running
        #[arg(long, short = 't', visible_alias = "target", default_value = "go")]
        to: String,
        /// Source language, overriding the one inferred from the extension
        #[arg(long, short = 'f')]
        from: Option<String>,
        /// Keep the generated file instead of deleting it after the run
        #[arg(long)]
        keep: bool,
    },
    /// Print the version
    Version,
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    match cli.command {
        Command::Transpile {
            path,
            to,
            from,
            output,
            verbose,
        } => transpile_command(&path, &to, from.as_deref(), output.as_deref(), verbose),
        Command::Run {
            path,
            to,
            from,
            keep,
        } => run_command(&path, &to, from.as_deref(), keep),
        Command::Version => {
            println!("GSET v{}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
    }
}

fn transpile_command(
    path: &Path,
    target: &str,
    from: Option<&str>,
    destination: Option<&Path>,
    verbose: bool,
) -> ExitCode {
    let output = match load(path, target, from) {
        Ok(output) => output,
        Err(code) => return code,
    };

    report(&output, verbose);

    // Generated code is written even on failure, because partial output is how
    // a user sees what did translate; the exit code still reflects the errors.
    match destination {
        Some(destination) => {
            if let Err(error) = std::fs::write(destination, &output.text) {
                eprintln!("gset: cannot write {}: {error}", destination.display());
                return ExitCode::from(1);
            }
        }
        None => print!("{}", output.text),
    }

    if output.failed {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}

fn run_command(path: &Path, target: &str, from: Option<&str>, keep: bool) -> ExitCode {
    let output = match load(path, target, from) {
        Ok(output) => output,
        Err(code) => return code,
    };
    report(&output, false);
    if output.failed {
        // For `run` the generated source is not the product, so it goes to
        // stderr where it explains the failure without polluting stdout.
        eprint!("{}", output.text);
        return ExitCode::from(1);
    }

    let Some(id) = TargetId::from_name(target) else {
        eprintln!("gset: `{target}` is not a target this build understands");
        return ExitCode::from(2);
    };
    let Some(mut command) = runtime_command(id) else {
        eprintln!("gset: running {id} is not supported yet");
        return ExitCode::from(2);
    };

    let directory = temp_directory(path);
    if let Err(error) = std::fs::create_dir_all(&directory) {
        eprintln!("gset: cannot create {}: {error}", directory.display());
        return ExitCode::from(1);
    }
    let program = directory.join(format!("program.{}", id.extension()));
    if let Err(error) = std::fs::write(&program, &output.text) {
        eprintln!("gset: cannot write {}: {error}", program.display());
        return ExitCode::from(1);
    }

    command.arg(&program);
    let code = match command.status() {
        Ok(status) => ExitCode::from(
            status
                .code()
                .filter(|code| (0..=255).contains(code))
                .unwrap_or(1) as u8,
        ),
        Err(error) => {
            eprintln!("gset: cannot run {id}: {error}");
            ExitCode::from(127)
        }
    };

    if keep {
        eprintln!("gset: kept {}", program.display());
    } else {
        let _ = std::fs::remove_dir_all(&directory);
    }
    code
}

/// The runtime invocation for a target, before the generated file is appended.
fn runtime_command(target: TargetId) -> Option<ProcessCommand> {
    match target {
        TargetId::GO => {
            let mut command = ProcessCommand::new("go");
            command.arg("run");
            Some(command)
        }
        _ => None,
    }
}

/// A per-invocation directory under the system temp directory.
///
/// The name includes the process id and a nanosecond timestamp so two `gset`
/// processes never collide, and includes the source stem so a `--keep` run
/// leaves a recognisable directory.
fn temp_directory(source: &Path) -> PathBuf {
    let stem = source
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("gset");
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_nanos())
        .unwrap_or(0);
    std::env::temp_dir().join(format!("gset-{stem}-{}-{nanos}", std::process::id()))
}

fn load(path: &Path, target: &str, from: Option<&str>) -> Result<Output, ExitCode> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) => {
            eprintln!("gset: cannot read {}: {error}", path.display());
            return Err(ExitCode::from(1));
        }
    };
    let name = path.to_string_lossy();

    match transpile(&name, &text, from, target) {
        Ok(output) => Ok(output),
        Err(error) => {
            print_pipeline_error(&error);
            Err(ExitCode::from(2))
        }
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
