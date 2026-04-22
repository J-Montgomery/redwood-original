use redwood::datalog::{parser, Value};

#[test]
fn namespace_rewriting_basic() {
    let content = r#"
        target("//foo:bar").
        deps("//foo:bar", "//baz:qux").
    "#;

    let (facts, _rules, _) =
        parser::parse_program_with_namespace(content, "test.datalog", "//external/boost").unwrap();

    assert_eq!(facts.len(), 2);

    let target_fact = &facts[0];
    assert_eq!(target_fact.predicate, "target");
    assert_eq!(
        target_fact.args[0],
        Value::String("//external/boost//foo:bar".to_string())
    );

    let deps_fact = &facts[1];
    assert_eq!(deps_fact.predicate, "deps");
    assert_eq!(
        deps_fact.args[0],
        Value::String("//external/boost//foo:bar".to_string())
    );
    assert_eq!(
        deps_fact.args[1],
        Value::String("//external/boost//baz:qux".to_string())
    );
}

#[test]
fn namespace_rewriting_main_workspace() {
    let content = r#"
        target("//foo:bar").
        deps("//foo:bar", "//baz:qux").
    "#;

    let (facts, _rules, _) =
        parser::parse_program_with_namespace(content, "test.datalog", "//").unwrap();

    assert_eq!(facts.len(), 2);

    let target_fact = &facts[0];
    assert_eq!(target_fact.args[0], Value::String("//foo:bar".to_string()));

    let deps_fact = &facts[1];
    assert_eq!(deps_fact.args[1], Value::String("//baz:qux".to_string()));
}

#[test]
fn namespace_rewriting_cross_namespace_refs() {
    let content = r#"
        target("//foo:bar").
        deps("//foo:bar", "//other/lib//baz:qux").
    "#;

    let (facts, _rules, _) =
        parser::parse_program_with_namespace(content, "test.datalog", "//external/boost").unwrap();

    assert_eq!(facts.len(), 2);

    let target_fact = &facts[0];
    assert_eq!(
        target_fact.args[0],
        Value::String("//external/boost//foo:bar".to_string())
    );

    let deps_fact = &facts[1];
    assert_eq!(
        deps_fact.args[0],
        Value::String("//external/boost//foo:bar".to_string())
    );
    assert_eq!(
        deps_fact.args[1],
        Value::String("//other/lib//baz:qux".to_string())
    );
}

#[test]
fn namespace_non_target_strings_not_rewritten() {
    let content = r#"
        attr("//foo:bar", "flag", "-O3").
        sources("//foo:bar", "src/main.rs").
    "#;

    let (facts, _rules, _) =
        parser::parse_program_with_namespace(content, "test.datalog", "//external/boost").unwrap();

    let attr_fact = &facts[0];
    assert_eq!(
        attr_fact.args[0],
        Value::String("//external/boost//foo:bar".to_string())
    );

    assert_eq!(attr_fact.args[2], Value::String("-O3".to_string()));

    let sources_fact = &facts[1];
    assert_eq!(
        sources_fact.args[1],
        Value::String("src/main.rs".to_string())
    );
}
