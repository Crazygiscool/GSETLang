//! The corpus-wide Go gate: every Python fixture is transpiled, and every
//! fixture the pipeline claims to support must be valid Go.
//!
//! The milestone 1 gate proves the emitter works on one program. That is not
//! enough: `gset` claimed 5 supported targets and produced output that no Go
//! compiler would accept, so a single golden proved nothing about the rest.
//! This gate walks all 22 fixtures and classifies each one.
//!
//! A fixture the frontend rejects, or the backend reports
//! `gset-backend-unsupported` for, is a *known gap* and is recorded as such. A
//! fixture with no error diagnostics is a *claim*: this backend said it can
//! express that program, so `gofmt` and `go vet` must accept the result. A
//! claim that does not compile is the defect class this gate exists to catch,
//! and it is the only state that fails the build.
//!
//! The expected classification lives in `tests/corpus/go_gate_baseline.txt`.
//! Shrinking it is progress; growing it is a regression, and both show up as a
//! diff in review rather than as a silently drifting number.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use gset_cli::transpile;

const BACKEND_UNSUPPORTED: &str = "gset-backend-unsupported";

fn workspace() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// What the pipeline did with one fixture.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Status {
    /// The frontend could not lower the source. Not the backend's problem.
    Frontend,
    /// The backend reported that it cannot express a construct.
    Backend,
    /// The backend claimed support but the output is not even parseable Go.
    Malformed,
    /// The output parses and is `gofmt`-clean but `go vet` rejects it.
    Invalid,
    /// Transpiled, formatted, and vetted.
    Ok,
}

impl Status {
    fn label(self) -> &'static str {
        match self {
            Status::Frontend => "frontend",
            Status::Backend => "backend",
            Status::Malformed => "malformed",
            Status::Invalid => "invalid",
            Status::Ok => "ok",
        }
    }
}

fn fixtures() -> Vec<PathBuf> {
    let dir = workspace().join("tests/corpus/py");
    let mut found: Vec<PathBuf> = std::fs::read_dir(&dir)
        .expect("the Python corpus must exist")
        .map(|entry| entry.expect("read corpus entry").path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "py"))
        .collect();
    found.sort();
    assert!(
        !found.is_empty(),
        "the corpus is empty; the gate would pass vacuously"
    );
    found
}

fn classify(path: &Path, dir: &Path) -> (Status, String) {
    let name = path.file_name().expect("fixture name").to_string_lossy();
    let text = std::fs::read_to_string(path).expect("read fixture");
    let output = match transpile(&name, &text, Some("python"), "go") {
        Ok(output) => output,
        Err(error) => return (Status::Frontend, format!("pipeline error: {error}")),
    };

    let errors: Vec<&gset_ir::Diagnostic> = output
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.severity == gset_ir::Severity::Error)
        .collect();
    if errors
        .iter()
        .any(|error| error.code == Some(BACKEND_UNSUPPORTED))
    {
        let detail = errors[0].message.clone();
        return (Status::Backend, detail);
    }
    if !errors.is_empty() {
        let detail = errors
            .iter()
            .map(|error| error.message.clone())
            .collect::<Vec<_>>()
            .join("; ");
        return (Status::Frontend, detail);
    }

    let stem = path.file_stem().expect("fixture stem").to_string_lossy();
    let generated = dir.join(format!("{stem}.go"));
    std::fs::write(&generated, &output.text).expect("write generated Go");

    let formatted = Command::new("gofmt")
        .arg("-l")
        .arg(&generated)
        .output()
        .expect("run gofmt");
    if !formatted.status.success() {
        return (
            Status::Malformed,
            sanitize(
                dir,
                &format!(
                    "gofmt could not parse it: {}",
                    String::from_utf8_lossy(&formatted.stderr)
                ),
            ),
        );
    }
    if !formatted.stdout.is_empty() {
        return (
            Status::Malformed,
            format!(
                "gofmt would reformat it: {}",
                String::from_utf8_lossy(&formatted.stdout).trim()
            ),
        );
    }

    let vetted = Command::new("go")
        .arg("vet")
        .arg(&generated)
        .current_dir(workspace())
        .output()
        .expect("run go vet");
    if !vetted.status.success() {
        let stderr = String::from_utf8_lossy(&vetted.stderr);
        let first = stderr
            .lines()
            .find(|line| line.contains("vet:"))
            .unwrap_or(&stderr)
            .trim()
            .to_string();
        return (Status::Invalid, sanitize(dir, &first));
    }
    (Status::Ok, String::new())
}

#[test]
fn the_go_backend_never_claims_a_program_it_cannot_compile() {
    let scratch = std::env::temp_dir().join(format!("gset-corpus-go-{}", std::process::id()));
    std::fs::create_dir_all(&scratch).expect("scratch dir");

    // The checked-in baseline holds classifications only. Diagnostics wording
    // changes whenever a message is improved, and a baseline that churns on
    // wording stops being read as evidence; the details are printed instead.
    let mut report = String::new();
    let mut counts = [0usize; 5];
    for path in fixtures() {
        let stem = path.file_stem().expect("fixture stem").to_string_lossy();
        let (status, detail) = classify(&path, &scratch);
        counts[status as usize] += 1;
        let _ = writeln!(report, "{:<10} {}", status.label(), stem);
        if !detail.is_empty() {
            eprintln!("  {stem}: {detail}");
        }
    }
    let _ = writeln!(report);
    let _ = writeln!(
        report,
        "{} ok / {} frontend / {} backend / {} malformed / {} invalid",
        counts[Status::Ok as usize],
        counts[Status::Frontend as usize],
        counts[Status::Backend as usize],
        counts[Status::Malformed as usize],
        counts[Status::Invalid as usize]
    );
    eprintln!("{report}");

    // The baseline records what the Go toolchain said, so a machine without it
    // cannot reproduce it. Printing the classification is still useful there;
    // comparing it is not.
    if !tool_starts("gofmt", "-h") || !tool_starts("go", "version") {
        eprintln!("the Go toolchain is incomplete; skipping the baseline comparison");
        return;
    }

    let path = workspace().join("tests/corpus/go_gate_baseline.txt");
    if std::env::var_os("UPDATE_CORPUS_GATE").is_some() {
        std::fs::write(&path, &report).expect("write baseline");
        eprintln!("wrote {}", path.display());
        return;
    }

    let baseline = std::fs::read_to_string(&path).expect(
        "the baseline file must exist; create it with UPDATE_CORPUS_GATE=1 cargo test --test \
         corpus_go_gate",
    );
    assert_eq!(
        report, baseline,
        "the Go corpus classification changed; a claim that no longer compiles is a regression, \
         and a new capability needs UPDATE_CORPUS_GATE=1 to record"
    );
}

/// Removes the scratch directory from a tool's message.
///
/// The baseline is checked in, so a path containing this process's pid would
/// make every run differ from it. What matters is which line of which file
/// `vet` complained about, not where the file happened to live.
fn sanitize(dir: &Path, message: &str) -> String {
    message.replace(&dir.display().to_string(), "<generated>")
}

/// Whether `tool` can be started with `probe`.
fn tool_starts(tool: &str, probe: &str) -> bool {
    Command::new(tool)
        .arg(probe)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok()
}
