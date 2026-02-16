//! Parser for hsab v2
//!
//! Converts tokens into a Program (sequence of expressions).
//! The parser is relatively simple - it maps tokens to expressions
//! and handles block nesting. The semantic complexity lives in the evaluator.

use crate::ast::{Expr, Program};
use crate::lexer::{Operator, Token};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ParseError {
    #[error("Unexpected end of input")]
    UnexpectedEof,
    #[error("Unexpected token: {0:?}")]
    UnexpectedToken(Token),
    #[error("Unmatched block start '['")]
    UnmatchedBlockStart,
    #[error("Unmatched block end ']'")]
    UnmatchedBlockEnd,
    #[error("Empty input")]
    EmptyInput,
}

/// Process escape sequences in double-quoted strings
/// Handles: \n, \t, \r, \\, \", \x1b (ANSI escape), \e (ANSI escape alias)
fn process_escapes(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('n') => result.push('\n'),
                Some('t') => result.push('\t'),
                Some('r') => result.push('\r'),
                Some('\\') => result.push('\\'),
                Some('"') => result.push('"'),
                Some('e') => result.push('\x1b'),  // ANSI escape alias
                Some('x') => {
                    // Hex escape: \x1b, \x1B, etc.
                    let mut hex = String::new();
                    for _ in 0..2 {
                        if let Some(&c) = chars.peek() {
                            if c.is_ascii_hexdigit() {
                                hex.push(chars.next().unwrap());
                            } else {
                                break;
                            }
                        }
                    }
                    if hex.len() == 2 {
                        if let Ok(byte) = u8::from_str_radix(&hex, 16) {
                            result.push(byte as char);
                        } else {
                            result.push_str("\\x");
                            result.push_str(&hex);
                        }
                    } else {
                        result.push_str("\\x");
                        result.push_str(&hex);
                    }
                }
                Some('0') => {
                    // Octal escape: \033 for ESC
                    let mut octal = String::from("0");
                    for _ in 0..2 {
                        if let Some(&c) = chars.peek() {
                            if c.is_ascii_digit() && c < '8' {
                                octal.push(chars.next().unwrap());
                            } else {
                                break;
                            }
                        }
                    }
                    if octal.len() == 3 {
                        if let Ok(byte) = u8::from_str_radix(&octal, 8) {
                            result.push(byte as char);
                        } else {
                            result.push('\\');
                            result.push_str(&octal);
                        }
                    } else {
                        result.push('\\');
                        result.push_str(&octal);
                    }
                }
                Some(other) => {
                    // Unknown escape - keep as-is
                    result.push('\\');
                    result.push(other);
                }
                None => result.push('\\'),
            }
        } else {
            result.push(c);
        }
    }

    result
}

/// Try to parse a word as a dynamic operator pattern.
/// Returns Some(vec of exprs) if the word matches a pattern, None otherwise.
///
/// Patterns:
///   <number><op> where op is +, -, *, /, % -> push number, then call arithmetic
///   <number>log -> push number, then call log-base
///   <number>pow -> push number, then call pow
fn try_dynamic_pattern(word: &str) -> Option<Vec<Expr>> {
    let bytes = word.as_bytes();
    let len = bytes.len();
    if len < 2 {
        return None;
    }

    // Check trailing operator: 3+, 14*, 2.5/, etc.
    let last = bytes[len - 1];
    if matches!(last, b'+' | b'-' | b'*' | b'/' | b'%') {
        let num_part = &word[..len - 1];
        if num_part.parse::<f64>().is_ok() {
            let op = match last {
                b'+' => "plus",
                b'-' => "minus",
                b'*' => "mul",
                b'/' => "div",
                b'%' => "mod",
                _ => unreachable!(),
            };
            return Some(vec![
                Expr::Literal(num_part.to_string()),
                Expr::Literal(op.to_string()),
            ]);
        }
    }

    // Check trailing "log": 2log, 10log
    if word.ends_with("log") && len > 3 {
        let num_part = &word[..len - 3];
        if !num_part.is_empty() && num_part.parse::<f64>().is_ok() {
            return Some(vec![
                Expr::Literal(num_part.to_string()),
                Expr::Literal("log-base".to_string()),
            ]);
        }
    }

    // Check trailing "pow": 2pow, 3pow
    if word.ends_with("pow") && len > 3 {
        let num_part = &word[..len - 3];
        if !num_part.is_empty() && num_part.parse::<f64>().is_ok() {
            return Some(vec![
                Expr::Literal(num_part.to_string()),
                Expr::Literal("pow".to_string()),
            ]);
        }
    }

    None
}

/// Parser state
pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Parser { tokens, pos: 0 }
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

    /// Parse the entire input into a Program
    pub fn parse(&mut self) -> Result<Program, ParseError> {
        let mut expressions = Vec::new();

        // Check for scoped variable assignments: NAME=value ... ;
        if let Some(scoped) = self.try_parse_scoped_block()? {
            expressions.push(scoped);
            // Parse any remaining expressions after the scoped block
            while !self.is_at_end() {
                let exprs = self.parse_expr()?;
                expressions.extend(exprs);
            }
        } else {
            while !self.is_at_end() {
                let exprs = self.parse_expr()?;
                expressions.extend(exprs);
            }
        }

        if expressions.is_empty() {
            return Err(ParseError::EmptyInput);
        }

        Ok(Program::new(expressions))
    }

    /// Try to parse a scoped block: NAME=value ... ; body
    /// Returns None if not a scoped assignment pattern
    fn try_parse_scoped_block(&mut self) -> Result<Option<Expr>, ParseError> {
        // Look ahead for assignment pattern: one or more Word(NAME=VALUE) followed by Semicolon
        let mut assignments = Vec::new();
        let mut lookahead = 0;

        // Gather potential assignments
        while let Some(token) = self.tokens.get(self.pos + lookahead) {
            match token {
                Token::Word(w) if w.contains('=') && !w.starts_with('-') => {
                    // Looks like an assignment (but not a flag like --foo=bar... actually that's fine too)
                    // Split at first = to get name and value
                    if let Some(eq_pos) = w.find('=') {
                        let name = &w[..eq_pos];
                        let value = &w[eq_pos + 1..];
                        // Name must be valid identifier (alphanumeric + underscore, not starting with digit)
                        if !name.is_empty() && name.chars().next().map(|c| c.is_ascii_alphabetic() || c == '_').unwrap_or(false)
                            && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
                            assignments.push((name.to_string(), value.to_string()));
                            lookahead += 1;
                            continue;
                        }
                    }
                    // Not a valid assignment, stop looking
                    break;
                }
                Token::Semicolon if !assignments.is_empty() => {
                    // Found semicolon after assignments - this is a scoped block!
                    // Consume the assignment tokens and semicolon
                    self.pos += lookahead + 1; // +1 for the semicolon

                    // Parse the body (everything after the semicolon)
                    let mut body = Vec::new();
                    while !self.is_at_end() {
                        let exprs = self.parse_expr()?;
                        body.extend(exprs);
                    }

                    return Ok(Some(Expr::ScopedBlock { assignments, body }));
                }
                _ => break, // Not an assignment or semicolon, stop looking
            }
        }

        // No scoped block pattern found
        Ok(None)
    }

    /// Parse a single expression (may return multiple for dynamic patterns like 3+)
    fn parse_expr(&mut self) -> Result<Vec<Expr>, ParseError> {
        let token = self.advance().ok_or(ParseError::UnexpectedEof)?;

        match token {
            Token::Word(s) => Ok(self.word_to_exprs(&s)),
            Token::DoubleQuoted(s) => Ok(vec![Expr::Quoted { content: process_escapes(&s), double: true }]),
            Token::SingleQuoted(s) => Ok(vec![Expr::Quoted { content: s, double: false }]),
            Token::Variable(s) => Ok(vec![Expr::Variable(s)]),
            Token::BlockStart => self.parse_block().map(|e| vec![e]),
            Token::BlockEnd => Err(ParseError::UnmatchedBlockEnd),
            Token::Operator(op) => Ok(vec![self.operator_to_expr(op)]),
            Token::Define(name) => Ok(vec![Expr::Define(name)]),
            Token::LimboRef(id) => Ok(vec![Expr::LimboRef(id)]),
            Token::Semicolon => {
                // Stray semicolon (not part of scoped block) - skip it and parse next
                if self.is_at_end() {
                    Err(ParseError::UnexpectedEof)
                } else {
                    self.parse_expr()
                }
            }
        }
    }

    /// Convert a word to expression(s) (handles special words and dynamic patterns)
    fn word_to_exprs(&self, word: &str) -> Vec<Expr> {
        // Try dynamic pattern first (e.g., 3+, 2/, 10log, 2pow)
        if let Some(exprs) = try_dynamic_pattern(word) {
            return exprs;
        }

        // Original word_to_expr logic
        vec![match word {
            // Stack operations
            "dup" => Expr::Dup,
            "swap" => Expr::Swap,
            "drop" => Expr::Drop,
            "over" => Expr::Over,
            "rot" => Expr::Rot,
            "depth" => Expr::Depth,
            // Path operations
            "path-join" => Expr::Join,
            "suffix" => Expr::Suffix,
            "dirname" => Expr::Dirname,
            "basename" => Expr::Basename,
            "path-resolve" => Expr::Realpath,
            // String operations
            "split1" => Expr::Split1,
            "rsplit1" => Expr::Rsplit1,
            // List operations
            "marker" => Expr::Marker,
            "spread" => Expr::Spread,
            "each" => Expr::Each,
            "collect" => Expr::Collect,
            "keep" => Expr::Keep,
            "map" => Expr::Map,
            "filter" => Expr::Filter,
            // Control flow
            "if" => Expr::If,
            "times" => Expr::Times,
            "while" => Expr::While,
            "until" => Expr::Until,
            "break" => Expr::Break,
            // Parallel execution
            "parallel" => Expr::Parallel,
            "fork" => Expr::Fork,
            // Process substitution
            "subst" => Expr::Subst,
            "fifo" => Expr::Fifo,
            // JSON / Structured data
            "json" => Expr::Json,
            "unjson" => Expr::Unjson,
            // Resource limits
            "timeout" => Expr::Timeout,
            // Pipeline status
            "pipestatus" => Expr::Pipestatus,
            // Module system
            ".import" => Expr::Import,
            // Regular word/literal
            _ => Expr::Literal(word.to_string()),
        }]
    }

    /// Convert an operator to an expression
    fn operator_to_expr(&self, op: Operator) -> Expr {
        match op {
            Operator::Apply => Expr::Apply,
            Operator::Pipe => Expr::Pipe,
            Operator::Write => Expr::RedirectOut,
            Operator::Append => Expr::RedirectAppend,
            Operator::Read => Expr::RedirectIn,
            Operator::WriteErr => Expr::RedirectErr,
            Operator::AppendErr => Expr::RedirectErrAppend,
            Operator::WriteBoth => Expr::RedirectBoth,
            Operator::ErrToOut => Expr::RedirectErrToOut,
            Operator::Background => Expr::Background,
            Operator::And => Expr::And,
            Operator::Or => Expr::Or,
        }
    }

    /// Parse a block (everything between [ and ])
    fn parse_block(&mut self) -> Result<Expr, ParseError> {
        let mut inner = Vec::new();

        while !self.is_at_end() {
            match self.peek() {
                Some(Token::BlockEnd) => {
                    self.advance(); // consume the ]
                    return Ok(Expr::Block(inner));
                }
                Some(_) => {
                    // parse_expr handles nested blocks via recursion
                    let exprs = self.parse_expr()?;
                    inner.extend(exprs);
                }
                None => {
                    return Err(ParseError::UnmatchedBlockStart);
                }
            }
        }

        Err(ParseError::UnmatchedBlockStart)
    }
}

/// Parse tokens into a Program
pub fn parse(tokens: Vec<Token>) -> Result<Program, ParseError> {
    let mut parser = Parser::new(tokens);
    parser.parse()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::lex;

    #[test]
    fn parse_simple_words() {
        let tokens = lex("hello world").unwrap();
        let program = parse(tokens).unwrap();
        assert_eq!(
            program.expressions,
            vec![
                Expr::Literal("hello".into()),
                Expr::Literal("world".into()),
            ]
        );
    }

    #[test]
    fn parse_block() {
        let tokens = lex("[hello echo]").unwrap();
        let program = parse(tokens).unwrap();
        assert_eq!(
            program.expressions,
            vec![Expr::Block(vec![
                Expr::Literal("hello".into()),
                Expr::Literal("echo".into()),
            ])]
        );
    }

    #[test]
    fn parse_apply() {
        let tokens = lex("hello [echo] @").unwrap();
        let program = parse(tokens).unwrap();
        assert_eq!(
            program.expressions,
            vec![
                Expr::Literal("hello".into()),
                Expr::Block(vec![Expr::Literal("echo".into())]),
                Expr::Apply,
            ]
        );
    }

    #[test]
    fn parse_stack_ops() {
        let tokens = lex("a b dup swap drop").unwrap();
        let program = parse(tokens).unwrap();
        assert_eq!(
            program.expressions,
            vec![
                Expr::Literal("a".into()),
                Expr::Literal("b".into()),
                Expr::Dup,
                Expr::Swap,
                Expr::Drop,
            ]
        );
    }

    #[test]
    fn parse_path_ops() {
        let tokens = lex("/path file path-join suffix").unwrap();
        let program = parse(tokens).unwrap();
        assert_eq!(
            program.expressions,
            vec![
                Expr::Literal("/path".into()),
                Expr::Literal("file".into()),
                Expr::Join,
                Expr::Suffix,
            ]
        );
    }

    #[test]
    fn parse_nested_blocks() {
        let tokens = lex("[[inner] outer]").unwrap();
        let program = parse(tokens).unwrap();
        assert_eq!(
            program.expressions,
            vec![Expr::Block(vec![
                Expr::Block(vec![Expr::Literal("inner".into())]),
                Expr::Literal("outer".into()),
            ])]
        );
    }

    #[test]
    fn parse_operators() {
        let tokens = lex("@ | > >> < & && ||").unwrap();
        let program = parse(tokens).unwrap();
        assert_eq!(
            program.expressions,
            vec![
                Expr::Apply,
                Expr::Pipe,
                Expr::RedirectOut,
                Expr::RedirectAppend,
                Expr::RedirectIn,
                Expr::Background,
                Expr::And,
                Expr::Or,
            ]
        );
    }

    #[test]
    fn parse_quoted_strings() {
        let tokens = lex("\"hello world\" 'literal'").unwrap();
        let program = parse(tokens).unwrap();
        assert_eq!(
            program.expressions,
            vec![
                Expr::Quoted { content: "hello world".into(), double: true },
                Expr::Quoted { content: "literal".into(), double: false },
            ]
        );
    }

    #[test]
    fn parse_variable() {
        let tokens = lex("$HOME echo").unwrap();
        let program = parse(tokens).unwrap();
        assert_eq!(
            program.expressions,
            vec![
                Expr::Variable("$HOME".into()),
                Expr::Literal("echo".into()),
            ]
        );
    }

    #[test]
    fn parse_unmatched_block_start() {
        let tokens = lex("[hello").unwrap();
        let result = parse(tokens);
        assert!(matches!(result, Err(ParseError::UnmatchedBlockStart)));
    }

    #[test]
    fn parse_unmatched_block_end() {
        let tokens = lex("hello]").unwrap();
        let result = parse(tokens);
        assert!(matches!(result, Err(ParseError::UnmatchedBlockEnd)));
    }

    #[test]
    fn parse_definition() {
        let tokens = lex("[dup .bak suffix cp] :backup").unwrap();
        let program = parse(tokens).unwrap();
        assert_eq!(
            program.expressions,
            vec![
                Expr::Block(vec![
                    Expr::Dup,
                    Expr::Literal(".bak".into()),
                    Expr::Suffix,
                    Expr::Literal("cp".into()),
                ]),
                Expr::Define("backup".into()),
            ]
        );
    }
}
