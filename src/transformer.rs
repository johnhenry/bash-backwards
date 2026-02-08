//! Transformer: AST transformations and normalization
//!
//! The parser already handles the primary postfix-to-infix conversion.
//! This module provides additional transformations like:
//! - Fixing argument order in groups (from "args cmd" to Command { name: cmd, args })
//! - Normalizing nested structures
//! - Optimization passes
//!
//! With executable-aware parsing, commands detected via resolver already have
//! correct order and don't need reordering. Only commands from %() groups
//! need postfix reordering.

use crate::ast::Ast;

/// Transform a parsed AST, fixing argument ordering for postfix notation
///
/// Commands with from_group=true came from %() groups and need reordering
/// (last token is the command). Commands with from_group=false were detected
/// via executable resolution and are already in correct order.
pub fn transform(ast: Ast) -> Ast {
    match ast {
        Ast::Command { name, args, from_group } => {
            if from_group && !args.is_empty() {
                // Group-based: needs postfix reordering
                // "hello echo" has name="hello", args=["echo"] → should become "echo hello"
                let args_len = args.len();
                let cmd_name = args.last().unwrap().clone();
                let mut new_args = vec![name];
                new_args.extend(args.into_iter().take(args_len.saturating_sub(1)));
                return Ast::Command {
                    name: cmd_name,
                    args: new_args,
                    from_group: false,
                };
            }
            // Executable-detected or single token: already in correct order
            Ast::Command { name, args, from_group: false }
        }

        Ast::Pipe { producer, consumer } => Ast::Pipe {
            producer: Box::new(transform(*producer)),
            consumer: Box::new(transform(*consumer)),
        },

        Ast::And { left, right } => Ast::And {
            left: Box::new(transform(*left)),
            right: Box::new(transform(*right)),
        },

        Ast::Or { left, right } => Ast::Or {
            left: Box::new(transform(*left)),
            right: Box::new(transform(*right)),
        },

        Ast::Redirect { cmd, file, mode } => Ast::Redirect {
            cmd: Box::new(transform(*cmd)),
            file,
            mode,
        },

        Ast::Background { cmd } => Ast::Background {
            cmd: Box::new(transform(*cmd)),
        },

        Ast::Subshell { inner } => Ast::Subshell {
            inner: Box::new(transform(*inner)),
        },

        Ast::BashPassthrough(s) => Ast::BashPassthrough(s),
    }
}

/// Compile with transformation applied
pub fn compile_transformed(source: &str) -> Result<String, String> {
    use crate::emitter::emit;
    use crate::lexer::lex;
    use crate::parser::parse;

    let tokens = lex(source).map_err(|e| e.to_string())?;
    let ast = parse(tokens).map_err(|e| e.to_string())?;
    let transformed = transform(ast);
    Ok(emit(&transformed))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transform_reverses_group_order() {
        // Input group like %(hello grep) gives Command { name: "hello", args: ["grep"], from_group: true }
        // Transform should reverse to Command { name: "grep", args: ["hello"] }
        let ast = Ast::Command {
            name: "hello".to_string(),
            args: vec!["grep".to_string()],
            from_group: true,
        };
        let transformed = transform(ast);
        assert_eq!(
            transformed,
            Ast::Command {
                name: "grep".to_string(),
                args: vec!["hello".to_string()],
                from_group: false,
            }
        );
    }

    #[test]
    fn transform_handles_flags() {
        // %(-n 5 head) -> head -n 5
        let ast = Ast::Command {
            name: "-n".to_string(),
            args: vec!["5".to_string(), "head".to_string()],
            from_group: true,
        };
        let transformed = transform(ast);
        assert_eq!(
            transformed,
            Ast::Command {
                name: "head".to_string(),
                args: vec!["-n".to_string(), "5".to_string()],
                from_group: false,
            }
        );
    }

    #[test]
    fn transform_preserves_simple_command() {
        let ast = Ast::cmd("ls");
        let transformed = transform(ast);
        assert_eq!(transformed, Ast::cmd("ls"));
    }

    #[test]
    fn transform_preserves_executable_detected_order() {
        // Commands with from_group=false came from executable detection
        // and are already in correct order - should NOT be reordered
        let ast = Ast::Command {
            name: "echo".to_string(),
            args: vec!["hello".to_string()],
            from_group: false,
        };
        let transformed = transform(ast);
        assert_eq!(
            transformed,
            Ast::Command {
                name: "echo".to_string(),
                args: vec!["hello".to_string()],
                from_group: false,
            }
        );
    }

    #[test]
    fn transform_pipe_children() {
        let ast = Ast::Pipe {
            producer: Box::new(Ast::cmd("ls")),
            consumer: Box::new(Ast::Command {
                name: "hello".to_string(),
                args: vec!["grep".to_string()],
                from_group: true,
            }),
        };
        let transformed = transform(ast);
        match transformed {
            Ast::Pipe { producer, consumer } => {
                assert_eq!(*producer, Ast::cmd("ls"));
                assert_eq!(
                    *consumer,
                    Ast::Command {
                        name: "grep".to_string(),
                        args: vec!["hello".to_string()],
                        from_group: false,
                    }
                );
            }
            _ => panic!("Expected Pipe"),
        }
    }

    #[test]
    fn full_compile_with_transform() {
        let bash = compile_transformed("%(hello grep) ls").unwrap();
        assert_eq!(bash, "ls | grep hello");
    }

    #[test]
    fn compile_chained_pipes_with_transform() {
        let bash = compile_transformed("%(-5 head) %(hello grep) ls").unwrap();
        assert_eq!(bash, "ls | grep hello | head -5");
    }

    #[test]
    fn compile_and_with_transform() {
        let bash = compile_transformed("ls %(done echo) &&").unwrap();
        assert_eq!(bash, "ls && echo done");
    }

    #[test]
    fn compile_postfix_echo() {
        // In hsab postfix: "hello echo" → bash: "echo hello"
        let bash = compile_transformed("hello echo").unwrap();
        assert_eq!(bash, "echo hello");
    }

    #[test]
    fn compile_single_command() {
        // Single command with no args stays as-is
        let bash = compile_transformed("ls").unwrap();
        assert_eq!(bash, "ls");
    }
}
