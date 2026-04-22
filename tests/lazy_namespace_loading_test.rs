use redwood::datalog::{parser, Engine};
use std::fs;
use tempfile::TempDir;

#[test]
fn lazy_namespace_loading_integration() {
    let temp_dir = TempDir::new().unwrap();
    let workspace = temp_dir.path();

    // Create main BUILD.datalog
    let main_build = workspace.join("BUILD.datalog");
    fs::write(
        &main_build,
        r#"
root("//", ".").
root("//external/boost", ".external/boost").

target("//app:server").
cargo_binary("//app:server").
deps("//app:server", "//external/boost//ranges:lib").
    "#,
    )
    .unwrap();

    // Create external/boost directory and BUILD.datalog
    let boost_dir = workspace.join(".external/boost");
    fs::create_dir_all(&boost_dir).unwrap();
    let boost_build = boost_dir.join("BUILD.datalog");
    fs::write(
        &boost_build,
        r#"
target("//ranges:lib").
cargo_lib("//ranges:lib").
sources("//ranges:lib", "src/ranges.rs").
    "#,
    )
    .unwrap();

    // Simulate the loading process
    let mut engine = Engine::new();

    // Load main workspace
    let main_content = fs::read_to_string(&main_build).unwrap();
    let (facts, rules, _) =
        parser::parse_program_with_namespace(&main_content, "BUILD.datalog", "//").unwrap();

    engine.insert_facts(facts);
    for rule in rules {
        engine.compile_rule(rule);
    }

    // At this point, boost is NOT loaded yet
    // Query for boost target should return nothing
    let boost_targets_before = engine.query("target", &[]);
    let boost_count_before = boost_targets_before
        .iter()
        .filter(|f| f.args[0].as_string().unwrap().contains("boost"))
        .count();
    assert_eq!(
        boost_count_before, 0,
        "Boost namespace should not be loaded yet"
    );

    // Now load boost namespace (simulating lazy loading)
    let boost_content = fs::read_to_string(&boost_build).unwrap();
    let (boost_facts, boost_rules, _) = parser::parse_program_with_namespace(
        &boost_content,
        "boost/BUILD.datalog",
        "//external/boost",
    )
    .unwrap();

    engine.insert_facts(boost_facts);
    for rule in boost_rules {
        engine.compile_rule(rule);
    }

    // Now boost targets should be visible with rewritten labels
    let all_targets = engine.query("target", &[]);

    // Check main workspace target
    let main_targets: Vec<_> = all_targets
        .iter()
        .filter(|f| {
            let label = f.args[0].as_string().unwrap();
            label == "//app:server"
        })
        .collect();
    assert_eq!(main_targets.len(), 1, "Main workspace target should exist");

    // Check boost target with rewritten namespace
    let boost_targets: Vec<_> = all_targets
        .iter()
        .filter(|f| {
            let label = f.args[0].as_string().unwrap();
            label == "//external/boost//ranges:lib"
        })
        .collect();
    assert_eq!(
        boost_targets.len(),
        1,
        "Boost target should have rewritten namespace"
    );

    // Verify dependency uses fully qualified name
    let deps = engine.query("deps", &[Some("//app:server")]);
    assert_eq!(deps.len(), 1);
    assert_eq!(
        deps[0].args[1].as_string().unwrap(),
        "//external/boost//ranges:lib"
    );
}

#[test]
fn namespace_extraction() {
    // Helper to extract namespace (mimics the function in cli/mod.rs)
    fn extract_namespace(target_label: &str) -> String {
        let double_slash_count = target_label.matches("//").count();
        if double_slash_count > 1 {
            let parts: Vec<_> = target_label.split("//").collect();
            if parts.len() >= 2 {
                format!("//{}", parts[1])
            } else {
                "//".to_string()
            }
        } else {
            "//".to_string()
        }
    }

    assert_eq!(extract_namespace("//app:server"), "//");
    assert_eq!(
        extract_namespace("//external/boost//ranges:lib"),
        "//external/boost"
    );
    assert_eq!(
        extract_namespace("//internal/auth//core:jwt"),
        "//internal/auth"
    );
}
