//! Tokenization for hsab v2
//!
//! Tokens represent the atomic elements of hsab syntax.

use nom::{
    branch::alt,
    bytes::complete::{escaped, tag, take_while1},
    character::complete::{char, multispace0, none_of, one_of},
    combinator::{map, opt, recognize, value},
    multi::many0,
    sequence::{delimited, preceded, tuple},
    IResult,
};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq)]
pub enum Operator {
    And,        // &&
    Or,         // ||
    Pipe,       // |
    Write,      // >
    Append,     // >>
    Read,       // <
    Background, // &
    Apply,      // @
}

#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    /// A word (command name, argument, flag)
    Word(String),
    /// Block/quotation start: [
    BlockStart,
    /// Block/quotation end: ]
    BlockEnd,
    /// An operator
    Operator(Operator),
    /// A double-quoted string
    DoubleQuoted(String),
    /// A single-quoted string
    SingleQuoted(String),
    /// Bash passthrough: #!bash ...
    BashPassthrough(String),
    /// Variable reference: $VAR or ${VAR}
    Variable(String),
    /// Definition: :name (stores block with given name)
    Define(String),
}

#[derive(Error, Debug)]
pub enum LexError {
    #[error("Unexpected character: {0}")]
    UnexpectedChar(char),
    #[error("Unterminated string")]
    UnterminatedString,
    #[error("Unterminated block")]
    UnterminatedBlock,
    #[error("Parse error: {0}")]
    ParseError(String),
}

/// Parse a double-quoted string
fn double_quoted_string(input: &str) -> IResult<&str, Token> {
    let (input, content) = delimited(
        char('"'),
        map(
            opt(escaped(none_of("\"\\"), '\\', one_of("\"\\nrt$`"))),
            |o| o.unwrap_or(""),
        ),
        char('"'),
    )(input)?;
    Ok((input, Token::DoubleQuoted(content.to_string())))
}

/// Parse a single-quoted string
fn single_quoted_string(input: &str) -> IResult<&str, Token> {
    let (input, content) = delimited(
        char('\''),
        map(opt(take_while1(|c| c != '\'')), |o| o.unwrap_or("")),
        char('\''),
    )(input)?;
    Ok((input, Token::SingleQuoted(content.to_string())))
}

/// Parse a block start: [
fn block_start(input: &str) -> IResult<&str, Token> {
    value(Token::BlockStart, char('['))(input)
}

/// Parse a block end: ]
fn block_end(input: &str) -> IResult<&str, Token> {
    value(Token::BlockEnd, char(']'))(input)
}

/// Parse && operator
fn and_op(input: &str) -> IResult<&str, Token> {
    value(Token::Operator(Operator::And), tag("&&"))(input)
}

/// Parse || operator
fn or_op(input: &str) -> IResult<&str, Token> {
    value(Token::Operator(Operator::Or), tag("||"))(input)
}

/// Parse >> operator (must come before >)
fn append_op(input: &str) -> IResult<&str, Token> {
    value(Token::Operator(Operator::Append), tag(">>"))(input)
}

/// Parse > operator
fn write_op(input: &str) -> IResult<&str, Token> {
    value(Token::Operator(Operator::Write), char('>'))(input)
}

/// Parse < operator
fn read_op(input: &str) -> IResult<&str, Token> {
    value(Token::Operator(Operator::Read), char('<'))(input)
}

/// Parse | operator
fn pipe_op(input: &str) -> IResult<&str, Token> {
    value(Token::Operator(Operator::Pipe), char('|'))(input)
}

/// Parse @ operator (apply)
fn apply_op(input: &str) -> IResult<&str, Token> {
    value(Token::Operator(Operator::Apply), char('@'))(input)
}

/// Parse & operator (background, but not &&)
fn background_op(input: &str) -> IResult<&str, Token> {
    let (input, _) = char('&')(input)?;
    // Peek ahead to make sure next char is not &
    if input.starts_with('&') {
        Err(nom::Err::Error(nom::error::Error::new(
            input,
            nom::error::ErrorKind::Tag,
        )))
    } else {
        Ok((input, Token::Operator(Operator::Background)))
    }
}

/// Parse a variable: $VAR or ${VAR}
fn variable(input: &str) -> IResult<&str, Token> {
    alt((
        // ${VAR} form
        map(
            preceded(
                tag("${"),
                recognize(tuple((
                    take_while1(|c: char| c.is_alphanumeric() || c == '_'),
                    char('}'),
                ))),
            ),
            |s: &str| Token::Variable(format!("${{{}", s)),
        ),
        // $VAR form
        map(
            preceded(
                char('$'),
                take_while1(|c: char| c.is_alphanumeric() || c == '_'),
            ),
            |s: &str| Token::Variable(format!("${}", s)),
        ),
    ))(input)
}

/// Parse a definition: :name
fn definition(input: &str) -> IResult<&str, Token> {
    map(
        preceded(
            char(':'),
            take_while1(|c: char| c.is_alphanumeric() || c == '_' || c == '-'),
        ),
        |s: &str| Token::Define(s.to_string()),
    )(input)
}

/// Parse a word (command name or argument)
fn word(input: &str) -> IResult<&str, Token> {
    map(
        take_while1(|c: char| {
            !c.is_whitespace()
                && c != '['
                && c != ']'
                && c != '$'
                && c != '&'
                && c != '|'
                && c != '>'
                && c != '<'
                && c != '@'
                && c != '"'
                && c != '\''
        }),
        |s: &str| Token::Word(s.to_string()),
    )(input)
}

/// Parse any single token
fn token(input: &str) -> IResult<&str, Token> {
    preceded(
        multispace0,
        alt((
            // Multi-char operators first
            and_op,
            or_op,
            append_op,
            // Block markers
            block_start,
            block_end,
            // Strings
            double_quoted_string,
            single_quoted_string,
            // Variable (before single-char operators)
            variable,
            // Definition (before word, so :name is parsed correctly)
            definition,
            // Single-char operators
            write_op,
            read_op,
            pipe_op,
            apply_op,
            background_op,
            // Words last
            word,
        )),
    )(input)
}

/// Strip inline comments from input (# to end of line, but not #!bash)
fn strip_comments(input: &str) -> String {
    let mut result = String::new();
    let mut in_single_quote = false;
    let mut in_double_quote = false;
    let mut chars = input.chars().peekable();

    while let Some(c) = chars.next() {
        match c {
            '\'' if !in_double_quote => {
                in_single_quote = !in_single_quote;
                result.push(c);
            }
            '"' if !in_single_quote => {
                in_double_quote = !in_double_quote;
                result.push(c);
            }
            '#' if !in_single_quote && !in_double_quote => {
                // Check for #!bash (don't strip)
                if chars.peek() == Some(&'!') {
                    result.push(c);
                } else {
                    // Skip to end of line
                    for remaining in chars.by_ref() {
                        if remaining == '\n' {
                            result.push('\n');
                            break;
                        }
                    }
                }
            }
            _ => result.push(c),
        }
    }
    result
}

/// Tokenize a complete input string
pub fn lex(input: &str) -> Result<Vec<Token>, LexError> {
    // Strip inline comments first
    let input = strip_comments(input);

    // Check for bash passthrough first
    let trimmed = input.trim();
    if trimmed.starts_with("#!bash") {
        let bash_code = trimmed.strip_prefix("#!bash").unwrap_or("").trim();
        return Ok(vec![Token::BashPassthrough(bash_code.to_string())]);
    }

    let (remaining, tokens) = many0(token)(&input)
        .map_err(|e| LexError::ParseError(format!("{:?}", e)))?;

    // Check for any remaining unparsed content
    let remaining = remaining.trim();
    if !remaining.is_empty() {
        return Err(LexError::UnexpectedChar(
            remaining.chars().next().unwrap(),
        ));
    }

    Ok(tokens)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokenize_simple_word() {
        let tokens = lex("ls").unwrap();
        assert_eq!(tokens, vec![Token::Word("ls".to_string())]);
    }

    #[test]
    fn tokenize_multiple_words() {
        let tokens = lex("hello world echo").unwrap();
        assert_eq!(
            tokens,
            vec![
                Token::Word("hello".to_string()),
                Token::Word("world".to_string()),
                Token::Word("echo".to_string()),
            ]
        );
    }

    #[test]
    fn tokenize_flags() {
        let tokens = lex("-la ls").unwrap();
        assert_eq!(
            tokens,
            vec![
                Token::Word("-la".to_string()),
                Token::Word("ls".to_string()),
            ]
        );
    }

    #[test]
    fn tokenize_block() {
        let tokens = lex("[hello echo]").unwrap();
        assert_eq!(
            tokens,
            vec![
                Token::BlockStart,
                Token::Word("hello".to_string()),
                Token::Word("echo".to_string()),
                Token::BlockEnd,
            ]
        );
    }

    #[test]
    fn tokenize_apply() {
        let tokens = lex("hello [echo] @").unwrap();
        assert_eq!(
            tokens,
            vec![
                Token::Word("hello".to_string()),
                Token::BlockStart,
                Token::Word("echo".to_string()),
                Token::BlockEnd,
                Token::Operator(Operator::Apply),
            ]
        );
    }

    #[test]
    fn tokenize_pipe() {
        let tokens = lex("ls [grep hello] |").unwrap();
        assert_eq!(
            tokens,
            vec![
                Token::Word("ls".to_string()),
                Token::BlockStart,
                Token::Word("grep".to_string()),
                Token::Word("hello".to_string()),
                Token::BlockEnd,
                Token::Operator(Operator::Pipe),
            ]
        );
    }

    #[test]
    fn tokenize_redirect() {
        let tokens = lex("[hello echo] [file.txt] >").unwrap();
        assert_eq!(
            tokens,
            vec![
                Token::BlockStart,
                Token::Word("hello".to_string()),
                Token::Word("echo".to_string()),
                Token::BlockEnd,
                Token::BlockStart,
                Token::Word("file.txt".to_string()),
                Token::BlockEnd,
                Token::Operator(Operator::Write),
            ]
        );
    }

    #[test]
    fn tokenize_background() {
        let tokens = lex("[10 sleep] &").unwrap();
        assert_eq!(
            tokens,
            vec![
                Token::BlockStart,
                Token::Word("10".to_string()),
                Token::Word("sleep".to_string()),
                Token::BlockEnd,
                Token::Operator(Operator::Background),
            ]
        );
    }

    #[test]
    fn tokenize_and_or() {
        let tokens = lex("ls && echo ||").unwrap();
        assert_eq!(
            tokens,
            vec![
                Token::Word("ls".to_string()),
                Token::Operator(Operator::And),
                Token::Word("echo".to_string()),
                Token::Operator(Operator::Or),
            ]
        );
    }

    #[test]
    fn tokenize_quoted_strings() {
        let tokens = lex("\"hello world\" 'single'").unwrap();
        assert_eq!(
            tokens,
            vec![
                Token::DoubleQuoted("hello world".to_string()),
                Token::SingleQuoted("single".to_string()),
            ]
        );
    }

    #[test]
    fn tokenize_variable() {
        let tokens = lex("$HOME ${USER}").unwrap();
        assert_eq!(
            tokens,
            vec![
                Token::Variable("$HOME".to_string()),
                Token::Variable("${USER}".to_string()),
            ]
        );
    }

    #[test]
    fn tokenize_bash_passthrough() {
        let tokens = lex("#!bash echo hello").unwrap();
        assert_eq!(
            tokens,
            vec![Token::BashPassthrough("echo hello".to_string())]
        );
    }

    #[test]
    fn tokenize_nested_blocks() {
        let tokens = lex("[[inner] outer]").unwrap();
        assert_eq!(
            tokens,
            vec![
                Token::BlockStart,
                Token::BlockStart,
                Token::Word("inner".to_string()),
                Token::BlockEnd,
                Token::Word("outer".to_string()),
                Token::BlockEnd,
            ]
        );
    }

    #[test]
    fn tokenize_definition() {
        let tokens = lex("[dup .bak reext cp] :backup").unwrap();
        assert_eq!(
            tokens,
            vec![
                Token::BlockStart,
                Token::Word("dup".to_string()),
                Token::Word(".bak".to_string()),
                Token::Word("reext".to_string()),
                Token::Word("cp".to_string()),
                Token::BlockEnd,
                Token::Define("backup".to_string()),
            ]
        );
    }

    #[test]
    fn tokenize_inline_comment() {
        let tokens = lex("hello echo # this is a comment").unwrap();
        assert_eq!(
            tokens,
            vec![
                Token::Word("hello".to_string()),
                Token::Word("echo".to_string()),
            ]
        );
    }

    #[test]
    fn tokenize_comment_preserves_quotes() {
        let tokens = lex("\"#not a comment\" echo # but this is").unwrap();
        assert_eq!(
            tokens,
            vec![
                Token::DoubleQuoted("#not a comment".to_string()),
                Token::Word("echo".to_string()),
            ]
        );
    }
}
