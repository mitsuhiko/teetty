use std::fs::{self, File};
use std::process::Command;

#[test]
fn test_basic() {
    let tempdir = tempfile::tempdir().unwrap();
    let stdout = tempdir.path().join("stdout");
    let stderr = tempdir.path().join("stderr");
    let out = tempdir.path().join("out");
    fs::write(&out, "before\n").unwrap();
    let status = Command::new(env!("CARGO_BIN_EXE_teetty"))
        .arg("--out")
        .arg(&out)
        .arg("--")
        .arg("tests/basic.sh")
        .stdout(File::create(&stdout).unwrap())
        .stderr(File::create(&stderr).unwrap())
        .status()
        .unwrap();
    let stdout = fs::read_to_string(&stdout).unwrap();
    let stderr = fs::read_to_string(&stderr).unwrap();
    let out = fs::read_to_string(&out).unwrap();

    assert_eq!(status.code(), Some(42));

    insta::assert_snapshot!(&stdout, @r###"
    stdout output
    stderr output
    stdin: tty
    stdout: tty
    stderr: tty
    "###);
    insta::assert_snapshot!(&stderr, @"");
    insta::assert_snapshot!(&out, @r###"
    before
    stdout output
    stderr output
    stdin: tty
    stdout: tty
    stderr: tty
    "###);
}

#[test]
fn test_basic_truncate() {
    let tempdir = tempfile::tempdir().unwrap();
    let stdout = tempdir.path().join("stdout");
    let stderr = tempdir.path().join("stderr");
    let out = tempdir.path().join("out");
    fs::write(&out, "before\n").unwrap();
    let status = Command::new(env!("CARGO_BIN_EXE_teetty"))
        .arg("--out")
        .arg(&out)
        .arg("--truncate")
        .arg("--")
        .arg("tests/basic.sh")
        .stdout(File::create(&stdout).unwrap())
        .stderr(File::create(&stderr).unwrap())
        .status()
        .unwrap();
    let stdout = fs::read_to_string(&stdout).unwrap();
    let stderr = fs::read_to_string(&stderr).unwrap();
    let out = fs::read_to_string(&out).unwrap();

    dbg!(&stdout);
    dbg!(&stderr);
    assert_eq!(status.code(), Some(42));

    insta::assert_snapshot!(&stdout, @r###"
    stdout output
    stderr output
    stdin: tty
    stdout: tty
    stderr: tty
    "###);
    insta::assert_snapshot!(&stderr, @"");
    insta::assert_snapshot!(&out, @r###"
    stdout output
    stderr output
    stdin: tty
    stdout: tty
    stderr: tty
    "###);
}
