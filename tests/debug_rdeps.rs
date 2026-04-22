use redwood::datalog::{parser, Engine};
use redwood::runtime::prelude;

#[test]
fn debug_all_rdeps_fixpoint() {
    let program = r#"
        deps("C", "B").
        deps("B", "A").
    "#;

    let mut db = Engine::new();
    let (facts, rules) = parser::parse_program(program).unwrap();
    db.insert_facts(facts);
    for rule in rules {
        db.compile_rule(rule);
    }

    let prelude_rules = prelude::get_prelude_rules();
    for rule in prelude_rules {
        db.compile_rule(rule);
    }

    let rdeps_all = db.query("rdeps", &[]);
    eprintln!("rdeps facts:");
    for r in &rdeps_all {
        eprintln!("  {:?}", r);
    }

    let all_rdeps_all = db.query("all_rdeps", &[]);
    eprintln!("\nall_rdeps facts:");
    for r in &all_rdeps_all {
        eprintln!("  {:?}", r);
    }

    assert_eq!(rdeps_all.len(), 2, "Should have 2 rdeps facts");
    assert_eq!(
        all_rdeps_all.len(),
        3,
        "Should have 3 all_rdeps facts (including transitive)"
    );
}
