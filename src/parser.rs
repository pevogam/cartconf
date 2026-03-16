use std::borrow::Cow;
use std::cmp::min;
use std::collections::{HashMap, VecDeque};
use std::hash::Hash;
use std::fmt::{Debug, Display};
use std::cell::RefCell;

use pyo3::prelude::*;
use pyo3::exceptions::{PyException, PyRuntimeError, PyTypeError, PyValueError};
use pyo3::types::{PyAny};

use crate::tokens::{ParamKey, ParamVal};
use crate::tokens::Tokens;
use crate::tokens::{drop_suffixes, apply_suffix_bounds};
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

    fn __str__(&self) -> String {
        self.long_name.clone()
    }

    fn __repr__(&self) -> String {
        self.long_name.clone()
    }

    fn __eq__(&self, other: &Bound<'_, PyAny>) -> bool {
        if let Ok(other_label) = other.extract::<Label>() {
            self.eq(&other_label)
        } else {
            false
        }
    }

    fn __ne__(&self, other: &Bound<'_, PyAny>) -> bool {
        if let Ok(other_label) = other.extract::<Label>() {
            if other_label.var_name.is_some() {
                self.long_name != other_label.long_name
            } else {
                self.name != other_label.name
            }
        } else {
            true
        }
    }

    fn __hash__(&self) -> i64 {
        self.hash_val
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
        Err(PyErr::new::<PyTypeError, _>(
            "Failed to extract ContentStep from python object",
        ))
    }
}

#[pyclass]
#[derive(Debug, Clone)]
pub struct Node {
    #[pyo3(get, set)]
    pub var_name: Vec<Label>,
    #[pyo3(get, set)]
    pub name: Vec<Label>,
    #[pyo3(get)]
    pub labels: Vec<Label>,
    #[pyo3(get, set)]
    pub filename: String,
    #[pyo3(get, set)]
    pub dep: Vec<Vec<Vec<Label>>>,
    pub content: Vec<ContentStep>,
    pub failed_cases: VecDeque<(Vec<Label>, Vec<ContentStep>, Vec<ContentStep>)>,
    #[pyo3(get, set)]
    pub append_to_shortname: bool,

    #[pyo3(get, set)]
    pub condition: Option<Filters>,

    #[pyo3(get, set)]
    pub default: bool,
    #[pyo3(get)]
    pub id: usize,
    children: VecDeque<usize>,
}

impl PartialEq for Node {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}
impl Default for Node {
    fn default() -> Self {
        Self::new(0)
    }
}

#[pymethods]
impl Node {
    #[pyo3(signature = (id=0))]
    #[new]
    pub fn new(id: usize) -> Self {
        Node {
            var_name: Vec::new(),
            name: Vec::new(),
            labels: Vec::new(),
            filename: String::new(),
            dep: Vec::new(),
            content: Vec::new(),
            failed_cases: VecDeque::new(),
            append_to_shortname: false,
            condition: None,
            default: false,
            id,
            children: VecDeque::new(),
        }
    }

    pub fn __eq__(&self, other: &Bound<'_, PyAny>) -> bool {
        if let Ok(other_node) = other.extract::<Node>() {
            self.id == other_node.id
        } else {
            false
        }
    }

    #[staticmethod]
    pub fn join_names(n1: &str, n2: &str) -> String {
        // find the common prefix between the two names
        let common_prefix_len = n1
            .chars()
            .zip(n2.chars())
            .take_while(|(a, b)| a == b)
            .count();
        let common_prefix = &n1[..common_prefix_len];

        // strip the last dot-separated component from the common prefix
        let p = common_prefix.rsplit_once('.').map_or("", |(before, _)| before);
        if p.is_empty() {
            format!("{}.{}", n1, n2)
        } else {
            // remove the common prefix part from both names
            let p1 = &n1[p.len()..];
            let p2 = &n2[p.len()..];
            format!("{}{}{}", p, p1, p2)
        }
    }

    pub fn update_labels(&mut self, new_labels: Vec<Label>) {
        for label in new_labels {
            if !self.labels.contains(&label) {
                self.labels.push(label);
            }
        }
    }

    pub fn prepend_child(&mut self, id: usize) {
        self.children.push_front(id);
    }

    pub fn append_child(&mut self, id: usize) {
        self.children.push_back(id);
    }

    pub fn get_content(&self) -> Vec<ContentStep> {
        self.content.clone()
    }

    pub fn add_content(&mut self, filename: String, linenum: isize, content_type: ContentType) {
        self.content.push(ContentStep { filename, linenum, content_type });
    }

    pub fn swap_content(&mut self, new_content: Vec<ContentStep>) {
        self.content = new_content;
    }

    #[pyo3(name = "process_content")]
    pub fn process_content_py(
        &mut self,
        ctx: Vec<Label>,
        labels: Vec<Label>
    ) -> PyResult<(Vec<ContentStep>, Vec<ContentStep>, Vec<ContentStep>)> {
        self.process_content(&ctx, &labels)
    }

    #[allow(clippy::type_complexity)]
    pub fn get_failed_cases(&self) -> Vec<(Vec<Label>, Vec<ContentStep>, Vec<ContentStep>)> {
        self.failed_cases.clone().into()
    }

    pub fn add_failed_case(
        &mut self,
        ctx: Vec<Label>,
        external_filters: Vec<ContentStep>,
        internal_filters: Vec<ContentStep>,
        capacity: usize,
    ) {
        self.failed_cases.push_front((ctx, external_filters, internal_filters));
        if self.failed_cases.len() > capacity {
            _ = self.failed_cases.pop_back()
        }
    }

    pub fn prioritize_failed_case(
        &mut self,
        idx: usize,
    ) {
        let failed_case = self.failed_cases.remove(idx);
        if let Some(f) = failed_case { self.failed_cases.push_front(f) }
    }

    #[pyo3(name = "failed_case_might_pass")]
    pub fn failed_case_might_pass_py(
        &self,
        idx: usize,
        ctx: Vec<Label>,
        labels: Vec<Label>,
        content: Vec<ContentStep>
    ) -> bool {
        self.failed_case_might_pass(idx, ctx, &labels, &content)
    }

    #[pyo3(signature = (indent))]
    pub fn dump(&self, indent: usize) -> String {
        let dump_lines = [
            format!("{:indent$}id: {:?}", "", self.id, indent = indent),
            format!("{:indent$}name: {:?}", "", self.name, indent = indent),
            format!("{:indent$}variable name: {:?}", "", self.var_name, indent = indent),
            format!("{:indent$}content: {:?}", "", self.content, indent = indent),
            format!("{:indent$}failed cases: {:?}", "", self.failed_cases, indent = indent),
        ];
        dump_lines.join("\n")
    }
}
impl Node {

    /*
    Process content with respect to the current context returning new
    processed content with failed regular and conditional filters.

    More details on how this works:
    1. Check that the filters in content are OK with the current
        context (ctx).
    2. Move the parts of content that are still relevant into
        new_content and unpack conditional blocks if appropriate.
        For example, if an 'only' statement fully matches ctx, it
        becomes irrelevant and is not appended to new_content.
        If a conditional block fully matches, its contents are
        unpacked into new_content.
    3. Move failed filters into failed_filters, so that next time we
        reach this node or one of its ancestors, we'll check those
        filters first.
    4. Optionally also return conditional failed filters if present.
    */
    pub fn process_content(
        &mut self,
        ctx: &Vec<Label>,
        labels: &Vec<Label>
    ) -> PyResult<(Vec<ContentStep>, Vec<ContentStep>, Vec<ContentStep>)> {
        let mut new_content: Vec<ContentStep> = Vec::new();
        let mut failed_filters: Vec<ContentStep> = Vec::new();

        for step in &self.content {
            match &step.content_type {
                // operator tokens are passed through unchanged
                ContentType::Tokens(_) | ContentType::String(_) => {
                    new_content.push(step.clone());
                }

                _ => {
                    // step is an OnlyFilter/NoFilter/Condition/NegativeCondition
                    let filter = match &step.content_type {
                        ContentType::Filters(f) => f,
                        ContentType::Node(n) => {
                            match n.condition {
                                Some(ref f) => f,
                                None => return Err(PyTypeError::new_err(
                                    format!("Empty conditional node in {:?}", step)
                                ))
                            }
                        },
                        _ => return Err(PyTypeError::new_err(
                            format!("Unexpected content type for {:?}", step.content_type)
                        )),
                    };
                    if filter.requires_action(ctx, labels) {
                        // this filter requires action now
                        match &step.content_type {
                            // node represents conditional block with its own content
                            ContentType::Node(n) => {
                                /* TODO: add optional logging
                                self._debug(
                                    "    conditional block matches:" " %r (%s:%s)",
                                    filter.line,
                                    filename,
                                    linenum,
                                )
                                */
                                let mut cond_node = n.clone();
                                // check and unpack the content inside this conditional node
                                let (cond_content,
                                    mut failed_cond_filters,
                                    deeper_failed_filters) = cond_node
                                        .process_content(ctx, labels)?;
                                new_content.extend(cond_content);
                                if !failed_cond_filters.is_empty() {
                                    // record the entire conditional step as a failing filter
                                    failed_filters.push(step.clone());
                                    failed_cond_filters.extend(deeper_failed_filters.into_iter());
                                    return Ok((new_content, failed_filters, failed_cond_filters));
                                }
                                // conditional block unpacked successfully
                                continue;
                            }
                            // plain filters (only/no) fail to apply
                            _ => {
                                /* TODO: add optional logging
                                self._debug(
                                    "    filter did not pass: %r (%s:%s)",
                                    filter.line,
                                    filename,
                                    linenum,
                                )
                                */
                                failed_filters.push(step.clone());
                                return Ok((new_content, failed_filters, Vec::new()));
                            }
                        }
                    }
                    else if filter.is_irrelevant(ctx, labels) {
                        // this filter is no longer relevant and can be removed
                        continue
                    }
                    else {
                        // keep the filter and check it again later
                        new_content.push(step.clone());
                    }
                }
            }
        }

        Ok((new_content, failed_filters, Vec::new()))
    }

    pub fn failed_case_might_pass(
        &self,
        idx: usize,
        ctx: Vec<Label>,
        labels: &[Label],
        content: &[ContentStep]
    ) -> bool {
        let node_content = &self.content;
        let all_content: Vec<&ContentStep> = content.iter().chain(node_content).collect();
        let failed_case = match self.failed_cases.get(idx) {
            Some(f) => f,
            None => { return false; }
        };
        let (failed_ctx, failed_external_filters, failed_internal_filters) = failed_case;

        // might pass if any filter (external or internal) is missing from all_content
        let in_all_content = |step: &ContentStep| all_content.contains(&step);
        if failed_external_filters.iter().any(|t| !in_all_content(t))
            || failed_internal_filters.iter().any(|t| !in_all_content(t))
        {
            return true;
        }

        // cannot pass if at least one external filter cannot pass
        for ContentStep {content_type, ..} in failed_external_filters {
            if let ContentType::Filters(external_filter) = content_type
                && !external_filter.might_pass(failed_ctx, &ctx, labels) {
                    return false;
                }
        }

        // might pass if any internal filter is missing only from the node content
        if failed_internal_filters
            .iter()
            .any(|t| !node_content.contains(t))
        {
            return true;
        }

        // cannot pass if at least one internal filter cannot pass
        for ContentStep {content_type, ..} in failed_internal_filters {
            if let ContentType::Filters(internal_filter) = content_type
                && !internal_filter.might_pass(failed_ctx, &ctx, labels) {
                    return false;
                }
        }

        true
    }
}
// TODO: this entire section must move the tree struct implementations
impl Tree {
    pub fn apply_dict(
        &mut self,
        lexer: &Lexer,
        dict: HashMap<ParamKey, ParamVal>
    ) -> PyResult<()> {
        // Build a LApplyDict from the Rust HashMap
        let content_type = ContentType::Tokens(Tokens::LApplyDict(String::new(), dict));

        // Add pre-dictionary content to this node
        self.borrow_root_mut()?.add_content(
            lexer.filename.clone(),
            lexer.linenum,
            content_type,
        );

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
        mut dict: HashMap<ParamKey, ParamVal>,
    ) -> PyResult<HashMap<ParamKey, ParamVal>> {
        // Build identifier_str
        let token_str = match token {
            Tokens::LIdentifier(s) => s.clone(),
            _ => return Err(PyValueError::new_err(
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
                    _ => return Err(PyValueError::new_err(
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
                return Err(PyValueError::new_err("Empty identifier"));
            }
        };
        let op_obj = op.like(identifier_str.clone(), value_str.clone())?;

        // If it's an LSet and value has no '$', apply directly to dict
        let d_nin_val = !value_str.contains('$');
        if matches!(op, Tokens::LSet(_,_)) && d_nin_val {
            op_obj.apply_to_dict(&mut dict)?;
        } else {
            // If dict has pending entries, either optimize or flush
            let pre_nonempty = !dict.is_empty();
            if pre_nonempty {
                // try to get op.name and check if it's present in dict
                let op_name = op_obj.name().unwrap_or_default();
                if !op_name.is_empty() && d_nin_val && dict.contains_key(&op_name.into()) {
                    // apply and consume EOL
                    op_obj.apply_to_dict(&mut dict)?;
                    lexer.get_next_token(Some(vec![lendl]), None)?;
                    return Ok(dict);
                } else {
                    // flush dict into node
                    self.apply_dict(lexer, dict)?;
                    dict = HashMap::new();
                }
            }

            // Add operator token as content
            self.borrow_root_mut()?.add_content(
                lexer.filename.clone(),
                lexer.linenum,
                ContentType::Tokens(op_obj),
            );
        }

        // consume end-of-line
        lexer.get_next_token(Some(vec![lendl]), None)?;
        Ok(dict)
    }

    /*
    Parse:
        del operand
    */
    pub fn apply_deletion(
        &mut self,
        lexer: &mut Lexer,
        dict: HashMap<ParamKey, ParamVal>,
    ) -> PyResult<()> {
        let lidentifier = Tokens::default("Identifier");
        let lendl = Tokens::default("endl");
        let to_del = lexer.get_next_token(Some(vec![lidentifier]), Some(true))?;
        // consume EOL
        lexer.get_next_token(Some(vec![lendl]), Some(true))?;
        let to_del_str: String = to_del.string()?;

        // flush dict and add token as content
        self.apply_dict(lexer, dict)?;
        self.borrow_root_mut()?.add_content(
            lexer.filename.clone(),
            lexer.linenum,
            ContentType::Tokens(Tokens::LDel(to_del_str, "".to_string())),
        );

        Ok(())
    }

    /*
    Parse:
        include relative file path to working directory.
    */
    pub fn apply_include(
        &mut self,
        lexer: &mut Lexer,
        dict: HashMap<ParamKey, ParamVal>,
    ) -> PyResult<()> {
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

        // Apply current dict and create new lexer for included file
        self.apply_dict(lexer, dict)?;

        let filepath_str = filepath.to_str()
            .ok_or_else(|| PyErr::new::<PyValueError, _>("Invalid filepath"))?;
        let mut new_lexer = Lexer::new(None, Some(filepath_str))?;

        // Parse with new lexer
        self.parse(&mut new_lexer, -1, false, None)?;
        Ok(())
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
        dict: HashMap<ParamKey, ParamVal>,
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
            lexer.line.as_deref(),
            lexer.filename.as_str(),
            lexer.linenum,
        )?;

        // Get the next line and set it in the lexer
        let next_line = lexer.get_rest_line_as_string_token()?;
        let next_line_str: String = next_line.string()?;
        if !next_line_str.is_empty() {
            lexer.set_next_line(next_line_str, (indent + 1) as usize, lexer.linenum as usize);
        }

        // Create a new Node for the condition
        let cond = self.borrow_new_node_mut()?;
        cond.condition = Some(Filters::Condition {
            filter : cfilter,
            line : lexer.line.clone().unwrap_or_default()
        });
        let cond_id = cond.id;

        // Parse the condition block
        let root_id = self.root;
        self.set_root(cond_id)?;
        self.parse(lexer, indent, false, None)?;
        self.set_root(root_id)?;

        // Apply the current dict and add the condition node as content
        self.apply_dict(lexer, dict)?;
        let cond = self.borrow_node(cond_id)?.clone();
        self.borrow_root_mut()?.add_content(
            lexer.filename.clone(),
            lexer.linenum,
            ContentType::Node(cond),
        );

        Ok(())
    }

    /*
    Parse:
        !xxx.yyy.(aaa=bbb): vvv
    */
    pub fn apply_notcondition(
        &mut self,
        lexer: &mut Lexer,
        dict: HashMap<ParamKey, ParamVal>,
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
            lexer.line.as_deref(),
            lexer.filename.as_str(),
            lexer.linenum,
        )?;

        // Get the next line and set it in the lexer
        let next_line = lexer.get_rest_line_as_string_token()?;
        let next_line_str: String = next_line.string()?;
        if !next_line_str.is_empty() {
            lexer.set_next_line(next_line_str, (indent + 1)  as usize, lexer.linenum as usize);
        }

        // Create a new Node for the negative condition
        let cond = self.borrow_new_node_mut()?;
        cond.condition = Some(Filters::NegativeCondition {
            filter : lfilter,
            line : lexer.line.clone().unwrap_or_default()
        });
        let cond_id = cond.id;

        // Parse the condition block
        let root_id = self.root;
        self.set_root(cond_id)?;
        self.parse(lexer, indent, false, None)?;
        self.set_root(root_id)?;

        // Apply the current dict and add the condition node as content
        self.apply_dict(lexer, dict)?;
        let cond = self.borrow_node(cond_id)?.clone();
        self.borrow_root_mut()?.add_content(
            lexer.filename.clone(),
            lexer.linenum,
            ContentType::Node(cond),
        );

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
        if self.borrow_root()?.condition.is_some() {
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
                return Err(PyValueError::new_err("Empty token list"));
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
                            return Err(PyValueError::new_err("Empty variants"));
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
        dict: HashMap<ParamKey, ParamVal>,
        indent: isize,
        variant_name: String,
        variant_indent: isize,
        meta: &mut HashMap<String, Vec<String>>,
        defaults : bool,
        expand_defaults : Vec<String>,
    ) -> PyResult<()> {
        let mut already_default = false;

        if !dict.is_empty() {
            self.apply_dict(lexer, dict)?;
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

        let node4_id = self.new_node()?;
        let root_id = self.root;
        let root_labels = self.borrow_root_mut()?.labels.clone();

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
                    lexer.line.as_deref(),
                    lexer.filename.as_str(),
                    lexer.linenum,
                )?;
            }

            // Create and parse the variant node
            let node2 = self.borrow_new_node_mut()?;
            node2.append_child(root_id);
            node2.update_labels(root_labels.clone());
            let node2_id = node2.id;

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
                );
            }

            self.set_root(node2_id)?;
            self.parse(lexer, indent, defaults, Some(expand_defaults.clone()))?;
            let node3 = self.borrow_root_mut()?;

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

            // Clone node3 data ultimately consumed at a later point
            let node3_id = node3.id;
            let node3_labels = node3.labels.clone();
            let node3_name = node3.name.clone();
            let node3_default = node3.default;

            // Update file mappings
            node3.add_content(
                lexer.filename.clone(),
                lexer.linenum,
                ContentType::Tokens(
                    Tokens::LUpdateFileMap(
                        lexer.filename.clone(),
                        node3_name.iter().map(|x| x.to_string()).collect::<Vec<_>>().join("."),
                        "_name_map_file".to_string(),
                    ),
                ),
            );
            node3.add_content(
                lexer.filename.clone(),
                lexer.linenum,
                ContentType::Tokens(
                    Tokens::LUpdateFileMap(
                        lexer.filename.clone(),
                        node3_name.iter().map(|x| x.name.clone()).collect::<Vec<_>>().join("."),
                        "_short_name_map_file".to_string(),
                    ),
                ),
            );

            // Update labels
            let node4 = self.borrow_node_mut(node4_id)?;
            node4.update_labels(node3_labels);
            node4.update_labels(node3_name);

            // Add node to children
            if node3_default && defaults {
                node4.prepend_child(node3_id);
            } else {
                node4.append_child(node3_id);
            }
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

        self.set_root(node4_id)?;
        Ok(())
    }

    pub fn parse(
        &mut self,
        lexer: &mut Lexer,
        prev_indent: isize,
        defaults: bool,
        expand_defaults: Option<Vec<String>>,
    ) -> PyResult<()> {
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

        // Suffix operator state
        // NOTE: Suffix should be applied as the last operator in the dictionary
        // Reasons:
        // 1. Escapes multiplying suffix operators
        // 2. Affects all elements in current block
        let mut suffix: Option<(String, isize, Tokens)> = None;

        // Dictionary contains block of operation without collision with
        // other blocks or operations which increases speed almost twice.
        let mut dict: HashMap<ParamKey, ParamVal> = HashMap::new();

        loop {
            lexer.set_prev_indent(prev_indent);

            // Handle indentation
            let token = lexer.get_next_token(
                Some(indent_allowed.to_vec()),
                None
            )?;

            if matches!(token, Tokens::LEndBlock(_)) {
                if !dict.is_empty() {
                    // Flush dict to node content
                    self.apply_dict(lexer, dict)?;
                }
                if let Some((filename, linenum, op)) = suffix {
                    // Node has suffix, apply it to all elements
                    let node = self.borrow_root_mut()?;
                    node.add_content(filename.clone(), linenum, ContentType::Tokens(op));
                }
                return Ok(());
            }

            let indent: isize = token.length()?;
            let token = lexer.get_next_token(Some(allowed.to_vec()), None)?;

            match token {
                Tokens::LInclude() => {
                    self.apply_include(lexer, dict)?;
                    dict = HashMap::new();
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
                            return Err(PyValueError::new_err("Empty identifier"));
                        }
                    };

                    if matches!(last_token, Tokens::LColon()) {
                        // Handle condition block
                        self.apply_condition(
                            identifier,
                            token,
                            lexer,
                            dict,
                            indent,
                        )?;
                        dict = HashMap::new();
                    } else if matches!(&last_token,
                        Tokens::LSet(_, _) | Tokens::LLazySet(_, _) | Tokens::LAppend(_, _) | Tokens::LPrepend(_, _) |
                        Tokens::LRegExpSet(_, _) | Tokens::LRegExpAppend(_, _) | Tokens::LRegExpPrepend(_, _)
                    ) {
                        // Handle operator
                        dict = self.apply_operator(
                            identifier,
                            token,
                            lexer,
                            dict,
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
                    self.apply_deletion(lexer, dict)?;
                    dict = HashMap::new();
                }

                Tokens::LNotCond() => {
                    self.apply_notcondition(lexer, dict, indent)?;
                    dict = HashMap::new();
                    lexer.set_prev_indent(prev_indent);
                }

                Tokens::LVariants() => {
                    let (name, meta_dict) = self.apply_variants(lexer)?;
                    variant_name = name;
                    variant_indent = indent;
                    for (key, values) in meta_dict {
                        meta.insert(key, values);
                    }
                    allowed = variants_allowed.to_vec();
                }

                Tokens::LVariant() => {
                    self.apply_variant(
                        lexer,
                        dict,
                        indent,
                        variant_name.clone(),
                        variant_indent,
                        &mut meta,
                        defaults,
                        expand_defaults.clone().unwrap_or_default(),
                    )?;
                    dict = HashMap::new();
                    allowed = block_allowed.to_vec();
                }

                Tokens::LNo() | Tokens::LOnly() | Tokens::LJoin() => {
                    // Parse:
                    //    only/no/join (filter=text)..aaa.bbb, xxxx
                    let rest_tokens: Vec<Tokens> = lexer.get_rest_line(None)?;
                    let filters: Vec<Vec<Vec<Label>>> = Filters::parse_filter(
                        rest_tokens,
                        lexer.line.as_deref(),
                        lexer.filename.as_str(),
                        lexer.linenum,
                    )?;
                    self.apply_dict(lexer, dict)?;
                    dict = HashMap::new();

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
                    let node = self.borrow_root_mut()?;
                    node.add_content(
                        lexer.filename.clone(),
                        lexer.linenum,
                        content_type,
                    );
                }

                Tokens::LSuffix() => {
                    // Parse:
                    //    suffix SUFFIX
                    if !dict.is_empty() {
                        self.apply_dict(lexer, dict)?;
                    }
                    dict = HashMap::new();
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
}

#[pyclass]
#[derive(Debug, Clone, Default)]
pub struct Tree {
    #[pyo3(get)]
    root: usize,
    nodes: Vec<Node>,
}

#[pymethods]
impl Tree {
    #[new]
    pub fn new() -> PyResult<Self> {
        let mut tree = Tree::default();
        tree.new_node()?;
        Ok(tree)
    }

    fn __copy__(&self) -> Tree {
        self.clone()
    }

    fn get_size(&self) -> usize {
        self.nodes.len()
    }

    pub fn get_node_children(&self, id: usize) -> PyResult<Vec<Node>> {
        Ok(self.borrow_node(id)?.children
            .iter()
            .filter_map(|c| self.get_node(*c))
            .cloned()
            .collect())
    }

    #[pyo3(signature = (indent))]
    pub fn dump(&self, indent: usize) -> PyResult<String> {
        let mut dump_lines = vec![
            format!("{:indent$}root: {}", "", self.root, indent = indent),
            format!("{:indent$}nodes: {}", "", self.nodes.len(), indent = indent),
        ];

        // Stack-based tree traversal: (node_id, current_indent)
        let mut stack: Vec<(usize, usize)> = Vec::new();
        stack.push((self.root, indent));

        while let Some((node_id, current_indent)) = stack.pop() {
            let node = self.borrow_node(node_id)?;
            dump_lines.push(node.dump(current_indent));

            // Add children to stack in reverse order for correct traversal
            for child_id in node.children.iter().rev() {
                stack.push((*child_id, current_indent + 3));
            }
        }

        Ok(dump_lines.join("\n"))
    }

    pub fn add_node(&mut self, node: Node) -> PyResult<()> {
        if node.id != self.get_size() {
            return Err(PyValueError::new_err(
                format!("Node ID {} not available (ID {} is next available)", node.id, self.get_size())
            ));
        }
        self.nodes.push(node);
        Ok(())
    }

    pub fn new_node(&mut self) -> PyResult<usize> {
        let node = Node::new(self.get_size());
        let id = node.id;
        self.add_node(node)?;
        Ok(id)
    }

    fn clone_node(&self, id: usize) -> PyResult<Node> {
        self.borrow_node(id).cloned()
            .map_err(|e| PyValueError::new_err(format!("Failed to clone node: {}", e)))
    }

    fn swap_node(&mut self, node: Node) -> PyResult<()> {
        if node.id >= self.get_size() {
            return Err(PyValueError::new_err(
                format!("Node ID {} not available for swapping (tree has {} nodes)", node.id, self.get_size())
            ));
        }
        let id = node.id;
        self.nodes[id] = node;
        Ok(())
    }

    pub fn set_root(&mut self, id: usize) -> PyResult<()> {
        if id >= self.get_size() {
            return Err(PyValueError::new_err(
                format!("Root node ID {} out of bounds (tree has {} nodes)", id, self.get_size())
            ));
        }
        self.root = id;
        Ok(())
    }

    #[pyo3(signature = (cfgstr, prev_indent=-1, defaults=true, expand_defaults=None))]
    pub fn parse_string(
        &mut self,
        cfgstr: String,
        prev_indent: isize,
        defaults: bool,
        expand_defaults: Option<Vec<String>>,
    ) -> PyResult<()> {
        let mut new_lexer = Lexer::new(Some(&cfgstr), None)?;
        self.borrow_root_mut()?.filename = new_lexer.filename.clone();
        self.parse(&mut new_lexer, prev_indent, defaults, expand_defaults)?;
        Ok(())
    }

    #[pyo3(signature = (cfgfile, prev_indent=-1, defaults=true, expand_defaults=None))]
    pub fn parse_file(
        &mut self,
        cfgfile: String,
        prev_indent: isize,
        defaults: bool,
        expand_defaults: Option<Vec<String>>,
    ) -> PyResult<()> {
        let mut new_lexer = Lexer::new(None, Some(&cfgfile))?;
        self.borrow_root_mut()?.filename = cfgfile;
        self.parse(&mut new_lexer, prev_indent, defaults, expand_defaults)?;
        Ok(())
    }
}
impl Tree {
    pub fn borrow_node(&self, id: usize) -> Result<&Node, PyErr> {
        if id >= self.get_size() {
            return Err(PyValueError::new_err(
                format!("Node ID {} out of bounds (tree has {} nodes)", id, self.get_size())
            ));
        }
        Ok(&self.nodes[id])
    }

    pub fn borrow_node_mut(&mut self, id: usize) -> Result<&mut Node, PyErr> {
        if id >= self.get_size() {
            return Err(PyValueError::new_err(
                format!("Node ID {} out of bounds (tree has {} nodes)", id, self.get_size())
            ));
        }
        Ok(&mut self.nodes[id])
    }

    pub fn borrow_new_node(&mut self) -> Result<&Node, PyErr> {
        let id = self.new_node()?;
        Ok(&self.nodes[id])
    }

    pub fn borrow_new_node_mut(&mut self) -> Result<&mut Node, PyErr> {
        let id = self.new_node()?;
        Ok(&mut self.nodes[id])
    }

    pub fn borrow_root(&self) -> Result<&Node, PyErr> {
        self.borrow_node(self.root)
    }

    pub fn borrow_root_mut(&mut self) -> Result<&mut Node, PyErr> {
        self.borrow_node_mut(self.root)
    }

    pub fn get_node(&self, id: usize) -> Option<&Node> {
        self.borrow_node(id).ok()
    }

    pub fn get_node_mut(&mut self, id: usize) -> Option<&mut Node> {
        self.borrow_node_mut(id).ok()
    }
}

#[pyclass(unsendable)]
#[derive(Debug, Clone)]
pub struct PreDict {
    _ctx: Vec<Vec<Label>>,
    _shortname: Vec<Vec<Label>>,

    _content: Vec<Vec<ContentStep>>,
    _ctx_content: Vec<Vec<ContentStep>>,

    _dep: Vec<Vec<String>>,

    _tree: RefCell<Tree>,

    // traversal state
    #[pyo3(get)]
    pub branch: Vec<usize>,
    #[pyo3(get)]
    pub route: Vec<Option<usize>>,
    #[pyo3(get, set)]
    pub joins: Vec<Option<Vec<ContentStep>>>,
    // TODO: refactor as we expanded to double option to complete the migration more easily
    #[allow(clippy::type_complexity)]
    #[pyo3(get, set)]
    pub join_dicts: Vec<Option<Vec<Option<HashMap<ParamKey, ParamVal>>>>>,
    #[pyo3(get, set)]
    pub join_pre_dicts: Vec<Option<Vec<Option<PreDict>>>>,

    // whether to use default variants
    #[pyo3(get, set)]
    pub defaults: bool,
    // list of default variants to expand
    #[pyo3(get, set)]
    pub expand_defaults: Vec<String>,
    // supported number of failed filters for a node
    #[pyo3(get)]
    pub num_failed_cases: usize,
}

impl Default for PreDict {
    fn default() -> Self {
        Self::new(Tree::default(), None, None, None, None, None, None)
    }
}

#[pymethods]
impl PreDict {
    #[getter]
    fn ctx(&self) -> Vec<Label> {
        self._ctx.iter().flatten().cloned().collect()
    }

    #[getter]
    fn shortname(&self) -> Vec<Label> {
        self._shortname.iter().flatten().cloned().collect()
    }

    #[getter]
    fn content(&self) -> Vec<ContentStep> {
        self._content.iter().flatten().cloned().collect()
    }

    #[getter]
    fn final_content(&self) -> Vec<ContentStep> {
        let mut res: Vec<ContentStep> = Vec::new();
        if let Some(last) = self._content.last() {
            res.extend(last.clone());
        }
        if let Some(last) = self._ctx_content.last() {
            res.extend(last.clone());
        }
        res
    }

    #[getter]
    fn dep(&self) -> Vec<String> {
        self._dep.iter().flatten().cloned().collect()
    }

    #[new]
    #[pyo3(signature = (tree, ctx=None, content=None, shortname=None, dep=None, defaults=None, expand_defaults=None))]
    fn new(
        tree: Tree,
        ctx: Option<Vec<Label>>,
        content: Option<Vec<ContentStep>>,
        shortname: Option<Vec<Label>>,
        dep: Option<Vec<String>>,
        defaults: Option<bool>,
        expand_defaults: Option<Vec<String>>,
    ) -> Self {
        let _ctx = vec![ctx.unwrap_or_default()];
        let _content = vec![content.unwrap_or_default()];
        let _ctx_content = vec![Vec::new()];
        let _shortname = vec![shortname.unwrap_or_default()];
        let _dep = vec![dep.unwrap_or_default()];

        PreDict {
            num_failed_cases: 5,
            _ctx,
            _shortname,
            _content,
            _ctx_content,
            _dep,
            _tree: RefCell::new(tree),
            branch: Vec::new(),
            route: Vec::new(),
            joins: Vec::new(),
            join_dicts: Vec::new(),
            join_pre_dicts: Vec::new(),
            defaults: defaults.unwrap_or(false),
            expand_defaults: expand_defaults.unwrap_or_default(),
        }
    }

    fn __str__(&self) -> String {
        let ctx = self.ctx();
        let content = self.content();
        let shortname = self.shortname();
        let dep = self.dep();
        format!(
            "PreDict(ctx={:?}, content={:?}, shortname={:?}, dep={:?})",
            ctx, content, shortname, dep
        )
    }

    fn __copy__(&self) -> PreDict {
        self.clone()
    }

    fn shallow_copy(&self) -> PreDict {
        let ctx = self.ctx();
        let content = self._content.last().cloned().unwrap_or_default();
        let ctx_content = self._ctx_content.last().cloned().unwrap_or_default();
        let shortname = self.shortname();
        let dep = self.dep();
        let mut new = PreDict::new(
            Tree::default(),
            Some(ctx),
            Some(content),
            Some(shortname),
            Some(dep),
            Some(self.defaults),
            Some(self.expand_defaults.clone()),
        );
        new._tree = self._tree.clone();
        new._ctx_content[0] = ctx_content;
        new
    }

    #[pyo3(signature = (node))]
    pub fn update_from_node(&mut self, mut node: Node) -> PyResult<bool> {
        /* TODO: add optional logging
        if self.debug:    #Print dict on which is working now.
            print(node.dump(0))
        */

        let ctx = node.name.clone();
        let labels = node.labels.clone();
        let shortname = if node.append_to_shortname {
            node.name.clone()
        } else {
            Vec::new()
        };

        // build dep strings using current flattened ctx and node.dep
        let ctx_flat: Vec<Label> = self.ctx();
        let mut dep: Vec<String> = Vec::new();
        for d in &node.dep {
            for dd in d {
                let mut parts: Vec<String> = Vec::new();
                parts.extend(ctx_flat.iter().map(|label| label.to_string()));
                parts.extend(dd.iter().map(|label| label.to_string()));
                dep.push(parts.join("."));
            }
        }

        /* TODO: add optional logging
        if node.name:
            self._debug("checking out %r", name)
        */
        
        // check previously failed filters
        for i in 0..node.failed_cases.len() {
            let mut probe_ctx = ctx_flat.clone();
            probe_ctx.extend(ctx.clone());
            if !node.failed_case_might_pass(i, probe_ctx, &labels, &self.content()) {
                /* TODO: add optional logging
                self._debug(
                    "\n*    this subtree has failed before %s\n"
                    "         content: %s\n"
                    "         failcase:%s\n",
                    name,
                    self.content + node.get_content(),
                    failed_case,
                )
                */
                node.prioritize_failed_case(i);
                return Ok(false);
            }
        }

        self._ctx.push(ctx);
        self._shortname.push(shortname);
        self._dep.push(dep);

        // push state machine stacks
        self.route.push(None);
        self.joins.push(None);
        self.join_dicts.push(None);
        self.join_pre_dicts.push(None);

        // recompute flattened ctx (includes the newly added ctx)
        let ctx_flat = self.ctx();
        // capture external content (final content) before node is pushed
        let content = self.final_content();

        // process internal content for the node
        let (internal_content, mut failed_internal, mut failed_internal_cond) =
            node.process_content(&ctx_flat, &labels)?;
        failed_internal.append(&mut failed_internal_cond);
        self._content.push(internal_content);

        // process external (previous) content against current context
        let mut content_node = self._tree.borrow_mut()
            .borrow_new_node_mut()
            .map(|node| node.clone())?;
        content_node.swap_content(content);
        let (external_content, failed_external, mut failed_external_cond) =
            content_node.process_content(&ctx_flat, &labels)?;
        // NOTE: the failed filters should go into the failed internal filters
        // because we don't expect them to come from outside this node, even if
        // the condition itself was external
        failed_internal.append(&mut failed_external_cond);
        self._ctx_content.push(external_content);

        // register failed case and return false if failed filters
        if !failed_internal.is_empty() || !failed_external.is_empty() {
            node.add_failed_case(ctx_flat.clone(), failed_external, failed_internal, self.num_failed_cases);
            /* TODO: add optional logging
            self._debug("Failed_cases %s", node.failed_cases)
            */
            self.branch.push(node.id);
            // TODO: oddly as in other hanging cases we cannot swap the node in case of success
            //self._tree.borrow_mut().swap_node(node.clone())?;
            return Ok(false);
        }

        self.branch.push(node.id);
        // TODO: oddly as in other hanging cases we cannot swap the node in case of success
        //self._tree.borrow_mut().swap_node(node.clone())?;
        Ok(true)
    }

    fn reset_from_last_node(&mut self) {
        self.branch.pop();
        self.route.pop();
        self.joins.pop();
        self.join_dicts.pop();
        self.join_pre_dicts.pop();

        self._ctx.pop();
        self._ctx_content.pop();
        self._content.pop();
        self._shortname.pop();
        self._dep.pop();
    }

    pub fn get_dict(&self) -> PyResult<HashMap<ParamKey, ParamVal>> {
        let mut dict: HashMap<ParamKey, ParamVal> = HashMap::new();

        let name = self.ctx().iter().map(|l| l.long_name.clone()).collect::<Vec<_>>().join(".");
        let shortname = self.shortname().iter().map(|l| l.name.clone()).collect::<Vec<_>>().join(".");
        let dep = self.dep();

        dict.insert("name".to_string().into(), name.into());
        dict.insert("shortname".to_string().into(), shortname.into());
        dict.insert("dep".to_string().into(), ParamVal::List(dep));

        for step in self.final_content() {
            match step.content_type {
                ContentType::Tokens(t) => {
                    // ignore inapplicable token types by ignoring the status
                    let _ = t.apply_to_dict(&mut dict);
                }
                _ => {
                    return Err(PyErr::new::<PyRuntimeError, _>("Unexpected content type"));
                }
            }
        }
        Ok(dict)
    }

    /*
    Generate dictionaries from the pre-dict parsed so far.

    This should be called after parsing something or seeding the
    pre-dict with an initial node.
    */
    pub fn get_dicts_plain(&mut self) -> PyResult<Option<HashMap<ParamKey, ParamVal>>> {
        if self.branch.is_empty() {
            return Err(PyErr::new::<PyRuntimeError, _>("Pre-dictionary needs at least one node"));
        }
        let mut depth = (self.branch.len() - 1) as isize;

        // recurse into children
        loop {
            if depth < 0 {
                break;
            }
            let i = depth as usize;
            let var_name = self._tree.borrow().borrow_node(self.branch[i])?.var_name.clone();
            let children = self._tree.borrow().borrow_node(self.branch[i])?.children.clone();

            if self.route[i].is_none() {
                // start with 0th child
                self.route[i] = Some(0);

                // reached leaf
                if children.is_empty() {
                    /* TODO: add optional logging
                    self._debug("    reached leaf, returning it")
                    */
                    let mut d = self.get_dict()?;
                    apply_suffix_bounds(&mut d);
                    return Ok(Some(d));
                }
            // one for leaf down from final index
            } else if i + 1 == self.route.len().saturating_sub(1) {
                // move to next child
                if let Some(ref mut r) = self.route[i] {
                    *r += 1;
                }
                // remove all previous grand children and their effects on pre-dict
                for _ in i + 1..self.route.len() {
                    self.reset_from_last_node();
                }
            }

            // if children pool exhausted or still no route
            let route_idx = match self.route[i] {
                Some(i) => i,
                None => {
                    depth -= 1;
                    continue;
                }
            };
            if route_idx + 1 > children.len() {
                depth -= 1;
                continue;
            }

            // the original parsed node is preserved as the pre-dict modifies a clone
            // for the purpose of traversal and dictionary getters
            let child = self._tree.borrow().clone_node(children[route_idx])?;
            if !self.update_from_node(child)? {
                continue;
            }
            if self.defaults {
                let var_name_str = var_name.iter()
                    .map(|l| l.to_string())
                    .collect::<Vec<_>>().join(".");
                let has_default_child = children.iter()
                    .any(|c| self._tree.borrow().get_node(*c)
                    .map(|n| n.default)
                    .unwrap_or(false));
                let is_current_child_default = self._tree.borrow()
                    .borrow_node(children[route_idx])
                    .map(|n| n.default)
                    .unwrap_or(false);
                if !self.expand_defaults.contains(&var_name_str)
                    && has_default_child
                    && !is_current_child_default
                {
                    return Ok(None);
                }
            }
            let d = self.get_dicts(false, true)?;
            // completed children recursion is consumed until we run out of children
            if d.is_none() {
                // handle earlier reset of the same pre-dict by a nested getter
                depth = min(depth, (self.branch.len() - 1) as isize);
                continue;
            }
            return Ok(d);
        }
        Ok(None)
    }

    /*
    Perform all joins as filters on added dictionaries.

    Each `join' is the same as an `only' filter.
    */
    pub fn get_dicts_joined(&mut self) -> PyResult<Option<HashMap<ParamKey, ParamVal>>> {
        if self.branch.is_empty() {
            return Err(PyErr::new::<PyRuntimeError, _>("Pre-dictionary needs at least one node"));
        }
        let depth = self.branch.len() - 1;
        let mut sub_pre_dict = self.clone();
        sub_pre_dict.reset_from_last_node();

        // TODO: this doesn't panic on out of bounds or uninitialized joins but is bulky to use
        // also in all other vector depth or width access cases - use anyhow or find a better way 
        let joins = self.joins
            .get_mut(depth)
            .ok_or_else(|| PyErr::new::<PyValueError, _>(format!("Joins index out of bounds: {depth}")))?
            .as_mut()
            .ok_or_else(|| PyErr::new::<PyValueError, _>(format!("Joins not initialized at depth {depth}")))?;
        let dicts = self.join_dicts
            .get_mut(depth)
            .ok_or_else(|| PyErr::new::<PyValueError, _>(format!("Join dicts index out of bounds: {depth}")))?
            .as_mut()
            .ok_or_else(|| PyErr::new::<PyValueError, _>(format!("Join dicts not initialized at depth {depth}")))?;
        let pre_dicts = self.join_pre_dicts
            .get_mut(depth)
            .ok_or_else(|| PyErr::new::<PyValueError, _>(format!("Join pre-dicts index out of bounds: {depth}")))?
            .as_mut()
            .ok_or_else(|| PyErr::new::<PyValueError, _>(format!("Join pre-dicts not initialized at depth {depth}")))?;

        // join requires greedy dictionary expansion for variants of the same node
        let mut width: isize = 0;
        loop {
            if width < 0 {
                break;
            }
            let j = width as usize;
            if j < dicts.len().saturating_sub(1) && let Some(_) = &dicts[j + 1] {
                width += 1;
                continue;
            }

            // initialize join pre-dict with a shallow copy of a node-reset pre-dict clone
            if pre_dicts[j].is_none() {
                pre_dicts[j] = Some(sub_pre_dict.shallow_copy());
            }
            // update the pre-dict with differently filtered current node
            if let Some(ref mut pre_dict) = pre_dicts[j] {
                let tree = self._tree.borrow();
                if !pre_dict.branch.contains(&self.branch[depth]) {
                    // current join/only
                    let step = &joins[j];
                    let mut node = tree.clone_node(self.branch[depth])?;
                    node.add_content(step.filename.clone(), step.linenum, step.content_type.clone());
                    if !pre_dict.update_from_node(node)? {
                        return Ok(None);
                    }
                }
                // compute dict for this width
                let d = pre_dict.get_dicts(false, true)?;
                dicts[j] = d;
            }

            if dicts[j].is_none() {
                // remove all previous grand children and their effects on current pre-dict clone
                if let Some(ref mut pre_dict) = pre_dicts[j] {
                    for _ in (pre_dict.route.len().saturating_sub(1))..pre_dict.route.len() {
                        pre_dict.reset_from_last_node();
                    }
                }
                width -= 1;
                continue;
            }

            // multiply current frame dict by all variants from before
            if j == dicts.len() - 1 {
                let mut d: HashMap<ParamKey, ParamVal> = HashMap::new();
                let mut name = String::new();
                let mut shortname = String::new();
                for di in dicts.iter().flatten() {
                    if name.is_empty() {
                        name = di.get(&"name".to_string().into()).map(|v| Cow::from(v).into()).unwrap_or_default();
                        shortname = di.get(&"shortname".to_string().into()).map(|v| Cow::from(v).into()).unwrap_or_default();
                    } else {
                        let other_name: String = di.get(&"name".to_string().into()).map(|v| Cow::from(v).into()).unwrap_or_default();
                        let other_short: String = di.get(&"shortname".to_string().into()).map(|v| Cow::from(v).into()).unwrap_or_default();
                        name = Node::join_names(&name, &other_name);
                        shortname = Node::join_names(&shortname, &other_short);
                    }
                    // update combined map d with di entries
                    for (k, v) in di {
                        d.insert(k.clone(), v.clone());
                    }
                }
                d.insert("name".to_string().into(), name.into());
                d.insert("shortname".to_string().into(), shortname.into());
                return Ok(Some(d));
            }

            width += 1;
        }

        Ok(None)
    }

    /*
    Get possibly joined dictionaries added using only filters.

    Process 'join' entries and unpack join filters in the node.

    Main rules for joining via filters:

    1) join filter_1 filter_2 ....
        multiplies all dictionaries as:
            all_variants_match_filter_1 * all_variants_match_filter_2 * ....
    2) join only_one_filter
            == only only_one_filter
    3) join filter_1 filter_1
        also works and transforms to:
            all_variants_match_filter_1 * all_variants_match_filter_1
        Example:
            join a
            join a
        Transforms into:
            join a a
    */
    #[pyo3(signature = (dropsufs=false, skipdups=true))]
    pub fn get_dicts(&mut self, dropsufs: bool, skipdups: bool) -> PyResult<Option<HashMap<ParamKey, ParamVal>>> {
        if self.branch.is_empty() {
            return Err(PyErr::new::<PyRuntimeError, _>("Pre-dictionary needs at least one node"));
        }
        let depth = self.branch.len() - 1;
        let mut joins = &mut self.joins[depth];
        // due to pre-dict cloning current pre-dict must only contain one join at the end
        if joins.is_none() {
            let mut tree = self._tree.borrow_mut();
            let node = tree.borrow_node_mut(self.branch[depth])?;

            // find joins from node content and prepare only-filters
            let mut plain_content: Vec<ContentStep> = Vec::with_capacity(node.content.len());
            let mut new_joins: Vec<ContentStep> = Vec::with_capacity(node.content.len());
            for t in node.get_content() {
                match t.content_type {
                    ContentType::Filters(Filters::JoinFilter {filter, line }) => {
                        // accumulate join steps
                        new_joins.push(ContentStep {
                            filename: t.filename.clone(),
                            linenum: t.linenum,
                            content_type: ContentType::Filters(
                                Filters::JoinFilter { filter, line }
                            )
                        });
                    }
                    _ => plain_content.push(t),
                }
            }

            // rewrite joins from the node into many 'only' filters
            let mut onlys: Vec<ContentStep> = Vec::new();
            for j in &new_joins {
                if let ContentType::Filters(Filters::JoinFilter { filter, line }) = j.content_type.clone() {
                    for word in filter {
                        let f = Filters::OnlyFilter { filter: vec![word], line: line.clone() };
                        onlys.push(ContentStep { filename: j.filename.clone(), linenum: j.linenum, content_type: ContentType::Filters(f) });
                    }
                }
            }

            if !new_joins.is_empty() {
                // register join recursion as leaf for current pre-dict and continue with copies
                self.joins[depth] = Some(onlys.clone());
                self.join_dicts[depth] = Some(vec![None; onlys.len()]);
                self.join_pre_dicts[depth] = Some(vec![None; onlys.len()]);
                // provide join-free content to processed node
                node.swap_content(plain_content);
                joins = &mut self.joins[depth];
            }
        }

        let mut dn: Option<HashMap<ParamKey, ParamVal>> = None;
        if joins.is_some() {
            dn = self.get_dicts_joined()?;
            if dn.is_none() {
                // consume all children for this node
                let node_children_len = self._tree.borrow()
                    .borrow_node(self.branch[depth])
                    .map(|n| n.children.len())?;
                self.route[depth] = Some(node_children_len);
            }
        }
        if dn.is_none() {
            dn = self.get_dicts_plain()?;
        }
        if dropsufs && let Some(d) = dn {
            return Ok(Some(drop_suffixes(&d, skipdups)?));
        }
        Ok(dn)
    }

}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_apply_dict() {
        let mut tree = Tree::new().unwrap();
        let lexer = Lexer::new(Some(""), None).expect("Failed to create lexer");
        let mut dict: HashMap<ParamKey, ParamVal> = HashMap::new();
        dict.insert("key".to_string().into(), "value".to_string().into());

        // apply_dict should add an LApplyDict content step and clear dict
        tree.apply_dict(&lexer, dict).unwrap();
        let content = tree.borrow_root().unwrap().get_content();
        assert_eq!(content.len(), 1);

        // content type should be Tokens::LApplyDict and contains our key/value
        match &content[0].content_type {
            ContentType::Tokens(Tokens::LApplyDict(_, map)) => {
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

        let mut tree = Tree::new().unwrap();
        let mut dict: HashMap<ParamKey, ParamVal> = HashMap::new();
        dict.insert("key1".to_string().into(), "value1".to_string().into());

        // apply_include should parse the included file and return a node with a child named "test"
        tree.apply_include(&mut lexer, dict).expect("apply_include failed");
        let children = tree.get_node_children(tree.root).unwrap();
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

        let mut tree = Tree::new().unwrap();
        let mut dict: HashMap<ParamKey, ParamVal> = HashMap::new();
        dict.insert("key1".to_string().into(), "value1".to_string().into());

        // apply_operator should add key2 to dict directly (optimized path)
        dict = tree.apply_operator(identifier, token, &mut lexer, dict).expect("apply_operator failed");
        // dict now should contain both key1 and key2, and node content should be empty
        assert_eq!(dict.get(&"key1".to_string().into()), Some(&"value1".to_string().into()));
        assert_eq!(dict.get(&"key2".to_string().into()), Some(&"value2".to_string().into()));
        let content = tree.borrow_root().unwrap().get_content();
        assert_eq!(content.len(), 0);
    }

    #[test]
    fn test_apply_operator_append_safe() {
        // content: "key1 += &value2"
        let mut lexer = Lexer::new(Some("key1 += &value2"), None).expect("Failed to create lexer");
        let _ = lexer.get_next_token(Some(vec![Tokens::default("indent")]), None).expect("indent token");
        let token = lexer.get_next_token(Some(vec![Tokens::default("Identifier")]), None).expect("identifier token");
        let identifier = lexer.get_until(vec![Tokens::default("+=")], None, Some(true)).expect("get_until identifier");

        let mut tree = Tree::new().unwrap();
        let mut dict: HashMap<ParamKey, ParamVal> = HashMap::new();
        dict.insert("key1".to_string().into(), "value1".to_string().into());

        dict = tree.apply_operator(identifier, token, &mut lexer, dict).expect("apply_operator failed");

        // dict should be updated
        assert_eq!(dict.get(&"key1".to_string().into()), Some(&"value1&value2".to_string().into()));
        // node content should be empty
        let content = tree.borrow_root().unwrap().get_content();
        assert_eq!(content.len(), 0);
    }

    #[test]
    fn test_apply_operator_append_unsafe() {
        // content: "key2 += &value2" (key2 not present in dict therefore flush)
        let mut lexer = Lexer::new(Some("key2 += &value2"), None).expect("Failed to create lexer");
        let _ = lexer.get_next_token(Some(vec![Tokens::default("indent")]), None).expect("indent token");
        let token = lexer.get_next_token(Some(vec![Tokens::default("Identifier")]), None).expect("identifier token");
        let identifier = lexer.get_until(vec![Tokens::default("+=")], None, Some(true)).expect("get_until identifier");

        let mut tree = Tree::new().unwrap();
        let mut dict: HashMap<ParamKey, ParamVal> = HashMap::new();
        dict.insert("key1".to_string().into(), "value1".to_string().into());

        tree.apply_operator(identifier, token, &mut lexer, dict).expect("apply_operator failed");

        // node content should be extended with append operation step
        let content = tree.borrow_root().unwrap().get_content();
        assert_eq!(content.len(), 2);
        // first should be an LApplyDict, second an LAppend
        match &content[0].content_type {
            ContentType::Tokens(Tokens::LApplyDict(_, map)) => {
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

        let mut tree = Tree::new().unwrap();
        let mut dict: HashMap<ParamKey, ParamVal> = HashMap::new();
        dict.insert("key1".to_string().into(), "value1".to_string().into());

        tree.apply_deletion(&mut lexer, dict).expect("apply_deletion failed");

        // node content should be extended with delete operation step
        let content = tree.borrow_root().unwrap().get_content();
        assert_eq!(content.len(), 2);
        match &content[0].content_type {
            ContentType::Tokens(Tokens::LApplyDict(_, map)) => {
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

        let mut tree = Tree::new().unwrap();
        let mut dict: HashMap<ParamKey, ParamVal> = HashMap::new();
        dict.insert("key1".to_string().into(), "value1".to_string().into());

        tree.apply_condition(identifier, token, &mut lexer, dict, 0).expect("apply_condition failed");

        let content = tree.borrow_root().unwrap().get_content();
        assert_eq!(content.len(), 2);
        // first should be LApplyDict, second a Node with a positive condition
        match &content[0].content_type {
            ContentType::Tokens(Tokens::LApplyDict(_, _)) => {}
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

        let mut tree = Tree::new().unwrap();
        let mut dict: HashMap<ParamKey, ParamVal> = HashMap::new();
        dict.insert("key1".to_string().into(), "value1".to_string().into());

        tree.apply_notcondition(&mut lexer, dict, 0).expect("apply_notcondition failed");

        let content = tree.borrow_root().unwrap().get_content();
        assert_eq!(content.len(), 2);
        // first should be LApplyDict, second a Node with a negative condition
        match &content[0].content_type {
            ContentType::Tokens(Tokens::LApplyDict(_, _)) => {}
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

        let tree = Tree::new().unwrap();
        let (variant_name, meta) = tree.apply_variants(&mut lexer).expect("apply_variants failed");

        // variant name should be "test" and meta empty
        assert_eq!(variant_name, "test");
        assert!(meta.is_empty());
        // content should be empty
        let content = tree.borrow_root().unwrap().get_content();
        assert_eq!(content.len(), 0);
    }

    #[test]
    fn test_apply_variants_meta() {
        // content: "variants test [meta1] [meta2=val2] [ meta3 ] [ meta4 = val4 val5 ]:"
        let txt = r#"variants test [meta1] [meta2=val2] [ meta3 ] [ meta4 = val4 val5 ]:"#;
        let mut lexer = Lexer::new(Some(txt), None).expect("Failed to create lexer");
        let _ = lexer.get_next_token(Some(vec![Tokens::default("indent")]), None).expect("indent");
        let _ = lexer.get_next_token(Some(vec![Tokens::default("variants")]), None).expect("variants");

        let tree = Tree::new().unwrap();
        let (variant_name, meta) = tree.apply_variants(&mut lexer).expect("apply_variants failed");

        // variant name should be "test"
        assert_eq!(variant_name, "test");
        // meta contents should be parsed appropriately
        assert_eq!(meta.get("meta1").map(|v| v.as_slice()), Some(&["true".to_string()][..]));
        assert_eq!(meta.get("meta2").map(|v| v.as_slice()), Some(&["val2".to_string()][..]));
        assert_eq!(meta.get("meta3").map(|v| v.as_slice()), Some(&["true".to_string()][..]));
        assert_eq!(meta.get("meta4").map(|v| v.as_slice()), Some(&["val4 val5".to_string()][..]));
        // content should be empty
        let content = tree.borrow_root().unwrap().get_content();
        assert_eq!(content.len(), 0);
    }

    #[test]
    fn test_apply_variant() {
        // content: "- test:"
        let mut lexer = Lexer::new(Some("- test:"), None).expect("Failed to create lexer");
        let _ = lexer.get_next_token(Some(vec![Tokens::default("indent")]), None).expect("indent");
        let _ = lexer.get_next_token(Some(vec![Tokens::default("-")]), None).expect("dash");

        let mut tree = Tree::new().unwrap();
        let mut dict: HashMap<ParamKey, ParamVal> = HashMap::new();
        dict.insert("key1".to_string().into(), "value1".to_string().into());
        let mut meta = HashMap::new();
        let root_id = tree.root;

        tree.apply_variant(
            &mut lexer,
            dict,
            0,
            "test".to_string(),
            0,
            &mut meta,
            false,
            Vec::new(),
        ).expect("apply_variant failed");

        // original node should receive the flushed dict content
        let node = tree.clone_node(root_id).unwrap();
        let content = node.get_content();
        assert!(!content.is_empty());
        match &content[0].content_type {
            ContentType::Tokens(Tokens::LApplyDict(_, map)) => {
                assert_eq!(map.get(&"key1".to_string().into()), Some(&"value1".to_string().into()));
            }
            other => panic!("Unexpected parent content: {:?}", other),
        }

        // grandparent node should have one child (the variant) whose name is "test"
        let parents = tree.get_node_children(tree.root).unwrap();
        assert_eq!(parents.len(), 1);
        let parent_node = &parents[0];
        assert_eq!(parent_node.name.len(), 1);
        assert!(matches!(parent_node.name[0], Label { .. }));
        assert_eq!(parent_node.name[0].name, "test".to_string());

        // child should include the original grand child as its child
        let children = tree.get_node_children(parent_node.id).unwrap();
        assert_eq!(children.len(), 1);
        assert_eq!(children[0], node);
    }
}
