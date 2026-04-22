use redwood::datalog::{parser, Engine, Fact, Value};
use redwood::runtime::prelude;
use std::process::Command;

fn setup_engine_with_prelude(program: &str) -> Engine {
    let mut db = Engine::new();

    let (facts, rules) = parser::parse_program(program).unwrap();
    db.insert_facts(facts);
    for rule in rules {
        db.compile_rule(rule);
    }

    db.set_resolve_callback(Box::new(|_target, tool, args| {
        let output = Command::new(tool)
            .args(args)
            .output()
            .map_err(|e| e.to_string())?;
        Ok(output.stdout)
    }));

    // Expand shell_command facts into base attr facts.
    // resolve() only queries base facts, so we need to materialize the attrs
    // that would otherwise be derived by prelude rules.
    let shell_commands = db.query("shell_command", &[]);
    for fact in shell_commands {
        if let Value::String(cmd) = &fact.args[0] {
            db.insert_facts(vec![
                Fact::new("target", vec![Value::String(cmd.clone())]),
                Fact::new(
                    "attr",
                    vec![
                        Value::String(cmd.clone()),
                        Value::String("tool".to_string()),
                        Value::String("sh".to_string()),
                    ],
                ),
                Fact::new(
                    "attr",
                    vec![
                        Value::String(cmd.clone()),
                        Value::String("0".to_string()),
                        Value::String("-c".to_string()),
                    ],
                ),
                Fact::new(
                    "attr",
                    vec![
                        Value::String(cmd.clone()),
                        Value::String("1".to_string()),
                        Value::String(cmd.clone()),
                    ],
                ),
            ]);
        }
    }

    let prelude_facts = prelude::get_prelude_facts();
    db.insert_facts(prelude_facts);

    let prelude_rules = prelude::get_prelude_rules();
    for rule in prelude_rules {
        db.compile_rule(rule);
    }

    db
}

#[test]
fn git_commit_from_prelude() {
    let build_content = r#"
build_id(Commit) :- git_commit(Commit).
"#;

    let mut db = setup_engine_with_prelude(build_content);

    if std::process::Command::new("git")
        .arg("rev-parse")
        .arg("--git-dir")
        .output()
        .is_ok()
    {
        let results = db.query("build_id", &[]);
        assert_eq!(results.len(), 1);

        if let Value::String(commit) = &results[0].args[0] {
            assert_eq!(commit.len(), 40);
            assert!(commit.chars().all(|c| c.is_ascii_hexdigit()));
        } else {
            panic!("Expected string commit hash");
        }
    }
}

#[test]
fn git_branch_from_prelude() {
    let build_content = r#"
on_branch(Branch) :- git_branch(Branch).
"#;

    let mut db = setup_engine_with_prelude(build_content);

    if std::process::Command::new("git")
        .arg("rev-parse")
        .arg("--git-dir")
        .output()
        .is_ok()
    {
        let results = db.query("on_branch", &[]);
        assert_eq!(results.len(), 1);

        if let Value::String(branch) = &results[0].args[0] {
            assert!(!branch.is_empty());
        } else {
            panic!("Expected string branch name");
        }
    }
}

#[test]
fn conditional_target_based_on_git_branch() {
    let build_content = r#"
target("//debug:tools") :- git_branch("develop").
target("//debug:tools") :- git_branch("main").
"#;

    let mut db = setup_engine_with_prelude(build_content);

    if std::process::Command::new("git")
        .arg("rev-parse")
        .arg("--git-dir")
        .output()
        .is_ok()
    {
        let current_branch_output = std::process::Command::new("git")
            .args(["branch", "--show-current"])
            .output()
            .unwrap();
        let current_branch = String::from_utf8_lossy(&current_branch_output.stdout)
            .trim()
            .to_string();

        let results = db.query("target", &[]);

        if current_branch == "develop" || current_branch == "main" {
            assert!(results.iter().any(|f| {
                if let Value::String(t) = &f.args[0] {
                    t == "//debug:tools"
                } else {
                    false
                }
            }));
        }
    }
}

#[test]
fn custom_shell_command() {
    let build_content = r#"
target("@shell:hostname").
attr("@shell:hostname", "tool", "sh").
attr("@shell:hostname", "0", "-c").
attr("@shell:hostname", "1", "echo test-hostname").

hostname(H) :- resolve("@shell:hostname", H).
"#;

    let mut db = setup_engine_with_prelude(build_content);
    let results = db.query("hostname", &[]);
    assert_eq!(results.len(), 1);
    assert_eq!(
        results[0].args[0],
        Value::String("test-hostname".to_string())
    );
}

#[test]
fn shell_command_with_multiple_outputs() {
    let build_content = r#"
target("@shell:multi").
attr("@shell:multi", "tool", "sh").
attr("@shell:multi", "0", "-c").
attr("@shell:multi", "1", "echo line1 && echo line2 && echo line3").

line(L) :- resolve("@shell:multi", L).
"#;

    let mut db = setup_engine_with_prelude(build_content);
    let results = db.query("line", &[]);
    assert_eq!(results.len(), 3);

    let lines: Vec<String> = results
        .iter()
        .filter_map(|f| {
            if let Value::String(s) = &f.args[0] {
                Some(s.clone())
            } else {
                None
            }
        })
        .collect();

    assert!(lines.contains(&"line1".to_string()));
    assert!(lines.contains(&"line2".to_string()));
    assert!(lines.contains(&"line3".to_string()));
}

#[test]
fn git_files_lists_tracked_files() {
    let build_content = r#"
tracked(File) :- git_files(File).
"#;

    let mut db = setup_engine_with_prelude(build_content);

    if std::process::Command::new("git")
        .arg("rev-parse")
        .arg("--git-dir")
        .output()
        .is_ok()
    {
        let results = db.query("tracked", &[]);
        assert!(!results.is_empty());

        let files: Vec<String> = results
            .iter()
            .filter_map(|f| {
                if let Value::String(s) = &f.args[0] {
                    Some(s.clone())
                } else {
                    None
                }
            })
            .collect();

        assert!(files.iter().any(|f| f.ends_with("Cargo.toml")));
    }
}

#[test]
fn shell_predicate_executes_command() {
    let build_content = r#"
shell_command("echo hello-world").
message(M) :- shell("echo hello-world", M).
"#;

    let mut db = setup_engine_with_prelude(build_content);
    let results = db.query("message", &[]);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].args[0], Value::String("hello-world".to_string()));
}

#[test]
fn shell_predicate_multiple_lines() {
    let build_content = r#"
shell_command("echo line1 && echo line2 && echo line3").
line(L) :- shell("echo line1 && echo line2 && echo line3", L).
"#;

    let mut db = setup_engine_with_prelude(build_content);
    let results = db.query("line", &[]);
    assert_eq!(results.len(), 3);

    let lines: Vec<String> = results
        .iter()
        .filter_map(|f| {
            if let Value::String(s) = &f.args[0] {
                Some(s.clone())
            } else {
                None
            }
        })
        .collect();

    assert!(lines.contains(&"line1".to_string()));
    assert!(lines.contains(&"line2".to_string()));
    assert!(lines.contains(&"line3".to_string()));
}

#[test]
fn platform_detection() {
    let build_content = r#"
linux_build(Target) :- target(Target), platform_os("Linux").
darwin_build(Target) :- target(Target), platform_os("Darwin").
x86_build(Target) :- target(Target), platform_arch("x86_64").
"#;

    let mut db = setup_engine_with_prelude(build_content);

    let os_results = db.query("platform_os", &[]);
    assert_eq!(os_results.len(), 1);

    let arch_results = db.query("platform_arch", &[]);
    assert_eq!(arch_results.len(), 1);

    if let Value::String(os) = &os_results[0].args[0] {
        assert!(os == "Linux" || os == "Darwin" || os.contains("Windows"));
    }

    if let Value::String(arch) = &arch_results[0].args[0] {
        assert!(arch == "x86_64" || arch == "aarch64" || arch == "arm64");
    }
}
