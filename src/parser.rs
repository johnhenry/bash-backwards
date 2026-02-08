//! Parser: converts tokens to AST
//!
//! The parser handles postfix notation where:
//! - Pipes are consumer-first: %(hello grep) ls → ls | grep hello
//! - Logic ops are execution order: ls %(done echo) && → ls && echo done
//! - Redirects are execution order: cmd %(file.txt) > → cmd > file.txt
//!
//! With executable-aware parsing:
//! - Auto-detects executables: -la ls → ls -la
//! - Multiple executables become pipes: -la ls hello grep → ls -la | grep hello

use crate::ast::{Ast, RedirectMode};
use crate::lexer::{Operator, Token};
use crate::resolver::ExecutableResolver;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ParseError {
    #[error("Unexpected end of input")]
    UnexpectedEof,
    #[error("Unexpected token: {0:?}")]
    UnexpectedToken(Token),
    #[error("Unmatched group start")]
    UnmatchedGroupStart,
    #[error("Unmatched group end")]
    UnmatchedGroupEnd,
    #[error("Empty group")]
    EmptyGroup,
    #[error("Invalid operator position")]
    InvalidOperatorPosition,
}

/// Parser state
pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
    resolver: ExecutableResolver,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Parser {
            tokens,
            pos: 0,
            resolver: ExecutableResolver::new(),
        }
    }

    /// Create a parser with a custom resolver (for testing)
    #[cfg(test)]
    pub fn with_resolver(tokens: Vec<Token>, resolver: ExecutableResolver) -> Self {
        Parser {
            tokens,
            pos: 0,
            resolver,
        }
    }

    /// Peek at the current token without consuming it
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    /// Consume and return the current token
    fn advance(&mut self) -> Option<Token> {
        if self.pos < self.tokens.len() {
            let token = self.tokens[self.pos].clone();
            self.pos += 1;
            Some(token)
        } else {
            None
        }
    }

    /// Check if we're at the end of input
    fn is_at_end(&self) -> bool {
        self.pos >= self.tokens.len()
    }

    /// Get the current position (number of tokens consumed)
    pub fn position(&self) -> usize {
        self.pos
    }

    /// Parse the entire input
    pub fn parse(&mut self) -> Result<Ast, ParseError> {
        if self.is_at_end() {
            return Err(ParseError::UnexpectedEof);
        }
        self.parse_expression()
    }

    /// Parse an expression (handles operators)
    fn parse_expression(&mut self) -> Result<Ast, ParseError> {
        // Collect all components (quotations, commands) and operators
        let mut stack: Vec<Ast> = Vec::new();
        let mut pending_quotations: Vec<Vec<Token>> = Vec::new();

        while !self.is_at_end() {
            match self.peek() {
                Some(Token::QuotationStart) => {
                    self.advance(); // consume [
                    let quotation = self.collect_quotation()?;
                    pending_quotations.push(quotation);
                }
                Some(Token::QuotationEnd) => {
                    // Unmatched ]
                    return Err(ParseError::UnmatchedGroupEnd);
                }
                Some(Token::GroupEnd) => {
                    // This is ) for subshell end
                    break;
                }
                Some(Token::SubshellStart) => {
                    self.advance(); // consume $(
                    let inner = self.parse_expression()?;
                    // Consume closing ) if present
                    if let Some(Token::GroupEnd) = self.peek() {
                        self.advance();
                    }
                    stack.push(Ast::Subshell { inner: Box::new(inner) });
                }
                Some(Token::Operator(op)) => {
                    let op = op.clone();
                    self.advance();

                    // Handle different operators
                    match op {
                        Operator::And | Operator::Or => {
                            // Pop the last item from stack as left side
                            // The pending group becomes the right side
                            if stack.is_empty() {
                                return Err(ParseError::InvalidOperatorPosition);
                            }
                            if pending_quotations.is_empty() {
                                return Err(ParseError::InvalidOperatorPosition);
                            }

                            let left = stack.pop().unwrap();
                            let right_tokens = pending_quotations.pop().unwrap();
                            let right = self.parse_group_as_command(right_tokens)?;

                            let ast = match op {
                                Operator::And => Ast::And {
                                    left: Box::new(left),
                                    right: Box::new(right),
                                },
                                Operator::Or => Ast::Or {
                                    left: Box::new(left),
                                    right: Box::new(right),
                                },
                                _ => unreachable!(),
                            };
                            stack.push(ast);
                        }
                        Operator::Write | Operator::Append | Operator::Read => {
                            // Redirect: cmd %(file) >
                            if stack.is_empty() {
                                return Err(ParseError::InvalidOperatorPosition);
                            }
                            if pending_quotations.is_empty() {
                                return Err(ParseError::InvalidOperatorPosition);
                            }

                            let cmd = stack.pop().unwrap();
                            let file_tokens = pending_quotations.pop().unwrap();
                            let file = self.tokens_to_string(&file_tokens);

                            let mode = match op {
                                Operator::Write => RedirectMode::Write,
                                Operator::Append => RedirectMode::Append,
                                Operator::Read => RedirectMode::Read,
                                _ => unreachable!(),
                            };

                            stack.push(Ast::Redirect {
                                cmd: Box::new(cmd),
                                file,
                                mode,
                            });
                        }
                        Operator::Background => {
                            // Background: cmd &
                            if stack.is_empty() {
                                return Err(ParseError::InvalidOperatorPosition);
                            }

                            let cmd = stack.pop().unwrap();
                            stack.push(Ast::Background { cmd: Box::new(cmd) });
                        }
                        Operator::Pipe => {
                            // Explicit pipe (normally we use postfix groups)
                            // This shouldn't appear in postfix notation
                            return Err(ParseError::UnexpectedToken(Token::Operator(Operator::Pipe)));
                        }
                    }
                }
                Some(Token::Word(_)) | Some(Token::DoubleQuoted(_))
                | Some(Token::SingleQuoted(_)) | Some(Token::Variable(_)) => {
                    // Parse executable sequence (auto-detects commands)
                    let cmd = self.parse_executable_sequence()?;

                    // If there are pending groups, they're pipe consumers
                    // pending_quotations are in consumer-first order
                    if !pending_quotations.is_empty() {
                        // Build pipe chain: producer is cmd, consumers are pending_quotations (in reverse)
                        let mut result = cmd;
                        for group in pending_quotations.drain(..).rev() {
                            let consumer = self.parse_group_as_command(group)?;
                            result = Ast::Pipe {
                                producer: Box::new(result),
                                consumer: Box::new(consumer),
                            };
                        }
                        stack.push(result);
                    } else {
                        stack.push(cmd);
                    }

                    // After parsing an executable, any remaining word tokens are leftovers
                    // Break out of the loop - the Shell will put them back on input
                    if !self.is_at_end() {
                        if let Some(Token::Word(_)) | Some(Token::DoubleQuoted(_))
                            | Some(Token::SingleQuoted(_)) | Some(Token::Variable(_)) = self.peek()
                        {
                            break;
                        }
                    }
                }
                Some(Token::BashPassthrough(s)) => {
                    let s = s.clone();
                    self.advance();
                    stack.push(Ast::BashPassthrough(s));
                }
                None => break,
            }
        }

        // Handle any remaining pending groups as pipe chain with last command
        if !pending_quotations.is_empty() && !stack.is_empty() {
            let cmd = stack.pop().unwrap();
            let mut result = cmd;
            for group in pending_quotations.drain(..).rev() {
                let consumer = self.parse_group_as_command(group)?;
                result = Ast::Pipe {
                    producer: Box::new(result),
                    consumer: Box::new(consumer),
                };
            }
            stack.push(result);
        }

        if stack.is_empty() {
            return Err(ParseError::UnexpectedEof);
        }

        // If multiple items on stack, error (should be composed into single AST)
        if stack.len() > 1 {
            // For now, just return the last one (could be improved to handle sequences)
            return Ok(stack.pop().unwrap());
        }

        Ok(stack.pop().unwrap())
    }

    /// Collect tokens until matching quotation end ]
    fn collect_quotation(&mut self) -> Result<Vec<Token>, ParseError> {
        let mut tokens = Vec::new();
        let mut depth = 1;

        while !self.is_at_end() {
            match self.peek() {
                Some(Token::QuotationStart) => {
                    depth += 1;
                    tokens.push(self.advance().unwrap());
                }
                Some(Token::QuotationEnd) => {
                    depth -= 1;
                    if depth == 0 {
                        self.advance(); // consume closing ]
                        break;
                    } else {
                        tokens.push(self.advance().unwrap());
                    }
                }
                Some(_) => {
                    tokens.push(self.advance().unwrap());
                }
                None => return Err(ParseError::UnmatchedGroupStart),
            }
        }

        if depth != 0 {
            return Err(ParseError::UnmatchedGroupStart);
        }

        Ok(tokens)
    }

    /// Parse a group's tokens as a command (marked as from_group for reordering)
    fn parse_group_as_command(&mut self, tokens: Vec<Token>) -> Result<Ast, ParseError> {
        if tokens.is_empty() {
            return Err(ParseError::EmptyGroup);
        }

        // Simple case: just words/strings
        let mut name: Option<String> = None;
        let mut args: Vec<String> = Vec::new();

        for token in tokens {
            let value = match token {
                Token::Word(s) => s,
                Token::DoubleQuoted(s) => format!("\"{}\"", s),
                Token::SingleQuoted(s) => format!("'{}'", s),
                Token::Variable(s) => s,
                _ => continue, // Skip other tokens for simple case
            };

            if name.is_none() {
                name = Some(value);
            } else {
                args.push(value);
            }
        }

        match name {
            Some(n) => Ok(Ast::Command { name: n, args, from_group: true }),
            None => Err(ParseError::EmptyGroup),
        }
    }

    /// Convert tokens to a single string (for file paths etc)
    fn tokens_to_string(&self, tokens: &[Token]) -> String {
        tokens
            .iter()
            .filter_map(|t| match t {
                Token::Word(s) => Some(s.clone()),
                Token::DoubleQuoted(s) => Some(format!("\"{}\"", s)),
                Token::SingleQuoted(s) => Some(format!("'{}'", s)),
                Token::Variable(s) => Some(s.clone()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Parse a sequence of words, detecting executables and creating pipe chains
    ///
    /// This is the core of executable-aware parsing:
    /// - Args accumulate left-to-right until an executable is found
    /// - Stack ops (dup, swap, drop, over, rot) manipulate the accumulated args
    /// - Path ops (join, basename, dirname, suffix, reext) transform args
    /// - When executable found, STOP - remaining tokens are leftovers
    /// - Fallback: if no executable found, last word becomes command (backward compat)
    fn parse_executable_sequence(&mut self) -> Result<Ast, ParseError> {
        use crate::resolver::ExecutableResolver;

        let mut current_args: Vec<String> = Vec::new();

        while !self.is_at_end() {
            match self.peek() {
                Some(Token::Word(s)) => {
                    let word = s.clone();

                    // Check for stack operations
                    if ExecutableResolver::is_stack_op(&word) {
                        self.advance();
                        Self::apply_stack_op(&word, &mut current_args);
                        continue;
                    }

                    // Check for path operations
                    if ExecutableResolver::is_path_op(&word) {
                        self.advance();
                        Self::apply_path_op(&word, &mut current_args);
                        continue;
                    }

                    if self.resolver.is_executable(&word) {
                        // Found an executable! Create command with accumulated args
                        self.advance();
                        return Ok(Ast::Command {
                            name: word,
                            args: current_args,
                            from_group: false,
                        });
                    } else {
                        // Accumulate as argument
                        current_args.push(word);
                        self.advance();
                    }
                }
                Some(Token::DoubleQuoted(s)) => {
                    let s = format!("\"{}\"", s);
                    current_args.push(s);
                    self.advance();
                }
                Some(Token::SingleQuoted(s)) => {
                    let s = format!("'{}'", s);
                    current_args.push(s);
                    self.advance();
                }
                Some(Token::Variable(s)) => {
                    let s = s.clone();
                    current_args.push(s);
                    self.advance();
                }
                _ => break,
            }
        }

        // No executable found - everything becomes leftovers
        Err(ParseError::UnexpectedEof)
    }

    /// Apply a stack operation to the argument list
    ///
    /// Stack operations:
    /// - dup:  a b → a b b       (duplicate top)
    /// - swap: a b → b a         (swap top two)
    /// - drop: a b → a           (remove top)
    /// - over: a b → a b a       (copy second to top)
    /// - rot:  a b c → b c a     (rotate top three)
    fn apply_stack_op(op: &str, args: &mut Vec<String>) {
        match op {
            "dup" => {
                // Duplicate top: a b → a b b
                if let Some(top) = args.last().cloned() {
                    args.push(top);
                }
            }
            "swap" => {
                // Swap top two: a b → b a
                let len = args.len();
                if len >= 2 {
                    args.swap(len - 1, len - 2);
                }
            }
            "drop" => {
                // Remove top: a b → a
                args.pop();
            }
            "over" => {
                // Copy second to top: a b → a b a
                let len = args.len();
                if len >= 2 {
                    let second = args[len - 2].clone();
                    args.push(second);
                }
            }
            "rot" => {
                // Rotate top three: a b c → b c a
                let len = args.len();
                if len >= 3 {
                    let a = args.remove(len - 3);
                    args.push(a);
                }
            }
            _ => {}
        }
    }

    /// Apply a path operation to the argument list
    ///
    /// Path operations:
    /// - join:     dir file → dir/file     (join two path components)
    /// - basename: /path/to/file → file    (extract filename)
    /// - dirname:  /path/to/file → /path/to (extract directory)
    /// - suffix:   file .bak → file.bak    (append suffix)
    /// - reext:    file.txt .md → file.md  (replace extension)
    fn apply_path_op(op: &str, args: &mut Vec<String>) {
        match op {
            "join" => {
                // Join two path components: dir file → dir/file
                if args.len() >= 2 {
                    let file = args.pop().unwrap();
                    let dir = args.pop().unwrap();
                    let joined = if dir.ends_with('/') {
                        format!("{}{}", dir, file)
                    } else {
                        format!("{}/{}", dir, file)
                    };
                    args.push(joined);
                }
            }
            "basename" => {
                // Extract filename: /path/to/file.txt → file (without extension)
                if let Some(path) = args.pop() {
                    let basename = std::path::Path::new(&path)
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or(&path)
                        .to_string();
                    args.push(basename);
                }
            }
            "dirname" => {
                // Extract directory: /path/to/file → /path/to
                if let Some(path) = args.pop() {
                    let dirname = std::path::Path::new(&path)
                        .parent()
                        .and_then(|p| p.to_str())
                        .unwrap_or(".")
                        .to_string();
                    args.push(dirname);
                }
            }
            "suffix" => {
                // Append suffix: file .bak → file.bak
                if args.len() >= 2 {
                    let suffix = args.pop().unwrap();
                    let file = args.pop().unwrap();
                    args.push(format!("{}{}", file, suffix));
                }
            }
            "reext" => {
                // Replace extension: file.txt .md → file.md
                if args.len() >= 2 {
                    let new_ext = args.pop().unwrap();
                    let file = args.pop().unwrap();
                    let stem = std::path::Path::new(&file)
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or(&file);
                    // Handle new_ext with or without leading dot
                    let ext = if new_ext.starts_with('.') {
                        new_ext
                    } else {
                        format!(".{}", new_ext)
                    };
                    args.push(format!("{}{}", stem, ext));
                }
            }
            _ => {}
        }
    }
}

/// Parse a token stream into an AST
pub fn parse(tokens: Vec<Token>) -> Result<Ast, ParseError> {
    let mut parser = Parser::new(tokens);
    parser.parse()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::lex;

    #[test]
    fn parse_simple_command() {
        let tokens = lex("ls").unwrap();
        let ast = parse(tokens).unwrap();
        assert_eq!(ast, Ast::cmd("ls"));
    }

    #[test]
    fn parse_command_with_args() {
        // With leftover behavior, "ls -la /tmp" produces just "ls"
        // because -la and /tmp are leftovers (put back on input)
        let tokens = lex("ls -la /tmp").unwrap();
        let ast = parse(tokens).unwrap();
        assert_eq!(ast, Ast::cmd("ls"));

        // To get ls with args, put args BEFORE the command (postfix style)
        let tokens = lex("-la /tmp ls").unwrap();
        let ast = parse(tokens).unwrap();
        assert_eq!(ast, Ast::cmd_with_args("ls", vec!["-la", "/tmp"]));
    }

    #[test]
    fn parse_simple_pipe() {
        // [hello grep] ls → ls | grep hello
        let tokens = lex("[hello grep] ls").unwrap();
        let ast = parse(tokens).unwrap();
        assert_eq!(
            ast,
            Ast::Pipe {
                producer: Box::new(Ast::cmd("ls")),
                consumer: Box::new(Ast::Command {
                    name: "hello".to_string(),
                    args: vec!["grep".to_string()],
                    from_group: true,
                }),
            }
        );
    }

    #[test]
    fn parse_pipe_with_args() {
        // "[hello grep] ls -la" → ls | grep hello (with -la as leftover)
        // To get ls -la | grep hello, use: "[hello grep] -la ls"
        let tokens = lex("[hello grep] ls -la").unwrap();
        let ast = parse(tokens).unwrap();
        assert_eq!(
            ast,
            Ast::Pipe {
                producer: Box::new(Ast::cmd("ls")),
                consumer: Box::new(Ast::Command {
                    name: "hello".to_string(),
                    args: vec!["grep".to_string()],
                    from_group: true,
                }),
            }
        );
    }

    #[test]
    fn parse_chained_pipes() {
        // [5 head] [hello grep] ls → ls | grep hello | head 5
        let tokens = lex("[5 head] [hello grep] ls").unwrap();
        let ast = parse(tokens).unwrap();

        // The structure should be: Pipe { producer: Pipe { producer: ls, consumer: grep }, consumer: head }
        match ast {
            Ast::Pipe { producer, consumer } => {
                // Outer consumer is head
                assert_eq!(
                    *consumer,
                    Ast::Command {
                        name: "5".to_string(),
                        args: vec!["head".to_string()],
                        from_group: true,
                    }
                );
                // Inner is another pipe
                match *producer {
                    Ast::Pipe { producer: inner_prod, consumer: inner_cons } => {
                        assert_eq!(*inner_prod, Ast::cmd("ls"));
                        assert_eq!(
                            *inner_cons,
                            Ast::Command {
                                name: "hello".to_string(),
                                args: vec!["grep".to_string()],
                                from_group: true,
                            }
                        );
                    }
                    _ => panic!("Expected inner Pipe"),
                }
            }
            _ => panic!("Expected Pipe, got {:?}", ast),
        }
    }

    #[test]
    fn parse_and_operator() {
        // ls [done echo] && → ls && echo done
        let tokens = lex("ls [done echo] &&").unwrap();
        let ast = parse(tokens).unwrap();
        assert_eq!(
            ast,
            Ast::And {
                left: Box::new(Ast::cmd("ls")),
                right: Box::new(Ast::Command {
                    name: "done".to_string(),
                    args: vec!["echo".to_string()],
                    from_group: true,
                }),
            }
        );
    }

    #[test]
    fn parse_or_operator() {
        // ls [error echo] || → ls || echo error
        let tokens = lex("ls [error echo] ||").unwrap();
        let ast = parse(tokens).unwrap();
        assert_eq!(
            ast,
            Ast::Or {
                left: Box::new(Ast::cmd("ls")),
                right: Box::new(Ast::Command {
                    name: "error".to_string(),
                    args: vec!["echo".to_string()],
                    from_group: true,
                }),
            }
        );
    }

    #[test]
    fn parse_redirect_write() {
        // cat [file.txt] > → cat > file.txt
        let tokens = lex("cat [file.txt] >").unwrap();
        let ast = parse(tokens).unwrap();
        assert_eq!(
            ast,
            Ast::Redirect {
                cmd: Box::new(Ast::cmd("cat")),
                file: "file.txt".to_string(),
                mode: RedirectMode::Write,
            }
        );
    }

    #[test]
    fn parse_redirect_append() {
        // cat [file.txt] >> → cat >> file.txt
        let tokens = lex("cat [file.txt] >>").unwrap();
        let ast = parse(tokens).unwrap();
        assert_eq!(
            ast,
            Ast::Redirect {
                cmd: Box::new(Ast::cmd("cat")),
                file: "file.txt".to_string(),
                mode: RedirectMode::Append,
            }
        );
    }

    #[test]
    fn parse_background() {
        // sleep & → sleep &
        let tokens = lex("sleep &").unwrap();
        let ast = parse(tokens).unwrap();
        assert_eq!(
            ast,
            Ast::Background {
                cmd: Box::new(Ast::cmd("sleep"))
            }
        );
    }

    #[test]
    fn parse_quoted_string_in_quotation() {
        // ["hello world" grep] ls → ls | grep "hello world"
        let tokens = lex("[ \"hello world\" grep] ls").unwrap();
        let ast = parse(tokens).unwrap();
        match ast {
            Ast::Pipe { consumer, .. } => {
                assert_eq!(
                    *consumer,
                    Ast::Command {
                        name: "\"hello world\"".to_string(),
                        args: vec!["grep".to_string()],
                        from_group: true,
                    }
                );
            }
            _ => panic!("Expected Pipe"),
        }
    }
}
