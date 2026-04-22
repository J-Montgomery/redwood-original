use redwood::datalog::parser;
use redwood::{datalog::Engine, runtime::prelude};
use std::fs;
use std::process::Command;
use std::time::Instant;
use tempfile::tempdir;

#[test]
fn parallel_build_faster_than_sequential() {
    let dir = tempdir().unwrap();
    let build_datalog = dir.path().join("BUILD.datalog");

    fs::write(
        &build_datalog,
        r#"
system_cc("//test:bin1").
sources("//test:bin1", "src1.c").
attr("//test:bin1", "-o", "{output}").

system_cc("//test:bin2").
sources("//test:bin2", "src2.c").
attr("//test:bin2", "-o", "{output}").

system_cc("//test:bin3").
sources("//test:bin3", "src3.c").
attr("//test:bin3", "-o", "{output}").
"#,
    )
    .unwrap();

    let src_template = r#"
int main() {
    volatile long x = 0;
    for (long i = 0; i < 50000000L; i++) {
        x += i;
    }
    return 0;
}
"#;

    fs::write(dir.path().join("src1.c"), src_template).unwrap();
    fs::write(dir.path().join("src2.c"), src_template).unwrap();
    fs::write(dir.path().join("src3.c"), src_template).unwrap();

    let binary = std::env::current_exe().unwrap();
    let redwood_bin = binary.parent().unwrap().join("redwood");

    if !redwood_bin.exists() {
        eprintln!("Skipping test: redwood binary not found");
        return;
    }

    let start = Instant::now();
    let output = Command::new(&redwood_bin)
        .current_dir(dir.path())
        .args(["build", "//test:bin1", "//test:bin2", "//test:bin3"])
        .output()
        .unwrap();
    let parallel_time = start.elapsed();

    assert!(
        output.status.success(),
        "Build failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    fs::remove_dir_all(dir.path().join("target")).ok();
    fs::remove_dir_all(dir.path().join(".redwood")).ok();

    let start = Instant::now();
    let output1 = Command::new(&redwood_bin)
        .current_dir(dir.path())
        .args(["build", "//test:bin1"])
        .output()
        .unwrap();
    assert!(output1.status.success());

    fs::remove_dir_all(dir.path().join("target")).ok();
    fs::remove_dir_all(dir.path().join(".redwood")).ok();

    let output2 = Command::new(&redwood_bin)
        .current_dir(dir.path())
        .args(["build", "//test:bin2"])
        .output()
        .unwrap();
    assert!(output2.status.success());

    fs::remove_dir_all(dir.path().join("target")).ok();
    fs::remove_dir_all(dir.path().join(".redwood")).ok();

    let output3 = Command::new(&redwood_bin)
        .current_dir(dir.path())
        .args(["build", "//test:bin3"])
        .output()
        .unwrap();
    assert!(output3.status.success());

    let sequential_time = start.elapsed();

    println!("Parallel time: {:?}", parallel_time);
    println!("Sequential time: {:?}", sequential_time);

    assert!(
        parallel_time < sequential_time,
        "Parallel build should be faster than sequential (parallel: {:?}, sequential: {:?})",
        parallel_time,
        sequential_time
    );
}

#[test]
fn build_levels_independent_targets() {
    let build_content = r#"
        system_cc("//app:a").
        sources("//app:a", "a.c").

        system_cc("//app:b").
        sources("//app:b", "b.c").

        system_cc("//app:c").
        sources("//app:c", "c.c").

        # Leaf targets have no dependencies
        leaf_target(Target) :- target(Target), not(deps(Target, _)).

        # All targets can build in parallel if no ordering constraints
        parallel_with(T1, T2) :-
            target(T1),
            target(T2),
            T1 != T2.
    "#;

    let mut db = Engine::new();
    let (facts, rules) = parser::parse_program(build_content).unwrap();
    db.insert_facts(facts);
    for rule in rules {
        db.compile_rule(rule);
    }

    let prelude_facts = prelude::get_prelude_facts();
    db.insert_facts(prelude_facts);
    let prelude_rules = prelude::get_prelude_rules();
    for rule in prelude_rules {
        db.compile_rule(rule);
    }

    let deps_facts = db.query("deps", &[]);
    assert!(
        deps_facts.is_empty(),
        "Independent targets should have no deps"
    );

    use redwood::datalog::Value;

    let leaves = db.query("leaf_target", &[]);
    let our_targets = ["//app:a", "//app:b", "//app:c"];
    let our_leaves: Vec<_> = leaves
        .iter()
        .filter(|f| {
            if let Some(Value::String(t)) = f.args.first() {
                our_targets.contains(&t.as_str())
            } else {
                false
            }
        })
        .collect();
    assert_eq!(our_leaves.len(), 3, "All 3 targets are leaves");

    let parallel = db.query("parallel_with", &[]);
    let our_parallel: Vec<_> = parallel
        .iter()
        .filter(|f| {
            if let (Some(Value::String(t1)), Some(Value::String(t2))) =
                (f.args.first(), f.args.get(1))
            {
                our_targets.contains(&t1.as_str()) && our_targets.contains(&t2.as_str())
            } else {
                false
            }
        })
        .collect();

    // Should have 6 parallel pairs: (a,b), (b,a), (a,c), (c,a), (b,c), (c,b)
    assert_eq!(our_parallel.len(), 6, "All targets can build in parallel");
}

#[test]
fn build_levels_with_dependencies() {
    let build_content = r#"
        system_cc("//app:final").
        sources("//app:final", "final.c").
        deps("//app:final", "//lib:b").
        deps("//app:final", "//lib:c").

        system_cc("//lib:b").
        sources("//lib:b", "b.c").
        deps("//lib:b", "//lib:a").

        system_cc("//lib:c").
        sources("//lib:c", "c.c").
        deps("//lib:c", "//lib:a").

        system_cc("//lib:a").
        sources("//lib:a", "a.c").

        # Leaf targets have no dependencies
        leaf_target(Target) :- target(Target), not(deps(Target, _)).

        # Build ordering: transitive closure of dependencies
        build_after(Target, Dep) :- deps(Target, Dep).
        build_after(Target, Dep) :- deps(Target, X), build_after(X, Dep).

        # Targets can build in parallel if there's no ordering constraint
        parallel_with(T1, T2) :-
            target(T1),
            target(T2),
            T1 != T2,
            not(build_after(T1, T2)),
            not(build_after(T2, T1)).
    "#;

    let mut db = Engine::new();
    let (facts, rules) = parser::parse_program(build_content).unwrap();
    db.insert_facts(facts);
    for rule in rules {
        db.compile_rule(rule);
    }

    let prelude_facts = prelude::get_prelude_facts();
    db.insert_facts(prelude_facts);
    let prelude_rules = prelude::get_prelude_rules();
    for rule in prelude_rules {
        db.compile_rule(rule);
    }

    let deps_facts = db.query("deps", &[]);
    assert_eq!(deps_facts.len(), 4, "Should have 4 dependency edges");

    use redwood::datalog::Value;

    let leaves = db.query("leaf_target", &[]);
    let our_targets = ["//lib:a", "//lib:b", "//lib:c", "//app:final"];
    let our_leaves: Vec<_> = leaves
        .iter()
        .filter(|f| {
            if let Some(Value::String(t)) = f.args.first() {
                our_targets.contains(&t.as_str())
            } else {
                false
            }
        })
        .collect();
    assert_eq!(our_leaves.len(), 1, "Only lib:a is a leaf");

    let ordering = db.query("build_after", &[]);

    let must_build_after = |target: &str, dep: &str| -> bool {
        ordering.iter().any(|f| {
            if let (Some(Value::String(t)), Some(Value::String(d))) =
                (f.args.first(), f.args.get(1))
            {
                t == target && d == dep
            } else {
                false
            }
        })
    };

    assert!(must_build_after("//lib:b", "//lib:a"), "b builds after a");
    assert!(must_build_after("//lib:c", "//lib:a"), "c builds after a");
    assert!(
        must_build_after("//app:final", "//lib:b"),
        "final builds after b"
    );
    assert!(
        must_build_after("//app:final", "//lib:c"),
        "final builds after c"
    );


    assert!(
        must_build_after("//app:final", "//lib:a"),
        "final transitively builds after a"
    );

    let parallel = db.query("parallel_with", &[]);

    let can_parallel = |t1: &str, t2: &str| -> bool {
        parallel.iter().any(|f| {
            if let (Some(Value::String(a)), Some(Value::String(b))) =
                (f.args.first(), f.args.get(1))
            {
                (a == t1 && b == t2) || (a == t2 && b == t1)
            } else {
                false
            }
        })
    };

    assert!(
        can_parallel("//lib:b", "//lib:c"),
        "b and c can build in parallel"
    );
    assert!(
        !can_parallel("//lib:a", "//lib:b"),
        "a must complete before b"
    );
    assert!(
        !can_parallel("//lib:a", "//app:final"),
        "a must complete before final"
    );
}
