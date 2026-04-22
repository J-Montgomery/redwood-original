use crate::datalog::{Predicate, Term, Value};
use std::collections::HashMap;
use std::sync::Arc;

pub fn is_special_predicate(name: &str) -> bool {
    name.starts_with("not:")
        || name == "="
        || name == "!="
        || name == "gt"
        || name == "lt"
        || name == "concat"
        || name == "matches_glob"
        || name == "split"
        || name == "prefix"
        || name == "suffix"
        || name == "substring"
        || name == "contains"
        || name == "strip_prefix"
        || name == "strip_suffix"
        || name == "before_char"
        || name == "after_char"
        || name == "resolve"
        || name == "count"
        || name == "min"
        || name == "max"
        || name == "parse_int"
        || name == "to_string"
        || name == "add"
        || name == "sub"
        || name == "mul"
        || name == "div"
        || name == "mod"
        || name == "source_location"
}

pub fn eval_equality(
    predicate: &Predicate,
    current_bindings: Vec<Arc<HashMap<String, Value>>>,
) -> Vec<Arc<HashMap<String, Value>>> {
    if predicate.args.len() != 2 {
        eprintln!(
            "Syntax error: = requires exactly 2 arguments, got {}",
            predicate.args.len()
        );
        std::process::exit(1);
    }

    let mut result = Vec::new();
    for binding in current_bindings {
        match (&predicate.args[0], &predicate.args[1]) {
            (Term::Variable(v), Term::Constant(c)) | (Term::Constant(c), Term::Variable(v)) => {
                if let Some(val) = binding.get(v) {
                    if val == c {
                        result.push(binding);
                    }
                } else {
                    let mut new_binding = Arc::clone(&binding);
                    Arc::make_mut(&mut new_binding).insert(v.clone(), c.clone());
                    result.push(new_binding);
                }
            }
            (Term::Variable(v1), Term::Variable(v2)) => {
                let val1 = binding.get(v1);
                let val2 = binding.get(v2);
                match (val1, val2) {
                    (Some(a), Some(b)) if a == b => result.push(binding),
                    (Some(a), None) => {
                        let mut new_binding = Arc::clone(&binding);
                    Arc::make_mut(&mut new_binding).insert(v2.clone(), a.clone());
                        result.push(new_binding);
                    }
                    (None, Some(b)) => {
                        let mut new_binding = Arc::clone(&binding);
                    Arc::make_mut(&mut new_binding).insert(v1.clone(), b.clone());
                        result.push(new_binding);
                    }
                    (None, None) => {
                        eprintln!(
                            "Error: equality (=) with two unbound variables ({}, {}) is not supported",
                            v1, v2
                        );
                        std::process::exit(1);
                    }
                    _ => {}
                }
            }
            (Term::Constant(c1), Term::Constant(c2)) => {
                if c1 == c2 {
                    result.push(binding);
                }
            }
        }
    }
    result
}

pub fn eval_inequality(
    predicate: &Predicate,
    current_bindings: Vec<Arc<HashMap<String, Value>>>,
) -> Vec<Arc<HashMap<String, Value>>> {
    if predicate.args.len() != 2 {
        eprintln!(
            "Syntax error: != requires exactly 2 arguments, got {}",
            predicate.args.len()
        );
        std::process::exit(1);
    }

    let mut result = Vec::new();
    for binding in current_bindings {
        let val1 = get_value(&predicate.args[0], &binding);
        let val2 = get_value(&predicate.args[1], &binding);

        if let (Some(v1), Some(v2)) = (val1, val2) {
            match (&v1, &v2) {
                (Value::Integer(a), Value::Integer(b)) => {
                    if a != b {
                        result.push(binding);
                    }
                }
                (Value::String(a), Value::String(b)) => {
                    if a != b {
                        result.push(binding);
                    }
                }
                (v1, v2) => {
                    eprintln!(
                        "Type error: != requires matching types, got {:?} and {:?}",
                        v1, v2
                    );
                    std::process::exit(1);
                }
            }
        }
    }
    result
}

pub fn eval_comparison(
    predicate: &Predicate,
    current_bindings: Vec<Arc<HashMap<String, Value>>>,
) -> Vec<Arc<HashMap<String, Value>>> {
    if predicate.args.len() != 2 {
        eprintln!(
            "Syntax error: {} requires exactly 2 arguments, got {}",
            predicate.name,
            predicate.args.len()
        );
        std::process::exit(1);
    }

    let mut result = Vec::new();
    for binding in current_bindings {
        let val1 = get_value(&predicate.args[0], &binding);
        let val2 = get_value(&predicate.args[1], &binding);
        require_bound(&val1, &predicate.name, "first argument");
        require_bound(&val2, &predicate.name, "second argument");

        if let (Some(v1), Some(v2)) = (val1, val2) {
            match (v1, v2) {
                (Value::Integer(a), Value::Integer(b)) => {
                    let cmp_result = match predicate.name.as_str() {
                        "gt" => a > b,
                        "lt" => a < b,
                        _ => false,
                    };
                    if cmp_result {
                        result.push(binding);
                    }
                }
                (v1, v2) => {
                    eprintln!(
                        "Type error: {} requires integer arguments, got {:?} and {:?}",
                        predicate.name, v1, v2
                    );
                    std::process::exit(1);
                }
            }
        }
    }
    result
}

pub fn eval_concat(
    predicate: &Predicate,
    current_bindings: Vec<Arc<HashMap<String, Value>>>,
) -> Vec<Arc<HashMap<String, Value>>> {
    if predicate.args.len() != 3 {
        eprintln!(
            "Syntax error: concat requires exactly 3 arguments, got {}",
            predicate.args.len()
        );
        std::process::exit(1);
    }

    let mut result = Vec::new();
    for binding in current_bindings {
        let val1 = get_value(&predicate.args[0], &binding);
        let val2 = get_value(&predicate.args[1], &binding);
        let val3_arg = &predicate.args[2];

        if let (Some(v1), Some(v2)) = (val1, val2) {
            match (v1, v2) {
                (Value::String(s1), Value::String(s2)) => {
                    let concatenated = Value::String(format!("{}{}", s1, s2));

                    match val3_arg {
                        Term::Variable(var_name) => {
                            if let Some(existing) = binding.get(var_name) {
                                if existing == &concatenated {
                                    result.push(binding);
                                }
                            } else {
                                let mut new_binding = Arc::clone(&binding);
                                Arc::make_mut(&mut new_binding).insert(var_name.clone(), concatenated);
                                result.push(new_binding);
                            }
                        }
                        Term::Constant(expected) => {
                            if expected == &concatenated {
                                result.push(binding);
                            }
                        }
                    }
                }
                (v1, v2) => {
                    eprintln!(
                        "Type error: concat requires string arguments, got {:?} and {:?}",
                        v1, v2
                    );
                    std::process::exit(1);
                }
            }
        }
    }
    result
}

fn get_value(term: &Term, binding: &HashMap<String, Value>) -> Option<Value> {
    match term {
        Term::Variable(var) => binding.get(var).cloned(),
        Term::Constant(val) => Some(val.clone()),
    }
}

fn require_bound(val: &Option<Value>, predicate_name: &str, arg_name: &str) {
    if val.is_none() {
        eprintln!(
            "Error: {} requires {} to be bound",
            predicate_name, arg_name
        );
        std::process::exit(1);
    }
}

pub fn eval_prefix(
    predicate: &Predicate,
    current_bindings: Vec<Arc<HashMap<String, Value>>>,
) -> Vec<Arc<HashMap<String, Value>>> {
    if predicate.args.len() != 2 {
        eprintln!(
            "Syntax error: prefix requires exactly 2 arguments, got {}",
            predicate.args.len()
        );
        std::process::exit(1);
    }

    let mut result = Vec::new();
    for binding in current_bindings {
        let val1 = get_value(&predicate.args[0], &binding);
        let val2 = get_value(&predicate.args[1], &binding);

        if let (Some(Value::String(s)), Some(Value::String(prefix))) = (val1, val2) {
            if s.starts_with(&prefix) {
                result.push(binding);
            }
        }
    }
    result
}

pub fn eval_suffix(
    predicate: &Predicate,
    current_bindings: Vec<Arc<HashMap<String, Value>>>,
) -> Vec<Arc<HashMap<String, Value>>> {
    if predicate.args.len() != 2 {
        eprintln!(
            "Syntax error: suffix requires exactly 2 arguments, got {}",
            predicate.args.len()
        );
        std::process::exit(1);
    }

    let mut result = Vec::new();
    for binding in current_bindings {
        let val1 = get_value(&predicate.args[0], &binding);
        let val2 = get_value(&predicate.args[1], &binding);

        if let (Some(Value::String(s)), Some(Value::String(suffix))) = (val1, val2) {
            if s.ends_with(&suffix) {
                result.push(binding);
            }
        }
    }
    result
}

pub fn eval_contains(
    predicate: &Predicate,
    current_bindings: Vec<Arc<HashMap<String, Value>>>,
) -> Vec<Arc<HashMap<String, Value>>> {
    if predicate.args.len() != 2 {
        eprintln!(
            "Syntax error: contains requires exactly 2 arguments, got {}",
            predicate.args.len()
        );
        std::process::exit(1);
    }

    let mut result = Vec::new();
    for binding in current_bindings {
        let val1 = get_value(&predicate.args[0], &binding);
        let val2 = get_value(&predicate.args[1], &binding);

        if let (Some(Value::String(s)), Some(Value::String(substring))) = (val1, val2) {
            if s.contains(&substring) {
                result.push(binding);
            }
        }
    }
    result
}

pub fn eval_substring(
    predicate: &Predicate,
    current_bindings: Vec<Arc<HashMap<String, Value>>>,
) -> Vec<Arc<HashMap<String, Value>>> {
    if predicate.args.len() != 4 {
        eprintln!(
            "Syntax error: substring requires exactly 4 arguments, got {}",
            predicate.args.len()
        );
        std::process::exit(1);
    }

    let mut result = Vec::new();
    for binding in current_bindings {
        let val_str = get_value(&predicate.args[0], &binding);
        let val_start = get_value(&predicate.args[1], &binding);
        let val_end = get_value(&predicate.args[2], &binding);
        let result_arg = &predicate.args[3];

        if let (Some(Value::String(s)), Some(Value::Integer(start)), Some(Value::Integer(end))) =
            (val_str, val_start, val_end)
        {
            let start = start.max(0) as usize;
            let end = end.max(0) as usize;

            if start <= s.len() && end <= s.len() && start <= end {
                let substring = Value::String(s[start..end].to_string());

                match result_arg {
                    Term::Variable(var_name) => {
                        if let Some(existing) = binding.get(var_name) {
                            if existing == &substring {
                                result.push(binding);
                            }
                        } else {
                            let mut new_binding = Arc::clone(&binding);
                    Arc::make_mut(&mut new_binding).insert(var_name.clone(), substring);
                            result.push(new_binding);
                        }
                    }
                    Term::Constant(expected) => {
                        if expected == &substring {
                            result.push(binding);
                        }
                    }
                }
            }
        }
    }
    result
}

pub fn eval_parse_int(
    predicate: &Predicate,
    current_bindings: Vec<Arc<HashMap<String, Value>>>,
) -> Vec<Arc<HashMap<String, Value>>> {
    if predicate.args.len() != 2 {
        eprintln!(
            "Syntax error: parse_int requires exactly 2 arguments, got {}",
            predicate.args.len()
        );
        std::process::exit(1);
    }

    let mut result = Vec::new();
    for binding in current_bindings {
        let val_str = get_value(&predicate.args[0], &binding);
        require_bound(&val_str, "parse_int", "first argument (string)");
        let result_arg = &predicate.args[1];

        if let Some(Value::String(s)) = val_str {
            if let Ok(parsed) = s.parse::<i64>() {
                let int_value = Value::Integer(parsed);

                match result_arg {
                    Term::Variable(var_name) => {
                        if let Some(existing) = binding.get(var_name) {
                            if existing == &int_value {
                                result.push(binding);
                            }
                        } else {
                            let mut new_binding = Arc::clone(&binding);
                    Arc::make_mut(&mut new_binding).insert(var_name.clone(), int_value);
                            result.push(new_binding);
                        }
                    }
                    Term::Constant(expected) => {
                        if expected == &int_value {
                            result.push(binding);
                        }
                    }
                }
            }
        }
    }
    result
}

pub fn eval_to_string(
    predicate: &Predicate,
    current_bindings: Vec<Arc<HashMap<String, Value>>>,
) -> Vec<Arc<HashMap<String, Value>>> {
    if predicate.args.len() != 2 {
        eprintln!(
            "Syntax error: to_string requires exactly 2 arguments, got {}",
            predicate.args.len()
        );
        std::process::exit(1);
    }

    let mut result = Vec::new();
    for binding in current_bindings {
        let val_int = get_value(&predicate.args[0], &binding);
        require_bound(&val_int, "to_string", "first argument (integer)");
        let result_arg = &predicate.args[1];

        if let Some(Value::Integer(n)) = val_int {
            let string_value = Value::String(n.to_string());

            match result_arg {
                Term::Variable(var_name) => {
                    if let Some(existing) = binding.get(var_name) {
                        if existing == &string_value {
                            result.push(binding);
                        }
                    } else {
                        let mut new_binding = Arc::clone(&binding);
                    Arc::make_mut(&mut new_binding).insert(var_name.clone(), string_value);
                        result.push(new_binding);
                    }
                }
                Term::Constant(expected) => {
                    if expected == &string_value {
                        result.push(binding);
                    }
                }
            }
        }
    }
    result
}

pub fn eval_add(
    predicate: &Predicate,
    current_bindings: Vec<Arc<HashMap<String, Value>>>,
) -> Vec<Arc<HashMap<String, Value>>> {
    if predicate.args.len() != 3 {
        eprintln!(
            "Syntax error: add requires exactly 3 arguments, got {}",
            predicate.args.len()
        );
        std::process::exit(1);
    }

    let mut result = Vec::new();
    for binding in current_bindings {
        let val1 = get_value(&predicate.args[0], &binding);
        let val2 = get_value(&predicate.args[1], &binding);
        require_bound(&val1, "add", "first argument");
        require_bound(&val2, "add", "second argument");
        let result_arg = &predicate.args[2];

        if let (Some(Value::Integer(a)), Some(Value::Integer(b))) = (val1, val2) {
            let sum = Value::Integer(a + b);

            match result_arg {
                Term::Variable(var_name) => {
                    if let Some(existing) = binding.get(var_name) {
                        if existing == &sum {
                            result.push(binding);
                        }
                    } else {
                        let mut new_binding = Arc::clone(&binding);
                    Arc::make_mut(&mut new_binding).insert(var_name.clone(), sum);
                        result.push(new_binding);
                    }
                }
                Term::Constant(expected) => {
                    if expected == &sum {
                        result.push(binding);
                    }
                }
            }
        }
    }
    result
}

pub fn eval_sub(
    predicate: &Predicate,
    current_bindings: Vec<Arc<HashMap<String, Value>>>,
) -> Vec<Arc<HashMap<String, Value>>> {
    if predicate.args.len() != 3 {
        eprintln!(
            "Syntax error: sub requires exactly 3 arguments, got {}",
            predicate.args.len()
        );
        std::process::exit(1);
    }

    let mut result = Vec::new();
    for binding in current_bindings {
        let val1 = get_value(&predicate.args[0], &binding);
        let val2 = get_value(&predicate.args[1], &binding);
        require_bound(&val1, "sub", "first argument");
        require_bound(&val2, "sub", "second argument");
        let result_arg = &predicate.args[2];

        if let (Some(Value::Integer(a)), Some(Value::Integer(b))) = (val1, val2) {
            let diff = Value::Integer(a - b);

            match result_arg {
                Term::Variable(var_name) => {
                    if let Some(existing) = binding.get(var_name) {
                        if existing == &diff {
                            result.push(binding);
                        }
                    } else {
                        let mut new_binding = Arc::clone(&binding);
                    Arc::make_mut(&mut new_binding).insert(var_name.clone(), diff);
                        result.push(new_binding);
                    }
                }
                Term::Constant(expected) => {
                    if expected == &diff {
                        result.push(binding);
                    }
                }
            }
        }
    }
    result
}

pub fn eval_mul(
    predicate: &Predicate,
    current_bindings: Vec<Arc<HashMap<String, Value>>>,
) -> Vec<Arc<HashMap<String, Value>>> {
    if predicate.args.len() != 3 {
        eprintln!(
            "Syntax error: mul requires exactly 3 arguments, got {}",
            predicate.args.len()
        );
        std::process::exit(1);
    }

    let mut result = Vec::new();
    for binding in current_bindings {
        let val1 = get_value(&predicate.args[0], &binding);
        let val2 = get_value(&predicate.args[1], &binding);
        require_bound(&val1, "mul", "first argument");
        require_bound(&val2, "mul", "second argument");
        let result_arg = &predicate.args[2];

        if let (Some(Value::Integer(a)), Some(Value::Integer(b))) = (val1, val2) {
            let prod = Value::Integer(a * b);

            match result_arg {
                Term::Variable(var_name) => {
                    if let Some(existing) = binding.get(var_name) {
                        if existing == &prod {
                            result.push(binding);
                        }
                    } else {
                        let mut new_binding = Arc::clone(&binding);
                    Arc::make_mut(&mut new_binding).insert(var_name.clone(), prod);
                        result.push(new_binding);
                    }
                }
                Term::Constant(expected) => {
                    if expected == &prod {
                        result.push(binding);
                    }
                }
            }
        }
    }
    result
}

pub fn eval_div(
    predicate: &Predicate,
    current_bindings: Vec<Arc<HashMap<String, Value>>>,
) -> Vec<Arc<HashMap<String, Value>>> {
    if predicate.args.len() != 3 {
        eprintln!(
            "Syntax error: div requires exactly 3 arguments, got {}",
            predicate.args.len()
        );
        std::process::exit(1);
    }

    let mut result = Vec::new();
    for binding in current_bindings {
        let val1 = get_value(&predicate.args[0], &binding);
        let val2 = get_value(&predicate.args[1], &binding);
        require_bound(&val1, "div", "first argument");
        require_bound(&val2, "div", "second argument");
        let result_arg = &predicate.args[2];

        if let (Some(Value::Integer(a)), Some(Value::Integer(b))) = (val1, val2) {
            if b != 0 {
                let quot = Value::Integer(a / b);

                match result_arg {
                    Term::Variable(var_name) => {
                        if let Some(existing) = binding.get(var_name) {
                            if existing == &quot {
                                result.push(binding);
                            }
                        } else {
                            let mut new_binding = Arc::clone(&binding);
                    Arc::make_mut(&mut new_binding).insert(var_name.clone(), quot);
                            result.push(new_binding);
                        }
                    }
                    Term::Constant(expected) => {
                        if expected == &quot {
                            result.push(binding);
                        }
                    }
                }
            }
        }
    }
    result
}

pub fn eval_mod(
    predicate: &Predicate,
    current_bindings: Vec<Arc<HashMap<String, Value>>>,
) -> Vec<Arc<HashMap<String, Value>>> {
    if predicate.args.len() != 3 {
        eprintln!(
            "Syntax error: mod requires exactly 3 arguments, got {}",
            predicate.args.len()
        );
        std::process::exit(1);
    }

    let mut result = Vec::new();
    for binding in current_bindings {
        let val1 = get_value(&predicate.args[0], &binding);
        let val2 = get_value(&predicate.args[1], &binding);
        require_bound(&val1, "mod", "first argument");
        require_bound(&val2, "mod", "second argument");
        let result_arg = &predicate.args[2];

        if let (Some(Value::Integer(a)), Some(Value::Integer(b))) = (val1, val2) {
            if b != 0 {
                let rem = Value::Integer(a % b);

                match result_arg {
                    Term::Variable(var_name) => {
                        if let Some(existing) = binding.get(var_name) {
                            if existing == &rem {
                                result.push(binding);
                            }
                        } else {
                            let mut new_binding = Arc::clone(&binding);
                    Arc::make_mut(&mut new_binding).insert(var_name.clone(), rem);
                            result.push(new_binding);
                        }
                    }
                    Term::Constant(expected) => {
                        if expected == &rem {
                            result.push(binding);
                        }
                    }
                }
            }
        }
    }
    result
}

pub fn eval_strip_prefix(
    predicate: &Predicate,
    current_bindings: Vec<Arc<HashMap<String, Value>>>,
) -> Vec<Arc<HashMap<String, Value>>> {
    if predicate.args.len() != 3 {
        eprintln!(
            "Syntax error: strip_prefix requires exactly 3 arguments, got {}",
            predicate.args.len()
        );
        std::process::exit(1);
    }

    let mut result = Vec::new();
    for binding in current_bindings {
        let val_str = get_value(&predicate.args[0], &binding);
        let val_prefix = get_value(&predicate.args[1], &binding);
        require_bound(&val_str, "strip_prefix", "first argument (string)");
        require_bound(&val_prefix, "strip_prefix", "second argument (prefix)");
        let result_arg = &predicate.args[2];

        if let (Some(Value::String(s)), Some(Value::String(prefix))) = (val_str, val_prefix) {
            if let Some(stripped) = s.strip_prefix(&prefix) {
                let result_value = Value::String(stripped.to_string());

                match result_arg {
                    Term::Variable(var_name) => {
                        if let Some(existing) = binding.get(var_name) {
                            if existing == &result_value {
                                result.push(binding);
                            }
                        } else {
                            let mut new_binding = Arc::clone(&binding);
                    Arc::make_mut(&mut new_binding).insert(var_name.clone(), result_value);
                            result.push(new_binding);
                        }
                    }
                    Term::Constant(expected) => {
                        if expected == &result_value {
                            result.push(binding);
                        }
                    }
                }
            }
        }
    }
    result
}

pub fn eval_before_char(
    predicate: &Predicate,
    current_bindings: Vec<Arc<HashMap<String, Value>>>,
) -> Vec<Arc<HashMap<String, Value>>> {
    if predicate.args.len() != 3 {
        eprintln!(
            "Syntax error: before_char requires exactly 3 arguments, got {}",
            predicate.args.len()
        );
        std::process::exit(1);
    }

    let mut result = Vec::new();
    for binding in current_bindings {
        let val_str = get_value(&predicate.args[0], &binding);
        let val_char = get_value(&predicate.args[1], &binding);
        require_bound(&val_str, "before_char", "first argument (string)");
        require_bound(&val_char, "before_char", "second argument (char)");
        let result_arg = &predicate.args[2];

        if let (Some(Value::String(s)), Some(Value::String(ch))) = (val_str, val_char) {
            if let Some(pos) = s.find(&ch) {
                let result_value = Value::String(s[..pos].to_string());

                match result_arg {
                    Term::Variable(var_name) => {
                        if let Some(existing) = binding.get(var_name) {
                            if existing == &result_value {
                                result.push(binding);
                            }
                        } else {
                            let mut new_binding = Arc::clone(&binding);
                    Arc::make_mut(&mut new_binding).insert(var_name.clone(), result_value);
                            result.push(new_binding);
                        }
                    }
                    Term::Constant(expected) => {
                        if expected == &result_value {
                            result.push(binding);
                        }
                    }
                }
            }
        }
    }
    result
}

pub fn eval_after_char(
    predicate: &Predicate,
    current_bindings: Vec<Arc<HashMap<String, Value>>>,
) -> Vec<Arc<HashMap<String, Value>>> {
    if predicate.args.len() != 3 {
        eprintln!(
            "Syntax error: after_char requires exactly 3 arguments, got {}",
            predicate.args.len()
        );
        std::process::exit(1);
    }

    let mut result = Vec::new();
    for binding in current_bindings {
        let val_str = get_value(&predicate.args[0], &binding);
        let val_char = get_value(&predicate.args[1], &binding);
        require_bound(&val_str, "after_char", "first argument (string)");
        require_bound(&val_char, "after_char", "second argument (char)");
        let result_arg = &predicate.args[2];

        if let (Some(Value::String(s)), Some(Value::String(ch))) = (val_str, val_char) {
            if let Some(pos) = s.find(&ch) {
                let start = pos + ch.len();
                if start <= s.len() {
                    let result_value = Value::String(s[start..].to_string());

                    match result_arg {
                        Term::Variable(var_name) => {
                            if let Some(existing) = binding.get(var_name) {
                                if existing == &result_value {
                                    result.push(binding);
                                }
                            } else {
                                let mut new_binding = Arc::clone(&binding);
                    Arc::make_mut(&mut new_binding).insert(var_name.clone(), result_value);
                                result.push(new_binding);
                            }
                        }
                        Term::Constant(expected) => {
                            if expected == &result_value {
                                result.push(binding);
                            }
                        }
                    }
                }
            }
        }
    }
    result
}
