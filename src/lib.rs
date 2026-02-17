//! hsab v2 - Hash Backwards
//!
//! # Overview
//!
//! hsab (Hash Backwards) is a stack-based postfix shell. Instead of writing
//! `command args`, you push args to the stack and then execute the command.
//! Executables pop their arguments and push their output.
//!
//! # Core Concepts
//!
//! ## Stack-Based Execution
//!
//! ```text
//! # Literals push themselves to the stack
//! hello world          # Stack: [hello, world]
//!
//! # Executables pop args, run, push output
//! hello echo           # echo hello -> Stack: [output]
//!
//! # LIFO ordering (true stack semantics)
//! dest src cp          # cp dest src (pops dest first, then src)
//! ```
//!
//! ## Command Substitution
//!
//! ```text
//! # Output threads through as arguments
//! pwd ls               # ls $(pwd)
//!
//! # Empty output becomes nil (skipped)
//! true ls              # ls (true produces no output)
//! ```
//!
//! ## Blocks (Deferred Execution)
//!
//! ```text
//! # Blocks are pushed without execution
//! [hello echo]         # Stack: [Block([hello, echo])]
//!
//! # apply executes a block (exec is an alias)
//! [hello echo] apply   # Runs: echo hello
//!
//! # Pipe (|) connects producer to consumer
//! ls [grep txt] |      # ls | grep txt
//! ```
//!
//! # Example
//!
//! ```rust
//! use hsab::{lex, parse, Evaluator};
//!
//! let tokens = lex("hello echo").unwrap();
//! let program = parse(tokens).unwrap();
//! let mut eval = Evaluator::new();
//! let result = eval.eval(&program).unwrap();
//! // result.output contains "hello\n"
//! ```

pub mod ast;
pub mod display;
pub mod eval;
pub mod lexer;
pub mod parser;
#[cfg(feature = "plugins")]
pub mod plugin;
pub mod resolver;
pub mod signals;

// Re-export commonly used items
pub use ast::{Expr, Program, Value, FutureState};
pub use eval::{EvalError, EvalResult, Evaluator};
pub use lexer::{lex, LexError, Operator, Token};
pub use parser::{parse, ParseError};
#[cfg(feature = "plugins")]
pub use plugin::{PluginError, PluginHost, PluginManifest};
pub use resolver::ExecutableResolver;

/// Convenience function to evaluate an hsab expression
pub fn eval(input: &str) -> Result<EvalResult, String> {
    let tokens = lex(input).map_err(|e| e.to_string())?;
    let program = parse(tokens).map_err(|e| e.to_string())?;
    let mut evaluator = Evaluator::new();
    evaluator.eval(&program).map_err(|e| e.to_string())
}
