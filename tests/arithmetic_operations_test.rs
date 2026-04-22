use redwood::datalog::{parser, Engine, Value};

#[test]
fn parse_int_converts_string_to_integer() {
    let program = r#"
        version("42").
        version("100").
        version("not_a_number").

        numeric_version(Ver, Num) :- version(Ver), parse_int(Ver, Num).
    "#;

    let mut db = Engine::new();
    let (facts, rules) = parser::parse_program(program).unwrap();
    db.insert_facts(facts);
    for rule in rules {
        db.compile_rule(rule);
    }

    let results = db.query("numeric_version", &[]);
    assert_eq!(results.len(), 2);

    let nums: Vec<i64> = results
        .iter()
        .filter_map(|f| {
            if let Some(Value::Integer(n)) = f.args.get(1) {
                Some(*n)
            } else {
                None
            }
        })
        .collect();

    assert!(nums.contains(&42));
    assert!(nums.contains(&100));
}

#[test]
fn to_string_converts_integer_to_string() {
    let program = r#"
        port(8080).
        port(9000).

        port_label(Port, Label) :-
            port(Port),
            to_string(Port, PortStr),
            concat("Port: ", PortStr, Label).
    "#;

    let mut db = Engine::new();
    let (facts, rules) = parser::parse_program(program).unwrap();
    db.insert_facts(facts);
    for rule in rules {
        db.compile_rule(rule);
    }

    let results = db.query("port_label", &[]);
    assert_eq!(results.len(), 2);

    let labels: Vec<String> = results
        .iter()
        .filter_map(|f| {
            if let Some(Value::String(s)) = f.args.get(1) {
                Some(s.clone())
            } else {
                None
            }
        })
        .collect();

    assert!(labels.contains(&"Port: 8080".to_string()));
    assert!(labels.contains(&"Port: 9000".to_string()));
}

#[test]
fn add_performs_integer_addition() {
    let program = r#"
        file_size("main.o", 1024).
        file_size("lib.o", 2048).

        total_size(Total) :-
            file_size("main.o", S1),
            file_size("lib.o", S2),
            add(S1, S2, Total).
    "#;

    let mut db = Engine::new();
    let (facts, rules) = parser::parse_program(program).unwrap();
    db.insert_facts(facts);
    for rule in rules {
        db.compile_rule(rule);
    }

    let results = db.query("total_size", &[]);
    assert_eq!(results.len(), 1);

    if let Some(Value::Integer(total)) = results[0].args.first() {
        assert_eq!(*total, 3072);
    } else {
        panic!("Expected integer result");
    }
}

#[test]
fn sub_performs_integer_subtraction() {
    let program = r#"
        max_memory(8192).
        used_memory(2048).

        available_memory(Avail) :-
            max_memory(Max),
            used_memory(Used),
            sub(Max, Used, Avail).
    "#;

    let mut db = Engine::new();
    let (facts, rules) = parser::parse_program(program).unwrap();
    db.insert_facts(facts);
    for rule in rules {
        db.compile_rule(rule);
    }

    let results = db.query("available_memory", &[]);
    assert_eq!(results.len(), 1);

    if let Some(Value::Integer(avail)) = results[0].args.first() {
        assert_eq!(*avail, 6144);
    } else {
        panic!("Expected integer result");
    }
}

#[test]
fn mul_performs_integer_multiplication() {
    let program = r#"
        block_size(512).
        block_count(10).

        total_bytes(Total) :-
            block_size(Size),
            block_count(Count),
            mul(Size, Count, Total).
    "#;

    let mut db = Engine::new();
    let (facts, rules) = parser::parse_program(program).unwrap();
    db.insert_facts(facts);
    for rule in rules {
        db.compile_rule(rule);
    }

    let results = db.query("total_bytes", &[]);
    assert_eq!(results.len(), 1);

    if let Some(Value::Integer(total)) = results[0].args.first() {
        assert_eq!(*total, 5120);
    } else {
        panic!("Expected integer result");
    }
}

#[test]
fn div_performs_integer_division() {
    let program = r#"
        total_bytes(5120).
        block_size(512).

        block_count(Count) :-
            total_bytes(Total),
            block_size(Size),
            div(Total, Size, Count).
    "#;

    let mut db = Engine::new();
    let (facts, rules) = parser::parse_program(program).unwrap();
    db.insert_facts(facts);
    for rule in rules {
        db.compile_rule(rule);
    }

    let results = db.query("block_count", &[]);
    assert_eq!(results.len(), 1);

    if let Some(Value::Integer(count)) = results[0].args.first() {
        assert_eq!(*count, 10);
    } else {
        panic!("Expected integer result");
    }
}

#[test]
fn mod_computes_remainder() {
    let program = r#"
        port(8000).
        port(8001).
        port(8002).
        port(8003).

        worker_id(Port, Worker) :-
            port(Port),
            mod(Port, 3, Worker).
    "#;

    let mut db = Engine::new();
    let (facts, rules) = parser::parse_program(program).unwrap();
    db.insert_facts(facts);
    for rule in rules {
        db.compile_rule(rule);
    }

    let results = db.query("worker_id", &[]);
    assert_eq!(results.len(), 4);

    // 8000 % 3 = 2, 8001 % 3 = 0, 8002 % 3 = 1, 8003 % 3 = 2
    let worker_for_8001 = results.iter().find(|f| {
        if let Some(Value::Integer(port)) = f.args.first() {
            *port == 8001
        } else {
            false
        }
    });

    if let Some(fact) = worker_for_8001 {
        if let Some(Value::Integer(worker)) = fact.args.get(1) {
            assert_eq!(*worker, 0);
        } else {
            panic!("Expected integer worker id");
        }
    } else {
        panic!("Expected result for port 8001");
    }
}

#[test]
fn version_comparison_with_parse_int() {
    let program = r#"
        tool("rustc", "1.75.0").
        tool("gcc", "11.3.0").

        major_version(Tool, Major) :-
            tool(Tool, Ver),
            split(Ver, ".", 0, MajorStr),
            parse_int(MajorStr, Major).

        recent_tool(Tool) :-
            major_version(Tool, Major),
            gt(Major, 5).
    "#;

    let mut db = Engine::new();
    let (facts, rules) = parser::parse_program(program).unwrap();
    db.insert_facts(facts);
    for rule in rules {
        db.compile_rule(rule);
    }

    let results = db.query("recent_tool", &[]);
    assert_eq!(results.len(), 1);

    let tools: Vec<String> = results
        .iter()
        .filter_map(|f| {
            if let Some(Value::String(tool)) = f.args.first() {
                Some(tool.clone())
            } else {
                None
            }
        })
        .collect();

    assert!(tools.contains(&"gcc".to_string()));
}

#[test]
fn arithmetic_checks_exact_values() {
    let program = r#"
        value(10).
        value(20).

        is_sum_30(X, Y) :-
            value(X),
            value(Y),
            X != Y,
            add(X, Y, 30).
    "#;

    let mut db = Engine::new();
    let (facts, rules) = parser::parse_program(program).unwrap();
    db.insert_facts(facts);
    for rule in rules {
        db.compile_rule(rule);
    }

    let results = db.query("is_sum_30", &[]);
    assert_eq!(results.len(), 2); // (10, 20) and (20, 10)

    for result in &results {
        if let (Some(Value::Integer(x)), Some(Value::Integer(y))) =
            (result.args.first(), result.args.get(1))
        {
            assert_eq!(x + y, 30);
        } else {
            panic!("Expected integer results");
        }
    }
}

#[test]
fn combined_type_conversion_and_arithmetic() {
    let program = r#"
        base_port("8000").
        offset(0).
        offset(1).
        offset(2).

        actual_port(Offset, PortStr) :-
            base_port(BaseStr),
            parse_int(BaseStr, Base),
            offset(Offset),
            add(Base, Offset, Port),
            to_string(Port, PortStr).
    "#;

    let mut db = Engine::new();
    let (facts, rules) = parser::parse_program(program).unwrap();
    db.insert_facts(facts);
    for rule in rules {
        db.compile_rule(rule);
    }

    let results = db.query("actual_port", &[]);
    assert_eq!(results.len(), 3);

    let ports: Vec<String> = results
        .iter()
        .filter_map(|f| {
            if let Some(Value::String(port)) = f.args.get(1) {
                Some(port.clone())
            } else {
                None
            }
        })
        .collect();

    assert!(ports.contains(&"8000".to_string()));
    assert!(ports.contains(&"8001".to_string()));
    assert!(ports.contains(&"8002".to_string()));
}
