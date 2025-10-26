use std::collections::HashMap;
use std::collections::VecDeque;
use std::fmt::Debug;
use std::rc::Rc;
use std::cell::RefCell;

use pyo3::{prelude::*, IntoPyObjectExt};
use pyo3::types::{PyAny, PyList, PyDict};

use crate::tokens::Tokens;
use crate::filters::Filters;
use crate::lexer::Lexer;

#[pyclass]
#[derive(Clone)]
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

impl PartialEq for Label {
    fn eq(&self, other: &Self) -> bool {
        if other.var_name.is_some() {
            self.long_name == other.long_name
        } else {
            self.name == other.name
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
            let content_type: ContentType = op_obj.extract()?;
            let filename: String = lexer.getattr("filename")?.extract()?;
            let linenum: i32 = lexer.getattr("linenum")?.extract()?;
            self.add_content(filename, linenum, content_type)?;
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
        let content_type = ContentType::Tokens(Tokens::LDel(to_del_str, "".to_string()));
        let filename: String = lexer.getattr("filename")?.extract()?;
        let linenum: i32 = lexer.getattr("linenum")?.extract()?;
        self.add_content(filename, linenum, content_type)?;
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
        let parser_type = py.import("cartconf.parser")?.getattr("Parser")?;
        let parser = parser_type.call0()?;
        let new_node = parser.call_method("_parse", (new_lexer, self.clone(), -1), None)?;
        Ok(new_node.extract::<Node>()?)
    }
}
