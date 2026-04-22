use std::process::Command;

#[test]
fn non_stratified_detected_order_p_then_q() {
    let output = Command::new("target/debug/test_stratification_order_p_q")
        .output()
        .expect("Failed to run test binary");

    assert!(
        !output.status.success(),
        "Expected program to exit with error when rules compiled in order p, q"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Non-stratified negation"));
}

#[test]
fn non_stratified_detected_order_q_then_p() {
    let output = Command::new("target/debug/test_stratification_order_q_p")
        .output()
        .expect("Failed to run test binary");

    assert!(
        !output.status.success(),
        "Expected program to exit with error when rules compiled in order q, p"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Non-stratified negation"));
}
