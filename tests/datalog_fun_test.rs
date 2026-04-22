use redwood::datalog::parser::parse_program;
use redwood::datalog::Engine;
use redwood::datalog::{Fact, Value};

#[test]
fn graph_reachability() {
    let mut db = Engine::new();

    db.insert_facts(vec![
        Fact {
            predicate: "edge".to_string(),
            args: vec![
                Value::String("a".to_string()),
                Value::String("b".to_string()),
            ],
        },
        Fact {
            predicate: "edge".to_string(),
            args: vec![
                Value::String("b".to_string()),
                Value::String("c".to_string()),
            ],
        },
        Fact {
            predicate: "edge".to_string(),
            args: vec![
                Value::String("c".to_string()),
                Value::String("d".to_string()),
            ],
        },
        Fact {
            predicate: "edge".to_string(),
            args: vec![
                Value::String("a".to_string()),
                Value::String("e".to_string()),
            ],
        },
        Fact {
            predicate: "edge".to_string(),
            args: vec![
                Value::String("e".to_string()),
                Value::String("d".to_string()),
            ],
        },
        Fact {
            predicate: "edge".to_string(),
            args: vec![
                Value::String("d".to_string()),
                Value::String("f".to_string()),
            ],
        },
    ]);

    let program = r#"
        reachable(X, Y) :- edge(X, Y).
        reachable(X, Z) :- edge(X, Y), reachable(Y, Z).
    "#;

    let (_, rules) = parse_program(program).expect("Failed to parse graph program");

    for rule in rules {
        db.compile_rule(rule);
    }

    let reachable = db.query("reachable", &[]);

    let has_path = |from: &str, to: &str| -> bool {
        reachable.iter().any(|fact| {
            if let [Value::String(x), Value::String(y)] = &fact.args[..] {
                x == from && y == to
            } else {
                false
            }
        })
    };

    assert!(has_path("a", "b"), "a can reach b (direct edge)");
    assert!(has_path("a", "c"), "a can reach c (through b)");
    assert!(has_path("a", "d"), "a can reach d (multiple paths)");
    assert!(has_path("a", "f"), "a can reach f (long path)");
    assert!(has_path("b", "d"), "b can reach d");
    assert!(has_path("b", "f"), "b can reach f");
    assert!(has_path("e", "f"), "e can reach f");
    assert!(!has_path("d", "a"), "d cannot reach a (no backward edges)");
    assert!(!has_path("c", "e"), "c cannot reach e");
    assert!(!has_path("f", "a"), "f cannot reach a");
}

#[test]
fn family_tree_ancestors() {
    let mut db = Engine::new();

    db.insert_facts(vec![
        Fact {
            predicate: "parent".to_string(),
            args: vec![
                Value::String("alice".to_string()),
                Value::String("bob".to_string()),
            ],
        },
        Fact {
            predicate: "parent".to_string(),
            args: vec![
                Value::String("alice".to_string()),
                Value::String("carol".to_string()),
            ],
        },
        Fact {
            predicate: "parent".to_string(),
            args: vec![
                Value::String("bob".to_string()),
                Value::String("dave".to_string()),
            ],
        },
        Fact {
            predicate: "parent".to_string(),
            args: vec![
                Value::String("carol".to_string()),
                Value::String("eve".to_string()),
            ],
        },
        Fact {
            predicate: "parent".to_string(),
            args: vec![
                Value::String("dave".to_string()),
                Value::String("frank".to_string()),
            ],
        },
        Fact {
            predicate: "parent".to_string(),
            args: vec![
                Value::String("eve".to_string()),
                Value::String("grace".to_string()),
            ],
        },
    ]);

    let program = r#"
        ancestor(X, Y) :- parent(X, Y).
        ancestor(X, Z) :- parent(X, Y), ancestor(Y, Z).

        sibling(X, Y) :- parent(P, X), parent(P, Y), X != Y.

        cousin(X, Y) :- parent(P1, X), parent(P2, Y), sibling(P1, P2).
    "#;

    let (_, rules) = parse_program(program).expect("Failed to parse family tree program");

    for rule in rules {
        db.compile_rule(rule);
    }

    let ancestors = db.query("ancestor", &[]);

    let is_ancestor = |ancestor: &str, descendant: &str| -> bool {
        ancestors.iter().any(|fact| {
            if let [Value::String(x), Value::String(y)] = &fact.args[..] {
                x == ancestor && y == descendant
            } else {
                false
            }
        })
    };

    assert!(is_ancestor("alice", "bob"), "alice is parent of bob");
    assert!(is_ancestor("alice", "dave"), "alice is grandparent of dave");
    assert!(
        is_ancestor("alice", "frank"),
        "alice is great-grandparent of frank"
    );
    assert!(
        is_ancestor("alice", "grace"),
        "alice is great-grandparent of grace"
    );
    assert!(is_ancestor("bob", "frank"), "bob is grandparent of frank");
    assert!(
        is_ancestor("carol", "grace"),
        "carol is grandparent of grace"
    );
    assert!(
        !is_ancestor("bob", "eve"),
        "bob is not ancestor of eve (different branch)"
    );
    assert!(
        !is_ancestor("dave", "grace"),
        "dave is not ancestor of grace (different branch)"
    );

    let siblings = db.query("sibling", &[]);
    let are_siblings = |x: &str, y: &str| -> bool {
        siblings.iter().any(|fact| {
            if let [Value::String(a), Value::String(b)] = &fact.args[..] {
                (a == x && b == y) || (a == y && b == x)
            } else {
                false
            }
        })
    };

    assert!(are_siblings("bob", "carol"), "bob and carol are siblings");
    assert!(
        !are_siblings("dave", "frank"),
        "dave and frank are not siblings"
    );

    let cousins = db.query("cousin", &[]);
    let are_cousins = |x: &str, y: &str| -> bool {
        cousins.iter().any(|fact| {
            if let [Value::String(a), Value::String(b)] = &fact.args[..] {
                (a == x && b == y) || (a == y && b == x)
            } else {
                false
            }
        })
    };

    assert!(
        are_cousins("dave", "eve"),
        "dave and eve are cousins (parents are siblings)"
    );
    assert!(
        are_cousins("eve", "dave"),
        "eve and dave are cousins (symmetric)"
    );
}

#[test]
fn transitive_closure_chain() {
    let mut db = Engine::new();

    let nodes = vec!["a", "b", "c", "d", "e", "f", "g", "h", "i", "j"];
    for i in 0..nodes.len() - 1 {
        db.insert_facts(vec![Fact {
            predicate: "next".to_string(),
            args: vec![
                Value::String(nodes[i].to_string()),
                Value::String(nodes[i + 1].to_string()),
            ],
        }]);
    }

    let program = r#"
        connected(X, Y) :- next(X, Y).
        connected(X, Z) :- next(X, Y), connected(Y, Z).
    "#;

    let (_, rules) = parse_program(program).expect("Failed to parse chain program");

    for rule in rules {
        db.compile_rule(rule);
    }

    let connected = db.query("connected", &[]);

    let is_connected = |from: &str, to: &str| -> bool {
        connected.iter().any(|fact| {
            if let [Value::String(x), Value::String(y)] = &fact.args[..] {
                x == from && y == to
            } else {
                false
            }
        })
    };

    assert!(is_connected("a", "b"), "Direct connection");
    assert!(is_connected("a", "j"), "Long chain from a to j");
    assert!(is_connected("a", "e"), "Mid chain");
    assert!(is_connected("e", "j"), "Partial chain");
    assert!(!is_connected("j", "a"), "No backward connection");
    assert!(!is_connected("e", "a"), "No backward connection in middle");
}

#[test]
fn dependency_graph_compilation_order() {
    let mut db = Engine::new();

    db.insert_facts(vec![
        Fact {
            predicate: "depends".to_string(),
            args: vec![
                Value::String("app".to_string()),
                Value::String("lib_http".to_string()),
            ],
        },
        Fact {
            predicate: "depends".to_string(),
            args: vec![
                Value::String("app".to_string()),
                Value::String("lib_db".to_string()),
            ],
        },
        Fact {
            predicate: "depends".to_string(),
            args: vec![
                Value::String("lib_http".to_string()),
                Value::String("lib_json".to_string()),
            ],
        },
        Fact {
            predicate: "depends".to_string(),
            args: vec![
                Value::String("lib_http".to_string()),
                Value::String("lib_net".to_string()),
            ],
        },
        Fact {
            predicate: "depends".to_string(),
            args: vec![
                Value::String("lib_db".to_string()),
                Value::String("lib_sql".to_string()),
            ],
        },
        Fact {
            predicate: "depends".to_string(),
            args: vec![
                Value::String("lib_net".to_string()),
                Value::String("lib_core".to_string()),
            ],
        },
        Fact {
            predicate: "depends".to_string(),
            args: vec![
                Value::String("lib_sql".to_string()),
                Value::String("lib_core".to_string()),
            ],
        },
    ]);

    let program = r#"
        transitive_dep(X, Y) :- depends(X, Y).
        transitive_dep(X, Z) :- depends(X, Y), transitive_dep(Y, Z).
    "#;

    let (_, rules) = parse_program(program).expect("Failed to parse dependency program");

    for rule in rules {
        db.compile_rule(rule);
    }

    let trans_deps = db.query("transitive_dep", &[]);

    let depends_on = |target: &str, dep: &str| -> bool {
        trans_deps.iter().any(|fact| {
            if let [Value::String(x), Value::String(y)] = &fact.args[..] {
                x == target && y == dep
            } else {
                false
            }
        })
    };

    assert!(
        depends_on("app", "lib_http"),
        "app depends on lib_http directly"
    );
    assert!(
        depends_on("app", "lib_json"),
        "app depends on lib_json transitively"
    );
    assert!(
        depends_on("app", "lib_core"),
        "app depends on lib_core transitively"
    );
    assert!(
        depends_on("lib_http", "lib_core"),
        "lib_http depends on lib_core transitively"
    );
    assert!(
        !depends_on("lib_core", "app"),
        "lib_core doesn't depend on app"
    );
    assert!(
        !depends_on("lib_json", "lib_db"),
        "lib_json doesn't depend on lib_db"
    );
}

#[test]
fn friend_of_friend_network() {
    let mut db = Engine::new();

    db.insert_facts(vec![
        Fact {
            predicate: "friend".to_string(),
            args: vec![
                Value::String("alice".to_string()),
                Value::String("bob".to_string()),
            ],
        },
        Fact {
            predicate: "friend".to_string(),
            args: vec![
                Value::String("bob".to_string()),
                Value::String("alice".to_string()),
            ],
        },
        Fact {
            predicate: "friend".to_string(),
            args: vec![
                Value::String("bob".to_string()),
                Value::String("charlie".to_string()),
            ],
        },
        Fact {
            predicate: "friend".to_string(),
            args: vec![
                Value::String("charlie".to_string()),
                Value::String("bob".to_string()),
            ],
        },
        Fact {
            predicate: "friend".to_string(),
            args: vec![
                Value::String("charlie".to_string()),
                Value::String("diana".to_string()),
            ],
        },
        Fact {
            predicate: "friend".to_string(),
            args: vec![
                Value::String("diana".to_string()),
                Value::String("charlie".to_string()),
            ],
        },
        Fact {
            predicate: "friend".to_string(),
            args: vec![
                Value::String("alice".to_string()),
                Value::String("eve".to_string()),
            ],
        },
        Fact {
            predicate: "friend".to_string(),
            args: vec![
                Value::String("eve".to_string()),
                Value::String("alice".to_string()),
            ],
        },
    ]);

    let program = r#"
        friend_of_friend(X, Z) :- friend(X, Y), friend(Y, Z), X != Z.

        connected_network(X, Y) :- friend(X, Y).
        connected_network(X, Z) :- friend(X, Y), connected_network(Y, Z).
    "#;

    let (_, rules) = parse_program(program).expect("Failed to parse friend network");

    for rule in rules {
        db.compile_rule(rule);
    }

    let fof = db.query("friend_of_friend", &[]);

    let is_fof = |person: &str, fof_person: &str| -> bool {
        fof.iter().any(|fact| {
            if let [Value::String(x), Value::String(y)] = &fact.args[..] {
                x == person && y == fof_person
            } else {
                false
            }
        })
    };

    assert!(is_fof("alice", "charlie"), "alice -> bob -> charlie");
    assert!(is_fof("bob", "diana"), "bob -> charlie -> diana");

    let network = db.query("connected_network", &[]);

    let in_same_network = |person1: &str, person2: &str| -> bool {
        network.iter().any(|fact| {
            if let [Value::String(x), Value::String(y)] = &fact.args[..] {
                x == person1 && y == person2
            } else {
                false
            }
        })
    };

    assert!(
        in_same_network("alice", "diana"),
        "alice and diana are in same network"
    );
    assert!(
        in_same_network("eve", "charlie"),
        "eve and charlie are in same network"
    );
}

#[test]
fn access_control_hierarchy() {
    let mut db = Engine::new();

    db.insert_facts(vec![
        Fact {
            predicate: "role".to_string(),
            args: vec![
                Value::String("alice".to_string()),
                Value::String("admin".to_string()),
            ],
        },
        Fact {
            predicate: "role".to_string(),
            args: vec![
                Value::String("bob".to_string()),
                Value::String("developer".to_string()),
            ],
        },
        Fact {
            predicate: "role".to_string(),
            args: vec![
                Value::String("carol".to_string()),
                Value::String("viewer".to_string()),
            ],
        },
        Fact {
            predicate: "role_hierarchy".to_string(),
            args: vec![
                Value::String("admin".to_string()),
                Value::String("developer".to_string()),
            ],
        },
        Fact {
            predicate: "role_hierarchy".to_string(),
            args: vec![
                Value::String("developer".to_string()),
                Value::String("viewer".to_string()),
            ],
        },
        Fact {
            predicate: "permission".to_string(),
            args: vec![
                Value::String("admin".to_string()),
                Value::String("delete".to_string()),
            ],
        },
        Fact {
            predicate: "permission".to_string(),
            args: vec![
                Value::String("developer".to_string()),
                Value::String("write".to_string()),
            ],
        },
        Fact {
            predicate: "permission".to_string(),
            args: vec![
                Value::String("viewer".to_string()),
                Value::String("read".to_string()),
            ],
        },
    ]);

    let program = r#"
        inherits(R1, R2) :- role_hierarchy(R1, R2).
        inherits(R1, R3) :- role_hierarchy(R1, R2), inherits(R2, R3).

        has_permission(User, Perm) :-
            role(User, Role),
            permission(Role, Perm).

        has_permission(User, Perm) :-
            role(User, Role),
            inherits(Role, InheritedRole),
            permission(InheritedRole, Perm).
    "#;

    let (_, rules) = parse_program(program).expect("Failed to parse access control");

    for rule in rules {
        db.compile_rule(rule);
    }

    let perms = db.query("has_permission", &[]);

    let can = |user: &str, perm: &str| -> bool {
        perms.iter().any(|fact| {
            if let [Value::String(u), Value::String(p)] = &fact.args[..] {
                u == user && p == perm
            } else {
                false
            }
        })
    };

    assert!(can("alice", "delete"), "admin can delete");
    assert!(can("alice", "write"), "admin can write (inherited)");
    assert!(can("alice", "read"), "admin can read (inherited)");

    assert!(can("bob", "write"), "developer can write");
    assert!(can("bob", "read"), "developer can read (inherited)");
    assert!(!can("bob", "delete"), "developer cannot delete");

    assert!(can("carol", "read"), "viewer can read");
    assert!(!can("carol", "write"), "viewer cannot write");
    assert!(!can("carol", "delete"), "viewer cannot delete");
}

#[test]
fn string_concatenation() {
    let mut db = Engine::new();

    db.insert_facts(vec![
        Fact {
            predicate: "first_name".to_string(),
            args: vec![
                Value::String("alice".to_string()),
                Value::String("Alice".to_string()),
            ],
        },
        Fact {
            predicate: "last_name".to_string(),
            args: vec![
                Value::String("alice".to_string()),
                Value::String("Smith".to_string()),
            ],
        },
        Fact {
            predicate: "first_name".to_string(),
            args: vec![
                Value::String("bob".to_string()),
                Value::String("Bob".to_string()),
            ],
        },
        Fact {
            predicate: "last_name".to_string(),
            args: vec![
                Value::String("bob".to_string()),
                Value::String("Jones".to_string()),
            ],
        },
        Fact {
            predicate: "path_prefix".to_string(),
            args: vec![Value::String("target/".to_string())],
        },
        Fact {
            predicate: "path_suffix".to_string(),
            args: vec![Value::String(".o".to_string())],
        },
        Fact {
            predicate: "filename".to_string(),
            args: vec![Value::String("main".to_string())],
        },
    ]);

    let program = r#"
        full_name(Person, Name) :-
            first_name(Person, First),
            last_name(Person, Last),
            concat(First, " ", WithSpace),
            concat(WithSpace, Last, Name).

        output_path(Path) :-
            path_prefix(Pre),
            filename(File),
            path_suffix(Suf),
            concat(Pre, File, Partial),
            concat(Partial, Suf, Path).
    "#;

    let (_, rules) = parse_program(program).expect("Failed to parse concat program");

    for rule in rules {
        db.compile_rule(rule);
    }

    let full_names = db.query("full_name", &[]);

    let has_full_name = |person: &str, expected: &str| -> bool {
        full_names.iter().any(|fact| {
            if let [Value::String(p), Value::String(name)] = &fact.args[..] {
                p == person && name == expected
            } else {
                false
            }
        })
    };

    assert!(has_full_name("alice", "Alice Smith"), "alice's full name");
    assert!(has_full_name("bob", "Bob Jones"), "bob's full name");

    let output_paths = db.query("output_path", &[]);
    assert_eq!(output_paths.len(), 1, "Should have one output path");

    if let Some(fact) = output_paths.first() {
        if let [Value::String(path)] = &fact.args[..] {
            assert_eq!(path, "target/main.o", "Should construct correct path");
        } else {
            panic!("Expected string path");
        }
    }
}

#[test]
fn build_system_path_construction() {
    let mut db = Engine::new();

    db.insert_facts(vec![
        Fact {
            predicate: "target".to_string(),
            args: vec![Value::String("//app:server".to_string())],
        },
        Fact {
            predicate: "target".to_string(),
            args: vec![Value::String("//lib:http".to_string())],
        },
        Fact {
            predicate: "kind".to_string(),
            args: vec![
                Value::String("//app:server".to_string()),
                Value::String("rust_binary".to_string()),
            ],
        },
        Fact {
            predicate: "kind".to_string(),
            args: vec![
                Value::String("//lib:http".to_string()),
                Value::String("rust_library".to_string()),
            ],
        },
        Fact {
            predicate: "target_name".to_string(),
            args: vec![
                Value::String("//app:server".to_string()),
                Value::String("server".to_string()),
            ],
        },
        Fact {
            predicate: "target_name".to_string(),
            args: vec![
                Value::String("//lib:http".to_string()),
                Value::String("http".to_string()),
            ],
        },
    ]);

    let program = r#"
        binary_output(Target, Path) :-
            target(Target),
            kind(Target, "rust_binary"),
            target_name(Target, Name),
            concat("target/release/", Name, Path).

        library_output(Target, Path) :-
            target(Target),
            kind(Target, "rust_library"),
            target_name(Target, Name),
            concat("target/release/lib", Name, Partial),
            concat(Partial, ".rlib", Path).

        output_path(Target, Path) :- binary_output(Target, Path).
        output_path(Target, Path) :- library_output(Target, Path).
    "#;

    let (_, rules) = parse_program(program).expect("Failed to parse build system program");

    for rule in rules {
        db.compile_rule(rule);
    }

    let outputs = db.query("output_path", &[]);

    let has_output = |target: &str, expected_path: &str| -> bool {
        outputs.iter().any(|fact| {
            if let [Value::String(t), Value::String(p)] = &fact.args[..] {
                t == target && p == expected_path
            } else {
                false
            }
        })
    };

    assert!(
        has_output("//app:server", "target/release/server"),
        "Binary output path"
    );
    assert!(
        has_output("//lib:http", "target/release/libhttp.rlib"),
        "Library output path"
    );
}
