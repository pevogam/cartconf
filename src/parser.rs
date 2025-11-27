use std::collections::{HashMap, VecDeque};
use std::hash::Hash;
use std::fmt::{Debug, Display};
use std::rc::Rc;
use std::cell::RefCell;

use pyo3::{prelude::*, IntoPyObjectExt};
use pyo3::exceptions::PyException;
use pyo3::types::{PyAny, PyDict, PyList};

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
            return Ok(ContentType::Tokens(tokens));
        }
        else if let Ok(filters) = object.extract::<Filters>() {
            return Ok(ContentType::Filters(filters));
        }
        else if let Ok(node) = object.extract::<Node>() {
            return Ok(ContentType::Node(node));
        }
        let s: String = object.extract()?;
        Ok(ContentType::String(s))
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct ContentStep {
    filename: String,
    linenum: i32,
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
        if let Ok((filename, linenum, content_type)) = object.extract::<(String, i32, ContentType)>() {
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

    pub fn add_content(&mut self, filename: String, linenum: i32, content_type: ContentType) -> PyResult<()> {
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

    #[pyo3(signature = (lexer, pre_dict))]
    pub fn apply_predict(&mut self, lexer: &Bound<'_, PyAny>, pre_dict: &Bound<'_, PyDict>) -> PyResult<()> {
        // TODO: we do not provide lexer auto-conversion and instead treat it within python
        // since we would need cloning trait not just for it but also for the reader enum
        // Extract filename and linenum from the lexer object
        let filename: String = lexer.getattr("filename")?.extract()?;
        let linenum: i32 = lexer.getattr("linenum")?.extract()?;

        // Build a LApplyPreDict from the original Python dict
        let map: HashMap<String, String> = pre_dict.extract()?;
        let content_type = ContentType::Tokens(Tokens::LApplyPreDict(String::new(), map));

        // Add pre-dictionary content to this node
        self.add_content(filename, linenum, content_type)?;

        // Clear the original pre_dict in-place
        pre_dict.call_method0("clear")?;

        Ok(())
    }

    /*
    Parse:
        identifier = xxx
        identifier <= xxx
        identifier ?= xxx
        etc..
    */
    #[pyo3(signature = (identifier, token, lexer, pre_dict))]
    pub fn apply_operator(
        &mut self,
        identifier: Vec<Tokens>,
        token: Tokens,
        lexer: &Bound<'_, PyAny>,
        pre_dict: &Bound<'_, PyDict>,
    ) -> PyResult<()> {
        let py = lexer.py();

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
        let lstring = Tokens::LString(String::new()).into_bound_py_any(py)?.get_type();
        let lendl = Tokens::LEndL().into_bound_py_any(py)?.get_type();
        let req_list = PyList::new(py, &[lstring])?;
        let value = lexer.call_method1("get_next_token", (req_list,))?;
        let mut value_str: String = value.getattr("string")?.extract()?;
        // strip surrounding quotes if present
        let first = value_str.chars().next().unwrap_or(' ');
        let last = value_str.chars().last().unwrap_or(' ');
        if (first == '"' && last == '"') || (first == '\'' && last == '\'') {
            value_str = value_str[1..value_str.len() - 1].to_string();
        }

        // Construct operator instance by calling its Python type
        let op: &Tokens = match identifier.last() {
            Some(last_token) => last_token,
            None => {
                return Err(pyo3::exceptions::PyValueError::new_err("Empty identifier"));
            }
        };
        let op_type = op.clone().into_bound_py_any(py)?.get_type();
        let op_obj = op_type.call1((identifier_str.clone(), value_str.clone()))?;

        // If it's an LSet and value has no '$', apply directly to pre_dict
        let d_nin_val = !value_str.contains('$');
        if matches!(op, Tokens::LSet(_,_)) && d_nin_val {
            op_obj.call_method1("apply_to_dict", (pre_dict,))?;
        } else {
            // If pre_dict has pending entries, either optimize or flush
            let pre_nonempty = pre_dict.len() != 0;
            if pre_nonempty {
                // try to get op.name and check if it's present in pre_dict
                let op_name = match op_obj.getattr("name") {
                    Ok(n) => n.extract::<String>()?,
                    Err(_) => String::new(),
                };
                if !op_name.is_empty() && d_nin_val && pre_dict.contains(op_name.as_str())? {
                    // apply and consume EOL
                    op_obj.call_method1("apply_to_dict", (pre_dict,))?;
                    let req_end = PyList::new(py, &[lendl])?;
                    lexer.call_method1("get_next_token", (req_end,))?;
                    return Ok(());
                } else {
                    // flush pre_dict into node
                    self.apply_predict(lexer, pre_dict)?;
                }
            }

            // Add operator token as content
            self.add_content(
                lexer.getattr("filename")?.extract()?,
                lexer.getattr("linenum")?.extract()?,
                op_obj.extract()?,
            )?;
        }

        // consume end-of-line
        let req_end = PyList::new(py, &[lendl])?;
        lexer.call_method1("get_next_token", (req_end,))?;
        Ok(())
    }

    /*
    Parse:
        del operand
    */
    #[pyo3(signature = (lexer, pre_dict))]
    pub fn apply_deletion(
        &mut self,
        lexer: &Bound<'_, PyAny>,
        pre_dict: &Bound<'_, PyDict>,
    ) -> PyResult<()> {
        let py = lexer.py();

        let lidentifier = Tokens::LIdentifier(String::new()).into_bound_py_any(py)?.get_type();
        let lendl = Tokens::LEndL().into_bound_py_any(py)?.get_type();
        let args = PyList::new(py, &[lidentifier])?;
        let kwargs = {
            let d = PyDict::new(py);
            d.set_item("no_white", true)?;
            d
        };
        let to_del = lexer.call_method("get_next_token", (args,), Some(&kwargs))?;
        // consume EOL
        let args_end = PyList::new(py, &[lendl])?;
        let kwargs_end = {
            let d = PyDict::new(py);
            d.set_item("no_white", true)?;
            d
        };
        lexer.call_method("get_next_token", (args_end,), Some(&kwargs_end))?;
        let to_del_str: String = to_del.getattr("string")?.extract()?;

        // flush pre_dict and add token as content
        self.apply_predict(lexer, pre_dict)?;
        self.add_content(
            lexer.getattr("filename")?.extract()?,
            lexer.getattr("linenum")?.extract()?,
            ContentType::Tokens(Tokens::LDel(to_del_str, "".to_string())),
        )?;

        Ok(())
    }

    /*
    Parse:
        include relative file path to working directory.
    */
    #[pyo3(signature = (lexer, pre_dict))]
    pub fn apply_include(
        &mut self,
        lexer: &Bound<'_, PyAny>,
        pre_dict: &Bound<'_, PyDict>,
    ) -> PyResult<Node> {
        let py = lexer.py();

        // Get path from rest of line
        let path = lexer.call_method0("get_rest_line_as_string_token")?;
        let path_str: String = path.getattr("string")?.extract()?;

        // Expand user path (~ -> $HOME)
        let expanded_path = if path_str.starts_with('~') {
            let home = std::env::var("HOME").unwrap_or_default();
            path_str.replacen('~', &home, 1)
        } else {
            path_str
        };

        // Make path absolute if needed
        let mut filepath = std::path::PathBuf::from(&expanded_path);
        let current_file: String = lexer.getattr("filename")?.extract()?;
        if current_file != "<string>" && !filepath.is_absolute()
            && let Some(parent) = std::path::Path::new(&current_file).parent() {
                filepath = parent.join(filepath);
        }

        // Check file exists
        if !filepath.is_file() {
            let line: String = lexer.getattr("line")?.extract()?;
            let filename: String = lexer.getattr("filename")?.extract()?;
            let linenum: i32 = lexer.getattr("linenum")?.extract()?;

            let exceptions = py.import("cartconf.exceptions")?;
            let err = exceptions.getattr("MissingIncludeError")?;
            return Err(PyErr::from_value(err.call1((line, filename, linenum))?));
        }

        // Apply current pre_dict and create new lexer for included file
        self.apply_predict(lexer, pre_dict)?;

        let filepath_str = filepath.to_str()
            .ok_or_else(|| PyErr::new::<pyo3::exceptions::PyValueError, _>("Invalid filepath"))?;
        let new_lexer = Lexer::new(None, Some(filepath_str))?;

        // Parse with new lexer
        parse(&new_lexer.into_bound_py_any(py)?, self.clone(), -1, false, None)
    }

    /*
    Parse:
        xxx.yyy.(aaa=bbb):
    */
    #[pyo3(signature = (identifier, token, lexer, pre_dict, indent))]
    pub fn apply_condition(
        &mut self,
        identifier: Vec<Tokens>,
        token: Tokens,
        lexer: &Bound<'_, PyAny>,
        pre_dict: &Bound<'_, PyDict>,
        indent: i32,
    ) -> PyResult<()> {
        // Build the full identifier list: [token] + identifier[:-1] + [LEndl]
        let mut tokens = vec![token];
        let identifier_len = identifier.len();
        tokens.extend(identifier.into_iter().take(identifier_len.saturating_sub(1)));
        tokens.push(Tokens::LEndL());

        // Parse the condition filter
        let cfilter: Vec<Vec<Vec<Label>>> = Filters::parse_filter(
            tokens,
            lexer.getattr("line")?.extract()?,
            lexer.getattr("filename")?.extract()?,
            lexer.getattr("linenum")?.extract()?,
        )?;

        // Get the next line and set it in the lexer
        let next_line = lexer.call_method0("get_rest_line_as_string_token")?;
        let next_line_str: String = next_line.getattr("string")?.extract()?;
        if !next_line_str.is_empty() {
            lexer.call_method1("set_next_line", (next_line_str, indent + 1, lexer.getattr("linenum")?))?;
        }

        // Create a new Node for the condition
        let mut cond = Node::new();
        cond.condition = Some(Filters::Condition { filter : cfilter, line : lexer.getattr("line")?.extract()? });

        // Parse the condition block
        cond = parse(lexer, cond, indent, false, None)?;

        // Apply the current pre_dict and add the condition node as content
        self.apply_predict(lexer, pre_dict)?;
        self.add_content(
            lexer.getattr("filename")?.extract()?,
            lexer.getattr("linenum")?.extract()?,
            ContentType::Node(cond),
        )?;

        Ok(())
    }

    /*
    Parse:
        !xxx.yyy.(aaa=bbb): vvv
    */
    #[pyo3(signature = (lexer, pre_dict, indent))]
    pub fn apply_notcondition(
        &mut self,
        lexer: &Bound<'_, PyAny>,
        pre_dict: &Bound<'_, PyDict>,
        indent: i32,
    ) -> PyResult<()> {
        let py = lexer.py();

        // Build the full token list
        let lcolon = Tokens::LColon().into_bound_py_any(py)?.get_type();
        let lendl = Tokens::LEndL().into_bound_py_any(py)?.get_type();
        let kwargs = {
            let d = PyDict::new(py);
            d.set_item("no_white", true)?;
            d
        };
        let tokens_pylist = lexer.call_method(
            "get_until",
            (PyList::new(py, &[lcolon, lendl])?,),
            Some(&kwargs),
        )?;
        let tokens: Vec<Tokens> = tokens_pylist.extract()?;
        let tokens_len = tokens.len();
        let tokens: Vec<_> = tokens
            .into_iter()
            .take(tokens_len.saturating_sub(1))
            .collect();

        // Parse the condition filter
        let lfilter: Vec<Vec<Vec<Label>>> = Filters::parse_filter(
            tokens,
            lexer.getattr("line")?.extract()?,
            lexer.getattr("filename")?.extract()?,
            lexer.getattr("linenum")?.extract()?,
        )?;

        // Get the next line and set it in the lexer
        let next_line = lexer.call_method0("get_rest_line_as_string_token")?;
        let next_line_str: String = next_line.getattr("string")?.extract()?;
        if !next_line_str.is_empty() {
            lexer.call_method1("set_next_line", (next_line_str, indent + 1, lexer.getattr("linenum")?))?;
        }

        // Create a new Node for the negative condition
        let mut cond = Node::new();
        cond.condition = Some(Filters::NegativeCondition { filter : lfilter, line : lexer.getattr("line")?.extract()? });

        // Parse the condition block
        cond = parse(lexer, cond, indent, false, None)?;

        // Apply the current pre_dict and add the condition node as content
        self.apply_predict(lexer, pre_dict)?;
        self.add_content(
            lexer.getattr("filename")?.extract()?,
            lexer.getattr("linenum")?.extract()?,
            ContentType::Node(cond),
        )?;

        Ok(())
    }

    /*
    Parse:
       variants _name_ [meta1] [meta2=val2]:
    */
    #[pyo3(signature = (lexer))]
    pub fn apply_variants(
        &self,
        lexer: &Bound<'_, PyAny>,
    ) -> PyResult<(String, HashMap<String, Vec<String>>)> {
        let py = lexer.py();

        // Check if node has conditions
        if self.condition.is_some() {
            return Err(PyErr::new::<ParserError, _>((
                "'variants' is not allowed inside a conditional block".to_string(),
                Some(lexer.getattr("line")?.extract::<String>()?),
                Some(lexer.getattr("filename")?.extract::<String>()?),
                Some(lexer.getattr("linenum")?.extract::<i32>()?),
            )));
        }

        // Get tokens until bracket, colon, identifier or end
        let allowed = [
            Tokens::LLBracket().into_bound_py_any(py)?.get_type(),
            Tokens::LColon().into_bound_py_any(py)?.get_type(),
            Tokens::LIdentifier(String::new()).into_bound_py_any(py)?.get_type(),
            Tokens::LEndL().into_bound_py_any(py)?.get_type(),
        ];
        let kwargs = {
            let d = PyDict::new(py);
            d.set_item("no_white", true)?;
            d
        };
        let tokens_pylist = lexer.call_method("get_until", (allowed.to_vec(),), Some(&kwargs))?;
        let tokens: Vec<Tokens> = tokens_pylist.extract()?;
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
                        Some(lexer.getattr("line")?.extract::<String>()?),
                        Some(lexer.getattr("filename")?.extract::<String>()?),
                        Some(lexer.getattr("linenum")?.extract::<i32>()?),
                    )));
                }
                variant_name = tokens_pylist.get_item(0)?.getattr("string")?.extract()?;
            } else if matches!(vtoken, Tokens::LLBracket()) {
                // Parse metadata in brackets
                let ident = lexer.call_method(
                    "get_next_token",
                    ([Tokens::LIdentifier(String::new()).into_bound_py_any(py)?.get_type()],),
                    Some(&kwargs),
                )?;
                let ident_str: String = ident.getattr("string")?.extract()?;

                let next = lexer.call_method(
                    "get_next_token",
                    ([Tokens::LSet(String::new(), String::new()).into_bound_py_any(py)?.get_type(),
                      Tokens::LRBracket().into_bound_py_any(py)?.get_type()],),
                    Some(&kwargs),
                )?;
                let next_token: Tokens = next.extract()?;

                if matches!(next_token, Tokens::LRBracket()) {
                    // Handle [xxx]
                    meta.entry(ident_str)
                        .or_insert_with(Vec::new)
                        .push(true.to_string());
                } else if matches!(next_token, Tokens::LSet(_, _)) {
                    // Handle [xxx = yyy]
                    let tokens = lexer.call_method(
                        "get_until",
                        ([Tokens::LRBracket().into_bound_py_any(py)?.get_type(),
                          Tokens::LEndL().into_bound_py_any(py)?.get_type()],),
                        Some(&kwargs),
                    )?;
                    let last = tokens.get_item(tokens.len()? - 1)?;
                    let last_token: Tokens = last.extract()?;

                    if matches!(last_token, Tokens::LRBracket()) {
                        let mut values = Vec::new();
                        for i in 0..tokens.len()? - 1 {
                            let token = tokens.get_item(i)?;
                            values.push(token.getattr("string")?.extract::<String>()?);
                        }
                        // The python side has an inner list that we stringify here (just like the bool above)
                        meta.entry(ident_str)
                            .or_insert_with(Vec::new)
                            .push(values.join(" ").to_string());
                    } else {
                        return Err(PyErr::new::<ParserError, _>((
                            "Syntax ERROR expected ']'".to_string(),
                            Some(lexer.getattr("line")?.extract::<String>()?),
                            Some(lexer.getattr("filename")?.extract::<String>()?),
                            Some(lexer.getattr("linenum")?.extract::<i32>()?),
                        )));
                    }
                }
            }

            // Get next token
            let next = lexer.call_method(
                "get_next_token",
                (allowed.to_vec(),),
                Some(&kwargs),
            )?;
            let next_token = next.extract::<Tokens>()?;
            vtoken = next_token;
        }

        // Verify default values if present
        if meta.contains_key("default") {
            for val in meta.get("default").unwrap_or(&Vec::new()) {
                if val == "true" {
                    return Err(PyErr::new::<ParserError, _>((
                        "Syntax ERROR expected [default=xxx]".to_string(),
                        Some(lexer.getattr("line")?.extract::<String>()?),
                        Some(lexer.getattr("filename")?.extract::<String>()?),
                        Some(lexer.getattr("linenum")?.extract::<i32>()?),
                    )));
                }
            }
        }

        // Check for required colon
        if matches!(vtoken, Tokens::LEndL()) {
            return Err(PyErr::new::<ParserError, _>((
                "Syntax ERROR expected ':'".to_string(),
                Some(lexer.getattr("line")?.extract::<String>()?),
                Some(lexer.getattr("filename")?.extract::<String>()?),
                Some(lexer.getattr("linenum")?.extract::<i32>()?),
            )));
        }

        // Consume end of line
        lexer.call_method(
            "get_next_token",
            ([Tokens::LEndL().into_bound_py_any(py)?.get_type()],),
            Some(&kwargs),
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
    #[pyo3(signature = (lexer, pre_dict, indent,
        variant_name, variant_indent, meta,
        defaults, expand_defaults
    ))]
    #[allow(clippy::too_many_arguments)]
    pub fn apply_variant(
        &mut self,
        lexer: &Bound<'_, PyAny>,
        pre_dict: &Bound<'_, PyDict>,
        indent: i32,
        variant_name: String,
        variant_indent: i32,
        meta: &Bound<'_, PyDict>,
        defaults : bool,
        expand_defaults : Vec<String>,
    ) -> PyResult<Node> {
        let py = lexer.py();
        let mut already_default = false;
        let mut node4 = Node::new();

        if !pre_dict.is_empty() {
            self.apply_predict(lexer, pre_dict)?;
        }

        // Handle default variants
        let meta_default = meta.get_item("default")?;
        let meta_in_expand_defaults = !expand_defaults.contains(&variant_name);

        // Data used for the entire loop
        let tokens = PyList::new(
            py,
            &[
                Tokens::LIdentifier(String::new()).into_bound_py_any(py)?.get_type(),
                Tokens::LDefault().into_bound_py_any(py)?.get_type(),
                Tokens::LIndent(-1).into_bound_py_any(py)?.get_type(),
                Tokens::LEndBlock(-1).into_bound_py_any(py)?.get_type(),
            ],
        )?;
        let kwargs = PyDict::new(py);
        kwargs.set_item("no_white", true)?;

        loop {
            lexer.call_method1("set_prev_indent", (variant_indent,))?;

            // Get token from lexer and check for end of block
            let token_py = lexer.call_method("get_next_token", (tokens.clone(),), Some(&kwargs))?;
            let token: Tokens = token_py.extract()?;
            if matches!(token, Tokens::LEndBlock(_)) {
                break;
            }

            let mut is_default = false;
            let mut name;

            if matches!(token, Tokens::LIndent(_)) {
                // Handle indented variant
                lexer.call_method(
                    "get_next_token",
                    (PyList::new(py, &[Tokens::LVariant().into_bound_py_any(py)?.get_type()])?,),
                    Some(&kwargs),
                )?;
                let token_py = lexer.call_method(
                    "get_next_token",
                    (PyList::new(py, &[
                        Tokens::LIdentifier(String::new()).into_bound_py_any(py)?.get_type(),
                        Tokens::LDefault().into_bound_py_any(py)?.get_type(),
                    ])?,),
                    Some(&kwargs),
                )?;
                let token: Tokens = token_py.extract()?;

                if matches!(token, Tokens::LDefault()) {
                    is_default = true;
                    name = lexer.call_method(
                        "get_until",
                        (PyList::new(py, &[Tokens::LColon().into_bound_py_any(py)?.get_type()])?,),
                        None,
                    )?.extract()?;
                } else {
                    name = vec![token];
                    name.extend(lexer.call_method(
                        "get_until",
                        (PyList::new(py, &[Tokens::LColon().into_bound_py_any(py)?.get_type()])?,),
                        None,
                    )?.extract::<Vec<Tokens>>()?);
                }
            } else if matches!(token, Tokens::LDefault()) {
                is_default = true;
                name = lexer.call_method(
                    "get_until",
                    (PyList::new(py, &[Tokens::LColon().into_bound_py_any(py)?.get_type()])?,),
                    None,
                )?.extract()?;
            } else {
                name = vec![token];
                name.extend(lexer.call_method(
                    "get_until",
                    (PyList::new(py, &[Tokens::LColon().into_bound_py_any(py)?.get_type()])?,),
                    None,
                )?.extract::<Vec<Tokens>>()?);
            }
            let name_len = name.len();
            // Drop the colon at the end of the parsed name
            name = name.into_iter().take(name_len.saturating_sub(1)).collect();

            // Get dependencies after colon
            let token_py = lexer.call_method("get_next_token", (), Some(&kwargs))?;
            let token: Tokens = token_py.extract()?;
            let mut deps = Vec::new();
            if !matches!(token, Tokens::LEndL()) {
                let mut filter_tokens = vec![token];
                filter_tokens.extend(lexer.call_method(
                    "get_until",
                    (PyList::new(py, &[Tokens::LEndL().into_bound_py_any(py)?.get_type()])?,),
                    None,
                )?.extract::<Vec<Tokens>>()?);
                deps = Filters::parse_filter(
                    filter_tokens,
                    lexer.getattr("line")?.extract()?,
                    lexer.getattr("filename")?.extract()?,
                    lexer.getattr("linenum")?.extract()?,
                )?;
            }

            // Create and parse the variant node
            let mut node2 = Node::new();
            node2.append_child(self.clone())?;
            node2.update_labels(self.labels.clone())?;

            if !variant_name.is_empty() {
                node2.add_content(
                    lexer.getattr("filename")?.extract()?,
                    lexer.getattr("linenum")?.extract()?,
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
            if let Some(ref s) = meta_default {
                let default_values = s.extract::<Vec<String>>()?;
                for default_str in default_values.iter() {
                    let default_seq = default_str.split(' ')
                        .map(|x| Tokens::LIdentifier(x.to_string()))
                        .collect::<Vec<_>>();
                    if default_seq.len() == name.len() && default_seq.iter().zip(name.iter()).all(|(x, y)| x == y) {
                        is_default = true;
                        // Remove the matched default name values sequence
                        s.call_method1("remove", (default_str.clone(),))?;
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
                lexer.getattr("filename")?.extract()?,
                lexer.getattr("linenum")?.extract()?,
                ContentType::Tokens(
                    Tokens::LUpdateFileMap(
                        lexer.getattr("filename")?.extract()?,
                        node3.name.iter().map(|x| x.to_string()).collect::<Vec<_>>().join("."),
                        "_name_map_file".to_string(),
                    ),
                ),
            )?;
            node3.add_content(
                lexer.getattr("filename")?.extract()?,
                lexer.getattr("linenum")?.extract()?,
                ContentType::Tokens(
                    Tokens::LUpdateFileMap(
                        lexer.getattr("filename")?.extract()?,
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
        if let Some(ref s) = meta_default {
            let default_values = s.extract::<Vec<String>>()?;
            if !default_values.is_empty() {
                return Err(PyErr::new::<ParserError, _>((
                    format!("Missing default variant {:?}", default_values),
                    Some(lexer.getattr("line")?.extract::<String>().unwrap_or("<none>".to_string())),
                    Some(lexer.getattr("filename")?.extract::<String>()?),
                    Some(lexer.getattr("linenum")?.extract::<i32>()?),
                )));
            }
        }

        Ok(node4)
    }
}

#[pyfunction]
#[pyo3(signature = (lexer, node, prev_indent=-1, defaults=true, expand_defaults=None))]
pub fn parse(
    lexer: &Bound<'_, PyAny>,
    mut node: Node,
    prev_indent: i32,
    defaults: bool,
    expand_defaults: Option<Vec<String>>,
) -> PyResult<Node> {
    let py = lexer.py();

    // Allowed token types for different contexts
    let block_allowed = [
        Tokens::LVariants().into_bound_py_any(py)?.get_type(),
        Tokens::LIdentifier(String::new()).into_bound_py_any(py)?.get_type(),
        Tokens::LOnly().into_bound_py_any(py)?.get_type(),
        Tokens::LNo().into_bound_py_any(py)?.get_type(),
        Tokens::LInclude().into_bound_py_any(py)?.get_type(),
        Tokens::LDel(String::new(), String::new()).into_bound_py_any(py)?.get_type(),
        Tokens::LNotCond().into_bound_py_any(py)?.get_type(),
        Tokens::LSuffix().into_bound_py_any(py)?.get_type(),
        Tokens::LJoin().into_bound_py_any(py)?.get_type(),
    ];
    let variants_allowed = [Tokens::LVariant().into_bound_py_any(py)?.get_type()];
    let identifier_allowed = [
        Tokens::LSet(String::new(), String::new()).into_bound_py_any(py)?.get_type(),
        Tokens::LAppend(String::new(), String::new()).into_bound_py_any(py)?.get_type(),
        Tokens::LPrepend(String::new(), String::new()).into_bound_py_any(py)?.get_type(),
        Tokens::LLazySet(String::new(), String::new()).into_bound_py_any(py)?.get_type(),
        Tokens::LRegExpSet(String::new(), String::new()).into_bound_py_any(py)?.get_type(),
        Tokens::LRegExpAppend(String::new(), String::new()).into_bound_py_any(py)?.get_type(),
        Tokens::LRegExpPrepend(String::new(), String::new()).into_bound_py_any(py)?.get_type(),
        Tokens::LColon().into_bound_py_any(py)?.get_type(),
        Tokens::LEndL().into_bound_py_any(py)?.get_type(),
    ];
    let indent_allowed = [
        Tokens::LIndent(0).into_bound_py_any(py)?.get_type(),
        Tokens::LEndBlock(0).into_bound_py_any(py)?.get_type(),
    ];
    let mut allowed = block_allowed.to_vec();

    // Variant tracking state
    let mut variant_name = String::new();
    let mut variant_indent = 0;
    let meta = PyDict::new(py);

    // Pre-dictionary contains block of operation without collision with
    // other blocks or operations which increases speed almost twice.
    let pre_dict = PyDict::new(py);

    // Suffix operator state
    // NOTE: Suffix should be applied as the last operator in the dictionary
    // Reasons:
    // 1. Escapes multiplying suffix operators
    // 2. Affects all elements in current block
    let mut suffix = None;

    loop {
        lexer.call_method1("set_prev_indent", (prev_indent,))?;

        // Handle indentation
        let token_py = lexer.call_method1("get_next_token", (indent_allowed.to_vec(),))?;
        let token: Tokens = token_py.extract()?;

        if matches!(token, Tokens::LEndBlock(_)) {
            if !pre_dict.is_empty() {
                // Flush pre_dict to node content
                node.apply_predict(lexer, &pre_dict)?;
            }
            if let Some((filename, linenum, op)) = suffix {
                // Node has suffix, apply it to all elements
                node.add_content(filename, linenum, ContentType::Tokens(op))?;
            }
            return Ok(node);
        }

        let indent: i32 = token_py.getattr("length")?.extract()?;
        let token_py = lexer.call_method1("get_next_token", (allowed.to_vec(),))?;
        let token: Tokens = token_py.extract()?;

        match token {
            Tokens::LInclude() => {
                node = node.apply_include(lexer, &pre_dict)?;
                lexer.call_method1("set_prev_indent", (prev_indent,))?;
            }

            Tokens::LIdentifier(_) => {
                // Parse:
                //    identifier .....
                // Get tokens until an operator or colon
                let kwargs = PyDict::new(py);
                kwargs.set_item("no_white", true)?;
                let identifier = lexer.call_method(
                    "get_until",
                    (identifier_allowed.to_vec(),),
                    Some(&kwargs),
                )?;
                let last_token_py = identifier.get_item(identifier.len()? - 1)?;
                let last_token: Tokens = last_token_py.extract()?;

                if matches!(last_token, Tokens::LColon()) {
                    // Handle condition block
                    node.apply_condition(
                        identifier.extract()?,
                        token,
                        lexer,
                        &pre_dict,
                        indent,
                    )?;
                } else if matches!(&last_token,
                    Tokens::LSet(_, _) | Tokens::LLazySet(_, _) | Tokens::LAppend(_, _) | Tokens::LPrepend(_, _) |
                    Tokens::LRegExpSet(_, _) | Tokens::LRegExpAppend(_, _) | Tokens::LRegExpPrepend(_, _)
                ) {
                    // Handle operator
                    node.apply_operator(
                        identifier.extract()?,
                        token,
                        lexer,
                        &pre_dict,
                    )?;
                } else {
                    return Err(PyErr::new::<ParserError, _>((
                        "Syntax ERROR expected ':' or operand".to_string(),
                        Some(lexer.getattr("line")?.extract::<String>()?),
                        Some(lexer.getattr("filename")?.extract::<String>()?),
                        Some(lexer.getattr("linenum")?.extract::<i32>()?),
                    )));
                }
            }

            Tokens::LDel(_, _) => {
                node.apply_deletion(lexer, &pre_dict)?;
            }

            Tokens::LNotCond() => {
                node.apply_notcondition(lexer, &pre_dict, indent)?;
                lexer.call_method1("set_prev_indent", (prev_indent,))?;
            }

            Tokens::LVariants() => {
                let (name, meta_dict) = node.apply_variants(lexer)?;
                variant_name = name;
                variant_indent = indent;
                for (key, values) in meta_dict {
                    meta.set_item(key, values)?;
                }
                allowed = variants_allowed.to_vec();
            }

            Tokens::LVariant() => {
                node = node.apply_variant(
                    lexer,
                    &pre_dict,
                    indent,
                    variant_name.clone(),
                    variant_indent,
                    &meta,
                    defaults,
                    expand_defaults.clone().unwrap_or_default(),
                )?;
                allowed = block_allowed.to_vec();
            }

            Tokens::LNo() | Tokens::LOnly() | Tokens::LJoin() => {
                // Parse:
                //    only/no/join (filter=text)..aaa.bbb, xxxx
                let rest_line = lexer.call_method0("get_rest_line")?;
                let rest_tokens: Vec<Tokens> = rest_line.extract()?;
                let filters: Vec<Vec<Vec<Label>>> = Filters::parse_filter(
                    rest_tokens,
                    lexer.getattr("line")?.extract()?,
                    lexer.getattr("filename")?.extract()?,
                    lexer.getattr("linenum")?.extract()?,
                )?;
                node.apply_predict(lexer, &pre_dict)?;

                let content_type = match token {
                    Tokens::LOnly() => ContentType::Filters(Filters::OnlyFilter {
                        filter: filters,
                        line: lexer.getattr("line")?.extract()?,
                    }),
                    Tokens::LNo() => ContentType::Filters(Filters::NoFilter {
                        filter: filters,
                        line: lexer.getattr("line")?.extract()?,
                    }),
                    _ => ContentType::Filters(Filters::JoinFilter {
                        filter: filters,
                        line: lexer.getattr("line")?.extract()?,
                    }),
                };
                node.add_content(
                    lexer.getattr("filename")?.extract()?,
                    lexer.getattr("linenum")?.extract()?,
                    content_type,
                )?;
            }

            Tokens::LSuffix() => {
                // Parse:
                //    suffix SUFFIX
                if !pre_dict.is_empty() {
                    node.apply_predict(lexer, &pre_dict)?;
                }
                let token_val = lexer.call_method1(
                    "get_next_token",
                    ([Tokens::LIdentifier(String::new()).into_bound_py_any(py)?.get_type()],),
                )?;
                lexer.call_method1(
                    "get_next_token",
                    ([Tokens::LEndL().into_bound_py_any(py)?.get_type()],),
                )?;

                suffix = Some((
                    lexer.getattr("filename")?.extract()?,
                    lexer.getattr("linenum")?.extract()?,
                    Tokens::Suffix(String::new(), token_val.getattr("string")?.extract()?),
                ));
            }

            _ => {
                return Err(PyErr::new::<ParserError, _>((
                    "Syntax ERROR expected".to_string(),
                    Some(lexer.getattr("line")?.extract::<String>()?),
                    Some(lexer.getattr("filename")?.extract::<String>()?),
                    Some(lexer.getattr("linenum")?.extract::<i32>()?),
                )));
            }
        }
    }
}

#[pyfunction]
#[pyo3(signature = (cfgstr, node, prev_indent=-1, defaults=true, expand_defaults=None))]
pub fn parse_string(
    py: Python<'_>,
    cfgstr: String,
    node: Node,
    prev_indent: i32,
    defaults: bool,
    expand_defaults: Option<Vec<String>>,
) -> PyResult<Node> {
    let new_lexer = Lexer::new(Some(&cfgstr), None)?;
    parse(&new_lexer.into_bound_py_any(py)?, node, prev_indent, defaults, expand_defaults)
}

#[pyfunction]
#[pyo3(signature = (cfgfile, node, prev_indent=-1, defaults=true, expand_defaults=None))]
pub fn parse_file(
    py: Python<'_>,
    cfgfile: String,
    node: Node,
    prev_indent: i32,
    defaults: bool,
    expand_defaults: Option<Vec<String>>,
) -> PyResult<Node> {
    let new_lexer = Lexer::new(None, Some(&cfgfile))?;
    parse(&new_lexer.into_bound_py_any(py)?, node, prev_indent, defaults, expand_defaults)
}
