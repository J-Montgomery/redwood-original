use redwood::datalog::{parser, Value};

/// Test package-local label rewriting for recursive builds.
/// When a package declares "//:target", it should become "//pkg:target"
#[test]
fn package_local_label_rewriting() {
    let content = r#"
        target("//:lib").
        deps("//:lib", "//:util").
    "#;

    let (facts, _rules, _) =
        parser::parse_program_with_namespace(content, "pkg/BUILD.datalog", "//pkg").unwrap();

    assert_eq!(facts.len(), 2);

    let target_fact = &facts[0];
    assert_eq!(target_fact.predicate, "target");
    // "//:lib" should become "//pkg:lib"
    assert_eq!(
        target_fact.args[0],
        Value::String("//pkg:lib".to_string())
    );

    let deps_fact = &facts[1];
    assert_eq!(
        deps_fact.args[0],
        Value::String("//pkg:lib".to_string())
    );
    assert_eq!(
        deps_fact.args[1],
        Value::String("//pkg:util".to_string())
    );
}

/// Test that absolute references aren't rewritten in package namespaces.
#[test]
fn package_absolute_ref_not_rewritten() {
    let content = r#"
        target("//:lib").
        deps("//:lib", "//other:target").
    "#;

    let (facts, _rules, _) =
        parser::parse_program_with_namespace(content, "pkg/BUILD.datalog", "//pkg").unwrap();

    let deps_fact = &facts[1];
    assert_eq!(
        deps_fact.args[0],
        Value::String("//pkg:lib".to_string())
    );
    assert_eq!(
        deps_fact.args[1],
        Value::String("//other:target".to_string())
    );
}

#[test]
fn external_namespace_rewriting_unchanged() {
    let content = r#"
        target("//foo:bar").
        deps("//foo:bar", "//baz:qux").
    "#;

    let (facts, _rules, _) =
        parser::parse_program_with_namespace(content, "test.datalog", "//external/boost").unwrap();

    let target_fact = &facts[0];
    // External namespace should use double-// style
    assert_eq!(
        target_fact.args[0],
        Value::String("//external/boost//foo:bar".to_string())
    );

    let deps_fact = &facts[1];
    assert_eq!(
        deps_fact.args[1],
        Value::String("//external/boost//baz:qux".to_string())
    );
}

#[test]
fn nested_package_namespace() {
    let content = r#"
        target("//foo:bar").
    "#;

    let (facts, _rules, _) =
        parser::parse_program_with_namespace(content, "src/lib/BUILD.datalog", "//src/lib").unwrap();

    let target_fact = &facts[0];
    // Nested packages use double-// style like external namespaces
    assert_eq!(
        target_fact.args[0],
        Value::String("//src/lib//foo:bar".to_string())
    );
}

#[test]
fn nested_package_local_ref() {
    let content = r#"
        target("//:lib").
    "#;

    let (facts, _rules, _) =
        parser::parse_program_with_namespace(content, "src/lib/BUILD.datalog", "//src/lib").unwrap();

    let target_fact = &facts[0];
    assert_eq!(
        target_fact.args[0],
        Value::String("//src/lib:lib".to_string())
    );
}
