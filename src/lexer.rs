use std::borrow::Cow;
use std::collections::VecDeque;
use std::fs;
use std::io::{self};
use std::mem::discriminant;
use std::sync::LazyLock;
use regex::Regex;

use pyo3::prelude::*;
use pyo3::exceptions::PyException;

use crate::tokens::Tokens;

#[pyclass]
pub struct Reader {
    #[pyo3(get)]
    pub filename: String,
    #[pyo3(get)]
    lines: Vec<(String, usize, usize)>,
    stored_line: Option<(String, usize, usize)>,
}

#[pymethods]
impl Reader {
    #[new]
    #[pyo3(signature = (content=None, filename=None))]
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

    pub fn get_next_line(&mut self, prev_indent: isize) -> (Option<String>, isize, isize) {
        if let Some((line, indent, linenum)) = self.stored_line.take() {
            return (Some(line), indent as isize, linenum as isize);
        }
        if self.lines.is_empty() {
            return (None, -1, -1);
        }
        // TODO: converting usize to isize can also overflow, unify all indents and linenums
        // to usize option to map -1 to None
        if self.lines[0].1 as isize <= prev_indent {
            return (None, self.lines[0].1 as isize, self.lines[0].2 as isize);
        }
        let (line, indent, linenum) = self.lines.remove(0);
        (Some(line), indent as isize, linenum as isize)
    }

    pub fn set_next_line(&mut self, line: String, indent: usize, linenum: usize) {
        let line = line.trim();
        if !line.is_empty() {
            self.stored_line = Some((line.to_string(), indent, linenum));
        }
    }
}

#[pyclass(extends=PyException)]
#[derive(Debug)]
pub struct LexerError {
    msg: String,
    line: Option<String>,
    filename : Option<String>,
    linenum : Option<isize>,
}
#[pymethods]
impl LexerError {
    #[new]
    #[pyo3(signature = (msg, line=None, filename=None, linenum=None))]
    pub fn new(msg: String, line: Option<String>, filename: Option<String>, linenum: Option<isize>) -> Self {
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

#[pyclass]
pub struct Lexer {
    pub reader: Reader,
    #[pyo3(get)]
    pub filename: String,
    #[pyo3(get, set)]
    pub line: Option<String>,
    #[pyo3(get, set)]
    pub linenum: isize,
    #[pyo3(get)]
    pub ignore_white: bool,
    #[pyo3(get, set)]
    pub rest_as_string: bool,
    #[pyo3(get)]
    pub prev_indent: isize,
    pub token_queue: VecDeque<Tokens>,
    // state machine parameters for the iterator
    #[pyo3(get, set)]
    pos : usize,
    char_buffer : String,
    oper_buffer : String,
    kind : LineKind,
}

#[pymethods]
impl Lexer {
    #[new]
    #[pyo3(signature = (content=None, filename=None))]
    pub fn new(content: Option<&str>, filename: Option<&str>) -> io::Result<Self> {
        let reader = Reader::new(content, filename)?;
        let filename = reader.filename.clone();
        Ok(Lexer {
            reader,
            filename,
            line: None,
            linenum: 0,
            ignore_white: false,
            rest_as_string: false,
            prev_indent: -1,
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

    pub fn set_prev_indent(&mut self, prev_indent: isize) {
        self.prev_indent = prev_indent;
    }

    /// Tokenize a single line, starting at a desired position.
    pub fn match_line(&mut self, line: &str, pos: usize) -> PyResult<Vec<Tokens>> {
        let mut tokens = Vec::new();
        let chars: Vec<char> = line.chars().collect();
        let len = chars.len();

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
                        line[..self.pos].contains(" ") ||
                        (self.pos + 1 < len && matches!(chars[self.pos + 1], '=' | '+' | '<'))
                    } else {
                        line[..self.pos].contains(" ") ||
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
                            return Err(PyErr::new::<LexerError, _>((
                                format!(
                                    "Unexpected operator '{}' at position {}",
                                    self.oper_buffer, self.pos,
                                ),
                                Some(line.to_string()),
                                Some(self.filename.clone()),
                                Some(self.linenum),
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
                    self.pos += 1; // skip closing quote
                    tokens.push(Tokens::LString(s));
                    return Ok(tokens);
                }
                '#' => {
                    break;
                }
                _ => {
                    return Err(PyErr::new::<LexerError, _>((
                        format!(
                            "Unexpected character '{}' at position {}",
                            c, self.pos,
                        ),
                        Some(line.to_string()),
                        Some(self.filename.clone()),
                        Some(self.linenum),
                    )));
                }
            }
            if self.rest_as_string {
                self.rest_as_string = false;
                tokens.push(Tokens::LString(line[pos..].trim_start().to_string()));
                self.pos = len;
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
    pub fn match_multiline(&mut self) -> PyResult<Vec<Tokens>> {
        let mut token_queue = Vec::new();
        let (line_opt, indent, linenum) = self.reader.get_next_line(self.prev_indent);
        self.line = line_opt.clone();
        self.linenum = linenum;
        if let Some(line) = line_opt {
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
                        if indent < 0 {
                            return Err(PyErr::new::<LexerError, _>((
                                format!(
                                    "Cannot store negative indent '{}' at position {}",
                                    indent, self.pos,
                                ),
                                Some(line),
                                Some(self.filename.clone()),
                                Some(self.linenum),
                            )));
                        }
                        if linenum < 0 {
                            return Err(PyErr::new::<LexerError, _>((
                                format!(
                                    "Cannot store negative line number '{}' at position {}",
                                    linenum, self.pos,
                                ),
                                Some(line),
                                Some(self.filename.clone()),
                                Some(self.linenum),
                            )));
                        }
                        // Keep the current line as the next line to comply with the line state machine.
                        self.reader.set_next_line(line, indent as usize, linenum as usize);
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
    #[pyo3(name = "check_token")]
    #[pyo3(signature = (token, check_tokens))]
    pub fn check_token_py(
        &self,
        token: Tokens,
        check_tokens: Vec<Tokens>,
    ) -> PyResult<()> {
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
    #[pyo3(signature = (check_tokens=None, no_white=None))]
    pub fn get_next_token(
        &mut self,
        check_tokens: Option<Vec<Tokens>>,
        no_white: Option<bool>,
    ) -> PyResult<Tokens> {
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
                    self.check_token(&token, &check_tokens_some).map_err(|err|
                        PyErr::new::<LexerError, _>((
                            err.msg,
                            err.line,
                            err.filename,
                            err.linenum,
                    )))?;
                }
                Ok(token)
            },
            None => Err(PyErr::new::<LexerError, _>((
                format!(
                    "Lexer returned no token at position {}",
                    self.pos,
                ),
                self.line.clone(),
                Some(self.filename.clone()),
                Some(self.linenum),
            )))
        }
    }

    /// Get all tokens until not allowed tokens or end tokens are found.
    #[pyo3(signature = (end_tokens, check_tokens=None, no_white=None))]
    pub fn get_until(
        &mut self,
        mut end_tokens: Vec<Tokens>,
        check_tokens: Option<Vec<Tokens>>,
        no_white: Option<bool>,
    ) -> PyResult<Vec<Tokens>> {
        let mut check_tokens = check_tokens.unwrap_or_default();
        let no_white = no_white.unwrap_or(false);
        if end_tokens.is_empty() {
            end_tokens.push(Tokens::LEndL());
        }
        if !check_tokens.is_empty() {
            check_tokens.extend(end_tokens.iter().cloned());
        }

        let mut tokens = Vec::new();
        while let Ok(next_token) = self.get_next_token(None, None) {
            if !check_tokens.is_empty() && !check_tokens.iter()
                    .any(|t| discriminant(t) == discriminant(&next_token)) {
                return Err(PyErr::new::<LexerError, _>((
                    format!(
                        "Unexpected token '{:?}' not among expected ones {:?}",
                        next_token, check_tokens.iter(),
                    ),
                    self.line.clone(),
                    Some(self.filename.clone()),
                    Some(self.linenum),
                )));
            }
            if no_white && matches!(next_token, Tokens::LWhite(_)) {
                continue;
            }
            tokens.push(next_token.clone());
            if end_tokens.iter()
                    .any(|t| discriminant(t) == discriminant(&next_token)) {
                break;
            }
        }
        Ok(tokens)
    }

    /// Skip all tokens until end tokens are found.
    pub fn flush_until(&mut self, end_tokens: Vec<Tokens>) -> PyResult<()> {
        let _ = self.get_until(end_tokens, None, None)?;
        Ok(())
    }

    /// Get all tokens from the rest of the line terminating only at an end-of-line token.
    #[pyo3(signature = (no_white=None))]
    pub fn get_rest_line(&mut self, no_white: Option<bool>) -> PyResult<Vec<Tokens>> {
        self.get_until(Vec::new(), None, no_white)
    }

    /// Get a string token from the rest of the line.
    pub fn get_rest_line_as_string_token(&mut self) -> PyResult<Tokens> {
        self.rest_as_string = true;
        let lstring: Tokens = {
            let remainder_str = self.get_next_token(Some(vec![Tokens::default("String")]), None)?;
            let _ = self.get_next_token(Some(vec![Tokens::default("endl")]), None)?;
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
    pub fn check_token(
        &self,
        token: &Tokens,
        check_tokens: &[Tokens],
    ) -> Result<(), LexerError> {
        if !check_tokens.is_empty() && !check_tokens.iter()
                .any(|t| discriminant(t) == discriminant(token)) {
            return Err(LexerError {
                msg: format!(
                    "Unexpected token '{:?}' not among expected ones {:?}",
                    token, check_tokens,
                ),
                line: self.line.clone(),
                filename: Some(self.filename.clone()),
                linenum: Some(self.linenum),
            });
        }
        Ok(())
    }
}
