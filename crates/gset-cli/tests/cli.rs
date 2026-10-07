//! The parts of the CLI surface that are not about Go correctness.
//!
//! `version` and `-o` are compatibility surfaces: scripts, packaging and the
//! recorded baseline all rely on them, so they get a regression test even
//! though each is a few lines.

use std::path::PathBuf;
use std::process::Command;

fn workspace() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn gset() -> Command {
    Command::new(env!("CARGO_BIN_EXE_gset"))
}

#[test]
fn version_prints_the_go_cli_shape() {
    let output = gset().arg("version").output().expect("spawn gset");
    assert!(output.status.success());
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(
        text.starts_with("GSET v"),
        "expected a `GSET v...` line, got {text:?}"
    );
}

#[test]
fn transpile_writes_to_a_file_with_output() {
    let destination = std::env::temp_dir().join(format!("gset-cli-{}.go", std::process::id()));
    let output = gset()
        .arg("transpile")
        .arg(workspace().join("tests/corpus/py/fib.py"))
        .args(["--to", "go", "-o"])
        .arg(&destination)
        .output()
        .expect("spawn gset");
    assert!(
        output.status.success(),
        "transpile failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let written = std::fs::read_to_string(&destination).expect("read generated file");
    let golden = std::fs::read_to_string(workspace().join("tests/golden/fib.go")).expect("golden");
    assert_eq!(written, golden);
    assert!(
        output.stdout.is_empty(),
        "with -o nothing should be printed to stdout"
    );

    let _ = std::fs::remove_file(&destination);
}
