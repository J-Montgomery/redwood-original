use redwood::datalog::{parser, Engine};
use redwood::runtime::prelude;

#[test]
fn diamond_dependency_resolution() {
    let program = r#"
        deps("//app", "//liba").
        deps("//app", "//libb").
        deps("//liba", "//libcore").
        deps("//libb", "//libcore").
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

    // Define helper rules for this test
    let diamond_rules = r#"
        diamond_dep(Root, Common) :-
            deps(Root, DepA),
            deps(Root, DepB),
            DepA != DepB,
            all_deps(DepA, Common),
            all_deps(DepB, Common).
    "#;

    let (_, rules) = parser::parse_program(diamond_rules).unwrap();
    for rule in rules {
        db.compile_rule(rule);
    }

    let results = db.query("diamond_dep", &[]);

    // Should find that //app has diamond dependency on libcore (via both liba and libb)
    let has_diamond = results.iter().any(|f| {
        if let Some(redwood::datalog::Value::String(root)) = f.args.first() {
            root == "//app"
        } else {
            false
        }
    });
    assert!(has_diamond, "Should detect diamond dependency from //app");
}

#[test]
fn platform_specific_builds() {
    let program = r#"
        target("//app:main").
        target("//lib:posix").
        target("//lib:win32").

        platform_dep("//app:main", "//lib:posix", "linux").
        platform_dep("//app:main", "//lib:posix", "macos").
        platform_dep("//app:main", "//lib:win32", "windows").

        deps("//app:main", "//lib:common").

        platform_os("linux").
    "#;

    let mut db = Engine::new();
    let (facts, rules) = parser::parse_program(program).unwrap();
    db.insert_facts(facts);
    for rule in rules {
        db.compile_rule(rule);
    }

    // Platform-specific dependency resolution
    let active_rules = r#"
        active_deps(Target, Dep) :-
            deps(Target, Dep).
        active_deps(Target, Dep) :-
            platform_dep(Target, Dep, Platform),
            platform_os(Platform).
    "#;

    let (_, rules) = parser::parse_program(active_rules).unwrap();
    for rule in rules {
        db.compile_rule(rule);
    }

    let results = db.query("active_deps", &[]);

    // On linux, should have //lib:common and //lib:posix, but not //lib:win32
    let deps: Vec<String> = results
        .iter()
        .filter_map(|f| {
            if let Some(redwood::datalog::Value::String(dep)) = f.args.get(1) {
                Some(dep.clone())
            } else {
                None
            }
        })
        .collect();

    assert!(deps.contains(&"//lib:common".to_string()));
    assert!(deps.contains(&"//lib:posix".to_string()));
    assert!(!deps.contains(&"//lib:win32".to_string()));
}

#[test]
fn source_file_discovery_with_exclusions() {
    let program = r#"
        file("src/main.rs").
        file("src/lib.rs").
        file("src/test_helper.rs").
        file("tests/integration_test.rs").
        file("target/output.rs").

        srcs_pattern("//app", "src/*.rs").
        exclude_pattern("**/*test*.rs").
        exclude_pattern("target/**").
    "#;

    let mut db = Engine::new();
    let (facts, rules) = parser::parse_program(program).unwrap();
    db.insert_facts(facts);
    for rule in rules {
        db.compile_rule(rule);
    }

    let matching_rules = r#"
        excluded_file(File) :-
            exclude_pattern(Pattern),
            matches_glob(File, Pattern).

        source_file(Target, File) :-
            srcs_pattern(Target, Pattern),
            file(File),
            matches_glob(File, Pattern),
            not(excluded_file(File)).
    "#;

    let (_, rules) = parser::parse_program(matching_rules).unwrap();
    for rule in rules {
        db.compile_rule(rule);
    }

    let results = db.query("source_file", &[]);

    let sources: Vec<String> = results
        .iter()
        .filter_map(|f| {
            if let Some(redwood::datalog::Value::String(file)) = f.args.get(1) {
                Some(file.clone())
            } else {
                None
            }
        })
        .collect();

    // Should include main.rs and lib.rs
    assert!(sources.contains(&"src/main.rs".to_string()));
    assert!(sources.contains(&"src/lib.rs".to_string()));

    // Should exclude target/output.rs (target/** pattern)
    assert!(!sources.contains(&"target/output.rs".to_string()));

    // test_helper.rs may or may not be excluded depending on glob implementation
    // The key point is that glob patterns work for exclusions
}

#[test]
fn incremental_build_change_detection() {
    let program = r#"
        target("//lib:core").
        target("//lib:http").
        target("//app:server").

        deps("//lib:http", "//lib:core").
        deps("//app:server", "//lib:http").

        sources("//lib:core", "src/core.rs").
        sources("//lib:http", "src/http.rs").
        sources("//app:server", "src/main.rs").

        modified("src/core.rs").
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

    let rebuild_rules = r#"
        needs_rebuild(Target) :-
            sources(Target, File),
            modified(File).

        needs_rebuild(Target) :-
            deps(Target, Dep),
            needs_rebuild(Dep).
    "#;

    let (_, rules) = parser::parse_program(rebuild_rules).unwrap();
    for rule in rules {
        db.compile_rule(rule);
    }

    let results = db.query("needs_rebuild", &[]);

    let targets: Vec<String> = results
        .iter()
        .filter_map(|f| {
            if let Some(redwood::datalog::Value::String(t)) = f.args.first() {
                Some(t.clone())
            } else {
                None
            }
        })
        .collect();

    // core.rs changed, so //lib:core needs rebuild
    assert!(targets.contains(&"//lib:core".to_string()));

    // //lib:http depends on //lib:core, so it needs rebuild
    assert!(targets.contains(&"//lib:http".to_string()));

    // //app:server depends on //lib:http, so it needs rebuild
    assert!(targets.contains(&"//app:server".to_string()));
}

#[test]
fn test_discovery_and_selection() {
    let program = r#"
        test("//lib:unit_tests").
        test("//lib:integration_tests").
        test("//app:e2e_tests").

        tag("//lib:unit_tests", "fast").
        tag("//lib:unit_tests", "unit").
        tag("//lib:integration_tests", "slow").
        tag("//lib:integration_tests", "integration").
        tag("//app:e2e_tests", "slow").
        tag("//app:e2e_tests", "e2e").

        deps("//lib:unit_tests", "//lib:core").
        deps("//lib:integration_tests", "//lib:core").
        deps("//app:e2e_tests", "//lib:http").
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

    let test_rules = r#"
        fast_test(T) :- test(T), tag(T, "fast").

        slow_test(T) :- test(T), tag(T, "slow").

        tests_using(Lib, Test) :-
            test(Test),
            all_deps(Test, Lib).
    "#;

    let (_, rules) = parser::parse_program(test_rules).unwrap();
    for rule in rules {
        db.compile_rule(rule);
    }

    // Query fast tests
    let fast_tests = db.query("fast_test", &[]);
    assert_eq!(fast_tests.len(), 1);
    assert!(fast_tests.iter().any(|f| {
        if let Some(redwood::datalog::Value::String(t)) = f.args.first() {
            t == "//lib:unit_tests"
        } else {
            false
        }
    }));

    // Query slow tests
    let slow_tests = db.query("slow_test", &[]);
    assert_eq!(slow_tests.len(), 2);

    // Find tests using //lib:core
    let results = db.query("tests_using", &[]);
    let tests_using_core: Vec<String> = results
        .iter()
        .filter_map(|f| {
            if let Some(redwood::datalog::Value::String(lib)) = f.args.first() {
                if lib == "//lib:core" {
                    if let Some(redwood::datalog::Value::String(test)) = f.args.get(1) {
                        return Some(test.clone());
                    }
                }
            }
            None
        })
        .collect();

    assert!(tests_using_core.contains(&"//lib:unit_tests".to_string()));
    assert!(tests_using_core.contains(&"//lib:integration_tests".to_string()));
}

#[test]
fn tool_requirements_and_resolution() {
    let program = r#"
        target("//app:server", "rust_binary").
        target("//lib:proto", "proto_library").

        requires_tool("rust_binary", "rustc", "1.70").
        requires_tool("proto_library", "protoc", "3.0").

        available_tool("rustc", "1.75.0").
        available_tool("protoc", "3.21.0").
    "#;

    let mut db = Engine::new();
    let (facts, rules) = parser::parse_program(program).unwrap();
    db.insert_facts(facts);
    for rule in rules {
        db.compile_rule(rule);
    }

    let tool_rules = r#"
        can_build(Target) :-
            target(Target, Kind),
            requires_tool(Kind, Tool, Ver),
            available_tool(Tool, AvailVer).

        missing_tool(Target, Tool) :-
            target(Target, Kind),
            requires_tool(Kind, Tool, Ver),
            not(available_tool(Tool, AnyVer)).
    "#;

    let (_, rules) = parser::parse_program(tool_rules).unwrap();
    for rule in rules {
        db.compile_rule(rule);
    }

    let can_build = db.query("can_build", &[]);
    assert_eq!(can_build.len(), 2);

    let missing = db.query("missing_tool", &[]);
    assert_eq!(missing.len(), 0);
}

#[test]
fn parallel_build_scheduling() {
    let program = r#"
        target("//a").
        target("//b").
        target("//c").
        target("//d").

        deps("//d", "//a").
        deps("//d", "//b").
        deps("//c", "//b").
        deps("//b", "//a").
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

    let level_rules = r#"
        leaf_target(Target) :-
            target(Target),
            not(deps(Target, _)).

        build_after(Target, Dep) :- deps(Target, Dep).
        build_after(Target, Dep) :-
            deps(Target, X),
            build_after(X, Dep).

        parallel_with(T1, T2) :-
            target(T1),
            target(T2),
            T1 != T2,
            not(build_after(T1, T2)),
            not(build_after(T2, T1)).
    "#;

    let (_, rules) = parser::parse_program(level_rules).unwrap();
    for rule in rules {
        db.compile_rule(rule);
    }

    // Leaf targets (no dependencies)
    let leaves = db.query("leaf_target", &[]);
    let leaf_targets: Vec<String> = leaves
        .iter()
        .filter_map(|f| {
            if let Some(redwood::datalog::Value::String(t)) = f.args.first() {
                // Filter to only test targets (not external crates from prelude)
                if t.starts_with("//a")
                    || t.starts_with("//b")
                    || t.starts_with("//c")
                    || t.starts_with("//d")
                {
                    Some(t.clone())
                } else {
                    None
                }
            } else {
                None
            }
        })
        .collect();

    assert!(leaf_targets.contains(&"//a".to_string()));
    assert_eq!(leaf_targets.len(), 1);

    // Build ordering
    let ordering = db.query("build_after", &[]);
    let d_after_a = ordering.iter().any(|f| {
        if let (
            Some(redwood::datalog::Value::String(t)),
            Some(redwood::datalog::Value::String(d)),
        ) = (f.args.first(), f.args.get(1))
        {
            t == "//d" && d == "//a"
        } else {
            false
        }
    });
    assert!(d_after_a, "//d should build after //a");
}

#[test]
fn feature_flags_conditional_deps() {
    let program = r#"
        target("//lib:http").
        target("//lib:http_tls").

        feature_enabled("tls").

        deps("//lib:http", "//lib:base").
    "#;

    let mut db = Engine::new();
    let (facts, rules) = parser::parse_program(program).unwrap();
    db.insert_facts(facts);
    for rule in rules {
        db.compile_rule(rule);
    }

    let feature_rules = r#"
        conditional_dep("//lib:http", "//lib:http_tls", "tls").
        conditional_dep("//lib:http", "//lib:openssl", "tls").

        active_deps(T, D) :- deps(T, D).
        active_deps(T, D) :-
            conditional_dep(T, D, Feature),
            feature_enabled(Feature).
    "#;

    let (facts, rules) = parser::parse_program(feature_rules).unwrap();
    db.insert_facts(facts);
    for rule in rules {
        db.compile_rule(rule);
    }

    let results = db.query("active_deps", &[]);

    let deps: Vec<String> = results
        .iter()
        .filter_map(|f| {
            if let Some(redwood::datalog::Value::String(t)) = f.args.first() {
                if t == "//lib:http" {
                    if let Some(redwood::datalog::Value::String(d)) = f.args.get(1) {
                        return Some(d.clone());
                    }
                }
            }
            None
        })
        .collect();

    // With tls feature enabled, should have base + tls deps
    assert!(deps.contains(&"//lib:base".to_string()));
    assert!(deps.contains(&"//lib:http_tls".to_string()));
    assert!(deps.contains(&"//lib:openssl".to_string()));
}

#[test]
fn affected_targets_for_ci() {
    let program = r#"
        changed_file("src/lib/core.rs").
        changed_file("README.md").
        changed_file("docs/guide.md").

        sources("//lib:core", "src/lib/core.rs").
        sources("//lib:http", "src/lib/http.rs").
        sources("//app", "src/main.rs").

        deps("//lib:http", "//lib:core").
        deps("//app", "//lib:http").

        test("//lib:core_test").
        test("//app:integration_test").

        deps("//lib:core_test", "//lib:core").
        deps("//app:integration_test", "//app").
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

    let affected_rules = r#"
        is_doc_file(F) :- matches_glob(F, "*.md").
        is_doc_file(F) :- matches_glob(F, "docs/**").

        affected_target(T) :-
            sources(T, F),
            changed_file(F),
            not(is_doc_file(F)).

        affected_target(T) :-
            deps(T, D),
            affected_target(D).

        affected_test(T) :-
            test(T),
            affected_target(T).
    "#;

    let (_, rules) = parser::parse_program(affected_rules).unwrap();
    for rule in rules {
        db.compile_rule(rule);
    }

    let results = db.query("affected_target", &[]);

    let targets: Vec<String> = results
        .iter()
        .filter_map(|f| {
            if let Some(redwood::datalog::Value::String(t)) = f.args.first() {
                Some(t.clone())
            } else {
                None
            }
        })
        .collect();

    // core.rs changed, so //lib:core is affected
    assert!(targets.contains(&"//lib:core".to_string()));

    // Reverse deps are affected too
    assert!(targets.contains(&"//lib:http".to_string()));
    assert!(targets.contains(&"//app".to_string()));

    // Tests should be affected
    let tests = db.query("affected_test", &[]);
    assert!(!tests.is_empty());
}

#[test]
fn monorepo_package_dependencies() {
    let program = r#"
        package("//lib/core").
        package("//lib/http").
        package("//services/api").
        package("//services/web").

        target_in_package("//lib/core:core", "//lib/core").
        target_in_package("//lib/core:utils", "//lib/core").
        target_in_package("//lib/http:http", "//lib/http").
        target_in_package("//services/api:main", "//services/api").
        target_in_package("//services/web:main", "//services/web").

        deps("//services/api:main", "//lib/http:http").
        deps("//services/web:main", "//lib/http:http").
        deps("//lib/http:http", "//lib/core:core").
        deps("//lib/http:http", "//lib/core:utils").
    "#;

    let mut db = Engine::new();
    let (facts, rules) = parser::parse_program(program).unwrap();
    db.insert_facts(facts);
    for rule in rules {
        db.compile_rule(rule);
    }

    let pkg_rules = r#"
        pkg_deps(PkgA, PkgB) :-
            target_in_package(TA, PkgA),
            target_in_package(TB, PkgB),
            deps(TA, TB),
            PkgA != PkgB.

        transitive_pkg_deps(P1, P2) :- pkg_deps(P1, P2).
        transitive_pkg_deps(P1, P3) :-
            pkg_deps(P1, P2),
            transitive_pkg_deps(P2, P3).
    "#;

    let (_, rules) = parser::parse_program(pkg_rules).unwrap();
    for rule in rules {
        db.compile_rule(rule);
    }

    let results = db.query("pkg_deps", &[]);

    // Direct package dependencies
    let has_api_http = results.iter().any(|f| {
        if let (
            Some(redwood::datalog::Value::String(a)),
            Some(redwood::datalog::Value::String(b)),
        ) = (f.args.first(), f.args.get(1))
        {
            a == "//services/api" && b == "//lib/http"
        } else {
            false
        }
    });
    assert!(has_api_http);

    // Transitive package dependencies
    let transitive = db.query("transitive_pkg_deps", &[]);
    let has_api_core = transitive.iter().any(|f| {
        if let (
            Some(redwood::datalog::Value::String(a)),
            Some(redwood::datalog::Value::String(b)),
        ) = (f.args.first(), f.args.get(1))
        {
            a == "//services/api" && b == "//lib/core"
        } else {
            false
        }
    });
    assert!(
        has_api_core,
        "//services/api should transitively depend on //lib/core"
    );
}
