//! AST node definitions for hsab

#[derive(Debug, Clone, PartialEq)]
pub enum RedirectMode {
    Write,      // >
    Append,     // >>
    Read,       // <
}

#[derive(Debug, Clone, PartialEq)]
pub enum Ast {
    /// A simple command with name and arguments
    /// If from_group is true, this command came from a %() group and needs reordering
    Command { name: String, args: Vec<String>, from_group: bool },

    /// Pipe: producer | consumer
    Pipe { producer: Box<Ast>, consumer: Box<Ast> },

    /// Logical AND: left && right
    And { left: Box<Ast>, right: Box<Ast> },

    /// Logical OR: left || right
    Or { left: Box<Ast>, right: Box<Ast> },

    /// Redirect: cmd > file, cmd >> file, cmd < file
    Redirect { cmd: Box<Ast>, file: String, mode: RedirectMode },

    /// Background execution: cmd &
    Background { cmd: Box<Ast> },

    /// Subshell grouping: (cmd)
    Subshell { inner: Box<Ast> },

    /// Bash passthrough: raw bash code
    BashPassthrough(String),
}

impl Ast {
    /// Create a simple command with no arguments
    pub fn cmd(name: &str) -> Self {
        Ast::Command {
            name: name.to_string(),
            args: vec![],
            from_group: false,
        }
    }

    /// Create a command with arguments
    pub fn cmd_with_args(name: &str, args: Vec<&str>) -> Self {
        Ast::Command {
            name: name.to_string(),
            args: args.into_iter().map(|s| s.to_string()).collect(),
            from_group: false,
        }
    }

    /// Create a command from a group (needs reordering)
    pub fn cmd_from_group(name: &str, args: Vec<&str>) -> Self {
        Ast::Command {
            name: name.to_string(),
            args: args.into_iter().map(|s| s.to_string()).collect(),
            from_group: true,
        }
    }

    /// Create a pipe
    pub fn pipe(producer: Ast, consumer: Ast) -> Self {
        Ast::Pipe {
            producer: Box::new(producer),
            consumer: Box::new(consumer),
        }
    }

    /// Create an AND expression
    pub fn and(left: Ast, right: Ast) -> Self {
        Ast::And {
            left: Box::new(left),
            right: Box::new(right),
        }
    }

    /// Create an OR expression
    pub fn or(left: Ast, right: Ast) -> Self {
        Ast::Or {
            left: Box::new(left),
            right: Box::new(right),
        }
    }

    /// Create a redirect
    pub fn redirect(cmd: Ast, file: &str, mode: RedirectMode) -> Self {
        Ast::Redirect {
            cmd: Box::new(cmd),
            file: file.to_string(),
            mode,
        }
    }

    /// Create a background command
    pub fn background(cmd: Ast) -> Self {
        Ast::Background { cmd: Box::new(cmd) }
    }

    /// Create a subshell
    pub fn subshell(inner: Ast) -> Self {
        Ast::Subshell { inner: Box::new(inner) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cmd_helper() {
        let ast = Ast::cmd("ls");
        assert_eq!(
            ast,
            Ast::Command {
                name: "ls".to_string(),
                args: vec![],
                from_group: false,
            }
        );
    }

    #[test]
    fn test_cmd_with_args_helper() {
        let ast = Ast::cmd_with_args("grep", vec!["hello", "-i"]);
        assert_eq!(
            ast,
            Ast::Command {
                name: "grep".to_string(),
                args: vec!["hello".to_string(), "-i".to_string()],
                from_group: false,
            }
        );
    }

    #[test]
    fn test_pipe_helper() {
        let ast = Ast::pipe(Ast::cmd("ls"), Ast::cmd_with_args("grep", vec!["txt"]));
        match ast {
            Ast::Pipe { producer, consumer } => {
                assert_eq!(*producer, Ast::cmd("ls"));
                assert_eq!(*consumer, Ast::cmd_with_args("grep", vec!["txt"]));
            }
            _ => panic!("Expected Pipe"),
        }
    }
}
