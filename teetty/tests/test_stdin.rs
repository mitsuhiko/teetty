use std::fs::{self, File};
use std::process::Command;

#[test]
fn test_stdin() {
    let tempdir = tempfile::tempdir().unwrap();
    let stdout = tempdir.path().join("stdout");
    let stderr = tempdir.path().join("stderr");
    let stdin = tempdir.path().join("stdin");
    let out = tempdir.path().join("out");
    fs::write(&stdin, "line1\nline2\n").unwrap();
    let status = Command::new(env!("CARGO_BIN_EXE_teetty"))
        .arg("--out")
        .arg(&out)
        .arg("--no-echo")
        .arg("--")
        .arg("tests/read.sh")
        .stdout(File::create(&stdout).unwrap())
        .stderr(File::create(&stderr).unwrap())
        .stdin(File::open(&stdin).unwrap())
        .status()
        .unwrap();
    let stdout = fs::read_to_string(&stdout).unwrap();
    let stderr = fs::read_to_string(&stderr).unwrap();
    let out = fs::read_to_string(&out).unwrap();

    assert_eq!(status.code(), Some(0));
    assert_eq!(stderr, "");
    assert_eq!(out, "BEGIN\r\n  line1\r\n  line2\r\nEND\r\n");
    assert_eq!(stdout, "BEGIN\r\n  line1\r\n  line2\r\nEND\r\n");
}

#[test]
fn test_stdin_script_mode() {
    let tempdir = tempfile::tempdir().unwrap();
    let stdout = tempdir.path().join("stdout");
    let stderr = tempdir.path().join("stderr");
    let stdin = tempdir.path().join("stdin");
    let out = tempdir.path().join("out");
    fs::write(&stdin, "line1\nline2\n").unwrap();
    let status = Command::new(env!("CARGO_BIN_EXE_teetty"))
        .arg("--out")
        .arg(&out)
        .arg("--script-mode")
        .arg("--")
        .arg("tests/read.sh")
        .stdout(File::create(&stdout).unwrap())
        .stderr(File::create(&stderr).unwrap())
        .stdin(File::open(&stdin).unwrap())
        .status()
        .unwrap();
    let stdout = fs::read_to_string(&stdout).unwrap();
    let stderr = fs::read_to_string(&stderr).unwrap();
    let out = fs::read_to_string(&out).unwrap();

    assert_eq!(status.code(), Some(0));
    assert_eq!(stderr, "");
    assert_eq!(out, "BEGIN\n  line1\n  line2\nEND\n");
    assert_eq!(stdout, "BEGIN\n  line1\n  line2\nEND\n");
}
