//! The milestone 1 gate: Python in, Go out, and the Go is well-formed.
//!
//! Two assertions, and they prove different things. The golden comparison
//! proves the emitter is deterministic and that a change to it is a change the
//! author sees in review. The toolchain check proves the output is not merely
//! self-consistent: `gofmt` and `go vet` are independent implementations of
//! "valid Go", and the audit's central defect was output that looked right to
//! the emitter but not to a compiler.

use std::path::PathBuf;
use std::process::{Command, Stdio};

use gset_cli::transpile;

fn workspace() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn fib_go() -> String {
    let root = workspace();
    let text = std::fs::read_to_string(root.join("tests/corpus/py/fib.py")).expect("read fib.py");
    let output = transpile("fib.py", &text, Some("python"), "go").expect("pipeline");
    assert!(
        !output.failed,
        "the gate input must transpile cleanly:\n{}",
        output.diagnostics.render_one_line(&output.source_map)
    );
    output.text
}

#[test]
fn fib_matches_the_recorded_golden() {
    let golden = std::fs::read_to_string(workspace().join("tests/golden/fib.go")).expect("golden");
    assert_eq!(
        fib_go(),
        golden,
        "the emitted Go changed; update tests/golden/fib.go if that was intended"
    );
}

#[test]
fn fib_go_is_accepted_by_the_go_toolchain() {
    let dir = std::env::temp_dir().join(format!("gset-go-gate-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let path = dir.join("main.go");
    std::fs::write(&path, fib_go()).expect("write generated Go");

    if tool_starts("gofmt", "-h") {
        let output = Command::new("gofmt")
            .arg("-l")
            .arg(&path)
            .output()
            .expect("run gofmt");
        assert!(output.status.success(), "gofmt failed: {output:?}");
        assert!(
            output.stdout.is_empty(),
            "gofmt would reformat the generated Go: {}",
            String::from_utf8_lossy(&output.stdout)
        );
    } else {
        eprintln!("gofmt not on PATH; skipping the formatting half of the gate");
    }

    if tool_starts("go", "version") {
        let output = Command::new("go")
            .arg("vet")
            .arg(&path)
            .current_dir(workspace())
            .output()
            .expect("run go vet");
        assert!(
            output.status.success(),
            "go vet rejected the generated Go:\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
    } else {
        eprintln!("go not on PATH; skipping the vet half of the gate");
    }
}

#[test]
fn run_transpiles_then_executes_on_the_go_runtime() {
    if !tool_starts("go", "version") {
        eprintln!("go not on PATH; skipping the `run` end-to-end test");
        return;
    }
    let output = Command::new(env!("CARGO_BIN_EXE_gset"))
        .arg("run")
        .arg(workspace().join("tests/corpus/py/fib.py"))
        .output()
        .expect("spawn gset");
    assert!(
        output.status.success(),
        "gset run failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "55",
        "the program's output should reach stdout"
    );
}

/// Whether `tool` can be started with `probe`.
///
/// A nonzero exit is fine; only a failure to start (the tool is missing) is
/// interesting. This keeps the gate meaningful wherever the toolchain exists
/// without turning every machine without Go into a red build.
fn tool_starts(tool: &str, probe: &str) -> bool {
    Command::new(tool)
        .arg(probe)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok()
}
