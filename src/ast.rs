//! AST for hsab v2 - Stack-based model
//!
//! The AST represents the parsed structure before evaluation.
//! Evaluation happens on a stack where:
//! - Literals push themselves
//! - Executables pop args, run, push output
//! - Blocks are deferred execution units

/// A value that can be on the stack
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    /// A literal string (argument, path, flag, etc.)
    Literal(String),
    /// Output from a command execution
    Output(String),
    /// A deferred block of expressions
    Block(Vec<Expr>),
    /// Nil - represents empty/no output
    Nil,
    /// Marker - boundary for spread/each/collect operations
    Marker,
}

impl Value {
    /// Convert value to string for use as command argument
    pub fn as_arg(&self) -> Option<String> {
        match self {
            Value::Literal(s) => Some(s.clone()),
            Value::Output(s) => {
                if s.is_empty() {
                    None // Treat empty output as nil
                } else {
                    Some(s.trim_end_matches('\n').to_string())
                }
            }
            Value::Block(_) => None, // Blocks can't be args directly
            Value::Nil => None,
            Value::Marker => None, // Markers can't be args
        }
    }

    /// Check if this is nil or empty
    pub fn is_nil(&self) -> bool {
        match self {
            Value::Nil => true,
            Value::Output(s) if s.is_empty() => true,
            _ => false,
        }
    }

    /// Check if this is a marker
    pub fn is_marker(&self) -> bool {
        matches!(self, Value::Marker)
    }
}

/// An expression in the hsab language
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    /// A literal value (string, path, flag, etc.)
    Literal(String),

    /// A quoted string (preserves quotes in output)
    Quoted { content: String, double: bool },

    /// A variable reference ($VAR or ${VAR})
    Variable(String),

    /// A block/quotation [...] - deferred execution
    Block(Vec<Expr>),

    /// Execute/apply: @ operator
    Apply,

    /// Pipe operator: |
    Pipe,

    /// Redirect operators
    RedirectOut,    // >
    RedirectAppend, // >>
    RedirectIn,     // <

    /// Background operator: &
    Background,

    /// Logical operators
    And, // &&
    Or,  // ||

    /// Stack operations
    Dup,
    Swap,
    Drop,
    Over,
    Rot,

    /// Path operations
    Join,
    Basename,
    Dirname,
    Suffix,
    Reext,

    /// List operations
    Spread,  // Split multi-line value into separate stack items
    Each,    // Apply block to each item on stack (until marker)
    Collect, // Gather stack items back into single value

    /// Control flow
    If, // [condition] [then] [else] if

    /// Bash passthrough
    BashPassthrough(String),

    /// Define a named word: :name (pops block from stack, stores it)
    Define(String),
}

/// A parsed hsab program is a sequence of expressions
#[derive(Debug, Clone, PartialEq)]
pub struct Program {
    pub expressions: Vec<Expr>,
}

impl Program {
    pub fn new(expressions: Vec<Expr>) -> Self {
        Program { expressions }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_value_as_arg() {
        assert_eq!(Value::Literal("hello".into()).as_arg(), Some("hello".into()));
        assert_eq!(Value::Output("world\n".into()).as_arg(), Some("world".into()));
        assert_eq!(Value::Nil.as_arg(), None);
        assert_eq!(Value::Output("".into()).as_arg(), None);
    }

    #[test]
    fn test_value_is_nil() {
        assert!(Value::Nil.is_nil());
        assert!(Value::Output("".into()).is_nil());
        assert!(!Value::Literal("x".into()).is_nil());
        assert!(!Value::Output("x".into()).is_nil());
    }
}
