use std::collections::{HashMap, VecDeque};
use std::hash::Hash;
use std::fmt::{Debug, Display};
use std::mem;
use std::rc::Rc;
use std::cell::RefCell;

use pyo3::prelude::*;
use pyo3::exceptions::PyException;
use pyo3::types::{PyAny};

use crate::tokens::{ParamKey, ParamVal};
use crate::tokens::Tokens;
use crate::filters::Filters;
use crate::lexer::Lexer;
use crate::lexer::LexerError;

#[pyclass(extends=PyException)]
#[derive(Debug)]
pub struct ParserError {
    lexer_error: LexerError,
}
#[pymethods]
impl ParserError {
    #[new]
    #[pyo3(signature = (msg, line=None, filename=None, linenum=None))]
    fn new(msg: String, line: Option<String>, filename: Option<String>, linenum: Option<isize>) -> Self {
        Self { lexer_error: LexerError::new(msg, line, filename, linenum) }
    }

    fn __str__(&self) -> String {
        self.lexer_error.__str__()
    }
}

#[pyclass]
#[derive(Clone, Eq)]
pub struct Label {
    #[pyo3(get, set)]
    pub name: String,
    #[pyo3(get, set)]
    pub var_name: Option<String>,
    #[pyo3(get, set)]
    pub long_name: String,
    #[pyo3(get, set)]
    pub hash_val: i64,
    #[pyo3(get, set)]
    pub hash_var: Option<i64>,
}

impl Debug for Label {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.long_name)
    }
}
impl Display for Label {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.long_name)
    }
}

impl PartialEq for Label {
    fn eq(&self, other: &Self) -> bool {
        if other.var_name.is_some() {
            self.long_name == other.long_name
        } else {
            self.name == other.name
        }
    }
}
impl Hash for Label {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        if let Some(hash_var) = self.hash_var {
            hash_var.hash(state);
        } else {
            self.hash_val.hash(state);
        }
    }
}

#[pymethods]
impl Label {
    #[new]
    #[pyo3(signature = (name, next_name=None))]
    pub fn new(name: String, next_name: Option<String>) -> Self {
        let actual_name = next_name.clone().unwrap_or_else(|| name.clone());
        let var_name = if next_name.is_some() { Some(name.clone()) } else { None };
        let long_name = if let Some(name) = var_name.clone() {
            format!("({}={})", name, actual_name)
        } else {
            actual_name.clone()
        };
        let hash_val = Label::hash_internal(&actual_name);
        let hash_var = var_name.as_ref().map(|_| Label::hash_internal(&long_name));
        Label {
            name: actual_name,
            var_name,
            long_name,
            hash_val,
            hash_var,
        }
    }

    fn __str__(&self) -> PyResult<String> {
        Ok(self.long_name.clone())
    }

    fn __repr__(&self) -> PyResult<String> {
        Ok(self.long_name.clone())
    }

    fn __eq__(&self, other: &Bound<'_, PyAny>) -> PyResult<bool> {
        if let Ok(other_label) = other.extract::<Label>() {
            Ok(self.eq(&other_label))
        } else {
            Ok(false)
        }
    }

    fn __ne__(&self, other: &Bound<'_, PyAny>) -> PyResult<bool> {
        if let Ok(other_label) = other.extract::<Label>() {
            if other_label.var_name.is_some() {
                Ok(self.long_name != other_label.long_name)
            } else {
                Ok(self.name != other_label.name)
            }
        } else {
            Ok(true)
        }
    }

    fn __hash__(&self) -> PyResult<i64> {
        Ok(self.hash_val)
    }

    #[staticmethod]
    fn hash_internal(name: &str) -> i64 {
        name.chars()
            .enumerate()
            .map(|(i, x)| ((i as i64) + 1) * (x as i64))
            .sum()
    }
}

#[derive(Debug, PartialEq, Clone)]
pub enum ContentType {
    Tokens(Tokens),
    Filters(Filters),
    Node(Node),
    String(String),
}
impl<'py> IntoPyObject<'py> for ContentType {
    type Target = PyAny;
    type Output = Bound<'py, Self::Target>;
    type Error = PyErr;

    fn into_pyobject(self, py: Python<'py>) -> Result<Self::Output, Self::Error> {
        use pyo3::IntoPyObjectExt;

        match self {
            ContentType::Tokens(s) => s.into_bound_py_any(py),
            ContentType::Filters(s) => s.into_bound_py_any(py),
            ContentType::Node(s) => s.into_bound_py_any(py),
            ContentType::String(s) => s.into_bound_py_any(py),
        }
    }
}
impl<'py> FromPyObject<'_, 'py> for ContentType {
    type Error = PyErr;

    fn extract(object: Borrowed<'_, 'py, PyAny>) -> Result<Self, Self::Error> {
        if let Ok(tokens) = object.extract::<Tokens>() {
            Ok(ContentType::Tokens(tokens))
        }
        else if let Ok(filters) = object.extract::<Filters>() {
            Ok(ContentType::Filters(filters))
        }
        else if let Ok(node) = object.extract::<Node>() {
            Ok(ContentType::Node(node))
        }
        else {
            let s: String = object.extract()?;
            Ok(ContentType::String(s))
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct ContentStep {
    filename: String,
    linenum: isize,
    content_type: ContentType,
}
impl<'py> IntoPyObject<'py> for ContentStep {
    type Target = PyAny;
    type Output = Bound<'py, Self::Target>;
    type Error = PyErr;

    fn into_pyobject(self, py: Python<'py>) -> Result<Self::Output, Self::Error> {
        use pyo3::IntoPyObjectExt;

        (self.filename, self.linenum, self.content_type).into_bound_py_any(py)
    }
}
impl<'py> FromPyObject<'_, 'py> for ContentStep {
    type Error = PyErr;

    fn extract(object: Borrowed<'_, 'py, PyAny>) -> Result<Self, Self::Error> {
        if let Ok((filename, linenum, content_type)) = object.extract::<(String, isize, ContentType)>() {
            return Ok(ContentStep { filename, linenum, content_type });
        }
        Err(PyErr::new::<pyo3::exceptions::PyTypeError, _>(
            "Failed to extract ContentStep from python object",
        ))
    }
}

#[pyclass(unsendable)]
#[derive(Debug, PartialEq, Clone)]
pub struct Node {
    #[pyo3(get, set)]
    pub var_name: Vec<Label>,
    #[pyo3(get, set)]
    pub name: Vec<Label>,
    #[pyo3(get, set)]
    pub filename: String,
    #[pyo3(get, set)]
    pub dep: Vec<Vec<Vec<Label>>>,
    #[pyo3(get, set)]
    pub condition: Option<Filters>,
    pub content: Vec<ContentStep>,
    children: VecDeque<Rc<RefCell<Node>>>,
    #[pyo3(get)]
    pub labels: Vec<Label>,
    #[pyo3(get, set)]
    pub append_to_shortname: bool,
    #[allow(clippy::type_complexity)]
    pub failed_cases: VecDeque<(Vec<Label>, Vec<ContentStep>, Vec<ContentStep>)>,
    #[pyo3(get, set)]
    pub default: bool,
}

impl Default for Node {
    fn default() -> Self {
        Self::new()
    }
}

#[pymethods]
impl Node {
    #[new]
    pub fn new() -> Self {
        Node {
            var_name: Vec::new(),
            name: Vec::new(),
            filename: String::new(),
            dep: Vec::new(),
            condition: None,
            content: Vec::new(),
            children: VecDeque::new(),
            labels: Vec::new(),
            append_to_shortname: false,
            failed_cases: VecDeque::new(),
            default: false,
        }
    }

    pub fn __eq__(&self, other: &Bound<'_, PyAny>) -> PyResult<bool> {
        if let Ok(other_node) = other.extract::<Node>() {
            Ok(self == &other_node)
        } else {
            Ok(false)
        }
    }

    pub fn update_labels(&mut self, new_labels: Vec<Label>) -> PyResult<()> {
        for label in &new_labels {
            if !self.labels.contains(label) {
                self.labels.push(label.clone());
            }
        }
        Ok(())
    }

    pub fn get_children(&self) -> PyResult<Vec<Node>> {
        Ok(self.children.iter().map(|child| child.borrow().clone()).collect())
    }

    pub fn prepend_child(&mut self, node: Node) -> PyResult<()> {
        self.children.push_front(Rc::new(RefCell::new(node)));
        Ok(())
    }

    pub fn append_child(&mut self, node: Node) -> PyResult<()> {
        self.children.push_back(Rc::new(RefCell::new(node)));
        Ok(())
    }

    pub fn get_content(&self) -> PyResult<Vec<ContentStep>> {
        Ok(self.content.clone())
    }

    pub fn add_content(&mut self, filename: String, linenum: isize, content_type: ContentType) -> PyResult<()> {
        self.content.push(ContentStep { filename, linenum, content_type });
        Ok(())
    }

    pub fn swap_content(&mut self, new_content: Vec<ContentStep>) -> PyResult<()> {
        self.content = new_content;
        Ok(())
    }

    #[allow(clippy::type_complexity)]
    pub fn get_failed_cases(&self) -> PyResult<Vec<(Vec<Label>, Vec<ContentStep>, Vec<ContentStep>)>> {
        Ok(self.failed_cases.clone().into())
    }

    pub fn add_failed_case(
        &mut self,
        ctx: Vec<Label>,
        external_filters: Vec<ContentStep>,
        internal_filters: Vec<ContentStep>,
        capacity: usize,
    ) -> PyResult<()> {
        self.failed_cases.push_front((ctx, external_filters, internal_filters));
        if self.failed_cases.len() > capacity {
            _ = self.failed_cases.pop_back()
        }
        Ok(())
    }

    pub fn update_failed_case(
        &mut self,
        idx: usize,
        ctx: Vec<Label>,
        external_filters: Vec<ContentStep>,
        internal_filters: Vec<ContentStep>,
    ) -> PyResult<()> {
        self.failed_cases.push_front((ctx, external_filters, internal_filters));
        _ = self.failed_cases.remove(idx);
        Ok(())
    }

    #[pyo3(signature = (indent, recurse=false))]
    pub fn dump(&self, indent: usize, recurse: bool) -> PyResult<String> {
        let mut dump_lines = vec![
            format!("{:indent$}name: {:?}", "", self.name, indent = indent),
            format!("{:indent$}variable name: {:?}", "", self.var_name, indent = indent),
            format!("{:indent$}content: {:?}", "", self.content, indent = indent),
            format!("{:indent$}failed cases: {:?}", "", self.failed_cases, indent = indent),
        ];
        if recurse {
            for child in &self.children {
                dump_lines.push(child.borrow().dump(indent + 3, recurse)?);
            }
        }
        Ok(dump_lines.join("\n"))
    }
}
impl Node {
    pub fn apply_predict(
        &mut self,
        lexer: &Lexer,
        pre_dict: &mut HashMap<ParamKey, ParamVal>
    ) -> PyResult<()> {
        // Take ownership of the entire HashMap, leaving an empty cleared one behind
        let map = mem::take(pre_dict);

        // Build a LApplyDict from the Rust HashMap
        let content_type = ContentType::Tokens(Tokens::LApplyPreDict(String::new(), map));

        // Add pre-dictionary content to this node
        self.add_content(
            lexer.filename.clone(),
            lexer.linenum,
            content_type,
        )?;

        Ok(())
    }

    /*
    Parse:
        identifier = xxx
        identifier <= xxx
        identifier ?= xxx
        etc..
    */
    pub fn apply_operator(
        &mut self,
        identifier: Vec<Tokens>,
        token: Tokens,
        lexer: &mut Lexer,
        pre_dict: &mut HashMap<ParamKey, ParamVal>,
    ) -> PyResult<()> {
        // Build identifier_str
        let token_str = match token {
            Tokens::LIdentifier(s) => s.clone(),
            _ => return Err(pyo3::exceptions::PyValueError::new_err(
                format!("Expected LIdentifier token but got {}", token)
            )),
        };
        let identifier_str = if identifier.len() == 1 {
            token_str
        } else {
            // [token] + identifier[:-1]
            let mut parts: Vec<String> = Vec::with_capacity(identifier.len());
            // token first
            parts.push(token_str);
            // then all except last
            for t in &identifier[..identifier.len() - 1] {
                parts.push(match t {
                    Tokens::LIdentifier(s) => s.clone(),
                    _ => return Err(pyo3::exceptions::PyValueError::new_err(
                        format!("Expected LIdentifier token but got {}", t)
                    )),
                });
            }
            parts.join("")
        };

        // Get the next token for the value (LString)
        let lstring = Tokens::default("String");
        let lendl = Tokens::default("endl");
        let value = lexer.get_next_token(Some(vec![lstring]), None)?;
        let mut value_str: String = value.string()?;
        // strip surrounding quotes if present
        let first = value_str.chars().next().unwrap_or(' ');
        let last = value_str.chars().last().unwrap_or(' ');
        if (first == '"' && last == '"') || (first == '\'' && last == '\'') {
            value_str = value_str[1..value_str.len() - 1].to_string();
        }

        // Construct operator instance by likeness of the provided one
        let op: &Tokens = match identifier.last() {
            Some(last_token) => last_token,
            None => {
                return Err(pyo3::exceptions::PyValueError::new_err("Empty identifier"));
            }
        };
        let op_obj = op.like(identifier_str.clone(), value_str.clone())?;

        // If it's an LSet and value has no '$', apply directly to pre_dict
        let d_nin_val = !value_str.contains('$');
        if matches!(op, Tokens::LSet(_,_)) && d_nin_val {
            op_obj.apply_to_predict(pre_dict)?;
        } else {
            // If pre_dict has pending entries, either optimize or flush
            let pre_nonempty = !pre_dict.is_empty();
            if pre_nonempty {
                // try to get op.name and check if it's present in pre_dict
                let op_name = op_obj.name().unwrap_or_default();
                if !op_name.is_empty() && d_nin_val && pre_dict.contains_key(&op_name.into()) {
                    // apply and consume EOL
                    op_obj.apply_to_predict(pre_dict)?;
                    lexer.get_next_token(Some(vec![lendl]), None)?;
                    return Ok(());
                } else {
                    // flush pre_dict into node
                    self.apply_predict(lexer, pre_dict)?;
                }
            }

            // Add operator token as content
            self.add_content(
                lexer.filename.clone(),
                lexer.linenum,
                ContentType::Tokens(op_obj),
            )?;
        }

        // consume end-of-line
        lexer.get_next_token(Some(vec![lendl]), None)?;
        Ok(())
    }

    /*
    Parse:
        del operand
    */
    pub fn apply_deletion(
        &mut self,
        lexer: &mut Lexer,
        pre_dict: &mut HashMap<ParamKey, ParamVal>,
    ) -> PyResult<()> {
        let lidentifier = Tokens::default("Identifier");
        let lendl = Tokens::default("endl");
        let to_del = lexer.get_next_token(Some(vec![lidentifier]), Some(true))?;
        // consume EOL
        lexer.get_next_token(Some(vec![lendl]), Some(true))?;
        let to_del_str: String = to_del.string()?;

        // flush pre_dict and add token as content
        self.apply_predict(lexer, pre_dict)?;
        self.add_content(
            lexer.filename.clone(),
            lexer.linenum,
            ContentType::Tokens(Tokens::LDel(to_del_str, "".to_string())),
        )?;

        Ok(())
    }

    /*
    Parse:
        include relative file path to working directory.
    */
    pub fn apply_include(
        &mut self,
        lexer: &mut Lexer,
        pre_dict: &mut HashMap<ParamKey, ParamVal>,
    ) -> PyResult<Node> {
        // Get path from rest of line
        let path = lexer.get_rest_line_as_string_token()?;
        let path_str: String = path.string()?;

        // Expand user path (~ -> $HOME)
        let expanded_path = if path_str.starts_with('~') {
            let home = std::env::var("HOME").unwrap_or_default();
            path_str.replacen('~', &home, 1)
        } else {
            path_str
        };

        // Make path absolute if needed
        let mut filepath = std::path::PathBuf::from(&expanded_path);
        let current_file: String = lexer.filename.clone();
        if current_file != "<string>" && !filepath.is_absolute()
                && let Some(parent) = std::path::Path::new(&current_file).parent() {
            filepath = parent.join(filepath);
        }

        // Check file exists
        if !filepath.is_file() {
            return Err(PyErr::new::<ParserError, _>((
                "file does not exist or it's not a regular file".to_string(),
                lexer.line.clone(),
                Some(lexer.filename.clone()),
                Some(lexer.linenum),
            )));
        }

        // Apply current pre_dict and create new lexer for included file
        self.apply_predict(lexer, pre_dict)?;

        let filepath_str = filepath.to_str()
            .ok_or_else(|| PyErr::new::<pyo3::exceptions::PyValueError, _>("Invalid filepath"))?;
        let mut new_lexer = Lexer::new(None, Some(filepath_str))?;

        // Parse with new lexer
        parse(&mut new_lexer, self.clone(), -1, false, None)
    }

    /*
    Parse:
        xxx.yyy.(aaa=bbb):
    */
    pub fn apply_condition(
        &mut self,
        identifier: Vec<Tokens>,
        token: Tokens,
        lexer: &mut Lexer,
        pre_dict: &mut HashMap<ParamKey, ParamVal>,
        indent: isize,
    ) -> PyResult<()> {
        // Build the full identifier list: [token] + identifier[:-1] + [LEndl]
        let mut tokens = vec![token];
        let identifier_len = identifier.len();
        tokens.extend(identifier.into_iter().take(identifier_len.saturating_sub(1)));
        tokens.push(Tokens::default("endl"));

        // Parse the condition filter
        let cfilter: Vec<Vec<Vec<Label>>> = Filters::parse_filter(
            tokens,
            lexer.line.clone().unwrap_or("<none>".to_string()),
            lexer.filename.clone(),
            lexer.linenum,
        )?;

        // Get the next line and set it in the lexer
        let next_line = lexer.get_rest_line_as_string_token()?;
        let next_line_str: String = next_line.string()?;
        if !next_line_str.is_empty() {
            lexer.set_next_line(&next_line_str, (indent + 1) as usize, lexer.linenum as usize);
        }

        // Create a new Node for the condition
        let mut cond = Node::new();
        cond.condition = Some(Filters::Condition { filter : cfilter, line : lexer.line.clone().unwrap_or_default() });

        // Parse the condition block
        cond = parse(lexer, cond, indent, false, None)?;

        // Apply the current pre_dict and add the condition node as content
        self.apply_predict(lexer, pre_dict)?;
        self.add_content(
            lexer.filename.clone(),
            lexer.linenum,
            ContentType::Node(cond),
        )?;

        Ok(())
    }

    /*
    Parse:
        !xxx.yyy.(aaa=bbb): vvv
    */
    pub fn apply_notcondition(
        &mut self,
        lexer: &mut Lexer,
        pre_dict: &mut HashMap<ParamKey, ParamVal>,
        indent: isize,
    ) -> PyResult<()> {
        // Build the full token list
        let lcolon = Tokens::default(":");
        let lendl = Tokens::default("endl");
        let tokens = lexer.get_until(
            vec![lcolon, lendl],
            None,
            Some(true),
        )?;
        let tokens_len = tokens.len();
        let tokens: Vec<_> = tokens
            .into_iter()
            .take(tokens_len.saturating_sub(1))
            .collect();

        // Parse the condition filter
        let lfilter: Vec<Vec<Vec<Label>>> = Filters::parse_filter(
            tokens,
            lexer.line.clone().unwrap_or("<none>".to_string()),
            lexer.filename.clone(),
            lexer.linenum,
        )?;

        // Get the next line and set it in the lexer
        let next_line = lexer.get_rest_line_as_string_token()?;
        let next_line_str: String = next_line.string()?;
        if !next_line_str.is_empty() {
            lexer.set_next_line(&next_line_str, (indent + 1)  as usize, lexer.linenum as usize);
        }

        // Create a new Node for the negative condition
        let mut cond = Node::new();
        cond.condition = Some(Filters::NegativeCondition {
            filter : lfilter,
            line : lexer.line.clone().unwrap_or_default()
        });

        // Parse the condition block
        cond = parse(lexer, cond, indent, false, None)?;

        // Apply the current pre_dict and add the condition node as content
        self.apply_predict(lexer, pre_dict)?;
        self.add_content(
            lexer.filename.clone(),
            lexer.linenum,
            ContentType::Node(cond),
        )?;

        Ok(())
    }

    /*
    Parse:
       variants _name_ [meta1] [meta2=val2]:
    */
    pub fn apply_variants(
        &self,
        lexer: &mut Lexer,
    ) -> PyResult<(String, HashMap<String, Vec<String>>)> {
        // Check if node has conditions
        if self.condition.is_some() {
            return Err(PyErr::new::<ParserError, _>((
                "'variants' is not allowed inside a conditional block".to_string(),
                lexer.line.clone(),
                Some(lexer.filename.clone()),
                Some(lexer.linenum),
            )));
        }

        // Get tokens until bracket, colon, identifier or end
        let allowed = [
            Tokens::default("["),
            Tokens::default(":"),
            Tokens::default("Identifier"),
            Tokens::default("endl"),
        ];
        let tokens = lexer.get_until(allowed.to_vec(), None, Some(true))?;
        let mut vtoken: Tokens = match tokens.last() {
            Some(last_token) => last_token.clone(),
            None => {
                return Err(pyo3::exceptions::PyValueError::new_err("Empty token list"));
            }
        };

        let mut variant_name = String::new();
        let mut meta = HashMap::new();

        // Parse tokens until colon or end
        while !matches!(vtoken, Tokens::LColon()) && !matches!(vtoken, Tokens::LEndL()) {
            if matches!(vtoken, Tokens::LIdentifier(_)) {
                if !variant_name.is_empty() {
                    return Err(PyErr::new::<ParserError, _>((
                        "Syntax ERROR expected '[' or ':'".to_string(),
                        lexer.line.clone(),
                        Some(lexer.filename.clone()),
                        Some(lexer.linenum),
                    )));
                }
                variant_name = tokens[0].string()?;
            } else if matches!(vtoken, Tokens::LLBracket()) {
                // Parse metadata in brackets
                let ident = lexer.get_next_token(
                    Some(vec![Tokens::default("Identifier")]),
                    Some(true),
                )?;
                let ident_str: String = ident.string()?;

                let next_token = lexer.get_next_token(
                    Some(vec![
                        Tokens::default("="),
                        Tokens::default("]"),
                    ]),
                    Some(true),
                )?;

                if matches!(next_token, Tokens::LRBracket()) {
                    // Handle [xxx]
                    meta.entry(ident_str)
                        .or_insert_with(Vec::new)
                        .push(true.to_string());
                } else if matches!(next_token, Tokens::LSet(_, _)) {
                    // Handle [xxx = yyy]
                    let tokens = lexer.get_until(
                        vec![
                            Tokens::default("]"),
                            Tokens::default("endl")
                        ],
                        None,
                        Some(true),
                    )?;
                    let last_token: &Tokens = match tokens.last() {
                        Some(last_token) => last_token,
                        None => {
                            return Err(pyo3::exceptions::PyValueError::new_err("Empty variants"));
                        }
                    };

                    if matches!(last_token, Tokens::LRBracket()) {
                        let mut values = Vec::new();
                        for token in tokens.iter().take(tokens.len() - 1) {
                            values.push(token.string()?);
                        }
                        // The meta has an inner list that we stringify here (just like the bool above)
                        meta.entry(ident_str)
                            .or_insert_with(Vec::new)
                            .push(values.join(" ").to_string());
                    } else {
                        return Err(PyErr::new::<ParserError, _>((
                            "Syntax ERROR expected ']'".to_string(),
                            lexer.line.clone(),
                            Some(lexer.filename.clone()),
                            Some(lexer.linenum),
                        )));
                    }
                }
            }

            // Get next token
            let next_token = lexer.get_next_token(
                Some(allowed.to_vec()),
                Some(true),
            )?;
            vtoken = next_token;
        }

        // Verify default values if present
        if meta.contains_key("default") {
            for val in meta.get("default").unwrap_or(&Vec::new()) {
                if val == "true" {
                    return Err(PyErr::new::<ParserError, _>((
                        "Syntax ERROR expected [default=xxx]".to_string(),
                        lexer.line.clone(),
                        Some(lexer.filename.clone()),
                        Some(lexer.linenum),
                    )));
                }
            }
        }

        // Check for required colon
        if matches!(vtoken, Tokens::LEndL()) {
            return Err(PyErr::new::<ParserError, _>((
                "Syntax ERROR expected ':'".to_string(),
                lexer.line.clone(),
                Some(lexer.filename.clone()),
                Some(lexer.linenum),
            )));
        }

        // Consume end of line
        lexer.get_next_token(
            Some(vec![Tokens::default("endl")]),
            Some(true),
        )?;

        Ok((variant_name, meta))
    }

    /*
    Parse:
     - var1: depend1, depend2
         block1
     - var2:
         block2
    */
    #[allow(clippy::too_many_arguments)]
    pub fn apply_variant(
        &mut self,
        lexer: &mut Lexer,
        pre_dict: &mut HashMap<ParamKey, ParamVal>,
        indent: isize,
        variant_name: String,
        variant_indent: isize,
        meta: &mut HashMap<String, Vec<String>>,
        defaults : bool,
        expand_defaults : Vec<String>,
    ) -> PyResult<Node> {
        let mut already_default = false;
        let mut node4 = Node::new();

        if !pre_dict.is_empty() {
            self.apply_predict(lexer, pre_dict)?;
        }

        // Handle default variants
        let meta_default = meta.get_mut("default");
        let meta_default_values_empty = &mut Vec::new();
        let meta_default_values = meta_default.unwrap_or(meta_default_values_empty);
        let meta_in_expand_defaults = !expand_defaults.contains(&variant_name);

        // Data used for the entire loop
        let tokens = [
                Tokens::default("Identifier"),
                Tokens::default("@"),
                Tokens::default("indent"),
                Tokens::default("endb"),
        ];

        loop {
            lexer.set_prev_indent(variant_indent);

            // Get token from lexer and check for end of block
            let token = lexer.get_next_token(Some(tokens.to_vec()), Some(true))?;
            if matches!(token, Tokens::LEndBlock(_)) {
                break;
            }

            let mut is_default = false;
            let mut name;

            if matches!(token, Tokens::LIndent(_)) {
                // Handle indented variant
                lexer.get_next_token(
                    Some(vec![Tokens::default("-")]),
                    Some(true),
                )?;
                let token = lexer.get_next_token(
                    Some(vec![
                        Tokens::default("Identifier"),
                        Tokens::default("@"),
                    ]),
                    Some(true),
                )?;

                if matches!(token, Tokens::LDefault()) {
                    is_default = true;
                    name = lexer.get_until(
                        vec![Tokens::default(":")],
                        None,
                        None,
                    )?;
                } else {
                    name = vec![token];
                    name.extend(lexer.get_until(
                        vec![Tokens::default(":")],
                        None,
                        None,
                    )?);
                }
            } else if matches!(token, Tokens::LDefault()) {
                is_default = true;
                name = lexer.get_until(
                    vec![Tokens::default(":")],
                    None,
                    None,
                )?;
            } else {
                name = vec![token];
                name.extend(lexer.get_until(
                    vec![Tokens::default(":")],
                    None,
                    None,
                )?);
            }
            let name_len = name.len();
            // Drop the colon at the end of the parsed name
            name = name.into_iter().take(name_len.saturating_sub(1)).collect();

            // Get dependencies after colon
            let token = lexer.get_next_token(None, Some(true))?;
            let mut deps = Vec::new();
            if !matches!(token, Tokens::LEndL()) {
                let mut filter_tokens = vec![token];
                filter_tokens.extend(lexer.get_until(
                    vec![Tokens::default("endl")],
                    None,
                    None,
                )?);
                deps = Filters::parse_filter(
                    filter_tokens,
                    lexer.line.clone().unwrap_or("<none>".to_string()),
                    lexer.filename.clone(),
                    lexer.linenum,
                )?;
            }

            // Create and parse the variant node
            let mut node2 = Node::new();
            node2.append_child(self.clone())?;
            node2.update_labels(self.labels.clone())?;

            if !variant_name.is_empty() {
                node2.add_content(
                    lexer.filename.clone(),
                    lexer.linenum,
                    ContentType::Tokens(
                        Tokens::LSet(
                            variant_name.clone(),
                            name.iter()
                                .filter_map(|t| match t {
                                    Tokens::LIdentifier(s) => Some(s.clone()),
                                    _ => None,
                                })
                                .collect::<Vec<_>>()
                                .join("."),
                        )
                    ),
                )?;
            }

            let mut node3 = parse(lexer, node2, indent, defaults, Some(expand_defaults.clone()))?;

            // Set variant name and dependencies
            if !variant_name.is_empty() {
                node3.var_name = vec![Label::new(variant_name.clone(), None)];
                node3.name = name.iter()
                    .filter_map(|t| match t {
                        Tokens::LIdentifier(s) => Some(Label::new(variant_name.clone(), Some(s.clone()))),
                        _ => None,
                    })
                    .collect();
            } else {
                node3.name = name.iter()
                    .filter_map(|t| match t {
                        Tokens::LIdentifier(s) => Some(Label::new(s.clone(), None)),
                        _ => None,
                    })
                    .collect();
            }
            node3.dep = deps;

            // Determine if current variant is default from the meta variant information
            if !meta_default_values.is_empty() {
                for (i, default_str) in meta_default_values.iter().enumerate() {
                    let default_seq = default_str.split(' ')
                        .map(|x| Tokens::LIdentifier(x.to_string()))
                        .collect::<Vec<_>>();
                    if default_seq.len() == name.len() && default_seq.iter().zip(name.iter()).all(|(x, y)| x == y) {
                        is_default = true;
                        // Remove the matched default name values sequence
                        meta_default_values.remove(i);
                        break;
                    }
                }
            }
            if is_default && !already_default && meta_in_expand_defaults {
                node3.default = true;
                already_default = true;
            }
            node3.append_to_shortname = !is_default;

            // Update file mappings
            node3.add_content(
                lexer.filename.clone(),
                lexer.linenum,
                ContentType::Tokens(
                    Tokens::LUpdateFileMap(
                        lexer.filename.clone(),
                        node3.name.iter().map(|x| x.to_string()).collect::<Vec<_>>().join("."),
                        "_name_map_file".to_string(),
                    ),
                ),
            )?;
            node3.add_content(
                lexer.filename.clone(),
                lexer.linenum,
                ContentType::Tokens(
                    Tokens::LUpdateFileMap(
                        lexer.filename.clone(),
                        node3.name.iter().map(|x| x.name.clone()).collect::<Vec<_>>().join("."),
                        "_short_name_map_file".to_string(),
                    ),
                ),
            )?;

            // Add node to children
            if node3.default && defaults {
                node4.prepend_child(node3.clone())?;
            } else {
                node4.append_child(node3.clone())?;
            }

            // Update labels (move out fields since we appended clones)
            node4.update_labels(node3.labels)?;
            node4.update_labels(node3.name)?;
        }

        // Check if all default variants were used
        if !meta_default_values.is_empty() {
            return Err(PyErr::new::<ParserError, _>((
                format!("Missing default variant {:?}", meta_default_values),
                lexer.line.clone(),
                Some(lexer.filename.clone()),
                Some(lexer.linenum),
            )));
        }

        Ok(node4)
    }
}

pub fn parse(
    lexer: &mut Lexer,
    mut node: Node,
    prev_indent: isize,
    defaults: bool,
    expand_defaults: Option<Vec<String>>,
) -> PyResult<Node> {
    // Allowed token types for different contexts
    // reuse default tokens as much as possible using their identifiers
    let block_allowed = [
        Tokens::default("variants"),
        Tokens::default("Identifier"),
        Tokens::default("only"),
        Tokens::default("no"),
        Tokens::default("include"),
        Tokens::default("del"),
        Tokens::default("!"),
        Tokens::default("suffix"),
        Tokens::default("join"),
    ];
    let variants_allowed = [Tokens::default("-")];
    let identifier_allowed = [
        Tokens::default("="),
        Tokens::default("+="),
        Tokens::default("<="),
        Tokens::default("~="),
        Tokens::default("?="),
        Tokens::default("?+="),
        Tokens::default("?<="),
        Tokens::default(":"),
        Tokens::default("endl"),
    ];
    let indent_allowed = [
        Tokens::default("indent"),
        Tokens::default("endb")
    ];
    let mut allowed = block_allowed.to_vec();

    // Variant tracking state
    let mut variant_name = String::new();
    let mut variant_indent = 0;
    let mut meta = HashMap::new();

    // Pre-dictionary contains block of operation without collision with
    // other blocks or operations which increases speed almost twice.
    let mut pre_dict: HashMap<ParamKey, ParamVal> = HashMap::new();

    // Suffix operator state
    // NOTE: Suffix should be applied as the last operator in the dictionary
    // Reasons:
    // 1. Escapes multiplying suffix operators
    // 2. Affects all elements in current block
    let mut suffix: Option<(String, isize, Tokens)> = None;

    loop {
        lexer.set_prev_indent(prev_indent);

        // Handle indentation
        let token = lexer.get_next_token(
            Some(indent_allowed.to_vec()),
            None
        )?;

        if matches!(token, Tokens::LEndBlock(_)) {
            if !pre_dict.is_empty() {
                // Flush pre_dict to node content
                node.apply_predict(lexer, &mut pre_dict)?;
            }
            if let Some((filename, linenum, op)) = suffix {
                // Node has suffix, apply it to all elements
                node.add_content(filename.clone(), linenum, ContentType::Tokens(op))?;
            }
            return Ok(node);
        }

        let indent: isize = token.length()?;
        let token = lexer.get_next_token(Some(allowed.to_vec()), None)?;

        match token {
            Tokens::LInclude() => {
                node = node.apply_include(lexer, &mut pre_dict)?;
                lexer.set_prev_indent(prev_indent);
            }

            Tokens::LIdentifier(_) => {
                // Parse:
                //    identifier .....
                // Get tokens until an operator or colon
                let identifier = lexer.get_until(
                    identifier_allowed.to_vec(),
                    None,
                    Some(true),
                )?;
                let last_token: &Tokens = match identifier.last() {
                    Some(last_token) => last_token,
                    None => {
                        return Err(pyo3::exceptions::PyValueError::new_err("Empty identifier"));
                    }
                };

                if matches!(last_token, Tokens::LColon()) {
                    // Handle condition block
                    node.apply_condition(
                        identifier,
                        token,
                        lexer,
                        &mut pre_dict,
                        indent,
                    )?;
                } else if matches!(&last_token,
                    Tokens::LSet(_, _) | Tokens::LLazySet(_, _) | Tokens::LAppend(_, _) | Tokens::LPrepend(_, _) |
                    Tokens::LRegExpSet(_, _) | Tokens::LRegExpAppend(_, _) | Tokens::LRegExpPrepend(_, _)
                ) {
                    // Handle operator
                    node.apply_operator(
                        identifier,
                        token,
                        lexer,
                        &mut pre_dict,
                    )?;
                } else {
                    return Err(PyErr::new::<ParserError, _>((
                        "Syntax ERROR expected ':' or operand".to_string(),
                        lexer.line.clone(),
                        Some(lexer.filename.clone()),
                        Some(lexer.linenum),
                    )));
                }
            }

            Tokens::LDel(_, _) => {
                node.apply_deletion(lexer, &mut pre_dict)?;
            }

            Tokens::LNotCond() => {
                node.apply_notcondition(lexer, &mut pre_dict, indent)?;
                lexer.set_prev_indent(prev_indent);
            }

            Tokens::LVariants() => {
                let (name, meta_dict) = node.apply_variants(lexer)?;
                variant_name = name;
                variant_indent = indent;
                for (key, values) in meta_dict {
                    meta.insert(key, values);
                }
                allowed = variants_allowed.to_vec();
            }

            Tokens::LVariant() => {
                node = node.apply_variant(
                    lexer,
                    &mut pre_dict,
                    indent,
                    variant_name.clone(),
                    variant_indent,
                    &mut meta,
                    defaults,
                    expand_defaults.clone().unwrap_or_default(),
                )?;
                allowed = block_allowed.to_vec();
            }

            Tokens::LNo() | Tokens::LOnly() | Tokens::LJoin() => {
                // Parse:
                //    only/no/join (filter=text)..aaa.bbb, xxxx
                let rest_tokens: Vec<Tokens> = lexer.get_rest_line(None)?;
                let filters: Vec<Vec<Vec<Label>>> = Filters::parse_filter(
                    rest_tokens,
                    lexer.line.clone().unwrap_or("<none>".to_string()),
                    lexer.filename.clone(),
                    lexer.linenum,
                )?;
                node.apply_predict(lexer, &mut pre_dict)?;

                let content_type = match token {
                    Tokens::LOnly() => ContentType::Filters(Filters::OnlyFilter {
                        filter: filters,
                        line: lexer.line.clone().unwrap_or_default(),
                    }),
                    Tokens::LNo() => ContentType::Filters(Filters::NoFilter {
                        filter: filters,
                        line: lexer.line.clone().unwrap_or_default(),
                    }),
                    _ => ContentType::Filters(Filters::JoinFilter {
                        filter: filters,
                        line: lexer.line.clone().unwrap_or_default(),
                    }),
                };
                node.add_content(
                    lexer.filename.clone(),
                    lexer.linenum,
                    content_type,
                )?;
            }

            Tokens::LSuffix() => {
                // Parse:
                //    suffix SUFFIX
                if !pre_dict.is_empty() {
                    node.apply_predict(lexer, &mut pre_dict)?;
                }
                let token_val = lexer.get_next_token(
                    Some(vec![Tokens::default("Identifier")]),
                    None,
                )?;
                lexer.get_next_token(
                    Some(vec![Tokens::default("endl")]),
                    None,
                )?;

                suffix = Some((
                    lexer.filename.clone(),
                    lexer.linenum,
                    Tokens::Suffix(String::new(), token_val.string()?),
                ));
            }

            _ => {
                return Err(PyErr::new::<ParserError, _>((
                    "Syntax ERROR expected".to_string(),
                    lexer.line.clone(),
                    Some(lexer.filename.clone()),
                    Some(lexer.linenum),
                )));
            }
        }
    }
}

#[pyfunction]
#[pyo3(signature = (cfgstr, node, prev_indent=-1, defaults=true, expand_defaults=None))]
pub fn parse_string(
    cfgstr: String,
    node: Node,
    prev_indent: isize,
    defaults: bool,
    expand_defaults: Option<Vec<String>>,
) -> PyResult<Node> {
    let mut new_lexer = Lexer::new(Some(&cfgstr), None)?;
    parse(&mut new_lexer, node, prev_indent, defaults, expand_defaults)
}

#[pyfunction]
#[pyo3(signature = (cfgfile, node, prev_indent=-1, defaults=true, expand_defaults=None))]
pub fn parse_file(
    cfgfile: String,
    node: Node,
    prev_indent: isize,
    defaults: bool,
    expand_defaults: Option<Vec<String>>,
) -> PyResult<Node> {
    let mut new_lexer = Lexer::new(None, Some(&cfgfile))?;
    parse(&mut new_lexer, node, prev_indent, defaults, expand_defaults)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_apply_predict() {
        let mut node = Node::new();
        let lexer = Lexer::new(Some(""), None).expect("Failed to create lexer");
        let mut pre_dict: HashMap<ParamKey, ParamVal> = HashMap::new();
        pre_dict.insert("key".to_string().into(), "value".to_string().into());

        // apply_predict should add an LApplyPreDict content step and clear pre_dict
        node.apply_predict(&lexer, &mut pre_dict).expect("apply_predict failed");
        assert!(pre_dict.is_empty());
        let content = node.get_content().expect("get_content failed");
        assert_eq!(content.len(), 1);

        // content type should be Tokens::LApplyPreDict and contains our key/value
        match &content[0].content_type {
            ContentType::Tokens(Tokens::LApplyPreDict(_, map)) => {
                assert_eq!(map.get(&"key".to_string().into()), Some(&"value".to_string().into()));
            }
            other => panic!("Unexpected content type: {:?}", other),
        }
    }

    #[test]
    fn test_apply_include() {
        // create temporary file with a minimal variants block
        let mut tmp = NamedTempFile::new().expect("unable to create temp file");
        writeln!(tmp, "variants:").unwrap();
        writeln!(tmp, "  - test:").unwrap();
        let tmp_path = tmp.path().to_str().unwrap().to_string();

        // create a lexer with "include <path>"
        let content = format!("include {}", tmp_path);
        let mut lexer = Lexer::new(Some(&content), None).expect("Failed to create lexer");
        // advance lexer to consume indent and include tokens
        let _ = lexer.get_next_token(Some(vec![Tokens::default("indent")]), None).expect("get_next_token indent");
        let _ = lexer.get_next_token(Some(vec![Tokens::default("include")]), None).expect("get_next_token include");

        let mut node = Node::new();
        let mut pre_dict: HashMap<ParamKey, ParamVal> = HashMap::new();
        pre_dict.insert("key1".to_string().into(), "value1".to_string().into());

        // apply_include should parse the included file and return a node with a child named "test"
        let returned = node.apply_include(&mut lexer, &mut pre_dict).expect("apply_include failed");
        assert!(pre_dict.is_empty());
        let children = returned.get_children().unwrap();
        assert_eq!(children.len(), 1);
        let child = &children[0];
        assert_eq!(child.name.len(), 1);
        assert!(matches!(child.name[0], Label { .. }));
        assert_eq!(child.name[0].name, "test".to_string());
    }

    #[test]
    fn test_apply_operator_set_optimized() {
        // create lexer for "key2 = value2"
        let mut lexer = Lexer::new(Some("key2 = value2"), None).expect("Failed to create lexer");
        let _ = lexer.get_next_token(Some(vec![Tokens::default("indent")]), None).expect("indent token");
        let token = lexer.get_next_token(Some(vec![Tokens::default("Identifier")]), None).expect("identifier token");
        // get identifier tokens up to '=' (no_white = true)
        let identifier = lexer.get_until(vec![Tokens::default("=")], None, Some(true)).expect("get_until identifier");

        let mut node = Node::new();
        let mut pre_dict: HashMap<ParamKey, ParamVal> = HashMap::new();
        pre_dict.insert("key1".to_string().into(), "value1".to_string().into());

        // apply_operator should add key2 to pre_dict directly (optimized path)
        node.apply_operator(identifier, token, &mut lexer, &mut pre_dict).expect("apply_operator failed");
        // pre_dict now should contain both key1 and key2, and node content should be empty
        assert_eq!(pre_dict.get(&"key1".to_string().into()), Some(&"value1".to_string().into()));
        assert_eq!(pre_dict.get(&"key2".to_string().into()), Some(&"value2".to_string().into()));
        let content = node.get_content().expect("get_content failed");
        assert_eq!(content.len(), 0);
    }

    #[test]
    fn test_apply_operator_append_safe() {
        // content: "key1 += &value2"
        let mut lexer = Lexer::new(Some("key1 += &value2"), None).expect("Failed to create lexer");
        let _ = lexer.get_next_token(Some(vec![Tokens::default("indent")]), None).expect("indent token");
        let token = lexer.get_next_token(Some(vec![Tokens::default("Identifier")]), None).expect("identifier token");
        let identifier = lexer.get_until(vec![Tokens::default("+=")], None, Some(true)).expect("get_until identifier");

        let mut node = Node::new();
        let mut pre_dict: HashMap<ParamKey, ParamVal> = HashMap::new();
        pre_dict.insert("key1".to_string().into(), "value1".to_string().into());

        node.apply_operator(identifier, token, &mut lexer, &mut pre_dict).expect("apply_operator failed");

        // pre_dict should be updated
        assert_eq!(pre_dict.get(&"key1".to_string().into()), Some(&"value1&value2".to_string().into()));
        // node content should be empty
        let content = node.get_content().expect("get_content failed");
        assert_eq!(content.len(), 0);
    }

    #[test]
    fn test_apply_operator_append_unsafe() {
        // content: "key2 += &value2" (key2 not present in pre_dict therefore flush)
        let mut lexer = Lexer::new(Some("key2 += &value2"), None).expect("Failed to create lexer");
        let _ = lexer.get_next_token(Some(vec![Tokens::default("indent")]), None).expect("indent token");
        let token = lexer.get_next_token(Some(vec![Tokens::default("Identifier")]), None).expect("identifier token");
        let identifier = lexer.get_until(vec![Tokens::default("+=")], None, Some(true)).expect("get_until identifier");

        let mut node = Node::new();
        let mut pre_dict: HashMap<ParamKey, ParamVal> = HashMap::new();
        pre_dict.insert("key1".to_string().into(), "value1".to_string().into());

        node.apply_operator(identifier, token, &mut lexer, &mut pre_dict).expect("apply_operator failed");

        // pre_dict should be flushed (cleared)
        assert!(pre_dict.is_empty());
        // node content should be extended with append operation step
        let content = node.get_content().expect("get_content failed");
        assert_eq!(content.len(), 2);
        // first should be an LApplyPreDict, second an LAppend
        match &content[0].content_type {
            ContentType::Tokens(Tokens::LApplyPreDict(_, map)) => {
                assert_eq!(map.get(&"key1".to_string().into()), Some(&"value1".to_string().into()));
            }
            other => panic!("Unexpected first content type: {:?}", other),
        }
        match &content[1].content_type {
            ContentType::Tokens(Tokens::LAppend(key, value)) => {
                assert_eq!(key, "key2");
                assert_eq!(value, "&value2");
            }
            other => panic!("Unexpected second content type: {:?}", other),
        }
    }

    #[test]
    fn test_apply_deletion() {
        // content: "del key"
        let mut lexer = Lexer::new(Some("del key"), None).expect("Failed to create lexer");
        let _ = lexer.get_next_token(Some(vec![Tokens::default("indent")]), None).expect("indent");
        let _ = lexer.get_next_token(Some(vec![Tokens::default("del")]), None).expect("del");

        let mut node = Node::new();
        let mut pre_dict: HashMap<ParamKey, ParamVal> = HashMap::new();
        pre_dict.insert("key1".to_string().into(), "value1".to_string().into());

        node.apply_deletion(&mut lexer, &mut pre_dict).expect("apply_deletion failed");

        // pre_dict should be flushed (cleared)
        assert!(pre_dict.is_empty());
        // node content should be extended with delete operation step
        let content = node.get_content().expect("get_content failed");
        assert_eq!(content.len(), 2);
        match &content[0].content_type {
            ContentType::Tokens(Tokens::LApplyPreDict(_, map)) => {
                assert_eq!(map.get(&"key1".to_string().into()), Some(&"value1".to_string().into()));
            }
            other => panic!("Unexpected first content type: {:?}", other),
        }
        match &content[1].content_type {
            ContentType::Tokens(Tokens::LDel(name, _)) => {
                assert_eq!(name, "key");
            }
            other => panic!("Unexpected second content type: {:?}", other),
        }
    }

    #[test]
    fn test_apply_condition() {
        // content: "key:\n value"
        let mut lexer = Lexer::new(Some("key:\nvalue"), None).expect("Failed to create lexer");
        let _ = lexer.get_next_token(Some(vec![Tokens::default("indent")]), None).expect("indent");
        let token = lexer.get_next_token(Some(vec![Tokens::default("Identifier")]), None).expect("identifier");
        let identifier = lexer.get_until(vec![Tokens::default(":")], None, Some(true)).expect("get_until");

        let mut node = Node::new();
        let mut pre_dict: HashMap<ParamKey, ParamVal> = HashMap::new();
        pre_dict.insert("key1".to_string().into(), "value1".to_string().into());

        node.apply_condition(identifier, token, &mut lexer, &mut pre_dict, 0).expect("apply_condition failed");

        // pre_dict should be flushed (cleared)
        assert!(pre_dict.is_empty());
        let content = node.get_content().expect("get_content failed");
        assert_eq!(content.len(), 2);
        // first should be LApplyPreDict, second a Node with a positive condition
        match &content[0].content_type {
            ContentType::Tokens(Tokens::LApplyPreDict(_, _)) => {}
            other => panic!("Unexpected first content type: {:?}", other),
        }
        match &content[1].content_type {
            ContentType::Node(n) => {
                // positive condition expected
                if let Some(Filters::Condition { .. }) = n.condition {
                    // ok
                } else {
                    panic!("Expected Condition, got {:?}", n.condition);
                }
            }
            other => panic!("Unexpected second content type: {:?}", other),
        }
    }

    #[test]
    fn test_apply_notcondition() {
        // content: "!key:\n value"
        let mut lexer = Lexer::new(Some("!key:\nvalue"), None).expect("Failed to create lexer");
        let _ = lexer.get_next_token(Some(vec![Tokens::default("indent")]), None).expect("indent");
        let _ = lexer.get_next_token(Some(vec![Tokens::default("!")]), None).expect("notcond");

        let mut node = Node::new();
        let mut pre_dict: HashMap<ParamKey, ParamVal> = HashMap::new();
        pre_dict.insert("key1".to_string().into(), "value1".to_string().into());

        node.apply_notcondition(&mut lexer, &mut pre_dict, 0).expect("apply_notcondition failed");

        // pre_dict should be flushed (cleared)
        assert!(pre_dict.is_empty());
        let content = node.get_content().expect("get_content failed");
        assert_eq!(content.len(), 2);
        // first should be LApplyPreDict, second a Node with a negative condition
        match &content[0].content_type {
            ContentType::Tokens(Tokens::LApplyPreDict(_, _)) => {}
            other => panic!("Unexpected first content type: {:?}", other),
        }
        match &content[1].content_type {
            ContentType::Node(n) => {
                // negative condition expected
                if let Some(Filters::NegativeCondition { .. }) = n.condition {
                    // ok
                } else {
                    panic!("Expected NegativeCondition, got {:?}", n.condition);
                }
            }
            other => panic!("Unexpected second content type: {:?}", other),
        }
    }

    #[test]
    fn test_apply_variants() {
        // content: "variants test:"
        let mut lexer = Lexer::new(Some("variants test:"), None).expect("Failed to create lexer");
        let _ = lexer.get_next_token(Some(vec![Tokens::default("indent")]), None).expect("indent");
        let _ = lexer.get_next_token(Some(vec![Tokens::default("variants")]), None).expect("variants");

        let node = Node::new();
        let (variant_name, meta) = node.apply_variants(&mut lexer).expect("apply_variants failed");

        // variant name should be "test" and meta empty
        assert_eq!(variant_name, "test");
        assert!(meta.is_empty());
        // content should be empty
        let content = node.get_content().expect("get_content failed");
        assert_eq!(content.len(), 0);
    }

    #[test]
    fn test_apply_variants_meta() {
        // content: "variants test [meta1] [meta2=val2] [ meta3 ] [ meta4 = val4 val5 ]:"
        let txt = r#"variants test [meta1] [meta2=val2] [ meta3 ] [ meta4 = val4 val5 ]:"#;
        let mut lexer = Lexer::new(Some(txt), None).expect("Failed to create lexer");
        let _ = lexer.get_next_token(Some(vec![Tokens::default("indent")]), None).expect("indent");
        let _ = lexer.get_next_token(Some(vec![Tokens::default("variants")]), None).expect("variants");

        let node = Node::new();
        let (variant_name, meta) = node.apply_variants(&mut lexer).expect("apply_variants failed");

        // variant name should be "test"
        assert_eq!(variant_name, "test");
        // meta contents should be parsed appropriately
        assert_eq!(meta.get("meta1").map(|v| v.as_slice()), Some(&["true".to_string()][..]));
        assert_eq!(meta.get("meta2").map(|v| v.as_slice()), Some(&["val2".to_string()][..]));
        assert_eq!(meta.get("meta3").map(|v| v.as_slice()), Some(&["true".to_string()][..]));
        assert_eq!(meta.get("meta4").map(|v| v.as_slice()), Some(&["val4 val5".to_string()][..]));
        // content should be empty
        let content = node.get_content().expect("get_content failed");
        assert_eq!(content.len(), 0);
    }

    #[test]
    fn test_apply_variant() {
        // content: "- test:"
        let mut lexer = Lexer::new(Some("- test:"), None).expect("Failed to create lexer");
        let _ = lexer.get_next_token(Some(vec![Tokens::default("indent")]), None).expect("indent");
        let _ = lexer.get_next_token(Some(vec![Tokens::default("-")]), None).expect("dash");

        let mut node = Node::new();
        let mut pre_dict: HashMap<ParamKey, ParamVal> = HashMap::new();
        pre_dict.insert("key1".to_string().into(), "value1".to_string().into());
        let mut meta = HashMap::new();

        let grandparent_node = node.apply_variant(
            &mut lexer,
            &mut pre_dict,
            0,
            "test".to_string(),
            0,
            &mut meta,
            false,
            Vec::new(),
        ).expect("apply_variant failed");

        // pre_dict should be flushed (cleared)
        assert!(pre_dict.is_empty());

        // original node should receive the flushed pre_dict content
        let content = node.get_content().expect("get_content");
        assert!(!content.is_empty());
        match &content[0].content_type {
            ContentType::Tokens(Tokens::LApplyPreDict(_, map)) => {
                assert_eq!(map.get(&"key1".to_string().into()), Some(&"value1".to_string().into()));
            }
            other => panic!("Unexpected parent content: {:?}", other),
        }

        // grandparent node should have one child (the variant) whose name is "test"
        let parents = grandparent_node.get_children().unwrap();
        assert_eq!(parents.len(), 1);
        let parent_node = &parents[0];
        assert_eq!(parent_node.name.len(), 1);
        assert!(matches!(parent_node.name[0], Label { .. }));
        assert_eq!(parent_node.name[0].name, "test".to_string());

        // child should include the original grand child as its child
        let children = parent_node.get_children().unwrap();
        assert_eq!(children.len(), 1);
        assert_eq!(children[0], node);
    }
}
