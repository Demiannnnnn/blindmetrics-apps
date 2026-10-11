#[test]
fn test_headless_flag_prohibited() {
    let exe = env!("CARGO_BIN_EXE_telemetry-agent");
    let output = std::process::Command::new(exe)
        .arg("--headless")
        .output()
        .expect("execute telemetry-agent --headless");
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Error: El modo --headless sin bandeja está prohibido (PROC-02)"),
        "Unexpected stderr: {}",
        stderr
    );
}
