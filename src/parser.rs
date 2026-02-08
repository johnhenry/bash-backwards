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
        // Collect all components (groups, commands) and operators
        let mut stack: Vec<Ast> = Vec::new();
        let mut pending_groups: Vec<Vec<Token>> = Vec::new();

        while !self.is_at_end() {
            match self.peek() {
                Some(Token::GroupStart) => {
                    self.advance(); // consume %(
                    let group = self.collect_group()?;
                    pending_groups.push(group);
                }
                Some(Token::GroupEnd) => {
                    // This would be an unmatched ), but could be subshell end
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
                            if pending_groups.is_empty() {
                                return Err(ParseError::InvalidOperatorPosition);
                            }

                            let left = stack.pop().unwrap();
                            let right_tokens = pending_groups.pop().unwrap();
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
                            if pending_groups.is_empty() {
                                return Err(ParseError::InvalidOperatorPosition);
                            }

                            let cmd = stack.pop().unwrap();
                            let file_tokens = pending_groups.pop().unwrap();
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
                    // pending_groups are in consumer-first order
                    if !pending_groups.is_empty() {
                        // Build pipe chain: producer is cmd, consumers are pending_groups (in reverse)
                        let mut result = cmd;
                        for group in pending_groups.drain(..).rev() {
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
        if !pending_groups.is_empty() && !stack.is_empty() {
            let cmd = stack.pop().unwrap();
            let mut result = cmd;
            for group in pending_groups.drain(..).rev() {
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

    /// Collect tokens until matching group end
    fn collect_group(&mut self) -> Result<Vec<Token>, ParseError> {
        let mut tokens = Vec::new();
        let mut depth = 1;

        while !self.is_at_end() {
            match self.peek() {
                Some(Token::GroupStart) | Some(Token::SubshellStart) => {
                    depth += 1;
                    tokens.push(self.advance().unwrap());
                }
                Some(Token::GroupEnd) => {
                    depth -= 1;
                    if depth == 0 {
                        self.advance(); // consume closing )
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
    /// - When executable found, STOP - remaining tokens are leftovers
    /// - Fallback: if no executable found, last word becomes command (backward compat)
    fn parse_executable_sequence(&mut self) -> Result<Ast, ParseError> {
        let mut current_args: Vec<String> = Vec::new();

        while !self.is_at_end() {
            match self.peek() {
                Some(Token::Word(s)) => {
                    let word = s.clone();
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

        // No executable found - fallback to last-word-is-command (backward compat)
        if !current_args.is_empty() {
            let cmd_name = current_args.pop().unwrap();
            return Ok(Ast::Command {
                name: cmd_name,
                args: current_args,
                from_group: false,
            });
        }

        Err(ParseError::UnexpectedEof)
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
        // %(hello grep) ls → ls | grep hello
        let tokens = lex("%(hello grep) ls").unwrap();
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
        // "%(hello grep) ls -la" → ls | grep hello (with -la as leftover)
        // To get ls -la | grep hello, use: "%(hello grep) -la ls"
        let tokens = lex("%(hello grep) ls -la").unwrap();
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
        // %(5 head) %(hello grep) ls → ls | grep hello | head 5
        let tokens = lex("%(5 head) %(hello grep) ls").unwrap();
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
        // ls %(done echo) && → ls && echo done
        let tokens = lex("ls %(done echo) &&").unwrap();
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
        // ls %(error echo) || → ls || echo error
        let tokens = lex("ls %(error echo) ||").unwrap();
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
        // cmd %(file.txt) > → cmd > file.txt
        let tokens = lex("cmd %(file.txt) >").unwrap();
        let ast = parse(tokens).unwrap();
        assert_eq!(
            ast,
            Ast::Redirect {
                cmd: Box::new(Ast::cmd("cmd")),
                file: "file.txt".to_string(),
                mode: RedirectMode::Write,
            }
        );
    }

    #[test]
    fn parse_redirect_append() {
        // cmd %(file.txt) >> → cmd >> file.txt
        let tokens = lex("cmd %(file.txt) >>").unwrap();
        let ast = parse(tokens).unwrap();
        assert_eq!(
            ast,
            Ast::Redirect {
                cmd: Box::new(Ast::cmd("cmd")),
                file: "file.txt".to_string(),
                mode: RedirectMode::Append,
            }
        );
    }

    #[test]
    fn parse_background() {
        // cmd & → cmd &
        let tokens = lex("cmd &").unwrap();
        let ast = parse(tokens).unwrap();
        assert_eq!(
            ast,
            Ast::Background {
                cmd: Box::new(Ast::cmd("cmd"))
            }
        );
    }

    #[test]
    fn parse_quoted_string_in_group() {
        // %("hello world" grep) ls → ls | grep "hello world"
        let tokens = lex("%( \"hello world\" grep) ls").unwrap();
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
