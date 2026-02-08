//! hsab - Hash Backwards
//!
//! # Overview
//!
//! hsab (Hash Backwards) is a postfix notation shell that transpiles to bash.
//! Instead of `command args`, you write `args command`. With executable-aware
//! parsing, commands are auto-detected. Parsing stops at the first executable
//! found; remaining tokens become leftovers for the next command.
//!
//! # Syntax
//!
//! ## Executable-Aware Parsing
//!
//! ```text
//! # Args accumulate until an executable is found
//! -la ls                    # ls -la
//! hello grep                # grep hello
//!
//! # Only first executable is parsed; rest are leftovers
//! -la ls hello grep         # ls -la (hello grep are leftovers)
//! ```
//!
//! ## Explicit Grouping with %()
//!
//! ```text
//! # Use %() for explicit postfix grouping and pipes
//! %(hello grep) ls           # ls | grep hello
//! %(pattern grep) file cat   # cat file | grep pattern
//!
//! # Logic ops: execution order (control flow)
//! ls %(done echo) &&         # ls && echo done
//! ls %(error echo) ||        # ls || echo error
//!
//! # Redirects: execution order
//! hello echo %(file.txt) >   # echo hello > file.txt
//!
//! # Background
//! 10 sleep &                 # sleep 10 &
//! ```
//!
//! # Example
//!
//! ```rust
//! use hsab::compile;
//!
//! // Executable-aware: args before command
//! let bash = compile("-la ls").unwrap();
//! assert_eq!(bash, "ls -la");
//!
//! // Use groups for pipes
//! let bash = compile("%(hello grep) ls").unwrap();
//! assert_eq!(bash, "ls | grep hello");
//! ```

pub mod ast;
pub mod emitter;
pub mod executor;
pub mod lexer;
pub mod parser;
pub mod resolver;
pub mod shell;
pub mod state;
pub mod transformer;

// Re-export commonly used items
pub use ast::{Ast, RedirectMode};
pub use emitter::{compile, emit};
pub use executor::{execute, execute_bash, execute_interactive, execute_line, ExecuteError, ExecuteResult};
pub use lexer::{lex, LexError, Operator, Token};
pub use parser::{parse, ParseError};
pub use resolver::ExecutableResolver;
pub use shell::{Execution, Shell, ShellError};
pub use state::ShellState;
pub use transformer::{compile_transformed, transform};
