//! Binary-level smoke test for the complete Runner path.

use std::{path::PathBuf, process::Command};

#[test]
fn returns_the_guest_exit_process_code() {
    let fixture =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/guest-programs/exit42.exe");
    let output = Command::new(env!("CARGO_BIN_EXE_vnrt-runner"))
        .arg(fixture)
        .arg("--max-instructions")
        .arg("16384")
        .output()
        .expect("runner binary should start");

    assert_eq!(output.status.code(), Some(42));
    assert!(String::from_utf8_lossy(&output.stdout).contains("guest-ok\n"));
}

#[test]
fn reports_machine_state_when_execution_fails() {
    let fixture =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/guest-programs/exit42.exe");
    let output = Command::new(env!("CARGO_BIN_EXE_vnrt-runner"))
        .arg(fixture)
        .arg("--max-instructions")
        .arg("1")
        .output()
        .expect("runner binary should start");
    let diagnostics = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    assert!(!output.status.success());
    assert!(diagnostics.contains("execution limit of 1 steps reached"));
    assert!(diagnostics.contains("registers="));
    assert!(diagnostics.contains("EIP"));
    assert!(diagnostics.contains("instruction_bytes="));
    assert!(diagnostics.contains("stack_words="));
    assert!(diagnostics.contains("recent_host_calls=[]"));
}
