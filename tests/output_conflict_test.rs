use std::fs;
use std::process::Command;
use tempfile::tempdir;

#[test]
fn output_conflict_detected() {
    let dir = tempdir().unwrap();
    let build_file = dir.path().join("BUILD.datalog");

    fs::write(
        &build_file,
        r#"
system_cc("//app:a").
sources("//app:a", "a.c").
outputs("//app:a", "bin/output").

system_cc("//app:b").
sources("//app:b", "b.c").
outputs("//app:b", "bin/output").
"#,
    )
    .unwrap();

    fs::write(dir.path().join("a.c"), "int main() { return 0; }").unwrap();
    fs::write(dir.path().join("b.c"), "int main() { return 1; }").unwrap();

    let binary = std::env::current_exe().unwrap();
    let redwood_bin = binary.parent().unwrap().join("redwood");

    if !redwood_bin.exists() {
        eprintln!("Skipping test: redwood binary not found");
        return;
    }

    let output = Command::new(&redwood_bin)
        .current_dir(dir.path())
        .args(["build", "//app:a", "//app:b"])
        .output()
        .unwrap();

    assert!(
        !output.status.success(),
        "Build should fail with output conflict"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Output conflicts detected"),
        "stderr: {}",
        stderr
    );
    assert!(stderr.contains("bin/output"), "stderr: {}", stderr);
    assert!(stderr.contains("//app:a"), "stderr: {}", stderr);
    assert!(stderr.contains("//app:b"), "stderr: {}", stderr);
}

#[test]
fn no_conflict_different_outputs() {
    let dir = tempdir().unwrap();
    let build_file = dir.path().join("BUILD.datalog");

    fs::write(
        &build_file,
        r#"
system_cc("//app:a").
sources("//app:a", "a.c").
outputs("//app:a", "bin/a").

system_cc("//app:b").
sources("//app:b", "b.c").
outputs("//app:b", "bin/b").
"#,
    )
    .unwrap();

    fs::write(dir.path().join("a.c"), "int main() { return 0; }").unwrap();
    fs::write(dir.path().join("b.c"), "int main() { return 1; }").unwrap();

    let binary = std::env::current_exe().unwrap();
    let redwood_bin = binary.parent().unwrap().join("redwood");

    if !redwood_bin.exists() {
        eprintln!("Skipping test: redwood binary not found");
        return;
    }

    let output = Command::new(&redwood_bin)
        .current_dir(dir.path())
        .args(["build", "//app:a", "//app:b"])
        .output()
        .unwrap();

    if !output.status.success() {
        eprintln!("stdout: {}", String::from_utf8_lossy(&output.stdout));
        eprintln!("stderr: {}", String::from_utf8_lossy(&output.stderr));
    }

    assert!(
        output.status.success(),
        "Build should succeed with different outputs"
    );
}

#[test]
fn no_conflict_single_target() {
    let dir = tempdir().unwrap();
    let build_file = dir.path().join("BUILD.datalog");

    fs::write(
        &build_file,
        r#"
system_cc("//app:linux").
sources("//app:linux", "app.c").
outputs("//app:linux", "bin/app").

system_cc("//app:windows").
sources("//app:windows", "app.c").
outputs("//app:windows", "bin/app").
"#,
    )
    .unwrap();

    fs::write(dir.path().join("app.c"), "int main() { return 0; }").unwrap();

    let binary = std::env::current_exe().unwrap();
    let redwood_bin = binary.parent().unwrap().join("redwood");

    if !redwood_bin.exists() {
        eprintln!("Skipping test: redwood binary not found");
        return;
    }

    let output = Command::new(&redwood_bin)
        .current_dir(dir.path())
        .args(["build", "//app:linux"])
        .output()
        .unwrap();

    if !output.status.success() {
        eprintln!("stdout: {}", String::from_utf8_lossy(&output.stdout));
        eprintln!("stderr: {}", String::from_utf8_lossy(&output.stderr));
    }

    assert!(
        output.status.success(),
        "Build should succeed when only one target is built"
    );
}
