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
    And,             // &&
    Or,              // ||
    Pipe,            // |
    Write,           // >
    Append,          // >>
    Read,            // <
    Background,      // &
    Apply,           // @
    WriteErr,        // 2>
    AppendErr,       // 2>>
    WriteBoth,       // &>
    ErrToOut,        // 2>&1
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
    /// Variable reference: $VAR or ${VAR}
    Variable(String),
    /// Definition: :name (stores block with given name)
    Define(String),
    /// Semicolon delimiter for scoped variable assignments
    Semicolon,
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

/// Parse a triple double-quoted string (multiline)
fn triple_double_quoted_string(input: &str) -> IResult<&str, Token> {
    let (input, _) = tag("\"\"\"")(input)?;
    // Find the closing """
    if let Some(end_pos) = input.find("\"\"\"") {
        let content = &input[..end_pos];
        let remaining = &input[end_pos + 3..];
        Ok((remaining, Token::DoubleQuoted(content.to_string())))
    } else {
        Err(nom::Err::Error(nom::error::Error::new(
            input,
            nom::error::ErrorKind::Tag,
        )))
    }
}

/// Parse a triple single-quoted string (multiline, literal)
fn triple_single_quoted_string(input: &str) -> IResult<&str, Token> {
    let (input, _) = tag("'''")(input)?;
    // Find the closing '''
    if let Some(end_pos) = input.find("'''") {
        let content = &input[..end_pos];
        let remaining = &input[end_pos + 3..];
        Ok((remaining, Token::SingleQuoted(content.to_string())))
    } else {
        Err(nom::Err::Error(nom::error::Error::new(
            input,
            nom::error::ErrorKind::Tag,
        )))
    }
}

/// Parse a double-quoted string
/// Allows escape sequences: \", \\, \n, \r, \t, \e, \x## (hex), \0## (octal)
fn double_quoted_string(input: &str) -> IResult<&str, Token> {
    let (input, _) = char('"')(input)?;

    let mut content = String::new();
    let mut remaining = input;

    loop {
        // Find next special character (quote or backslash)
        match remaining.find(|c| c == '"' || c == '\\') {
            Some(0) => {
                // Special char at start
                let c = remaining.chars().next().unwrap();
                if c == '"' {
                    // End of string
                    return Ok((&remaining[1..], Token::DoubleQuoted(content)));
                } else {
                    // Backslash - consume escape sequence
                    if remaining.len() > 1 {
                        let next = remaining.chars().nth(1).unwrap();
                        match next {
                            'x' if remaining.len() >= 4 => {
                                // Hex escape \x## - keep raw for parser to process
                                content.push_str(&remaining[..4]);
                                remaining = &remaining[4..];
                            }
                            '0' if remaining.len() >= 4 => {
                                // Octal escape \0## - keep raw for parser to process
                                content.push_str(&remaining[..4]);
                                remaining = &remaining[4..];
                            }
                            'e' | 'n' | 'r' | 't' | '"' | '\\' | '$' | '`' => {
                                // Known escape - keep raw for parser to process
                                content.push_str(&remaining[..2]);
                                remaining = &remaining[2..];
                            }
                            _ => {
                                // Unknown escape - keep the backslash and char
                                content.push_str(&remaining[..2]);
                                remaining = &remaining[2..];
                            }
                        }
                    } else {
                        // Trailing backslash
                        content.push('\\');
                        remaining = &remaining[1..];
                    }
                }
            }
            Some(pos) => {
                // Add chars before special char
                content.push_str(&remaining[..pos]);
                remaining = &remaining[pos..];
            }
            None => {
                // No closing quote found
                return Err(nom::Err::Error(nom::error::Error::new(
                    input,
                    nom::error::ErrorKind::Tag,
                )));
            }
        }
    }
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

/// Parse 2>&1 operator (must come before 2>> and 2>)
fn err_to_out_op(input: &str) -> IResult<&str, Token> {
    value(Token::Operator(Operator::ErrToOut), tag("2>&1"))(input)
}

/// Parse 2>> operator (must come before 2>)
fn append_err_op(input: &str) -> IResult<&str, Token> {
    value(Token::Operator(Operator::AppendErr), tag("2>>"))(input)
}

/// Parse 2> operator
fn write_err_op(input: &str) -> IResult<&str, Token> {
    value(Token::Operator(Operator::WriteErr), tag("2>"))(input)
}

/// Parse &> operator (must come before & background)
fn write_both_op(input: &str) -> IResult<&str, Token> {
    value(Token::Operator(Operator::WriteBoth), tag("&>"))(input)
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

/// Parse & operator (background, but not && or &>)
fn background_op(input: &str) -> IResult<&str, Token> {
    let (input, _) = char('&')(input)?;
    // Peek ahead to make sure next char is not & or >
    if input.starts_with('&') || input.starts_with('>') {
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
/// Names can include alphanumeric, underscore, hyphen, and ? (for predicates)
fn definition(input: &str) -> IResult<&str, Token> {
    map(
        preceded(
            char(':'),
            take_while1(|c: char| c.is_alphanumeric() || c == '_' || c == '-' || c == '?'),
        ),
        |s: &str| Token::Define(s.to_string()),
    )(input)
}

/// Parse a semicolon delimiter
fn semicolon(input: &str) -> IResult<&str, Token> {
    value(Token::Semicolon, char(';'))(input)
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
                && c != ';'
        }),
        |s: &str| Token::Word(s.to_string()),
    )(input)
}

/// Parse any single token
fn token(input: &str) -> IResult<&str, Token> {
    preceded(
        multispace0,
        alt((
            // Group 1: Multi-char operators first (order matters!)
            alt((
                and_op,
                or_op,
                err_to_out_op,   // 2>&1 before 2>> and 2>
                append_err_op,   // 2>> before 2>
                write_err_op,    // 2>
                append_op,       // >> before >
                write_both_op,   // &> before &
            )),
            // Group 2: Block markers and strings
            alt((
                block_start,
                block_end,
                triple_double_quoted_string,
                triple_single_quoted_string,
                double_quoted_string,
                single_quoted_string,
            )),
            // Group 3: Variable, definition, semicolon, single-char operators, word
            alt((
                variable,
                definition,
                semicolon,
                write_op,
                read_op,
                pipe_op,
                apply_op,
                background_op,
                word,
            )),
        )),
    )(input)
}

/// Strip inline comments from input (# to end of line)
/// Handles triple-quoted strings correctly
fn strip_comments(input: &str) -> String {
    let mut result = String::new();
    let mut in_single_quote = false;
    let mut in_double_quote = false;
    let mut in_triple_single = false;
    let mut in_triple_double = false;
    let chars: Vec<char> = input.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        // Check for triple quotes first
        if i + 2 < chars.len() {
            let triple: String = chars[i..i + 3].iter().collect();
            if triple == "'''" && !in_double_quote && !in_triple_double {
                in_triple_single = !in_triple_single;
                result.push_str("'''");
                i += 3;
                continue;
            }
            if triple == "\"\"\"" && !in_single_quote && !in_triple_single {
                in_triple_double = !in_triple_double;
                result.push_str("\"\"\"");
                i += 3;
                continue;
            }
        }

        let c = chars[i];
        let in_any_quote = in_single_quote || in_double_quote || in_triple_single || in_triple_double;

        match c {
            '\'' if !in_double_quote && !in_triple_single && !in_triple_double => {
                in_single_quote = !in_single_quote;
                result.push(c);
            }
            '"' if !in_single_quote && !in_triple_single && !in_triple_double => {
                in_double_quote = !in_double_quote;
                result.push(c);
            }
            '#' if !in_any_quote => {
                // Skip to end of line
                while i < chars.len() && chars[i] != '\n' {
                    i += 1;
                }
                if i < chars.len() && chars[i] == '\n' {
                    result.push('\n');
                }
            }
            _ => result.push(c),
        }
        i += 1;
    }
    result
}

/// Expand brace patterns in a token
/// Handles: {a,b,c}, {1..5}, prefix{a,b}suffix
fn expand_braces(token: Token) -> Vec<Token> {
    match &token {
        Token::Word(s) if s.contains('{') && s.contains('}') => {
            match expand_brace_pattern(s) {
                Some(expansions) => expansions.into_iter().map(Token::Word).collect(),
                None => vec![token], // No valid brace pattern, return as-is
            }
        }
        _ => vec![token],
    }
}

/// Expand a brace pattern in a string
/// Returns None if no valid brace pattern found
fn expand_brace_pattern(s: &str) -> Option<Vec<String>> {
    // Find the first '{' and matching '}'
    let brace_start = s.find('{')?;
    let brace_end = find_matching_brace(s, brace_start)?;

    let prefix = &s[..brace_start];
    let brace_content = &s[brace_start + 1..brace_end];
    let suffix = &s[brace_end + 1..];

    // Check for range pattern: {1..5} or {a..z}
    if let Some(expansions) = try_range_expansion(brace_content) {
        let results: Vec<String> = expansions
            .into_iter()
            .map(|item| format!("{}{}{}", prefix, item, suffix))
            .collect();
        // Recursively expand any remaining braces in suffix
        return Some(results.into_iter()
            .flat_map(|s| expand_brace_pattern(&s).unwrap_or_else(|| vec![s]))
            .collect());
    }

    // Check for comma-separated list: {a,b,c}
    if brace_content.contains(',') {
        let items: Vec<&str> = split_brace_items(brace_content);
        let results: Vec<String> = items
            .into_iter()
            .map(|item| format!("{}{}{}", prefix, item, suffix))
            .collect();
        // Recursively expand any remaining braces
        return Some(results.into_iter()
            .flat_map(|s| expand_brace_pattern(&s).unwrap_or_else(|| vec![s]))
            .collect());
    }

    None
}

/// Find the matching closing brace, handling nested braces
fn find_matching_brace(s: &str, start: usize) -> Option<usize> {
    let chars: Vec<char> = s.chars().collect();
    let mut depth = 0;
    for i in start..chars.len() {
        match chars[i] {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
    }
    None
}

/// Split brace content by commas, respecting nested braces
fn split_brace_items(content: &str) -> Vec<&str> {
    let mut items = Vec::new();
    let mut start = 0;
    let mut depth = 0;
    let chars: Vec<char> = content.chars().collect();

    for (i, &c) in chars.iter().enumerate() {
        match c {
            '{' => depth += 1,
            '}' => depth -= 1,
            ',' if depth == 0 => {
                items.push(&content[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    items.push(&content[start..]);
    items
}

/// Try to expand a range pattern like 1..5 or a..z
fn try_range_expansion(content: &str) -> Option<Vec<String>> {
    let parts: Vec<&str> = content.split("..").collect();
    if parts.len() != 2 {
        return None;
    }

    let start = parts[0].trim();
    let end = parts[1].trim();

    // Try numeric range
    if let (Ok(start_num), Ok(end_num)) = (start.parse::<i64>(), end.parse::<i64>()) {
        let range: Vec<String> = if start_num <= end_num {
            (start_num..=end_num).map(|n| n.to_string()).collect()
        } else {
            (end_num..=start_num).rev().map(|n| n.to_string()).collect()
        };
        return Some(range);
    }

    // Try single-char range (a..z)
    if start.len() == 1 && end.len() == 1 {
        let start_char = start.chars().next()?;
        let end_char = end.chars().next()?;

        if start_char.is_ascii_alphabetic() && end_char.is_ascii_alphabetic() {
            let range: Vec<String> = if start_char <= end_char {
                (start_char..=end_char).map(|c| c.to_string()).collect()
            } else {
                (end_char..=start_char).rev().map(|c| c.to_string()).collect()
            };
            return Some(range);
        }
    }

    None
}

/// Tokenize a complete input string
pub fn lex(input: &str) -> Result<Vec<Token>, LexError> {
    // Strip inline comments first
    let input = strip_comments(input);

    let (remaining, tokens) = many0(token)(&input)
        .map_err(|e| LexError::ParseError(format!("{:?}", e)))?;

    // Check for any remaining unparsed content
    let remaining = remaining.trim();
    if !remaining.is_empty() {
        return Err(LexError::UnexpectedChar(
            remaining.chars().next().unwrap(),
        ));
    }

    // Expand brace patterns
    let tokens: Vec<Token> = tokens.into_iter()
        .flat_map(expand_braces)
        .collect();

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
    fn tokenize_stderr_redirects() {
        let tokens = lex("cmd 2> file").unwrap();
        assert_eq!(
            tokens,
            vec![
                Token::Word("cmd".to_string()),
                Token::Operator(Operator::WriteErr),
                Token::Word("file".to_string()),
            ]
        );

        let tokens = lex("cmd 2>> file").unwrap();
        assert_eq!(
            tokens,
            vec![
                Token::Word("cmd".to_string()),
                Token::Operator(Operator::AppendErr),
                Token::Word("file".to_string()),
            ]
        );

        let tokens = lex("cmd &> file").unwrap();
        assert_eq!(
            tokens,
            vec![
                Token::Word("cmd".to_string()),
                Token::Operator(Operator::WriteBoth),
                Token::Word("file".to_string()),
            ]
        );

        let tokens = lex("cmd 2>&1").unwrap();
        assert_eq!(
            tokens,
            vec![
                Token::Word("cmd".to_string()),
                Token::Operator(Operator::ErrToOut),
            ]
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

    #[test]
    fn tokenize_triple_double_quoted() {
        let tokens = lex("\"\"\"line 1\nline 2\"\"\" echo").unwrap();
        assert_eq!(
            tokens,
            vec![
                Token::DoubleQuoted("line 1\nline 2".to_string()),
                Token::Word("echo".to_string()),
            ]
        );
    }

    #[test]
    fn tokenize_triple_single_quoted() {
        let tokens = lex("'''$HOME stays literal''' echo").unwrap();
        assert_eq!(
            tokens,
            vec![
                Token::SingleQuoted("$HOME stays literal".to_string()),
                Token::Word("echo".to_string()),
            ]
        );
    }

    // ============================================
    // Brace expansion tests
    // ============================================

    #[test]
    fn brace_expansion_comma_list() {
        let tokens = lex("{a,b,c}").unwrap();
        assert_eq!(
            tokens,
            vec![
                Token::Word("a".to_string()),
                Token::Word("b".to_string()),
                Token::Word("c".to_string()),
            ]
        );
    }

    #[test]
    fn brace_expansion_numeric_range() {
        let tokens = lex("{1..5}").unwrap();
        assert_eq!(
            tokens,
            vec![
                Token::Word("1".to_string()),
                Token::Word("2".to_string()),
                Token::Word("3".to_string()),
                Token::Word("4".to_string()),
                Token::Word("5".to_string()),
            ]
        );
    }

    #[test]
    fn brace_expansion_char_range() {
        let tokens = lex("{a..d}").unwrap();
        assert_eq!(
            tokens,
            vec![
                Token::Word("a".to_string()),
                Token::Word("b".to_string()),
                Token::Word("c".to_string()),
                Token::Word("d".to_string()),
            ]
        );
    }

    #[test]
    fn brace_expansion_with_prefix_suffix() {
        let tokens = lex("file{1,2}.txt").unwrap();
        assert_eq!(
            tokens,
            vec![
                Token::Word("file1.txt".to_string()),
                Token::Word("file2.txt".to_string()),
            ]
        );
    }

    #[test]
    fn brace_expansion_reverse_range() {
        let tokens = lex("{5..1}").unwrap();
        assert_eq!(
            tokens,
            vec![
                Token::Word("5".to_string()),
                Token::Word("4".to_string()),
                Token::Word("3".to_string()),
                Token::Word("2".to_string()),
                Token::Word("1".to_string()),
            ]
        );
    }

    #[test]
    fn brace_expansion_in_context() {
        let tokens = lex("{a,b,c} echo").unwrap();
        assert_eq!(
            tokens,
            vec![
                Token::Word("a".to_string()),
                Token::Word("b".to_string()),
                Token::Word("c".to_string()),
                Token::Word("echo".to_string()),
            ]
        );
    }

    #[test]
    fn brace_expansion_multiple() {
        // Multiple braces in same word: {a,b}{1,2} -> a1, a2, b1, b2
        let tokens = lex("{a,b}{1,2}").unwrap();
        assert_eq!(
            tokens,
            vec![
                Token::Word("a1".to_string()),
                Token::Word("a2".to_string()),
                Token::Word("b1".to_string()),
                Token::Word("b2".to_string()),
            ]
        );
    }
}
