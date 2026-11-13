use std::borrow::Cow;
use std::collections::VecDeque;
use std::fs;
use std::io::{self};
use std::sync::LazyLock;
use regex::Regex;

use crate::error::{Error, Result};

#[cfg(feature = "python")]
use pyo3::prelude::*;
#[cfg(feature = "python")]
use pyo3::exceptions::PyException;

use crate::tokens::{TokenKind, Tokens};

#[cfg_attr(feature = "python", cfg_eval::cfg_eval)]
#[cfg_attr(feature = "python", pyclass)]
pub struct Reader {
    #[cfg_attr(feature = "python", pyo3(get))]
    pub filename: String,
    #[cfg_attr(feature = "python", pyo3(get))]
    lines: Vec<(String, usize, usize)>,
    stored_line: Option<(String, usize, usize)>,
}

#[cfg_attr(feature = "python", cfg_eval::cfg_eval)]
#[cfg_attr(feature = "python", pymethods)]
impl Reader {
    #[cfg_attr(feature = "python", new)]
    #[cfg_attr(feature = "python", pyo3(signature = (content=None, filename=None)))]
    pub fn new(content: Option<&str>, filename: Option<&str>) -> io::Result<Self> {
        // Nice and tight closed block of code covering all cases instead of separate loosely
        // connected `if` statements spread out.
        let (filename, content) = match (filename, content) {
            (Some(_filename), Some(_content)) => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "Only one of filename or content can be provided",
                ));
            }
            (None, Some(content)) => ("<string>".to_string(), Cow::from(content)),
            (Some(filename), None) => (
                filename.to_string(),
                Cow::from(fs::read_to_string(filename)?),
            ),
            (None, None) => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "Either filename or content must be provided",
                ));
            }
        };

        // lines() works on &str purely so it is either a &str directly from the Cow or
        // the held String's as_str() returned &str
        let content_lines = content.lines();
        // preallocate vector to guessed capacity so that no regrows needed in case of pushes
        let mut lines = Vec::with_capacity(match content_lines.size_hint() {
            (_lb, Some(ub)) => ub,
            (lb, None) => lb,
        });
        for (linenum, line) in content_lines.enumerate() {
            let line = line.trim_end().replace('\t', "    ");
            let stripped_line = line.trim_start();
            let indent = line.len() - stripped_line.len();
            if stripped_line.is_empty()
                || stripped_line.starts_with('#')
                || stripped_line.starts_with("//")
            {
                continue;
            }
            lines.push((stripped_line.to_string(), indent, linenum + 1));
        }

        Ok(Reader {
            filename,
            lines,
            stored_line: None,
        })
    }

    #[cfg_attr(feature = "python", pyo3(signature = (prev_indent=None)))]
    pub fn get_next_line(&mut self, prev_indent: Option<usize>) -> (Option<String>, Option<usize>, Option<usize>) {
        if let Some((line, indent, linenum)) = self.stored_line.take() {
            return (Some(line), Some(indent), Some(linenum));
        }
        if self.lines.is_empty() {
            return (None, None, None);
        }
        if prev_indent.is_some_and(|prev_indent| self.lines[0].1 <= prev_indent) {
            return (None, Some(self.lines[0].1), Some(self.lines[0].2));
        }
        let (line, indent, linenum) = self.lines.remove(0);
        (Some(line), Some(indent), Some(linenum))
    }

    pub fn set_next_line(&mut self, line: String, indent: usize, linenum: usize) {
        let line = line.trim();
        if !line.is_empty() {
            self.stored_line = Some((line.to_string(), indent, linenum));
        }
    }
}

#[cfg_attr(feature = "python", cfg_eval::cfg_eval)]
#[cfg_attr(feature = "python", pyclass(extends=PyException))]
#[derive(Debug)]
pub struct LexerError {
    msg: String,
    line: Option<String>,
    filename : Option<String>,
    linenum : Option<usize>,
}
#[cfg_attr(feature = "python", cfg_eval::cfg_eval)]
#[cfg_attr(feature = "python", pymethods)]
impl LexerError {
    #[cfg_attr(feature = "python", new)]
    #[cfg_attr(feature = "python", pyo3(signature = (msg, line=None, filename=None, linenum=None)))]
    pub fn new(msg: String, line: Option<String>, filename: Option<String>, linenum: Option<usize>) -> Self {
        Self { msg, line, filename, linenum }
    }

    pub fn __str__(&self) -> String {
        let mut full_msg = self.msg.clone();
        if let Some(line) = &self.line {
            full_msg.push_str(&format!(": '{}'", line));
        }
        if let Some(filename) = &self.filename {
            full_msg.push_str(&format!(" ({}:", filename));
        } else {
            full_msg.push_str(" (unknown:");
        }
        if let Some(linenum) = self.linenum {
            full_msg.push_str(&format!("{})", linenum));
        } else {
            full_msg.push_str("unknown)");
        }
        full_msg
    }
}

impl std::fmt::Display for LexerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.__str__())
    }
}

impl std::error::Error for LexerError {}

#[cfg(feature = "python")]
impl LexerError {
    pub(crate) fn into_parts(self) -> (String, Option<String>, Option<String>, Option<usize>) {
        (self.msg, self.line, self.filename, self.linenum)
    }
}

#[derive(PartialEq, Debug)]
enum LineKind {
    Unknown,
    Variants,
    Variant,
    Only,
    No,
    Include,
    Del,
    Suffix,
    Join,
    Operator,
    Else,
}

static OPERATOR_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^[^:\s]*\s*[\+<\~\?]*=.*").expect("Invalid OPERATOR_REGEX pattern")
});

#[cfg_attr(feature = "python", cfg_eval::cfg_eval)]
#[cfg_attr(feature = "python", pyclass)]
pub struct Lexer {
    pub reader: Reader,
    #[cfg_attr(feature = "python", pyo3(get))]
    pub filename: String,
    #[cfg_attr(feature = "python", pyo3(get, set))]
    pub line: Option<String>,
    #[cfg_attr(feature = "python", pyo3(get, set))]
    pub linenum: Option<usize>,
    #[cfg_attr(feature = "python", pyo3(get))]
    pub ignore_white: bool,
    #[cfg_attr(feature = "python", pyo3(get, set))]
    pub rest_as_string: bool,
    #[cfg_attr(feature = "python", pyo3(get))]
    pub prev_indent: Option<usize>,
    pub token_queue: VecDeque<Tokens>,
    // state machine parameters for the iterator
    #[cfg_attr(feature = "python", pyo3(get, set))]
    pos : usize,
    char_buffer : String,
    oper_buffer : String,
    kind : LineKind,
}

#[cfg_attr(feature = "python", cfg_eval::cfg_eval)]
#[cfg_attr(feature = "python", pymethods)]
impl Lexer {
    #[cfg_attr(feature = "python", new)]
    #[cfg_attr(feature = "python", pyo3(signature = (content=None, filename=None)))]
    pub fn new(content: Option<&str>, filename: Option<&str>) -> io::Result<Self> {
        let reader = Reader::new(content, filename)?;
        let filename = reader.filename.clone();
        Ok(Lexer {
            reader,
            filename,
            line: None,
            linenum: None,
            ignore_white: false,
            rest_as_string: false,
            prev_indent: None,
            token_queue: VecDeque::new(),
            pos : 0,
            char_buffer : String::new(),
            oper_buffer : String::new(),
            kind : LineKind::Unknown,
        })
    }

    pub fn restart(&mut self) {
        self.kind = LineKind::Unknown;
        self.pos = 0;
    }

    #[cfg_attr(feature = "python", pyo3(signature = (prev_indent=None)))]
    pub fn set_prev_indent(&mut self, prev_indent: Option<usize>) {
        self.prev_indent = prev_indent;
    }

    /// Tokenize a single line, starting at a desired position.
    pub fn match_line(&mut self, line: &str, pos: usize) -> Result<Vec<Tokens>> {
        let mut tokens = Vec::new();
        let chars: Vec<char> = line.chars().collect();
        let len = chars.len();
        let position = if self.kind == LineKind::Unknown { pos } else { self.pos };
        if position > len {
            return Err(Error::Lexer(LexerError::new(
                format!("Position {position} out of bounds for line of length {len}"),
                Some(line.to_string()),
                Some(self.filename.clone()),
                self.linenum,
            )));
        }

        if self.kind == LineKind::Unknown {
            self.pos = pos;

            // determine the line kind from starting or other keywords
            if let Some(_rest) = line.strip_prefix("variants:") {
                self.kind = LineKind::Variants;
                tokens.push(Tokens::LVariants());
                tokens.push(Tokens::LColon());
                self.pos = 9;
                return Ok(tokens);
            } else if let Some(_rest) = line.strip_prefix("variants ") {
                self.kind = LineKind::Variants;
                tokens.push(Tokens::LVariants());
                self.pos = 8;
                return Ok(tokens);
            } else if let Some(_rest) = line.strip_prefix("-") {
                self.kind = LineKind::Variant;
                tokens.push(Tokens::LVariant());
                self.pos = 1;
                return Ok(tokens);
            } else if let Some(_rest) = line.strip_prefix("only ") {
                self.kind = LineKind::Only;
                tokens.push(Tokens::LOnly());
                self.pos = 4;
                while self.pos < len && chars[self.pos].is_whitespace() { self.pos += 1; }
                return Ok(tokens);
            } else if let Some(_rest) = line.strip_prefix("no ") {
                self.kind = LineKind::No;
                tokens.push(Tokens::LNo());
                self.pos = 2;
                while self.pos < len && chars[self.pos].is_whitespace() { self.pos += 1; }
                return Ok(tokens);
            } else if let Some(_rest) = line.strip_prefix("include ") {
                self.kind = LineKind::Include;
                tokens.push(Tokens::LInclude());
                self.pos = 7;
                return Ok(tokens);
            } else if let Some(_rest) = line.strip_prefix("del ") {
                self.kind = LineKind::Del;
                tokens.push(Tokens::LDel("".to_string(), "".to_string()));
                self.pos = 3;
                while self.pos < len && chars[self.pos].is_whitespace() { self.pos += 1; }
                return Ok(tokens);
            } else if let Some(_rest) = line.strip_prefix("suffix ") {
                self.kind = LineKind::Suffix;
                tokens.push(Tokens::LSuffix());
                self.pos = 6;
                while self.pos < len && chars[self.pos].is_whitespace() { self.pos += 1; }
                return Ok(tokens);
            } else if let Some(_rest) = line.strip_prefix("join ") {
                self.kind = LineKind::Join;
                tokens.push(Tokens::LJoin());
                self.pos = 4;
                while self.pos < len && chars[self.pos].is_whitespace() { self.pos += 1; }
                return Ok(tokens);
            } else if OPERATOR_REGEX.is_match(line) {
                self.kind = LineKind::Operator;
            } else {
                self.kind = LineKind::Else;
            }

        }
        if self.rest_as_string {
            self.rest_as_string = false;
            while self.pos < len && chars[self.pos].is_whitespace() {
                self.pos += 1;
            }
            tokens.push(Tokens::LString(chars[self.pos..].iter().collect()));
            self.pos = len;
            return Ok(tokens);
        }

        // main tokenization loop
        while self.pos < len {
            let c = chars[self.pos];
            if c.is_alphanumeric() || c == '_' || c == '-' || c == '\\' || c == '|' || c == '*' {
                // standard identifier character or a clear cut regex character
                self.char_buffer.push(c);
                self.pos += 1;
                continue;
            }
            if !self.char_buffer.is_empty() {
                tokens.push(Tokens::LIdentifier(self.char_buffer.clone()));
                self.char_buffer.clear();
                return Ok(tokens);
            }
            // handle all remaining cases
            match c {
                _ if c.is_whitespace() => {
                    let mut ws = String::new();
                    while self.pos < len && chars[self.pos].is_whitespace() {
                        ws.push(chars[self.pos]);
                        self.pos += 1;
                    }
                    if !self.ignore_white {
                        tokens.push(Tokens::LWhite(ws));
                    }
                    return Ok(tokens);
                }
                '<' | '+' | '?' | '~' => {
                    let should_push = if c == '?' {
                        // for ? check if followed by =, +, or < to handle three-char operators
                        chars[..self.pos].contains(&' ') ||
                        (self.pos + 1 < len && matches!(chars[self.pos + 1], '=' | '+' | '<'))
                    } else {
                        chars[..self.pos].contains(&' ') ||
                        (self.pos + 1 < len && chars[self.pos + 1] == '=')
                    };
                    if should_push {
                        self.oper_buffer.push(c);
                        self.pos += 1;
                    } else {
                        self.char_buffer.push(c);
                        self.pos += 1;
                        continue;
                    }
                }
                '=' => {
                    match &self.oper_buffer[..] {
                        "" => tokens.push(Tokens::LSet("".to_string(), "".to_string())),
                        "~" => tokens.push(Tokens::LLazySet("".to_string(), "".to_string())),
                        "+" => tokens.push(Tokens::LAppend("".to_string(), "".to_string())),
                        "<" => tokens.push(Tokens::LPrepend("".to_string(), "".to_string())),
                        "?" => tokens.push(Tokens::LRegExpSet("".to_string(), "".to_string())),
                        "?+" => tokens.push(Tokens::LRegExpAppend("".to_string(), "".to_string())),
                        "?<" => tokens.push(Tokens::LRegExpPrepend("".to_string(), "".to_string())),
                        "del" => tokens.push(Tokens::LDel("".to_string(), "".to_string())),
                        _ => {
                            return Err(Error::Lexer(LexerError::new(
                                format!(
                                    "Unexpected operator '{}' at position {}",
                                    self.oper_buffer, self.pos,
                                ),
                                Some(line.to_string()),
                                Some(self.filename.clone()),
                                self.linenum,
                            )));
                        }
                    }
                    // the "=" is also used in expressions like "(a=b)" or "[a=b]"
                    if self.kind == LineKind::Operator {
                        self.pos += 1;
                        while self.pos < len && chars[self.pos].is_whitespace() {
                            self.pos += 1;
                        }
                        tokens.push(Tokens::LString(chars[self.pos..].iter().collect()));
                        self.pos = len;
                    } else {
                        self.pos += 1;
                    }
                    self.oper_buffer.clear();
                    return Ok(tokens);
                }
                '-' => {
                    tokens.push(Tokens::LVariant());
                    self.pos += 1;
                    return Ok(tokens);
                }
                '.' => {
                    if self.kind != LineKind::Operator {
                        tokens.push(Tokens::LDot());
                        self.pos += 1;
                        return Ok(tokens);
                    } else {
                        self.char_buffer.push(c);
                        self.pos += 1;
                        continue;
                    }
                }
                ':' => {
                    tokens.push(Tokens::LColon());
                    self.pos += 1;
                    return Ok(tokens);
                }
                '@' => {
                    tokens.push(Tokens::LDefault());
                    self.pos += 1;
                    return Ok(tokens);
                }
                ',' => {
                    tokens.push(Tokens::LComa());
                    self.pos += 1;
                    return Ok(tokens);
                }
                '[' => {
                    if self.kind != LineKind::Operator {
                        tokens.push(Tokens::LLBracket());
                        self.pos += 1;
                        return Ok(tokens);
                    } else {
                        self.char_buffer.push(c);
                        self.pos += 1;
                        continue;
                    }
                }
                ']' => {
                    if self.kind != LineKind::Operator {
                        tokens.push(Tokens::LRBracket());
                        self.pos += 1;
                        return Ok(tokens);
                    } else {
                        self.char_buffer.push(c);
                        self.pos += 1;
                        continue;
                    }
                }
                '(' => {
                    if self.kind != LineKind::Operator {
                        tokens.push(Tokens::LLRBracket());
                        self.pos += 1;
                        return Ok(tokens);
                    } else {
                        self.char_buffer.push(c);
                        self.pos += 1;
                        continue;
                    }
                }
                ')' => {
                    if self.kind != LineKind::Operator {
                        tokens.push(Tokens::LRRBracket());
                        self.pos += 1;
                        return Ok(tokens);
                    } else {
                        self.char_buffer.push(c);
                        self.pos += 1;
                        continue;
                    }
                }
                '!' => {
                    tokens.push(Tokens::LNotCond());
                    self.pos += 1;
                    return Ok(tokens);
                }
                '"' => {
                    // Parse quoted string
                    self.pos += 1;
                    let mut s = String::new();
                    while self.pos < len && chars[self.pos] != '"' {
                        s.push(chars[self.pos]);
                        self.pos += 1;
                    }
                    if self.pos == len {
                        return Err(Error::Lexer(LexerError::new(
                            "Unterminated quoted string".to_string(),
                            Some(line.to_string()),
                            Some(self.filename.clone()),
                            self.linenum,
                        )));
                    }
                    self.pos += 1; // skip closing quote
                    tokens.push(Tokens::LString(s));
                    return Ok(tokens);
                }
                '#' => {
                    break;
                }
                _ => {
                    return Err(Error::Lexer(LexerError::new(
                        format!(
                            "Unexpected character '{}' at position {}",
                            c, self.pos,
                        ),
                        Some(line.to_string()),
                        Some(self.filename.clone()),
                        self.linenum,
                    )));
                }
            }
        }
        if !self.char_buffer.is_empty() {
            tokens.push(Tokens::LIdentifier(self.char_buffer.clone()));
            self.pos = len;
            self.char_buffer.clear();
            return Ok(tokens);
        }
        tokens.push(Tokens::LEndL());
        Ok(tokens)
    }

    /// Tokenize multiple lines.
    pub fn match_multiline(&mut self) -> Result<Vec<Tokens>> {
        let mut token_queue = Vec::new();
        let (line_opt, indent, linenum) = self.reader.get_next_line(self.prev_indent);
        self.line = line_opt.clone();
        self.linenum = linenum;
        if let (Some(line), Some(indent), Some(linenum)) = (line_opt, indent, linenum) {
            if self.pos == 0 {
                token_queue.push(Tokens::LIndent(indent));

            }
            let tokens = self.match_line(&line, 0)?;
            if let Some(last_token) = tokens.last() {
                match last_token {
                    Tokens::LEndL() => {
                        self.restart();
                    }
                    _ => {
                        // Keep the current line as the next line to comply with the line state machine.
                        self.reader.set_next_line(line, indent, linenum);
                    }
                }
            }
            for t in tokens {
                token_queue.push(t);
            }
        } else {
            token_queue.push(Tokens::LEndBlock(indent));
        }
        Ok(token_queue)
    }

    /// Check that a token type is among the allowed token types.
    #[cfg_attr(feature = "python", pyo3(name = "check_token"))]
    #[cfg_attr(feature = "python", pyo3(signature = (token, check_tokens)))]
    #[cfg(feature = "python")]
    pub fn check_token_py(
        &self,
        token: Tokens,
        check_tokens: Vec<Tokens>,
    ) -> PyResult<()> {
        let check_tokens: Vec<_> = check_tokens.iter().map(Tokens::kind).collect();
        self.check_token(&token, &check_tokens).map_err(|err|
            PyErr::new::<LexerError, _>((
                err.msg,
                err.line,
                err.filename,
                err.linenum,
            ))
        )
    }

    /// Get the next token from one or more tokenized lines.
    #[cfg_attr(feature = "python", pyo3(name = "get_next_token", signature = (check_tokens=None, no_white=None)))]
    #[cfg(feature = "python")]
    pub fn get_next_token_py(
        &mut self,
        check_tokens: Option<Vec<Tokens>>,
        no_white: Option<bool>,
    ) -> Result<Tokens> {
        let check_tokens = check_tokens.map(|tokens| tokens.iter().map(Tokens::kind).collect::<Vec<_>>());
        self.get_next_token(check_tokens.as_deref(), no_white)
    }

    /// Get all tokens until disallowed tokens or end tokens are found.
    #[cfg_attr(feature = "python", pyo3(name = "get_until", signature = (end_tokens, check_tokens=None, no_white=None)))]
    #[cfg(feature = "python")]
    pub fn get_until_py(
        &mut self,
        end_tokens: Vec<Tokens>,
        check_tokens: Option<Vec<Tokens>>,
        no_white: Option<bool>,
    ) -> Result<Vec<Tokens>> {
        let end_tokens: Vec<_> = end_tokens.iter().map(Tokens::kind).collect();
        let check_tokens = check_tokens.map(|tokens| tokens.iter().map(Tokens::kind).collect::<Vec<_>>());
        self.get_until(&end_tokens, check_tokens.as_deref(), no_white)
    }

    /// Skip all tokens until end tokens are found.
    #[cfg(feature = "python")]
    pub fn flush_until(&mut self, end_tokens: Vec<Tokens>) -> PyResult<()> {
        let _ = self.get_until_py(end_tokens, None, None)?;
        Ok(())
    }

    /// Get all tokens from the rest of the line terminating only at an end-of-line token.
    #[cfg_attr(feature = "python", pyo3(signature = (no_white=None)))]
    pub fn get_rest_line(&mut self, no_white: Option<bool>) -> Result<Vec<Tokens>> {
        self.get_until(&[], None, no_white)
    }

    /// Get a string token from the rest of the line.
    pub fn get_rest_line_as_string_token(&mut self) -> Result<Tokens> {
        self.rest_as_string = true;
        let lstring: Tokens = {
            let remainder_str = self.get_next_token(Some(&[TokenKind::LString]), None)?;
            let _ = self.get_next_token(Some(&[TokenKind::LEndL]), None)?;
            remainder_str
        };
        Ok(lstring)
    }

    /// Make the next line to get return the given line instead of the real next line.
    pub fn set_next_line(&mut self, line: String, indent: usize, linenum: usize) {
        self.reader.set_next_line(line, indent, linenum);
    }

}
impl Lexer {
    /// Get the next token from one or more tokenized lines.
    pub fn get_next_token(
        &mut self,
        check_tokens: Option<&[TokenKind]>,
        no_white: Option<bool>,
    ) -> Result<Tokens> {
        let no_white = no_white.unwrap_or(false);
        if self.token_queue.is_empty() {
            let tokens = self.match_multiline()?;
            self.token_queue.extend(tokens);
        }
        match self.token_queue.pop_front() {
            Some(token) => {
                if no_white && matches!(token, Tokens::LWhite(_)) {
                    return self.get_next_token(check_tokens, Some(no_white));
                }
                if let Some(check_tokens_some) = check_tokens {
                    self.check_token(&token, check_tokens_some)?;
                }
                Ok(token)
            },
            None => Err(Error::Lexer(LexerError::new(
                format!(
                    "Lexer returned no token at position {}",
                    self.pos,
                ),
                self.line.clone(),
                Some(self.filename.clone()),
                self.linenum,
            )))
        }
    }

    /// Get all tokens until not allowed tokens or end tokens are found.
    pub fn get_until(
        &mut self,
        end_tokens: &[TokenKind],
        check_tokens: Option<&[TokenKind]>,
        no_white: Option<bool>,
    ) -> Result<Vec<Tokens>> {
        let check_tokens = check_tokens.unwrap_or_default();
        let no_white = no_white.unwrap_or(false);
        let end_tokens = if end_tokens.is_empty() {
            &[TokenKind::LEndL]
        } else {
            end_tokens
        };

        let mut tokens = Vec::new();
        while let Ok(next_token) = self.get_next_token(None, None) {
            let kind = next_token.kind();
            let is_end = end_tokens.contains(&kind);
            if !check_tokens.is_empty() && !check_tokens.contains(&kind) && !is_end {
                return Err(Error::Lexer(LexerError::new(
                    format!(
                        "Unexpected token '{:?}' not among expected ones {:?}",
                        next_token, check_tokens.iter().chain(end_tokens).collect::<Vec<_>>(),
                    ),
                    self.line.clone(),
                    Some(self.filename.clone()),
                    self.linenum,
                )));
            }
            if no_white && matches!(next_token, Tokens::LWhite(_)) {
                continue;
            }
            tokens.push(next_token);
            if is_end {
                break;
            }
        }
        Ok(tokens)
    }

    /// Get the next token from one or more tokenized lines.
    pub fn check_token(
        &self,
        token: &Tokens,
        check_tokens: &[TokenKind],
    ) -> std::result::Result<(), LexerError> {
        if !check_tokens.is_empty() && !check_tokens.contains(&token.kind()) {
            return Err(LexerError {
                msg: format!(
                    "Unexpected token '{:?}' not among expected ones {:?}",
                    token, check_tokens,
                ),
                line: self.line.clone(),
                filename: Some(self.filename.clone()),
                linenum: self.linenum,
            });
        }
        Ok(())
    }
}
