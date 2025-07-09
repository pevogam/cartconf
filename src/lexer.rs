use std::io::{self};

use pyo3::{prelude::*};

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

#[pyclass]
pub struct Lexer {
    // TODO: use the string or file reader inside a lexer
    //pub reader: Reader,
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
}

#[pymethods]
impl Lexer {
    #[new]
    pub fn new() -> Self {
        //let filename = reader.filename.to_string();
        Lexer {
            //reader,
            filename: "<string>".to_string(),
            line: None,
            linenum: 0,
            ignore_white: false,
            rest_as_string: false,
            prev_indent: -1,
        }
    }

    pub fn set_prev_indent(&mut self, prev_indent: isize) {
        self.prev_indent = prev_indent;
    }
}

impl Default for Lexer {
    fn default() -> Self {
        Self::new()
    }
}
