mod builtins;
mod engine;
pub mod parser;

pub use engine::Engine;

use std::fmt;
use std::path::PathBuf;

#[derive(Clone, Hash, PartialEq, Eq, Debug, PartialOrd, Ord)]
pub struct SourceLocation {
    pub file: String,
    pub line: usize,
}

#[derive(Clone, Hash, PartialEq, Eq, Debug, PartialOrd, Ord)]
pub struct TargetLabel {
    pub package: String,
    pub target: String,
}

#[derive(Clone, Hash, PartialEq, Eq, Debug, PartialOrd, Ord)]
pub enum Value {
    String(String),
    Integer(i64),
    Bool(bool),
    Label(TargetLabel),
    Path(PathBuf),
}

#[derive(Clone, Hash, PartialEq, Eq, Debug, PartialOrd, Ord)]
pub struct Fact {
    pub predicate: String,
    pub args: Vec<Value>,
}

impl Fact {
    pub fn new(predicate: impl Into<String>, args: Vec<Value>) -> Self {
        Self {
            predicate: predicate.into(),
            args,
        }
    }
}

#[derive(Clone, Hash, PartialEq, Eq, Debug)]
pub enum Term {
    Variable(String),
    Constant(Value),
}

#[derive(Clone, Hash, PartialEq, Eq, Debug)]
pub struct Predicate {
    pub name: String,
    pub args: Vec<Term>,
}

#[derive(Clone, Debug)]
pub struct Rule {
    pub head: Predicate,
    pub body: Vec<Predicate>,
}

impl Rule {
    pub fn new(head: Predicate, body: Vec<Predicate>) -> Self {
        Self { head, body }
    }
}

impl TargetLabel {
    pub fn new(package: impl Into<String>, target: impl Into<String>) -> Self {
        Self {
            package: package.into(),
            target: target.into(),
        }
    }

    pub fn parse(s: &str) -> Result<Self, String> {
        let parts: Vec<&str> = s.split(':').collect();
        if parts.len() != 2 {
            return Err(format!("Invalid target label: {}", s));
        }
        if !parts[0].starts_with("//") {
            return Err(format!("Target label must start with //: {}", s));
        }
        Ok(Self {
            package: parts[0].to_string(),
            target: parts[1].to_string(),
        })
    }
}

impl fmt::Display for TargetLabel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.package, self.target)
    }
}

impl Value {
    pub fn as_string(&self) -> Option<&str> {
        match self {
            Value::String(s) => Some(s),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_target_label() {
        let label = TargetLabel::new("//app", "cli");
        assert_eq!(label.package, "//app");
        assert_eq!(label.target, "cli");
    }

    #[test]
    fn create_fact_with_string_value() {
        let fact = Fact::new("target", vec![Value::String("//app:cli".to_string())]);
        assert_eq!(fact.predicate, "target");
        assert_eq!(fact.args.len(), 1);
    }

    #[test]
    fn target_label_display() {
        let label = TargetLabel::new("//app", "cli");
        assert_eq!(label.to_string(), "//app:cli");
    }

    #[test]
    fn parse_target_label_from_string() {
        let label = TargetLabel::parse("//app:cli").unwrap();
        assert_eq!(label.package, "//app");
        assert_eq!(label.target, "cli");
    }

    #[test]
    fn parse_target_label_invalid() {
        assert!(TargetLabel::parse("invalid").is_err());
        assert!(TargetLabel::parse("//app").is_err());
        assert!(TargetLabel::parse("app:cli").is_err());
    }
}
