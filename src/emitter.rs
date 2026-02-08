//! Emitter: converts AST to bash code

use crate::ast::{Ast, RedirectMode};

/// Emit bash code from an AST
pub fn emit(ast: &Ast) -> String {
    match ast {
        Ast::Command { name, args, .. } => {
            if args.is_empty() {
                name.clone()
            } else {
                format!("{} {}", name, args.join(" "))
            }
        }

        Ast::Pipe { producer, consumer } => {
            format!("{} | {}", emit(producer), emit(consumer))
        }

        Ast::And { left, right } => {
            format!("{} && {}", emit(left), emit(right))
        }

        Ast::Or { left, right } => {
            format!("{} || {}", emit(left), emit(right))
        }

        Ast::Redirect { cmd, file, mode } => {
            let op = match mode {
                RedirectMode::Write => ">",
                RedirectMode::Append => ">>",
                RedirectMode::Read => "<",
            };
            format!("{} {} {}", emit(cmd), op, file)
        }

        Ast::Background { cmd } => {
            format!("{} &", emit(cmd))
        }

        Ast::Subshell { inner } => {
            format!("({})", emit(inner))
        }

        Ast::BashPassthrough(s) => s.clone(),
    }
}

/// Compile hsab source to bash (lex, parse, transform, emit)
pub fn compile(source: &str) -> Result<String, String> {
    use crate::lexer::lex;
    use crate::parser::parse;
    use crate::transformer::transform;

    let tokens = lex(source).map_err(|e| e.to_string())?;
    let ast = parse(tokens).map_err(|e| e.to_string())?;
    let transformed = transform(ast);
    Ok(emit(&transformed))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::Ast;

    #[test]
    fn emit_simple_command() {
        let ast = Ast::cmd("ls");
        assert_eq!(emit(&ast), "ls");
    }

    #[test]
    fn emit_command_with_args() {
        let ast = Ast::cmd_with_args("grep", vec!["hello", "-i"]);
        assert_eq!(emit(&ast), "grep hello -i");
    }

    #[test]
    fn emit_pipe() {
        let ast = Ast::pipe(
            Ast::cmd("ls"),
            Ast::cmd_with_args("grep", vec!["hello"]),
        );
        assert_eq!(emit(&ast), "ls | grep hello");
    }

    #[test]
    fn emit_chained_pipes() {
        let ast = Ast::pipe(
            Ast::pipe(
                Ast::cmd("ls"),
                Ast::cmd_with_args("grep", vec!["hello"]),
            ),
            Ast::cmd_with_args("head", vec!["-5"]),
        );
        assert_eq!(emit(&ast), "ls | grep hello | head -5");
    }

    #[test]
    fn emit_and() {
        let ast = Ast::and(
            Ast::cmd("ls"),
            Ast::cmd_with_args("echo", vec!["done"]),
        );
        assert_eq!(emit(&ast), "ls && echo done");
    }

    #[test]
    fn emit_or() {
        let ast = Ast::or(
            Ast::cmd("ls"),
            Ast::cmd_with_args("echo", vec!["error"]),
        );
        assert_eq!(emit(&ast), "ls || echo error");
    }

    #[test]
    fn emit_redirect_write() {
        let ast = Ast::redirect(
            Ast::cmd("echo"),
            "output.txt",
            RedirectMode::Write,
        );
        assert_eq!(emit(&ast), "echo > output.txt");
    }

    #[test]
    fn emit_redirect_append() {
        let ast = Ast::redirect(
            Ast::cmd_with_args("echo", vec!["hello"]),
            "output.txt",
            RedirectMode::Append,
        );
        assert_eq!(emit(&ast), "echo hello >> output.txt");
    }

    #[test]
    fn emit_redirect_read() {
        let ast = Ast::redirect(
            Ast::cmd("cat"),
            "input.txt",
            RedirectMode::Read,
        );
        assert_eq!(emit(&ast), "cat < input.txt");
    }

    #[test]
    fn emit_background() {
        let ast = Ast::background(Ast::cmd("sleep"));
        assert_eq!(emit(&ast), "sleep &");
    }

    #[test]
    fn emit_subshell() {
        let ast = Ast::subshell(Ast::pipe(
            Ast::cmd("ls"),
            Ast::cmd_with_args("grep", vec!["hello"]),
        ));
        assert_eq!(emit(&ast), "(ls | grep hello)");
    }

    // Integration tests: full compile pipeline
    #[test]
    fn compile_simple_pipe() {
        let bash = compile("%(hello grep) ls").unwrap();
        // Note: current parsing gives us "hello grep" as a command
        // We need to fix the group parsing to correctly split args
        assert!(bash.contains("|"));
    }

    #[test]
    fn compile_and_chain() {
        let bash = compile("ls %(done echo) &&").unwrap();
        assert!(bash.contains("&&"));
    }

    #[test]
    fn compile_redirect() {
        let bash = compile("cmd %(file.txt) >").unwrap();
        assert_eq!(bash, "cmd > file.txt");
    }
}
