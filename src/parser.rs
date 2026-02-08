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

        while !self.is_at_end() {
            let expr = self.parse_expr()?;
            expressions.push(expr);
        }

        if expressions.is_empty() {
            return Err(ParseError::EmptyInput);
        }

        Ok(Program::new(expressions))
    }

    /// Parse a single expression
    fn parse_expr(&mut self) -> Result<Expr, ParseError> {
        let token = self.advance().ok_or(ParseError::UnexpectedEof)?;

        match token {
            Token::Word(s) => Ok(self.word_to_expr(&s)),
            Token::DoubleQuoted(s) => Ok(Expr::Quoted { content: s, double: true }),
            Token::SingleQuoted(s) => Ok(Expr::Quoted { content: s, double: false }),
            Token::Variable(s) => Ok(Expr::Variable(s)),
            Token::BlockStart => self.parse_block(),
            Token::BlockEnd => Err(ParseError::UnmatchedBlockEnd),
            Token::Operator(op) => Ok(self.operator_to_expr(op)),
            Token::BashPassthrough(s) => Ok(Expr::BashPassthrough(s)),
            Token::Define(name) => Ok(Expr::Define(name)),
        }
    }

    /// Convert a word to an expression (handles special words)
    fn word_to_expr(&self, word: &str) -> Expr {
        match word {
            // Stack operations
            "dup" => Expr::Dup,
            "swap" => Expr::Swap,
            "drop" => Expr::Drop,
            "over" => Expr::Over,
            "rot" => Expr::Rot,
            "depth" => Expr::Depth,
            // Path operations
            "join" => Expr::Join,
            "basename" => Expr::Basename,
            "dirname" => Expr::Dirname,
            "suffix" => Expr::Suffix,
            "reext" => Expr::Reext,
            // List operations
            "spread" => Expr::Spread,
            "each" => Expr::Each,
            "collect" => Expr::Collect,
            "keep" => Expr::Keep,
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
            // Interactive TTY
            "tty" => Expr::Tty,
            // Regular word/literal
            _ => Expr::Literal(word.to_string()),
        }
    }

    /// Convert an operator to an expression
    fn operator_to_expr(&self, op: Operator) -> Expr {
        match op {
            Operator::Apply => Expr::Apply,
            Operator::Pipe => Expr::Pipe,
            Operator::Write => Expr::RedirectOut,
            Operator::Append => Expr::RedirectAppend,
            Operator::Read => Expr::RedirectIn,
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
                    let expr = self.parse_expr()?;
                    inner.push(expr);
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
        let tokens = lex("/path file join basename").unwrap();
        let program = parse(tokens).unwrap();
        assert_eq!(
            program.expressions,
            vec![
                Expr::Literal("/path".into()),
                Expr::Literal("file".into()),
                Expr::Join,
                Expr::Basename,
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
    fn parse_bash_passthrough() {
        let tokens = lex("#!bash echo hello").unwrap();
        let program = parse(tokens).unwrap();
        assert_eq!(
            program.expressions,
            vec![Expr::BashPassthrough("echo hello".into())]
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
        let tokens = lex("[dup .bak reext cp] :backup").unwrap();
        let program = parse(tokens).unwrap();
        assert_eq!(
            program.expressions,
            vec![
                Expr::Block(vec![
                    Expr::Dup,
                    Expr::Literal(".bak".into()),
                    Expr::Reext,
                    Expr::Literal("cp".into()),
                ]),
                Expr::Define("backup".into()),
            ]
        );
    }
}
