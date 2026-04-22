use redwood::datalog::{parser, Engine};
use redwood::runtime::prelude;

fn setup_engine_with_prelude() -> Engine {
    let mut db = Engine::new();
    let (prelude_facts, prelude_rules, _) = prelude::get_prelude_with_locations();
    db.insert_facts(prelude_facts);
    for rule in prelude_rules {
        db.compile_rule(rule);
    }
    db
}

#[test]
fn proprietary_cannot_depend_on_gpl() {
    let mut db = setup_engine_with_prelude();

    let program = r#"
        target("//app:proprietary").
        license("//app:proprietary", "Proprietary").
        deps("//app:proprietary", "//lib:gpl").

        target("//lib:gpl").
        license("//lib:gpl", "GPL-3.0").
    "#;

    let (facts, rules, _) = parser::parse_program_with_file(program, "test.datalog").unwrap();
    db.insert_facts(facts);
    for rule in rules {
        db.compile_rule(rule);
    }

    let constraint_failures = db.query("constraint_failed", &[Some("//app:proprietary")]);

    assert!(
        !constraint_failures.is_empty(),
        "Should have constraint failure"
    );

    let failure_message = constraint_failures[0].args[1].as_string().unwrap();
    assert!(
        failure_message.contains("GPL") || failure_message.contains("proprietary"),
        "Failure message should mention GPL/proprietary conflict: {}",
        failure_message
    );
}

#[test]
fn mit_can_depend_on_mit() {
    let mut db = setup_engine_with_prelude();

    let program = r#"
        target("//app:main").
        license("//app:main", "MIT").
        deps("//app:main", "//lib:helper").

        target("//lib:helper").
        license("//lib:helper", "MIT").
    "#;

    let (facts, rules, _) = parser::parse_program_with_file(program, "test.datalog").unwrap();
    db.insert_facts(facts);
    for rule in rules {
        db.compile_rule(rule);
    }

    let constraint_failures = db.query("constraint_failed", &[Some("//app:main")]);
    assert!(
        constraint_failures.is_empty(),
        "MIT should be compatible with MIT"
    );
}

#[test]
fn gpl_can_depend_on_mit() {
    let mut db = setup_engine_with_prelude();

    let program = r#"
        target("//app:gpl").
        license("//app:gpl", "GPL-3.0").
        deps("//app:gpl", "//lib:mit").

        target("//lib:mit").
        license("//lib:mit", "MIT").
    "#;

    let (facts, rules, _) = parser::parse_program_with_file(program, "test.datalog").unwrap();
    db.insert_facts(facts);
    for rule in rules {
        db.compile_rule(rule);
    }

    let constraint_failures = db.query("constraint_failed", &[Some("//app:gpl")]);
    assert!(
        constraint_failures.is_empty(),
        "GPL can incorporate permissive licenses"
    );
}

#[test]
fn gpl2_and_gpl3_incompatible() {
    let mut db = setup_engine_with_prelude();

    let program = r#"
        target("//app:mixed").
        license("//app:mixed", "GPL-3.0").
        deps("//app:mixed", "//lib:gpl2").
        deps("//app:mixed", "//lib:gpl3").

        target("//lib:gpl2").
        license("//lib:gpl2", "GPL-2.0").

        target("//lib:gpl3").
        license("//lib:gpl3", "GPL-3.0").
    "#;

    let (facts, rules, _) = parser::parse_program_with_file(program, "test.datalog").unwrap();
    db.insert_facts(facts);
    for rule in rules {
        db.compile_rule(rule);
    }

    let constraint_failures = db.query("constraint_failed", &[Some("//app:mixed")]);

    assert!(
        !constraint_failures.is_empty(),
        "Should detect GPL-2.0/GPL-3.0 incompatibility"
    );

    let failure_message = constraint_failures[0].args[1].as_string().unwrap();
    assert!(
        failure_message.contains("GPL-2.0") && failure_message.contains("GPL-3.0"),
        "Should mention both GPL versions: {}",
        failure_message
    );
}

#[test]
fn proprietary_with_lgpl_requires_exception() {
    let mut db = setup_engine_with_prelude();

    let program = r#"
        target("//app:proprietary").
        license("//app:proprietary", "Proprietary").
        deps("//app:proprietary", "//lib:lgpl").

        target("//lib:lgpl").
        license("//lib:lgpl", "LGPL-3.0").
    "#;

    let (facts, rules, _) = parser::parse_program_with_file(program, "test.datalog").unwrap();
    db.insert_facts(facts);
    for rule in rules {
        db.compile_rule(rule);
    }

    let constraint_failures = db.query("constraint_failed", &[Some("//app:proprietary")]);

    assert!(
        !constraint_failures.is_empty(),
        "Should require license exception for LGPL"
    );

    let failure_message = constraint_failures[0].args[1].as_string().unwrap();
    assert!(
        failure_message.contains("LGPL") || failure_message.contains("license_exception"),
        "Should mention LGPL or exception: {}",
        failure_message
    );
}

#[test]
fn proprietary_with_lgpl_exception_allowed() {
    let mut db = setup_engine_with_prelude();

    let program = r#"
        target("//app:proprietary").
        license("//app:proprietary", "Proprietary").
        deps("//app:proprietary", "//lib:lgpl").
        license_exception("//app:proprietary", "LGPL-3.0").

        target("//lib:lgpl").
        license("//lib:lgpl", "LGPL-3.0").
    "#;

    let (facts, rules, _) = parser::parse_program_with_file(program, "test.datalog").unwrap();
    db.insert_facts(facts);
    for rule in rules {
        db.compile_rule(rule);
    }

    let constraint_failures = db.query("constraint_failed", &[Some("//app:proprietary")]);
    assert!(
        constraint_failures.is_empty(),
        "Exception should allow LGPL with proprietary"
    );
}

#[test]
fn transitive_gpl_dependency_detected() {
    let mut db = setup_engine_with_prelude();

    let program = r#"
        target("//app:proprietary").
        license("//app:proprietary", "Proprietary").
        deps("//app:proprietary", "//lib:wrapper").

        target("//lib:wrapper").
        license("//lib:wrapper", "MIT").
        deps("//lib:wrapper", "//lib:gpl").

        target("//lib:gpl").
        license("//lib:gpl", "GPL-3.0").
    "#;

    let (facts, rules, _) = parser::parse_program_with_file(program, "test.datalog").unwrap();
    db.insert_facts(facts);
    for rule in rules {
        db.compile_rule(rule);
    }

    let constraint_failures = db.query("constraint_failed", &[Some("//app:proprietary")]);

    assert!(
        !constraint_failures.is_empty(),
        "Should detect transitive GPL dependency"
    );
}

#[test]
fn unlicensed_target_fails() {
    let mut db = setup_engine_with_prelude();

    let program = r#"
        target("//app:unlicensed").
    "#;

    let (facts, rules, _) = parser::parse_program_with_file(program, "test.datalog").unwrap();
    db.insert_facts(facts);
    for rule in rules {
        db.compile_rule(rule);
    }

    let constraint_failures = db.query("constraint_failed", &[Some("//app:unlicensed")]);

    assert!(
        !constraint_failures.is_empty(),
        "Should require license declaration"
    );

    let failure_message = constraint_failures[0].args[1].as_string().unwrap();
    assert!(
        failure_message.contains("license"),
        "Should mention missing license: {}",
        failure_message
    );
}

#[test]
fn unlicensed_with_exemption_allowed() {
    let mut db = setup_engine_with_prelude();

    let program = r#"
        target("//test:fixture").
        license_exempt("//test:fixture").
    "#;

    let (facts, rules, _) = parser::parse_program_with_file(program, "test.datalog").unwrap();
    db.insert_facts(facts);
    for rule in rules {
        db.compile_rule(rule);
    }

    let constraint_failures = db.query("constraint_failed", &[Some("//test:fixture")]);
    assert!(
        constraint_failures.is_empty(),
        "Exemption should allow unlicensed target"
    );
}

#[test]
fn apache_and_mit_compatible() {
    let mut db = setup_engine_with_prelude();

    let program = r#"
        target("//app:main").
        license("//app:main", "Apache-2.0").
        deps("//app:main", "//lib:mit").
        deps("//app:main", "//lib:bsd").

        target("//lib:mit").
        license("//lib:mit", "MIT").

        target("//lib:bsd").
        license("//lib:bsd", "BSD-3-Clause").
    "#;

    let (facts, rules, _) = parser::parse_program_with_file(program, "test.datalog").unwrap();
    db.insert_facts(facts);
    for rule in rules {
        db.compile_rule(rule);
    }

    let constraint_failures = db.query("constraint_failed", &[Some("//app:main")]);
    assert!(
        constraint_failures.is_empty(),
        "Permissive licenses should be compatible"
    );
}
