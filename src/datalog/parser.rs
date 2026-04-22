use crate::datalog::{Fact, Predicate, Rule, SourceLocation, Term, Value};
use std::collections::HashMap;

pub fn parse(input: &str) -> Result<Vec<Fact>, String> {
    let tokens = tokenize(input);
    parse_facts(&tokens)
}

pub fn parse_program(input: &str) -> Result<(Vec<Fact>, Vec<Rule>), String> {
    let tokens = tokenize(input);
    let mut facts = Vec::new();
    let mut rules = Vec::new();
    let mut i = 0;

    while i < tokens.len() {
        if let Token::Ident(_) = &tokens[i] {
            if i + 1 < tokens.len() {
                match &tokens[i + 1] {
                    Token::LParen => {
                        if let Some(dot_pos) =
                            tokens[i..].iter().position(|t| matches!(t, Token::Dot))
                        {
                            if let Some(_rule_pos) = tokens[i..i + dot_pos]
                                .iter()
                                .position(|t| matches!(t, Token::Rule))
                            {
                                let (rule, consumed) = parse_rule(&tokens[i..])?;
                                rules.push(rule);
                                i += consumed;
                            } else {
                                let (fact, consumed) = parse_fact(&tokens[i..])?;
                                facts.push(fact);
                                i += consumed;
                            }
                        } else {
                            return Err("Expected '.' after predicate".to_string());
                        }
                    }
                    _ => i += 1,
                }
            } else {
                i += 1;
            }
        } else {
            i += 1;
        }
    }

    Ok((facts, rules))
}

pub type ParseResult = Result<(Vec<Fact>, Vec<Rule>, HashMap<String, SourceLocation>), String>;

pub fn parse_program_with_file(input: &str, filename: &str) -> ParseResult {
    parse_program_with_namespace(input, filename, "//")
}

pub fn parse_program_with_namespace(input: &str, filename: &str, namespace: &str) -> ParseResult {
    // Parse the entire program first
    let (facts, rules) = parse_program(input)?;

    // Rewrite facts with namespace
    let rewritten_facts = if namespace == "//" {
        facts
    } else {
        facts
            .into_iter()
            .map(|f| rewrite_fact_labels(f, namespace))
            .collect()
    };

    // Build a map from position to line number for location tracking
    let mut line_starts = vec![0];
    for (i, byte) in input.bytes().enumerate() {
        if byte == b'\n' {
            line_starts.push(i + 1);
        }
    }

    let mut locations = HashMap::new();

    // Target-defining predicates where we record source_location for the first argument
    let target_defining_predicates = [
        "target",
        "system_rustc",
        "cargo_binary",
        "alias",
        "crate_name",
        "primary_source",
        "sources",
        "license",
        "kind",
        "attr",
    ];

    // For each fact, find its location in the source
    // Only record location for the first argument of target-defining predicates
    for fact in &rewritten_facts {
        // Skip facts that aren't target-defining predicates
        if !target_defining_predicates.contains(&fact.predicate.as_str()) {
            continue;
        }

        if let Some(Value::String(target)) = fact.args.first() {
            // Skip if we've already recorded this target (keep the first occurrence)
            if locations.contains_key(target) {
                continue;
            }

            // Search for the original target string (before rewriting) in the source
            let original_target = if namespace != "//" && target.starts_with(namespace) {
                target.strip_prefix(namespace).unwrap_or(target)
            } else {
                target
            };

            // Search for the predicate followed by the target string
            // This ensures we find the target in the context of this specific predicate
            let search_pattern = format!("{}(\"{}\"", fact.predicate, original_target);

            if let Some(pos) = input.find(&search_pattern) {
                let line = line_starts
                    .iter()
                    .rposition(|&start| start <= pos)
                    .unwrap_or(0)
                    + 1;
                locations.insert(
                    target.clone(),
                    SourceLocation {
                        file: filename.to_string(),
                        line,
                    },
                );
            }
        }
    }

    // For each rule, find its location
    for rule in &rules {
        let search_str = format!("{}(", rule.head.name);
        if let Some(pos) = input.find(&search_str) {
            let line = line_starts
                .iter()
                .rposition(|&start| start <= pos)
                .unwrap_or(0)
                + 1;
            locations.insert(
                rule.head.name.clone(),
                SourceLocation {
                    file: filename.to_string(),
                    line,
                },
            );
        }
    }

    Ok((rewritten_facts, rules, locations))
}

fn rewrite_fact_labels(fact: Fact, namespace: &str) -> Fact {
    let new_args = fact
        .args
        .into_iter()
        .map(|arg| match arg {
            Value::String(s) if is_target_label(&s) => {
                Value::String(rewrite_target_label(&s, namespace))
            }
            other => other,
        })
        .collect();

    Fact {
        predicate: fact.predicate,
        args: new_args,
    }
}

fn is_target_label(s: &str) -> bool {
    s.starts_with("//") && s.contains(':')
}

fn rewrite_target_label(label: &str, namespace: &str) -> String {
    // If already fully qualified (contains multiple //), don't rewrite
    if label.matches("//").count() > 1 {
        return label.to_string();
    }

    // Check if this is a package-local label (starts with "//:")
    // For recursive builds: "//:target" in //pkg1 becomes "//pkg1:target"
    if let Some(target_name) = label.strip_prefix("//:") {
        return format!("{}:{}", namespace, target_name);
    }

    // Check if namespace is an external namespace (contains / after //)
    // External namespaces: "//external/boost", "//third_party/lib"
    // Package namespaces: "//pkg1", "//src"
    let ns_path = namespace.strip_prefix("//").unwrap_or(namespace);
    let is_external_namespace = ns_path.contains('/');

    if is_external_namespace {
        // For external namespaces: "//foo:bar" → "//external/boost//foo:bar"
        format!("{}{}", namespace, label)
    } else {
        // For package namespaces: "//other:target" stays as "//other:target"
        // Only package-local labels (//:target) are rewritten (handled above)
        label.to_string()
    }
}

pub fn parse_query_body(input: &str) -> Result<Vec<Predicate>, String> {
    let tokens = tokenize(input);
    let mut body = Vec::new();
    let mut i = 0;

    loop {
        if i >= tokens.len() {
            return Err("Unexpected end of input".to_string());
        }

        let (pred, next_i) = parse_predicate(&tokens[i..])?;
        body.push(pred);
        i += next_i;

        if i >= tokens.len() {
            return Err("Expected '.' or ',' after predicate".to_string());
        }

        match &tokens[i] {
            Token::Comma => {
                i += 1;
            }
            Token::Dot => {
                break;
            }
            _ => return Err("Expected '.' or ',' after predicate".to_string()),
        }
    }

    Ok(body)
}

fn parse_rule(tokens: &[Token<'_>]) -> Result<(Rule, usize), String> {
    let (head, mut i) = parse_predicate(tokens)?;

    if i >= tokens.len() || !matches!(tokens[i], Token::Rule) {
        return Err("Expected ':-' after head predicate".to_string());
    }
    i += 1;

    let mut body = Vec::new();
    loop {
        let (predicate, consumed) = parse_predicate(&tokens[i..])?;
        body.push(predicate);
        i += consumed;

        if i >= tokens.len() {
            return Err("Expected '.' or ',' after body predicate".to_string());
        }

        match &tokens[i] {
            Token::Comma => i += 1,
            Token::Dot => {
                i += 1;
                break;
            }
            _ => return Err("Expected '.' or ',' after body predicate".to_string()),
        }
    }

    Ok((Rule::new(head, body), i))
}

fn parse_predicate(tokens: &[Token<'_>]) -> Result<(Predicate, usize), String> {
    if tokens.is_empty() {
        return Err("Unexpected end of input".to_string());
    }

    let name = match &tokens[0] {
        Token::Ident(n) => n.to_string(),
        _ => return Err("Expected predicate name".to_string()),
    };

    if tokens.len() < 2 {
        return Err("Expected '(', '=', or '!=' after predicate name".to_string());
    }

    if matches!(tokens[1], Token::Eq) {
        if tokens.len() < 3 {
            return Err("Expected argument after '='".to_string());
        }
        let arg1 = match &tokens[0] {
            Token::Ident(s) if s.chars().next().unwrap().is_uppercase() => {
                Term::Variable(s.to_string())
            }
            _ => return Err("Expected variable before '='".to_string()),
        };
        let arg2 = match &tokens[2] {
            Token::Ident(s) if s.chars().next().unwrap().is_uppercase() => {
                Term::Variable(s.to_string())
            }
            Token::Ident(s) => Term::Constant(Value::String(s.to_string())),
            Token::String(s) => Term::Constant(Value::String(s.to_string())),
            Token::OwnedString(s) => Term::Constant(Value::String(s.clone())),
            Token::Int(n) => Term::Constant(Value::Integer(*n)),
            _ => return Err("Expected variable or constant after '='".to_string()),
        };
        return Ok((
            Predicate {
                name: "=".to_string(),
                args: vec![arg1, arg2],
            },
            3,
        ));
    }

    if matches!(tokens[1], Token::NotEq) {
        if tokens.len() < 3 {
            return Err("Expected argument after '!='".to_string());
        }
        let arg1 = match &tokens[0] {
            Token::Ident(s) if s.chars().next().unwrap().is_uppercase() => {
                Term::Variable(s.to_string())
            }
            _ => return Err("Expected variable before '!='".to_string()),
        };
        let arg2 = match &tokens[2] {
            Token::Ident(s) if s.chars().next().unwrap().is_uppercase() => {
                Term::Variable(s.to_string())
            }
            Token::Ident(s) => Term::Constant(Value::String(s.to_string())),
            Token::String(s) => Term::Constant(Value::String(s.to_string())),
            Token::OwnedString(s) => Term::Constant(Value::String(s.clone())),
            Token::Int(n) => Term::Constant(Value::Integer(*n)),
            _ => return Err("Expected variable or constant after '!='".to_string()),
        };
        return Ok((
            Predicate {
                name: "!=".to_string(),
                args: vec![arg1, arg2],
            },
            3,
        ));
    }

    if !matches!(tokens[1], Token::LParen) {
        return Err("Expected '(' after predicate name".to_string());
    }

    let mut i = 2;
    let mut args = Vec::new();
    let mut anon_counter = 0;

    if name == "not" {
        let (inner_pred, consumed) = parse_predicate(&tokens[i..])?;
        i += consumed;

        if i >= tokens.len() || !matches!(tokens[i], Token::RParen) {
            return Err("Expected ')' after not() predicate".to_string());
        }
        i += 1;

        return Ok((
            Predicate {
                name: format!("not:{}", inner_pred.name),
                args: inner_pred.args,
            },
            i,
        ));
    }

    while i < tokens.len() {
        match &tokens[i] {
            Token::RParen => {
                i += 1;
                break;
            }
            Token::Ident(s) => {
                if *s == "_" {
                    args.push(Term::Variable(format!("_anon_{}", anon_counter)));
                    anon_counter += 1;
                } else if s.chars().next().unwrap().is_uppercase() {
                    args.push(Term::Variable(s.to_string()));
                } else {
                    args.push(Term::Constant(Value::String(s.to_string())));
                }
                i += 1;
            }
            Token::String(s) => {
                args.push(Term::Constant(Value::String(s.to_string())));
                i += 1;
            }
            Token::OwnedString(s) => {
                args.push(Term::Constant(Value::String(s.clone())));
                i += 1;
            }
            Token::Int(n) => {
                args.push(Term::Constant(Value::Integer(*n)));
                i += 1;
            }
            Token::Comma => i += 1,
            _ => return Err(format!("Unexpected token in predicate: {:?}", tokens[i])),
        }
    }

    Ok((Predicate { name, args }, i))
}

fn tokenize(input: &str) -> Vec<Token<'_>> {
    let mut tokens = Vec::new();
    let bytes = input.as_bytes();
    let mut i = 0;

    'outer: while i < bytes.len() {
        match bytes[i] {
            b'#' => {
                while i < bytes.len() && bytes[i] != b'\n' {
                    i += 1;
                }
            }
            b' ' | b'\t' | b'\n' | b'\r' => {
                i += 1;
            }
            b'(' => {
                tokens.push(Token::LParen);
                i += 1;
            }
            b')' => {
                tokens.push(Token::RParen);
                i += 1;
            }
            b'.' => {
                tokens.push(Token::Dot);
                i += 1;
            }
            b',' => {
                tokens.push(Token::Comma);
                i += 1;
            }
            b':' => {
                i += 1;
                if i < bytes.len() && bytes[i] == b'-' {
                    i += 1;
                    tokens.push(Token::Rule);
                }
            }
            b'=' => {
                tokens.push(Token::Eq);
                i += 1;
            }
            b'!' => {
                i += 1;
                if i < bytes.len() && bytes[i] == b'=' {
                    i += 1;
                    tokens.push(Token::NotEq);
                }
            }
            b'"' => {
                i += 1;
                let start = i;

                // Fast path: scan for escape sequences
                let mut end = i;
                while end < bytes.len() && bytes[end] != b'"' {
                    if bytes[end] == b'\\' {
                        // Found escape - process from here
                        let mut string_content = String::with_capacity(end - i + 16);
                        // Copy prefix before first escape
                        string_content.push_str(&input[i..end]);
                        i = end;

                        while i < bytes.len() && bytes[i] != b'"' {
                            if bytes[i] == b'\\' && i + 1 < bytes.len() {
                                i += 1;
                                match bytes[i] {
                                    b'n' => string_content.push('\n'),
                                    b't' => string_content.push('\t'),
                                    b'r' => string_content.push('\r'),
                                    b'\\' => string_content.push('\\'),
                                    b'"' => string_content.push('"'),
                                    b'0' => string_content.push('\0'),
                                    _ => {
                                        string_content.push('\\');
                                        string_content.push(bytes[i] as char);
                                    }
                                }
                                i += 1;
                            } else {
                                string_content.push(bytes[i] as char);
                                i += 1;
                            }
                        }
                        tokens.push(Token::OwnedString(string_content));
                        i += 1;
                        continue 'outer;
                    }
                    end += 1;
                }

                // No escapes found - use zero-copy
                tokens.push(Token::String(&input[start..end]));
                i = end + 1;
            }
            b'0'..=b'9' => {
                let start = i;
                while i < bytes.len() && bytes[i].is_ascii_digit() {
                    i += 1;
                }
                if let Ok(num) = input[start..i].parse::<i64>() {
                    tokens.push(Token::Int(num));
                }
            }
            b'-' if i + 1 < bytes.len() && bytes[i + 1].is_ascii_digit() => {
                let start = i;
                i += 1;
                while i < bytes.len() && bytes[i].is_ascii_digit() {
                    i += 1;
                }
                if let Ok(num) = input[start..i].parse::<i64>() {
                    tokens.push(Token::Int(num));
                }
            }
            b'a'..=b'z' | b'A'..=b'Z' | b'_' => {
                let start = i;
                while i < bytes.len()
                    && matches!(bytes[i], b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'_')
                {
                    i += 1;
                }
                tokens.push(Token::Ident(&input[start..i]));
            }
            _ => {
                i += 1;
            }
        }
    }

    tokens
}

fn parse_facts(tokens: &[Token<'_>]) -> Result<Vec<Fact>, String> {
    let mut facts = Vec::new();
    let mut i = 0;

    while i < tokens.len() {
        if let Token::Ident(_name) = &tokens[i] {
            if i + 1 < tokens.len() && matches!(tokens[i + 1], Token::LParen) {
                let (fact, consumed) = parse_fact(&tokens[i..])?;
                facts.push(fact);
                i += consumed;
            } else {
                i += 1;
            }
        } else {
            i += 1;
        }
    }

    Ok(facts)
}

fn parse_fact(tokens: &[Token<'_>]) -> Result<(Fact, usize), String> {
    if tokens.is_empty() {
        return Err("Unexpected end of input".to_string());
    }

    let predicate = match &tokens[0] {
        Token::Ident(name) => name.to_string(),
        _ => return Err("Expected predicate name".to_string()),
    };

    if tokens.len() < 2 || !matches!(tokens[1], Token::LParen) {
        return Err("Expected '(' after predicate".to_string());
    }

    let mut i = 2;
    let mut args = Vec::new();

    while i < tokens.len() {
        match &tokens[i] {
            Token::RParen => {
                i += 1;
                break;
            }
            Token::String(s) => {
                args.push(Value::String(s.to_string()));
                i += 1;
            }
            Token::OwnedString(s) => {
                args.push(Value::String(s.clone()));
                i += 1;
            }
            Token::Int(n) => {
                args.push(Value::Integer(*n));
                i += 1;
            }
            Token::Ident(s) => {
                args.push(Value::String(s.to_string()));
                i += 1;
            }
            Token::Comma => {
                i += 1;
            }
            _ => return Err(format!("Unexpected token: {:?}", tokens[i])),
        }
    }

    if i >= tokens.len() || !matches!(tokens[i], Token::Dot) {
        return Err("Expected '.' after fact".to_string());
    }

    Ok((Fact::new(predicate, args), i + 1))
}

#[derive(Debug, Clone)]
enum Token<'a> {
    Ident(&'a str),
    String(&'a str),
    OwnedString(String),
    Int(i64),
    LParen,
    RParen,
    Dot,
    Comma,
    Rule,
    Eq,
    NotEq,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_simple_fact() {
        let input = r#"target("//app:cli")."#;
        let facts = parse(input).unwrap();
        assert_eq!(facts.len(), 1);
        assert_eq!(facts[0].predicate, "target");
    }

    #[test]
    fn parse_fact_with_multiple_args() {
        let input = r#"kind("//app:cli", rust_binary)."#;
        let facts = parse(input).unwrap();
        assert_eq!(facts.len(), 1);
        assert_eq!(facts[0].predicate, "kind");
        assert_eq!(facts[0].args.len(), 2);
    }

    #[test]
    fn parse_multiple_facts() {
        let input = r#"
            target("//app:cli").
            kind("//app:cli", rust_binary).
        "#;
        let facts = parse(input).unwrap();
        assert_eq!(facts.len(), 2);
    }

    #[test]
    fn parse_ignores_comments() {
        let input = r#"
            # This is a comment
            target("//app:cli").  # inline comment
        "#;
        let facts = parse(input).unwrap();
        assert_eq!(facts.len(), 1);
    }

    #[test]
    fn parse_handles_whitespace() {
        let input = "target(  \"//app:cli\"  )  .";
        let facts = parse(input).unwrap();
        assert_eq!(facts.len(), 1);
    }

    #[test]
    fn parse_rejects_invalid_syntax() {
        assert!(parse("target(").is_err());
        assert!(parse("target().").is_ok());
        let result = parse("target");
        assert!(result.is_ok() && result.unwrap().is_empty());
    }

    #[test]
    fn parse_simple_rule() {
        let input = r#"target(X) :- kind(X, rust_binary)."#;
        let (_facts, rules) = parse_program(input).unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].head.name, "target");
        assert_eq!(rules[0].body.len(), 1);
    }

    #[test]
    fn parse_rule_with_multiple_body_clauses() {
        let input = r#"
            target(X) :-
                kind(X, rust_binary),
                sources(X, Path),
                file_exists(Path).
        "#;
        let (_facts, rules) = parse_program(input).unwrap();
        assert_eq!(rules[0].body.len(), 3);
    }

    #[test]
    fn parse_rule_distinguishes_variables_and_constants() {
        let input = r#"target(X) :- kind(X, rust_binary)."#;
        let (_facts, rules) = parse_program(input).unwrap();

        match &rules[0].head.args[0] {
            Term::Variable(v) => assert_eq!(v, "X"),
            _ => panic!("Expected Variable"),
        }

        match &rules[0].body[0].args[1] {
            Term::Constant(Value::String(s)) => assert_eq!(s, "rust_binary"),
            _ => panic!("Expected Constant"),
        }
    }

    #[test]
    fn parse_mixed_facts_and_rules() {
        let input = r#"
            target("//app:cli").
            target(X) :- kind(X, rust_binary).
        "#;
        let (facts, rules) = parse_program(input).unwrap();
        assert_eq!(facts.len(), 1);
        assert_eq!(rules.len(), 1);
    }

    #[test]
    fn parse_program_with_file_handles_multiline_rules() {
        // This test verifies the fix for the bug where parse_program_with_file
        // was processing line-by-line and incorrectly creating facts from
        // parts of multi-line rules
        let input = r#"
# Comment line
target("//a").

# Multi-line rule that should NOT create spurious facts
needs_rebuild(Target) :-
    deps(Target, Dep),
    needs_rebuild(Dep).

another_fact("//b").
"#;

        let (facts, rules, locations) = parse_program_with_file(input, "test.datalog").unwrap();

        // Should have 2 facts, not 3 (the bug would create needs_rebuild("Dep") as a fact)
        assert_eq!(facts.len(), 2, "Should have exactly 2 facts");
        assert_eq!(facts[0].predicate, "target");
        assert_eq!(facts[1].predicate, "another_fact");

        // Verify no spurious "Dep" fact was created
        for fact in &facts {
            for arg in &fact.args {
                if let Value::String(s) = arg {
                    assert_ne!(
                        s, "Dep",
                        "Should not create fact with 'Dep' string from variable name"
                    );
                }
            }
        }

        // Should have 1 rule with 2 body predicates
        assert_eq!(rules.len(), 1, "Should have exactly 1 rule");
        assert_eq!(rules[0].head.name, "needs_rebuild");
        assert_eq!(
            rules[0].body.len(),
            2,
            "Multi-line rule should have 2 body predicates"
        );
        assert_eq!(rules[0].body[0].name, "deps");
        assert_eq!(rules[0].body[1].name, "needs_rebuild");

        // Verify locations were tracked
        assert!(!locations.is_empty(), "Should track source locations");
    }

    #[test]
    fn parse_program_with_file_handles_multiple_multiline_rules() {
        let input = r#"
# First rule
foo(X) :-
    bar(X),
    baz(X).

# Second rule
qux(Y) :-
    quux(Y),
    corge(Y),
    grault(Y).

# A fact
fact("test").
"#;

        let (facts, rules, _) = parse_program_with_file(input, "test.datalog").unwrap();

        assert_eq!(facts.len(), 1);
        assert_eq!(facts[0].predicate, "fact");

        assert_eq!(rules.len(), 2);
        assert_eq!(rules[0].body.len(), 2);
        assert_eq!(rules[1].body.len(), 3);
    }

    #[test]
    fn parse_program_with_file_tracks_correct_line_numbers() {
        let input = r#"# Comment on line 1
target("//first").
target("//second").

# Rule on line 5
my_rule(X) :- target(X).
"#;

        let (facts, rules, locations) = parse_program_with_file(input, "test.datalog").unwrap();

        assert_eq!(facts.len(), 2);
        assert_eq!(rules.len(), 1);

        // Check that each target is tracked with the correct line number
        assert_eq!(locations.get("//first").unwrap().line, 2);
        assert_eq!(locations.get("//second").unwrap().line, 3);

        // Check that the rule is tracked
        assert_eq!(locations.get("my_rule").unwrap().line, 6);
    }

    #[test]
    fn parse_program_with_file_line_numbers_comprehensive() {
        let input = r#"# Build configuration file
# Line 2 comment

cargo_binary("//redwood:redwood").
cargo_binary("//redwood:fake_tool").
cargo_binary("//redwood:another").

# Dependencies section starts on line 8
deps("//redwood:redwood", "//lib:core").
deps("//redwood:fake_tool", "//lib:utils").

# Multi-line rule on line 12
transitive_deps(Target, Dep) :-
    deps(Target, Dep).

transitive_deps(Target, Dep) :-
    deps(Target, X),
    transitive_deps(X, Dep).

# More facts
target("//app:main").
"#;

        let (facts, rules, locations) = parse_program_with_file(input, "BUILD.datalog").unwrap();

        assert_eq!(facts.len(), 6);
        assert_eq!(rules.len(), 2);

        assert_eq!(locations.get("//redwood:redwood").unwrap().line, 4);
        assert_eq!(locations.get("//redwood:fake_tool").unwrap().line, 5);
        assert_eq!(locations.get("//redwood:another").unwrap().line, 6);
        assert_eq!(locations.get("transitive_deps").unwrap().line, 13);
        assert_eq!(locations.get("//app:main").unwrap().line, 21);

        for (_, loc) in locations.iter() {
            assert_eq!(loc.file, "BUILD.datalog");
        }
    }

    #[test]
    fn parse_program_with_file_alias_handles_target_references_correctly() {
        // Test for the alias bug: when a target appears as both the second argument
        // of an alias and as the first argument of another predicate, the parser
        // should record the location where the target is actually defined, not where
        // it's referenced in the alias
        let input = r#"# Test file
alias("//bootstrap:redwood", "//bootstrap:bin").
system_rustc("//bootstrap:bin").
crate_name("//bootstrap:bin", "redwood").
"#;

        let (_facts, _rules, locations) = parse_program_with_file(input, "BUILD.datalog").unwrap();

        // The alias target should be on line 2 (where alias is defined)
        assert_eq!(locations.get("//bootstrap:redwood").unwrap().line, 2);

        // The actual target should be on line 3 (where system_rustc is defined),
        // NOT line 2 (where it appears as second argument of alias)
        assert_eq!(locations.get("//bootstrap:bin").unwrap().line, 3);
    }
}
