use std::fs::{self, File};
use std::process::{Command, ExitStatus};

use tempfile::TempDir;

struct Output {
    stdout: String,
    stderr: String,
    out: String,
    status: ExitStatus,
}

fn run_teetty(tempdir: &TempDir, args: &[&str]) -> Output {
    let stdout = tempdir.path().join("stdout");
    let stderr = tempdir.path().join("stderr");
    let out = tempdir.path().join("out");
    let status = Command::new(env!("CARGO_BIN_EXE_teetty"))
        .arg("--out")
        .arg(&out)
        .args(args)
        .arg("--")
        .arg("tests/basic.sh")
        .stdout(File::create(&stdout).unwrap())
        .stderr(File::create(&stderr).unwrap())
        .status()
        .unwrap();
    Output {
        stdout: fs::read_to_string(&stdout).unwrap(),
        stderr: fs::read_to_string(&stdout).unwrap(),
        out: fs::read_to_string(&out).unwrap(),
        status,
    }
}

#[test]
fn test_basic() {
    let tempdir = tempfile::tempdir().unwrap();
    let out = run_teetty(&tempdir, &[]);
    assert_eq!(out.status.code(), Some(42));

    insta::assert_snapshot!(&out.stdout, @r###"
    stdout output
    stderr output
    stdin: tty
    stdout: tty
    stderr: tty
    "###);
    insta::assert_snapshot!(&out.stderr, @r###"
    stdout output
    stderr output
    stdin: tty
    stdout: tty
    stderr: tty
    "###);
    insta::assert_snapshot!(&out.out, @r###"
    stdout output
    stderr output
    stdin: tty
    stdout: tty
    stderr: tty
    "###);
}
