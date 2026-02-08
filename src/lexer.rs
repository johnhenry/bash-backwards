//! Tokenization for hsab

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
    Pipe,       // | (explicit pipe, though we use postfix)
    Write,      // >
    Append,     // >>
    Read,       // <
    Background, // &
}

#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    /// A word (command name or argument)
    Word(String),
    /// Group start: %(
    GroupStart,
    /// Group end: )
    GroupEnd,
    /// Subshell start: $(
    SubshellStart,
    /// An operator (&&, ||, |, >, >>, <, &)
    Operator(Operator),
    /// A double-quoted string
    DoubleQuoted(String),
    /// A single-quoted string
    SingleQuoted(String),
    /// Bash passthrough: \{...}
    BashPassthrough(String),
    /// Variable reference: $VAR or ${VAR}
    Variable(String),
}

#[derive(Error, Debug)]
pub enum LexError {
    #[error("Unexpected character: {0}")]
    UnexpectedChar(char),
    #[error("Unterminated string")]
    UnterminatedString,
    #[error("Unterminated group")]
    UnterminatedGroup,
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

/// Parse a group start: %(
fn group_start(input: &str) -> IResult<&str, Token> {
    value(Token::GroupStart, tag("%("))(input)
}

/// Parse a group end: )
fn group_end(input: &str) -> IResult<&str, Token> {
    value(Token::GroupEnd, char(')'))(input)
}

/// Parse a subshell start: $(
fn subshell_start(input: &str) -> IResult<&str, Token> {
    value(Token::SubshellStart, tag("$("))(input)
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

/// Parse | operator (explicit pipe)
fn pipe_op(input: &str) -> IResult<&str, Token> {
    value(Token::Operator(Operator::Pipe), char('|'))(input)
}

/// Parse & operator (background, but not &&)
fn background_op(input: &str) -> IResult<&str, Token> {
    // We need to make sure we don't match the first & of &&
    let (input, _) = char('&')(input)?;
    // Peek ahead to make sure next char is not &
    if input.starts_with('&') {
        // This is &&, not a single &
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
        // $VAR form (but not $( which is subshell)
        map(
            preceded(
                char('$'),
                take_while1(|c: char| c.is_alphanumeric() || c == '_'),
            ),
            |s: &str| Token::Variable(format!("${}", s)),
        ),
    ))(input)
}

/// Parse bash passthrough: \{...}
fn bash_passthrough(input: &str) -> IResult<&str, Token> {
    let (input, _) = tag("\\{")(input)?;
    // Find matching closing brace, handling nesting
    let mut depth = 1;
    let mut end_idx = 0;
    for (i, c) in input.char_indices() {
        match c {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    end_idx = i;
                    break;
                }
            }
            _ => {}
        }
    }
    if depth != 0 {
        return Err(nom::Err::Error(nom::error::Error::new(
            input,
            nom::error::ErrorKind::TakeUntil,
        )));
    }
    let content = &input[..end_idx];
    let remaining = &input[end_idx + 1..];
    Ok((remaining, Token::BashPassthrough(content.to_string())))
}

/// Parse a word (command name or argument)
fn word(input: &str) -> IResult<&str, Token> {
    map(
        take_while1(|c: char| {
            !c.is_whitespace()
                && c != '%'
                && c != '$'
                && c != '('
                && c != ')'
                && c != '&'
                && c != '|'
                && c != '>'
                && c != '<'
                && c != '"'
                && c != '\''
                && c != '\\'
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
            // Group/subshell markers
            group_start,
            subshell_start,
            group_end,
            // Strings
            double_quoted_string,
            single_quoted_string,
            // Bash passthrough
            bash_passthrough,
            // Variable (before single-char operators)
            variable,
            // Single-char operators
            write_op,
            read_op,
            pipe_op,
            background_op,
            // Words last
            word,
        )),
    )(input)
}

/// Tokenize a complete input string
pub fn lex(input: &str) -> Result<Vec<Token>, LexError> {
    let (remaining, tokens) = many0(token)(input)
        .map_err(|e| LexError::ParseError(format!("{:?}", e)))?;

    // Check for any remaining unparsed content (after trimming whitespace)
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
    fn tokenize_simple_command() {
        let tokens = lex("ls").unwrap();
        assert_eq!(tokens, vec![Token::Word("ls".to_string())]);
    }

    #[test]
    fn tokenize_command_with_args() {
        let tokens = lex("ls -la /tmp").unwrap();
        assert_eq!(
            tokens,
            vec![
                Token::Word("ls".to_string()),
                Token::Word("-la".to_string()),
                Token::Word("/tmp".to_string()),
            ]
        );
    }

    #[test]
    fn tokenize_grouped_args() {
        let tokens = lex("%(hello grep) ls").unwrap();
        assert_eq!(
            tokens,
            vec![
                Token::GroupStart,
                Token::Word("hello".to_string()),
                Token::Word("grep".to_string()),
                Token::GroupEnd,
                Token::Word("ls".to_string()),
            ]
        );
    }

    #[test]
    fn tokenize_and_operator() {
        let tokens = lex("cmd1 cmd2 &&").unwrap();
        assert_eq!(
            tokens,
            vec![
                Token::Word("cmd1".to_string()),
                Token::Word("cmd2".to_string()),
                Token::Operator(Operator::And),
            ]
        );
    }

    #[test]
    fn tokenize_or_operator() {
        let tokens = lex("cmd1 cmd2 ||").unwrap();
        assert_eq!(
            tokens,
            vec![
                Token::Word("cmd1".to_string()),
                Token::Word("cmd2".to_string()),
                Token::Operator(Operator::Or),
            ]
        );
    }

    #[test]
    fn tokenize_redirect_operators() {
        let tokens = lex("cmd file.txt >").unwrap();
        assert_eq!(
            tokens,
            vec![
                Token::Word("cmd".to_string()),
                Token::Word("file.txt".to_string()),
                Token::Operator(Operator::Write),
            ]
        );

        let tokens = lex("cmd file.txt >>").unwrap();
        assert_eq!(
            tokens,
            vec![
                Token::Word("cmd".to_string()),
                Token::Word("file.txt".to_string()),
                Token::Operator(Operator::Append),
            ]
        );
    }

    #[test]
    fn tokenize_background() {
        let tokens = lex("cmd &").unwrap();
        assert_eq!(
            tokens,
            vec![
                Token::Word("cmd".to_string()),
                Token::Operator(Operator::Background),
            ]
        );
    }

    #[test]
    fn tokenize_double_quoted_string() {
        let tokens = lex("\"hello world\"").unwrap();
        assert_eq!(
            tokens,
            vec![Token::DoubleQuoted("hello world".to_string())]
        );
    }

    #[test]
    fn tokenize_single_quoted_string() {
        let tokens = lex("'hello world'").unwrap();
        assert_eq!(
            tokens,
            vec![Token::SingleQuoted("hello world".to_string())]
        );
    }

    #[test]
    fn tokenize_variable() {
        let tokens = lex("$HOME").unwrap();
        assert_eq!(tokens, vec![Token::Variable("$HOME".to_string())]);
    }

    #[test]
    fn tokenize_braced_variable() {
        let tokens = lex("${HOME}").unwrap();
        assert_eq!(tokens, vec![Token::Variable("${HOME}".to_string())]);
    }

    #[test]
    fn tokenize_subshell() {
        let tokens = lex("$( cmd )").unwrap();
        assert_eq!(
            tokens,
            vec![
                Token::SubshellStart,
                Token::Word("cmd".to_string()),
                Token::GroupEnd, // ) is ambiguous, parser handles context
            ]
        );
    }

    #[test]
    fn tokenize_complex_expression() {
        let tokens = lex("%(5 head) %(hello grep) ls").unwrap();
        assert_eq!(
            tokens,
            vec![
                Token::GroupStart,
                Token::Word("5".to_string()),
                Token::Word("head".to_string()),
                Token::GroupEnd,
                Token::GroupStart,
                Token::Word("hello".to_string()),
                Token::Word("grep".to_string()),
                Token::GroupEnd,
                Token::Word("ls".to_string()),
            ]
        );
    }

    #[test]
    fn tokenize_mixed_operators() {
        let tokens = lex("ls %(done echo) && %(fail echo) ||").unwrap();
        assert_eq!(
            tokens,
            vec![
                Token::Word("ls".to_string()),
                Token::GroupStart,
                Token::Word("done".to_string()),
                Token::Word("echo".to_string()),
                Token::GroupEnd,
                Token::Operator(Operator::And),
                Token::GroupStart,
                Token::Word("fail".to_string()),
                Token::Word("echo".to_string()),
                Token::GroupEnd,
                Token::Operator(Operator::Or),
            ]
        );
    }
}
