use std::io::{self};
use std::sync::LazyLock;
use regex::Regex;

use pyo3::{prelude::*};
use pyo3::exceptions::PyException;

use crate::tokens::Tokens;

#[pyclass]
pub struct Reader {
    #[pyo3(get)]
    pub filename: String,
    #[pyo3(get)]
    lines: Vec<(String, usize, usize)>,
    line_index: usize,
    stored_line: Option<(String, usize, usize)>,
}

#[pymethods]
impl Reader {
    #[new]
    #[pyo3(signature = (content=None, filename=None))]
    pub fn new(content: Option<&str>, filename: Option<&str>) -> io::Result<Self> {
        if filename.is_some() && content.is_some() {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "Only one of filename or content can be provided"));
        }
        let content = if let Some(content) = content {
            Ok(content.to_string())
        } else if let Some(filename) = filename {
                Ok(std::fs::read_to_string(filename)?)
        }
        else {
            Err(io::Error::new(io::ErrorKind::InvalidInput, "Either filename or content must be provided"))
        }?;
        let filename = if let Some(filename) = filename {
            filename.to_string()
        } else {
            "<string>".to_string()
        };

        let mut lines = Vec::new();
        for (linenum, line) in content.lines().enumerate() {
            let line = line.trim_end().replace('\t', "    ");
            let stripped_line = line.trim_start();
            let indent = line.len() - stripped_line.len();
            if stripped_line.is_empty() || stripped_line.starts_with('#') || stripped_line.starts_with("//") {
                continue;
            }
            lines.push((stripped_line.to_string(), indent, linenum + 1));
        }

        Ok(Reader {
            filename,
            lines,
            line_index: 0,
            stored_line: None,
        })
    }

    pub fn get_next_line(&mut self, prev_indent: isize) -> (Option<String>, isize, isize) {
        if let Some((line, indent, linenum)) = self.stored_line.take() {
            return (Some(line), indent as isize, linenum as isize);
        }
        if self.line_index >= self.lines.len() {
            return (None, -1, -1);
        }
        let (line, indent, linenum) = &self.lines[self.line_index];
        if *indent as isize <= prev_indent {
            return (None, *indent as isize, *linenum as isize);
        }
        self.line_index += 1;
        (Some(line.clone()), *indent as isize, *linenum as isize)
    }

    pub fn set_next_line(&mut self, line: &str, indent: usize, linenum: usize) {
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
    fn new(msg: String, line: Option<String>, filename: Option<String>, linenum: Option<isize>) -> Self {
        Self { msg, line, filename, linenum }
    }

    fn __str__(&self) -> String {
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
            pos : 0,
            char_buffer : String::new(),
            oper_buffer : String::new(),
            kind : LineKind::Unknown,
        })
    }

    pub fn restart(&mut self) {
        self.kind = LineKind::Unknown;
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
                    if line[..self.pos].contains(" ") {
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

    pub fn get_next_line(&mut self, prev_indent: isize) -> (Option<String>, isize, isize) {
        self.reader.get_next_line(prev_indent)
    }

    pub fn set_next_line(&mut self, line: &str, indent: usize, linenum: usize) {
        self.reader.set_next_line(line, indent, linenum);
    }

}
