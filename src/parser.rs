use std::borrow::Cow;
use std::collections::VecDeque;
use hashbrown::HashMap;
use std::hash::Hash;
use std::sync::atomic::{AtomicU64, Ordering};
use std::fmt::{Debug, Display};
use std::rc::Rc;

use crate::error::{Error, Result};

#[cfg(feature = "python")]
use pyo3::prelude::*;
#[cfg(feature = "python")]
use pyo3::exceptions::{PyException, PyTypeError};
#[cfg(feature = "python")]
use pyo3::types::{PyAny};

use crate::tokens::{ParamKey, ParamVal};
use crate::tokens::ParamKeyHashMapExt;
use crate::tokens::{TokenKind, Tokens};
use crate::tokens::{drop_suffixes, apply_suffix_bounds};
use crate::filters::Filters;
use crate::lexer::Lexer;
use crate::lexer::LexerError;

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

#[cfg_attr(feature = "python", cfg_eval::cfg_eval)]
#[cfg_attr(feature = "python", pyclass(extends=PyException))]
#[derive(Debug)]
pub struct ParserError {
    pub(crate) lexer_error: LexerError,
}
#[cfg_attr(feature = "python", cfg_eval::cfg_eval)]
#[cfg_attr(feature = "python", pymethods)]
impl ParserError {
    #[cfg_attr(feature = "python", new)]
    #[cfg_attr(feature = "python", pyo3(signature = (msg, line=None, filename=None, linenum=None)))]
    pub(crate) fn new(msg: String, line: Option<String>, filename: Option<String>, linenum: Option<usize>) -> Self {
        Self { lexer_error: LexerError::new(msg, line, filename, linenum) }
    }

    fn __str__(&self) -> String {
        self.lexer_error.__str__()
    }
}

impl Display for ParserError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self.lexer_error, f)
    }
}

impl std::error::Error for ParserError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.lexer_error)
    }
}

#[cfg_attr(feature = "python", cfg_eval::cfg_eval)]
#[cfg_attr(feature = "python", pyclass(from_py_object))]
#[derive(Clone, Eq)]
pub struct Label {
    #[cfg_attr(feature = "python", pyo3(get, set))]
    pub name: String,
    #[cfg_attr(feature = "python", pyo3(get, set))]
    pub var_name: Option<String>,
    #[cfg_attr(feature = "python", pyo3(get, set))]
    pub long_name: String,
    #[cfg_attr(feature = "python", pyo3(get, set))]
    pub hash_val: i64,
    #[cfg_attr(feature = "python", pyo3(get, set))]
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

#[cfg_attr(feature = "python", cfg_eval::cfg_eval)]
#[cfg_attr(feature = "python", pymethods)]
impl Label {
    #[cfg_attr(feature = "python", new)]
    #[cfg_attr(feature = "python", pyo3(signature = (name, next_name=None)))]
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

    #[cfg(feature = "python")]
    fn __eq__(&self, other: &Bound<'_, PyAny>) -> bool {
        if let Ok(other_label) = other.extract::<Label>() {
            self.eq(&other_label)
        } else {
            false
        }
    }

    #[cfg(feature = "python")]
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

    #[cfg_attr(feature = "python", staticmethod)]
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
    Node(Rc<Node>),
    String(String),
}
#[cfg(feature = "python")]
impl<'py> IntoPyObject<'py> for ContentType {
    type Target = PyAny;
    type Output = Bound<'py, Self::Target>;
    type Error = PyErr;

    fn into_pyobject(self, py: Python<'py>) -> std::result::Result<Self::Output, Self::Error> {
        use pyo3::IntoPyObjectExt;

        match self {
            ContentType::Tokens(s) => s.into_bound_py_any(py),
            ContentType::Filters(s) => s.into_bound_py_any(py),
            ContentType::Node(s) => s.as_ref().clone().into_bound_py_any(py),
            ContentType::String(s) => s.into_bound_py_any(py),
        }
    }
}
#[cfg(feature = "python")]
impl<'py> FromPyObject<'_, 'py> for ContentType {
    type Error = PyErr;

    fn extract(object: Borrowed<'_, 'py, PyAny>) -> std::result::Result<Self, Self::Error> {
        if let Ok(tokens) = object.extract::<Tokens>() {
            Ok(ContentType::Tokens(tokens))
        }
        else if let Ok(filters) = object.extract::<Filters>() {
            Ok(ContentType::Filters(filters))
        }
        else if let Ok(node) = object.extract::<Node>() {
            Ok(ContentType::Node(Rc::new(node)))
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
    linenum: Option<usize>,
    content_type: ContentType,
}
#[cfg(feature = "python")]
impl<'py> IntoPyObject<'py> for ContentStep {
    type Target = PyAny;
    type Output = Bound<'py, Self::Target>;
    type Error = PyErr;

    fn into_pyobject(self, py: Python<'py>) -> std::result::Result<Self::Output, Self::Error> {
        use pyo3::IntoPyObjectExt;

        (self.filename, self.linenum, self.content_type).into_bound_py_any(py)
    }
}
#[cfg(feature = "python")]
impl<'py> FromPyObject<'_, 'py> for ContentStep {
    type Error = PyErr;

    fn extract(object: Borrowed<'_, 'py, PyAny>) -> std::result::Result<Self, Self::Error> {
        if let Ok((filename, linenum, content_type)) = object.extract::<(String, Option<usize>, ContentType)>() {
            return Ok(ContentStep { filename, linenum, content_type });
        }
        Err(PyErr::new::<PyTypeError, _>(
            "Failed to extract ContentStep from python object",
        ))
    }
}

#[cfg_attr(feature = "python", cfg_eval::cfg_eval)]
#[cfg_attr(feature = "python", pyclass(from_py_object,unsendable))]
#[derive(Debug, Clone)]
pub struct Node {
    #[cfg_attr(feature = "python", pyo3(get, set))]
    pub var_name: Vec<Label>,
    #[cfg_attr(feature = "python", pyo3(get, set))]
    pub name: Vec<Label>,
    #[cfg_attr(feature = "python", pyo3(get))]
    pub labels: Vec<Label>,
    #[cfg_attr(feature = "python", pyo3(get, set))]
    pub filename: String,
    #[cfg_attr(feature = "python", pyo3(get, set))]
    pub dep: Vec<Vec<Vec<Label>>>,
    pub content: Vec<ContentStep>,
    #[cfg_attr(feature = "python", pyo3(get, set))]
    pub append_to_shortname: bool,

    #[cfg_attr(feature = "python", pyo3(get, set))]
    pub condition: Option<Filters>,

    #[cfg_attr(feature = "python", pyo3(get, set))]
    pub default: bool,
    #[cfg_attr(feature = "python", pyo3(get))]
    pub id: u64,
    children: VecDeque<Rc<Node>>,
}

impl PartialEq for Node {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}
impl Default for Node {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg_attr(feature = "python", cfg_eval::cfg_eval)]
#[cfg_attr(feature = "python", pymethods)]
impl Node {
    #[cfg_attr(feature = "python", new)]
    pub fn new() -> Self {
        Node {
            var_name: Vec::new(),
            name: Vec::new(),
            labels: Vec::new(),
            filename: String::new(),
            dep: Vec::new(),
            content: Vec::new(),
            append_to_shortname: false,
            condition: None,
            default: false,
            id: NEXT_ID.fetch_add(1, Ordering::Relaxed),
            children: VecDeque::new(),
        }
    }

    #[cfg(feature = "python")]
    pub fn __eq__(&self, other: &Bound<'_, PyAny>) -> bool {
        if let Ok(other_node) = other.extract::<Node>() {
            self.id == other_node.id
        } else {
            false
        }
    }

    #[cfg_attr(feature = "python", staticmethod)]
    pub fn join_names(n1: &str, n2: &str) -> String {
        // find the common prefix between the two names
        let common_prefix_len = n1
            .chars()
            .zip(n2.chars())
            .take_while(|(a, b)| a == b)
            .map(|(c, _)| c.len_utf8())
            .sum();
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

    pub fn get_children(&self) -> Vec<Node> {
        self.children.iter().map(|child| child.as_ref().clone()).collect()
    }

    pub fn prepend_child(&mut self, node: Node) {
        self.children.push_front(Rc::new(node));
    }

    pub fn append_child(&mut self, node: Node) {
        self.children.push_back(Rc::new(node));
    }

    pub fn get_content(&self) -> Vec<ContentStep> {
        self.content.clone()
    }

    pub fn add_content(&mut self, filename: String, linenum: Option<usize>, content_type: ContentType) {
        self.content.push(ContentStep { filename, linenum, content_type });
    }

    pub fn swap_content(&mut self, new_content: Vec<ContentStep>) {
        self.content = new_content;
    }

    #[cfg_attr(feature = "python", pyo3(name = "process_content"))]
    #[cfg(feature = "python")]
    pub fn process_content_py(
        &self,
        ctx: Vec<Label>,
        labels: Vec<Label>
    ) -> Result<(Vec<ContentStep>, Vec<ContentStep>, Vec<ContentStep>)> {
        self.process_content(&ctx, &labels)
    }

    #[cfg_attr(feature = "python", pyo3(signature = (indent, recurse=false)))]
    pub fn dump(&self, indent: usize, recurse: bool) -> String {
        let mut dump_lines = vec![
            format!("{:indent$}name: {:?}", "", self.name, indent = indent),
            format!("{:indent$}variable name: {:?}", "", self.var_name, indent = indent),
            format!("{:indent$}content: {:?}", "", self.content, indent = indent),
        ];
        if recurse {
            for child in &self.children {
                dump_lines.push(child.dump(indent + 3, recurse));
            }
        }
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
        &self,
        ctx: &[Label],
        labels: &[Label]
    ) -> Result<(Vec<ContentStep>, Vec<ContentStep>, Vec<ContentStep>)> {
        Self::process_content_steps(&self.content, ctx, labels)
    }

    fn process_content_steps<'a>(
        content: impl IntoIterator<Item = &'a ContentStep>,
        ctx: &[Label],
        labels: &[Label],
    ) -> Result<(Vec<ContentStep>, Vec<ContentStep>, Vec<ContentStep>)> {
        let mut new_content: Vec<ContentStep> = Vec::new();
        let mut failed_filters: Vec<ContentStep> = Vec::new();

        for step in content {
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
                                None => return Err(Error::type_error(
                                    format!("Empty conditional node in {:?}", step)
                                ))
                            }
                        },
                        _ => return Err(Error::type_error(
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
                                // check and unpack the content inside this conditional node
                                let (cond_content,
                                    mut failed_cond_filters,
                                    deeper_failed_filters) = n
                                        .process_content(ctx, labels)?;
                                new_content.extend(cond_content);
                                if !failed_cond_filters.is_empty() {
                                    // record the entire conditional step as a failing filter
                                    failed_filters.push(step.clone());
                                    failed_cond_filters.extend(deeper_failed_filters);
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
}
impl Node {
    pub fn apply_dict(
        &mut self,
        lexer: &Lexer,
        dict: HashMap<ParamKey, ParamVal>
    ) {
        // Build a LApplyDict from the Rust HashMap
        let content_type = ContentType::Tokens(Tokens::LApplyDict(String::new(), dict));

        // Add pre-dictionary content to this node
        self.add_content(
            lexer.filename.clone(),
            lexer.linenum,
            content_type,
        );

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
    ) -> Result<HashMap<ParamKey, ParamVal>> {
        // Build identifier_str
        let token_str = match token {
            Tokens::LIdentifier(s) => s.clone(),
            _ => return Err(Error::value(
                format!("Expected LIdentifier token but got {}", token)
            )),
        };
        let Some(op) = identifier.last() else {
            return Err(Error::value("Empty identifier"));
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
                    _ => return Err(Error::value(
                        format!("Expected LIdentifier token but got {}", t)
                    )),
                });
            }
            parts.join("")
        };

        // Get the next token for the value (LString)
        let lstring = TokenKind::LString;
        let lendl = TokenKind::LEndL;
        let value = lexer.get_next_token(Some(&[lstring]), None)?;
        let mut value_str: String = value.string()?;
        // strip surrounding quotes if present
        let first = value_str.chars().next().unwrap_or(' ');
        let last = value_str.chars().last().unwrap_or(' ');
        if value_str.len() >= 2 && ((first == '"' && last == '"') || (first == '\'' && last == '\'')) {
            value_str = value_str[1..value_str.len() - 1].to_string();
        }

        // Construct operator instance by likeness of the provided one
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
                if !op_name.is_empty() && d_nin_val && dict.contains_key_str(op_name.as_str()) {
                    // apply and consume EOL
                    op_obj.apply_to_dict(&mut dict)?;
                    lexer.get_next_token(Some(&[lendl]), None)?;
                    return Ok(dict);
                } else {
                    // flush dict into node
                    self.apply_dict(lexer, dict);
                    dict = HashMap::new();
                }
            }

            // Add operator token as content
            self.add_content(
                lexer.filename.clone(),
                lexer.linenum,
                ContentType::Tokens(op_obj),
            );
        }

        // consume end-of-line
        lexer.get_next_token(Some(&[lendl]), None)?;
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
    ) -> Result<()> {
        let lidentifier = TokenKind::LIdentifier;
        let lendl = TokenKind::LEndL;
        let to_del = lexer.get_next_token(Some(&[lidentifier]), Some(true))?;
        // consume EOL
        lexer.get_next_token(Some(&[lendl]), Some(true))?;
        let to_del_str: String = to_del.string()?;

        // flush dict and add token as content
        self.apply_dict(lexer, dict);
        self.add_content(
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
    ) -> Result<Node> {
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
            return Err(Error::Parser(ParserError::new(
                "file does not exist or it's not a regular file".to_string(),
                lexer.line.clone(),
                Some(lexer.filename.clone()),
                lexer.linenum,
            )));
        }

        // Apply current dict and create new lexer for included file
        self.apply_dict(lexer, dict);

        let filepath_str = filepath.to_str()
            .ok_or_else(|| Error::value("Invalid filepath"))?;
        let mut new_lexer = Lexer::new(None, Some(filepath_str))?;

        // Parse with new lexer
        parse(&mut new_lexer, self.clone(), None, false, None)
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
        indent: usize,
    ) -> Result<()> {
        // Build the full identifier list: [token] + identifier[:-1] + [LEndl]
        let mut tokens = vec![token];
        let identifier_len = identifier.len();
        tokens.extend(identifier.into_iter().take(identifier_len.saturating_sub(1)));
        tokens.push(Tokens::LEndL());

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
            let linenum = lexer.linenum.ok_or_else(|| Error::value("Missing line number"))?;
            lexer.set_next_line(next_line_str, indent + 1, linenum);
        }

        // Create a new Node for the condition
        let mut cond = Node::new();
        cond.condition = Some(Filters::Condition { filter : cfilter, line : lexer.line.clone().unwrap_or_default() });

        // Parse the condition block
        cond = parse(lexer, cond, Some(indent), false, None)?;

        // Apply the current dict and add the condition node as content
        self.apply_dict(lexer, dict);
        self.add_content(
            lexer.filename.clone(),
            lexer.linenum,
            ContentType::Node(Rc::new(cond)),
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
        indent: usize,
    ) -> Result<()> {
        // Build the full token list
        let lcolon = TokenKind::LColon;
        let lendl = TokenKind::LEndL;
        let tokens = lexer.get_until(
            &[lcolon, lendl],
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
            let linenum = lexer.linenum.ok_or_else(|| Error::value("Missing line number"))?;
            lexer.set_next_line(next_line_str, indent + 1, linenum);
        }

        // Create a new Node for the negative condition
        let mut cond = Node::new();
        cond.condition = Some(Filters::NegativeCondition {
            filter : lfilter,
            line : lexer.line.clone().unwrap_or_default()
        });

        // Parse the condition block
        cond = parse(lexer, cond, Some(indent), false, None)?;

        // Apply the current dict and add the condition node as content
        self.apply_dict(lexer, dict);
        self.add_content(
            lexer.filename.clone(),
            lexer.linenum,
            ContentType::Node(Rc::new(cond)),
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
    ) -> Result<(String, HashMap<String, Vec<String>>)> {
        // Check if node has conditions
        if self.condition.is_some() {
            return Err(Error::Parser(ParserError::new(
                "'variants' is not allowed inside a conditional block".to_string(),
                lexer.line.clone(),
                Some(lexer.filename.clone()),
                lexer.linenum,
            )));
        }

        // Get tokens until bracket, colon, identifier or end
        let allowed = [
            TokenKind::LLBracket,
            TokenKind::LColon,
            TokenKind::LIdentifier,
            TokenKind::LEndL,
        ];
        let tokens = lexer.get_until(&allowed, None, Some(true))?;
        let mut vtoken: Tokens = match tokens.last() {
            Some(last_token) => last_token.clone(),
            None => {
                return Err(Error::value("Empty token list"));
            }
        };

        let mut variant_name = String::new();
        let mut meta = HashMap::new();

        // Parse tokens until colon or end
        while !matches!(vtoken, Tokens::LColon()) && !matches!(vtoken, Tokens::LEndL()) {
            if matches!(vtoken, Tokens::LIdentifier(_)) {
                if !variant_name.is_empty() {
                    return Err(Error::Parser(ParserError::new(
                        "Syntax ERROR expected '[' or ':'".to_string(),
                        lexer.line.clone(),
                        Some(lexer.filename.clone()),
                        lexer.linenum,
                    )));
                }
                variant_name = tokens[0].string()?;
            } else if matches!(vtoken, Tokens::LLBracket()) {
                // Parse metadata in brackets
                let ident = lexer.get_next_token(
                    Some(&[TokenKind::LIdentifier]),
                    Some(true),
                )?;
                let ident_str: String = ident.string()?;

                let next_token = lexer.get_next_token(
                    Some(&[
                        TokenKind::LSet,
                        TokenKind::LRBracket,
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
                        &[
                            TokenKind::LRBracket,
                            TokenKind::LEndL
                        ],
                        None,
                        Some(true),
                    )?;
                    let last_token: &Tokens = match tokens.last() {
                        Some(last_token) => last_token,
                        None => {
                            return Err(Error::value("Empty variants"));
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
                        return Err(Error::Parser(ParserError::new(
                            "Syntax ERROR expected ']'".to_string(),
                            lexer.line.clone(),
                            Some(lexer.filename.clone()),
                            lexer.linenum,
                        )));
                    }
                }
            }

            // Get next token
            let next_token = lexer.get_next_token(
                Some(&allowed),
                Some(true),
            )?;
            vtoken = next_token;
        }

        // Verify default values if present
        if meta.contains_key("default") {
            for val in meta.get("default").unwrap_or(&Vec::new()) {
                if val == "true" {
                    return Err(Error::Parser(ParserError::new(
                        "Syntax ERROR expected [default=xxx]".to_string(),
                        lexer.line.clone(),
                        Some(lexer.filename.clone()),
                        lexer.linenum,
                    )));
                }
            }
        }

        // Check for required colon
        if matches!(vtoken, Tokens::LEndL()) {
            return Err(Error::Parser(ParserError::new(
                "Syntax ERROR expected ':'".to_string(),
                lexer.line.clone(),
                Some(lexer.filename.clone()),
                lexer.linenum,
            )));
        }

        // Consume end of line
        lexer.get_next_token(
            Some(&[TokenKind::LEndL]),
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
        indent: usize,
        variant_name: String,
        variant_indent: usize,
        meta: &mut HashMap<String, Vec<String>>,
        defaults : bool,
        expand_defaults : Vec<String>,
    ) -> Result<Node> {
        let mut already_default = false;
        let mut node4 = Node::new();

        if !dict.is_empty() {
            self.apply_dict(lexer, dict);
        }

        // Share parent among siblings
        let parent = Rc::new(self.clone());

        // Handle default variants
        let meta_default = meta.get_mut("default");
        let meta_default_values_empty = &mut Vec::new();
        let meta_default_values = meta_default.unwrap_or(meta_default_values_empty);
        let meta_in_expand_defaults = !expand_defaults.contains(&variant_name);

        // Data used for the entire loop
        let tokens = [
                TokenKind::LIdentifier,
                TokenKind::LDefault,
                TokenKind::LIndent,
                TokenKind::LEndBlock,
        ];

        loop {
            lexer.set_prev_indent(Some(variant_indent));

            // Get token from lexer and check for end of block
            let token = lexer.get_next_token(Some(&tokens), Some(true))?;
            if matches!(token, Tokens::LEndBlock(_)) {
                break;
            }

            let mut is_default = false;
            let mut name;

            if matches!(token, Tokens::LIndent(_)) {
                // Handle indented variant
                lexer.get_next_token(
                    Some(&[TokenKind::LVariant]),
                    Some(true),
                )?;
                let token = lexer.get_next_token(
                    Some(&[
                        TokenKind::LIdentifier,
                        TokenKind::LDefault,
                    ]),
                    Some(true),
                )?;

                if matches!(token, Tokens::LDefault()) {
                    is_default = true;
                    name = lexer.get_until(
                        &[TokenKind::LColon],
                        None,
                        None,
                    )?;
                } else {
                    name = vec![token];
                    name.extend(lexer.get_until(
                        &[TokenKind::LColon],
                        None,
                        None,
                    )?);
                }
            } else if matches!(token, Tokens::LDefault()) {
                is_default = true;
                name = lexer.get_until(
                    &[TokenKind::LColon],
                    None,
                    None,
                )?;
            } else {
                name = vec![token];
                name.extend(lexer.get_until(
                    &[TokenKind::LColon],
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
                    &[TokenKind::LEndL],
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
            let mut node2 = Node::new();
            node2.children.push_back(Rc::clone(&parent));
            node2.update_labels(self.labels.clone());

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

            let mut node3 = parse(lexer, node2, Some(indent), defaults, Some(expand_defaults.clone()))?;

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
            );
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
            );

            // Update labels (move out fields since we appended clones)
            node4.update_labels(node3.labels.clone());
            node4.update_labels(node3.name.clone());

            // Add node to children
            if node3.default && defaults {
                node4.prepend_child(node3);
            } else {
                node4.append_child(node3);
            }
        }

        // Check if all default variants were used
        if !meta_default_values.is_empty() {
            return Err(Error::Parser(ParserError::new(
                format!("Missing default variant {:?}", meta_default_values),
                lexer.line.clone(),
                Some(lexer.filename.clone()),
                lexer.linenum,
            )));
        }

        Ok(node4)
    }
}

pub fn parse(
    lexer: &mut Lexer,
    mut node: Node,
    prev_indent: Option<usize>,
    defaults: bool,
    expand_defaults: Option<Vec<String>>,
) -> Result<Node> {
    // Allowed token types for different contexts
    let block_allowed = [
        TokenKind::LVariants,
        TokenKind::LIdentifier,
        TokenKind::LOnly,
        TokenKind::LNo,
        TokenKind::LInclude,
        TokenKind::LDel,
        TokenKind::LNotCond,
        TokenKind::LSuffix,
        TokenKind::LJoin,
    ];
    let variants_allowed = [TokenKind::LVariant];
    let identifier_allowed = [
        TokenKind::LSet,
        TokenKind::LAppend,
        TokenKind::LPrepend,
        TokenKind::LLazySet,
        TokenKind::LRegExpSet,
        TokenKind::LRegExpAppend,
        TokenKind::LRegExpPrepend,
        TokenKind::LColon,
        TokenKind::LEndL,
    ];
    let indent_allowed = [
        TokenKind::LIndent,
        TokenKind::LEndBlock
    ];
    let mut allowed = block_allowed.as_slice();

    // Variant tracking state
    let mut variant_name = String::new();
    let mut variant_indent = 0;
    let mut meta = HashMap::new();

    // Suffix operator state
    // NOTE: Suffix should be applied as the last operator in the dictionary
    // Reasons:
    // 1. Escapes multiplying suffix operators
    // 2. Affects all elements in current block
    let mut suffix: Option<(String, Option<usize>, Tokens)> = None;

    // Dictionary contains block of operation without collision with
    // other blocks or operations which increases speed almost twice.
    let mut dict: HashMap<ParamKey, ParamVal> = HashMap::new();

    loop {
        lexer.set_prev_indent(prev_indent);

        // Handle indentation
        let token = lexer.get_next_token(
            Some(&indent_allowed),
            None
        )?;

        let Tokens::LIndent(indent) = token else {
            if !dict.is_empty() {
                // Flush dict to node content
                node.apply_dict(lexer, dict);
            }
            if let Some((filename, linenum, op)) = suffix {
                // Node has suffix, apply it to all elements
                node.add_content(filename.clone(), linenum, ContentType::Tokens(op));
            }
            return Ok(node);
        };
        let token = lexer.get_next_token(Some(allowed), None)?;

        match token {
            Tokens::LInclude() => {
                node = node.apply_include(lexer, dict)?;
                dict = HashMap::new();
                lexer.set_prev_indent(prev_indent);
            }

            Tokens::LIdentifier(_) => {
                // Parse:
                //    identifier .....
                // Get tokens until an operator or colon
                let identifier = lexer.get_until(
                    &identifier_allowed,
                    None,
                    Some(true),
                )?;
                let last_token: &Tokens = match identifier.last() {
                    Some(last_token) => last_token,
                    None => {
                        return Err(Error::value("Empty identifier"));
                    }
                };

                if matches!(last_token, Tokens::LColon()) {
                    // Handle condition block
                    node.apply_condition(
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
                    dict = node.apply_operator(
                        identifier,
                        token,
                        lexer,
                        dict,
                    )?;
                } else {
                    return Err(Error::Parser(ParserError::new(
                        "Syntax ERROR expected ':' or operand".to_string(),
                        lexer.line.clone(),
                        Some(lexer.filename.clone()),
                        lexer.linenum,
                    )));
                }
            }

            Tokens::LDel(_, _) => {
                node.apply_deletion(lexer, dict)?;
                dict = HashMap::new();
            }

            Tokens::LNotCond() => {
                node.apply_notcondition(lexer, dict, indent)?;
                dict = HashMap::new();
                lexer.set_prev_indent(prev_indent);
            }

            Tokens::LVariants() => {
                let (name, meta_dict) = node.apply_variants(lexer)?;
                variant_name = name;
                variant_indent = indent;
                for (key, values) in meta_dict {
                    meta.insert(key, values);
                }
                allowed = &variants_allowed;
            }

            Tokens::LVariant() => {
                node = node.apply_variant(
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
                allowed = &block_allowed;
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
                node.apply_dict(lexer, dict);
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
                    node.apply_dict(lexer, dict);
                }
                dict = HashMap::new();
                let token_val = lexer.get_next_token(
                    Some(&[TokenKind::LIdentifier]),
                    None,
                )?;
                lexer.get_next_token(
                    Some(&[TokenKind::LEndL]),
                    None,
                )?;

                suffix = Some((
                    lexer.filename.clone(),
                    lexer.linenum,
                    Tokens::Suffix(String::new(), token_val.string()?),
                ));
            }

            _ => {
                return Err(Error::Parser(ParserError::new(
                    "Syntax ERROR expected".to_string(),
                    lexer.line.clone(),
                    Some(lexer.filename.clone()),
                    lexer.linenum,
                )));
            }
        }
    }
}

#[cfg_attr(feature = "python", cfg_eval::cfg_eval)]
#[cfg_attr(feature = "python", pyfunction)]
#[cfg_attr(feature = "python", pyo3(signature = (cfgstr, node, prev_indent=None, defaults=true, expand_defaults=None)))]
pub fn parse_string(
    cfgstr: String,
    node: Node,
    prev_indent: Option<usize>,
    defaults: bool,
    expand_defaults: Option<Vec<String>>,
) -> Result<Node> {
    let mut new_lexer = Lexer::new(Some(&cfgstr), None)?;
    parse(&mut new_lexer, node, prev_indent, defaults, expand_defaults)
}

#[cfg_attr(feature = "python", cfg_eval::cfg_eval)]
#[cfg_attr(feature = "python", pyfunction)]
#[cfg_attr(feature = "python", pyo3(signature = (cfgfile, node, prev_indent=None, defaults=true, expand_defaults=None)))]
pub fn parse_file(
    cfgfile: String,
    node: Node,
    prev_indent: Option<usize>,
    defaults: bool,
    expand_defaults: Option<Vec<String>>,
) -> Result<Node> {
    let mut new_lexer = Lexer::new(None, Some(&cfgfile))?;
    parse(&mut new_lexer, node, prev_indent, defaults, expand_defaults)
}

#[cfg_attr(feature = "python", cfg_eval::cfg_eval)]
#[cfg_attr(feature = "python", pyclass(from_py_object, frozen, unsendable))]
#[derive(Debug, Clone)]
pub struct Tree {
    root: Rc<Node>,
}

#[cfg_attr(feature = "python", cfg_eval::cfg_eval)]
#[cfg_attr(feature = "python", pymethods)]
impl Tree {
    #[cfg_attr(feature = "python", new)]
    #[cfg_attr(feature = "python", pyo3(signature = (node=None)))]
    pub fn new(node: Option<Node>) -> Self {
        Self { root: Rc::new(node.unwrap_or_default()) }
    }

    /// Return a detached root for the legacy Node inspection API.
    #[cfg_attr(feature = "python", getter)]
    pub fn node(&self) -> Node {
        self.root.as_ref().clone()
    }

    pub fn is_empty(&self) -> bool {
        self.root.children.is_empty() && self.root.content.is_empty()
    }

    #[cfg_attr(feature = "python", pyo3(signature = (cfgstr, defaults=false, expand_defaults=None)))]
    pub fn parse_string(&self, cfgstr: String, defaults: bool, expand_defaults: Option<Vec<String>>) -> Result<Self> {
        let mut node = self.root.as_ref().clone();
        node.filename = "<string>".to_string();
        Ok(Self { root: Rc::new(parse_string(cfgstr, node, None, defaults, expand_defaults)?) })
    }

    #[cfg_attr(feature = "python", pyo3(signature = (cfgfile, defaults=false, expand_defaults=None)))]
    pub fn parse_file(&self, cfgfile: String, defaults: bool, expand_defaults: Option<Vec<String>>) -> Result<Self> {
        let mut node = self.root.as_ref().clone();
        node.filename = cfgfile.clone();
        Ok(Self { root: Rc::new(parse_file(cfgfile, node, None, defaults, expand_defaults)?) })
    }
}

type FailedCase = (Vec<Label>, Vec<ContentStep>, Vec<ContentStep>);

enum TraversalStep {
    Join(usize, usize),
    Dict(HashMap<ParamKey, ParamVal>),
    Done,
}

#[cfg_attr(feature = "python", cfg_eval::cfg_eval)]
#[cfg_attr(feature = "python", pyclass(from_py_object,unsendable))]
#[derive(Debug, Clone)]
pub struct PreDict {
    _ctx: Vec<Vec<Label>>,
    _shortname: Vec<Vec<Label>>,

    _content: Vec<Vec<ContentStep>>,
    _ctx_content: Vec<Vec<ContentStep>>,

    _dep: Vec<Vec<String>>,

    // traversal state
    branch: Vec<Rc<Node>>,
    processed_content: Vec<Option<Vec<ContentStep>>>,
    join_width: Vec<usize>,
    failed_cases: HashMap<u64, VecDeque<FailedCase>>,
    #[cfg_attr(feature = "python", pyo3(get))]
    pub route: Vec<Option<usize>>,
    #[cfg_attr(feature = "python", pyo3(get))]
    pub joins: Vec<Option<Vec<ContentStep>>>,
    // TODO: refactor as we expanded to double option to complete the migration more easily
    #[allow(clippy::type_complexity)]
    #[cfg_attr(feature = "python", pyo3(get))]
    pub join_dicts: Vec<Option<Vec<Option<HashMap<ParamKey, ParamVal>>>>>,
    #[cfg_attr(feature = "python", pyo3(get))]
    pub join_pre_dicts: Vec<Option<Vec<Option<PreDict>>>>,

    // whether to use default variants
    #[cfg_attr(feature = "python", pyo3(get, set))]
    pub defaults: bool,
    // list of default variants to expand
    #[cfg_attr(feature = "python", pyo3(get, set))]
    pub expand_defaults: Vec<String>,
    // supported number of failed filters for a node
    #[cfg_attr(feature = "python", pyo3(get))]
    pub num_failed_cases: usize,
}

impl Default for PreDict {
    fn default() -> Self {
        Self::new(None, None, None, None, None, None)
    }
}

#[cfg_attr(feature = "python", cfg_eval::cfg_eval)]
#[cfg_attr(feature = "python", pymethods)]
impl PreDict {
    #[cfg_attr(feature = "python", getter)]
    #[cfg(feature = "python")]
    fn branch(&self) -> Vec<Node> {
        self.branch.iter().map(|node| node.as_ref().clone()).collect()
    }

    #[cfg_attr(feature = "python", getter)]
    fn ctx(&self) -> Vec<Label> {
        self._ctx.iter().flatten().cloned().collect()
    }

    #[cfg_attr(feature = "python", getter)]
    #[cfg(feature = "python")]
    fn shortname(&self) -> Vec<Label> {
        self._shortname.iter().flatten().cloned().collect()
    }

    #[cfg_attr(feature = "python", getter)]
    #[cfg(feature = "python")]
    fn content(&self) -> Vec<ContentStep> {
        self._content.iter().flatten().cloned().collect()
    }

    #[cfg_attr(feature = "python", getter)]
    #[cfg(feature = "python")]
    fn final_content(&self) -> Vec<ContentStep> {
        self.iter_final_content().cloned().collect()
    }

    #[cfg_attr(feature = "python", getter)]
    fn dep(&self) -> Vec<String> {
        self._dep.iter().flatten().cloned().collect()
    }

    #[cfg_attr(feature = "python", new)]
    #[cfg_attr(feature = "python", pyo3(signature = (ctx=None, content=None, shortname=None, dep=None, defaults=None, expand_defaults=None)))]
    pub fn new(
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
            branch: Vec::new(),
            processed_content: Vec::new(),
            join_width: Vec::new(),
            failed_cases: HashMap::new(),
            route: Vec::new(),
            joins: Vec::new(),
            join_dicts: Vec::new(),
            join_pre_dicts: Vec::new(),
            defaults: defaults.unwrap_or(false),
            expand_defaults: expand_defaults.unwrap_or_default(),
        }
    }

    #[cfg(feature = "python")]
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

    #[cfg(feature = "python")]
    fn __copy__(&self) -> PreDict {
        self.clone()
    }

    #[cfg(feature = "python")]
    fn shallow_copy(&self) -> PreDict {
        Self::from_frames(
            &self._ctx, &self._content, &self._ctx_content, &self._shortname,
            &self._dep, self.defaults, &self.expand_defaults,
        )
    }

    #[cfg_attr(feature = "python", pyo3(name = "update_from_node", signature = (node)))]
    #[cfg(feature = "python")]
    pub fn update_from_node_py(&mut self, node: Node) -> Result<bool> {
        self.update_from_node(Rc::new(node), None)
    }

    pub fn update_from_tree(&mut self, ast: &Tree) -> Result<bool> {
        self.update_from_node(Rc::clone(&ast.root), None)
    }

    fn reset_from_last_node(&mut self) {
        if self.branch.is_empty() {
            return;
        }
        self.branch.pop();
        self.processed_content.pop();
        self.join_width.pop();
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

    pub fn get_dict(&self) -> Result<HashMap<ParamKey, ParamVal>> {
        let mut dict: HashMap<ParamKey, ParamVal> = HashMap::new();

        let name = self._ctx.iter().flatten().map(|l| l.long_name.as_str()).collect::<Vec<_>>().join(".");
        let shortname = self._shortname.iter().flatten().map(|l| l.name.as_str()).collect::<Vec<_>>().join(".");
        let dep = self.dep();

        dict.insert_str("name", name.into());
        dict.insert_str("shortname", shortname.into());
        dict.insert_str("dep", ParamVal::List(dep));

        for step in self.iter_final_content() {
            match &step.content_type {
                ContentType::Tokens(t) => {
                    // ignore inapplicable token types by ignoring the status
                    let _ = t.clone().apply_to_dict(&mut dict);
                }
                _ => {
                    return Err(Error::runtime("Unexpected content type"));
                }
            }
        }
        Ok(dict)
    }

    /*
    Generate dictionaries from the pre-dict parsed so far.

    This should be called after parsing something or seeding the
    pre-dict with an initial node.

    Get possibly joined dictionaries from processed 'join' entries as
    unpacked 'only' filters in the pre-dict state.

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
    #[cfg_attr(feature = "python", pyo3(signature = (dropsufs=false, skipdups=true)))]
    pub fn get_dicts(&mut self, dropsufs: bool, skipdups: bool) -> Result<Option<HashMap<ParamKey, ParamVal>>> {
        let dn = self.next_dict()?;
        if dropsufs && let Some(d) = dn {
            return Ok(Some(match drop_suffixes(&d, skipdups)? {
                Cow::Borrowed(_) => d,
                Cow::Owned(flat) => flat,
            }));
        }
        Ok(dn)
    }

}

impl PreDict {
    fn iter_final_content(&self) -> impl Iterator<Item = &ContentStep> {
        self._content.last().into_iter().flatten()
            .chain(self._ctx_content.last().into_iter().flatten())
    }

    fn check_failed_case<'a>(
        failed_case: &FailedCase,
        ctx: &[Label],
        labels: &[Label],
        content: impl Iterator<Item = &'a ContentStep> + Clone,
        node_content: &[ContentStep],
    ) -> bool {
        let (failed_ctx, failed_external_filters, failed_internal_filters) = failed_case;

        // might pass if any filter (external or internal) is missing from all content
        let in_all_content = |step: &ContentStep| {
            content.clone().any(|candidate| candidate == step) || node_content.contains(step)
        };
        if failed_external_filters.iter().any(|t| !in_all_content(t))
            || failed_internal_filters.iter().any(|t| !in_all_content(t))
        {
            return true;
        }

        // cannot pass if at least one external filter cannot pass
        for ContentStep {content_type, ..} in failed_external_filters {
            if let ContentType::Filters(external_filter) = content_type
                && !external_filter.might_pass(failed_ctx, ctx, labels) {
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
                && !internal_filter.might_pass(failed_ctx, ctx, labels) {
                    return false;
                }
        }

        true
    }

    fn from_frames(
        ctx: &[Vec<Label>],
        content: &[Vec<ContentStep>],
        ctx_content: &[Vec<ContentStep>],
        shortname: &[Vec<Label>],
        dep: &[Vec<String>],
        defaults: bool,
        expand_defaults: &[String],
    ) -> Self {
        let mut new = Self::new(
            Some(ctx.iter().flatten().cloned().collect()),
            Some(content.last().cloned().unwrap_or_default()),
            Some(shortname.iter().flatten().cloned().collect()),
            Some(dep.iter().flatten().cloned().collect()),
            Some(defaults),
            Some(expand_defaults.to_vec()),
        );
        new._ctx_content[0] = ctx_content.last().cloned().unwrap_or_default();
        new
    }

    pub fn update_from_node(&mut self, node: Rc<Node>, effective_content: Option<Vec<ContentStep>>) -> Result<bool> {
        /* TODO: add optional logging
        if self.debug:    #Print dict on which is working now.
            print(node.dump(0))
        */

        let ctx = node.name.clone();
        let labels = &node.labels;
        let shortname = if node.append_to_shortname {
            node.name.clone()
        } else {
            Vec::new()
        };

        // build dep strings using current flattened ctx and node.dep
        let mut ctx_flat: Vec<Label> = self.ctx();
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

        ctx_flat.extend_from_slice(&ctx);

        let node_content = effective_content.as_deref().unwrap_or(&node.content);
        let failed = self.failed_cases.get_mut(&node.id).and_then(|cases| {
            let i = cases.iter().position(|case| {
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
                !Self::check_failed_case(
                    case, &ctx_flat, labels, self._content.iter().flatten(), node_content,
                )
            })?;
            // prioritize any first detected failed case
            let case = cases.remove(i)?;
            cases.push_front(case);
            Some(i)
        });

        self._ctx.push(ctx);
        self._shortname.push(shortname);
        self._dep.push(dep);

        // push state machine stacks
        self.route.push(None);
        self.join_width.push(0);
        self.joins.push(None);
        self.join_dicts.push(None);
        self.join_pre_dicts.push(None);
        if failed.is_some() {
            // keep the failed frame so its parent advances before trying another child
            self._content.push(Vec::new());
            self._ctx_content.push(Vec::new());
            self.branch.push(node);
            self.processed_content.push(effective_content);
            return Ok(false);
        }

        // process internal content for the node
        let (internal_content, mut failed_internal, mut failed_internal_cond) =
            Node::process_content_steps(node_content, &ctx_flat, labels)?;
        failed_internal.append(&mut failed_internal_cond);

        // process external (previous) content against current context
        let (external_content, failed_external, mut failed_external_cond) =
            Node::process_content_steps(self.iter_final_content(), &ctx_flat, labels)?;
        // NOTE: the failed filters should go into the failed internal filters
        // because we don't expect them to come from outside this node, even if
        // the condition itself was external
        failed_internal.append(&mut failed_external_cond);
        self._content.push(internal_content);
        self._ctx_content.push(external_content);

        // register failed case and return false if failed filters
        if !failed_internal.is_empty() || !failed_external.is_empty() {
            let cases = self.failed_cases.entry(node.id).or_default();
            cases.push_front((ctx_flat, failed_external, failed_internal));
            cases.truncate(self.num_failed_cases);
            /* TODO: add optional logging
            self._debug("Failed_cases %s", cases)
            */
            self.branch.push(node);
            self.processed_content.push(effective_content);
            return Ok(false);
        }

        self.branch.push(node);
        self.processed_content.push(effective_content);
        Ok(true)
    }

    fn prepare_joins(&mut self, depth: usize) {
        let joins = &self.joins[depth];
        let node = &self.branch[depth];
        let content = self.processed_content[depth].as_ref().unwrap_or(&node.content);

        // only copy into processed content when a join needs to specialize this traversal
        if joins.is_none() && content.iter().any(|step| {
            matches!(&step.content_type, ContentType::Filters(Filters::JoinFilter { .. }))
        }) {

            // find joins from node content and prepare only-filters
            let mut plain_content: Vec<ContentStep> = Vec::with_capacity(node.content.len());
            let mut new_joins: Vec<ContentStep> = Vec::with_capacity(node.content.len());
            for t in content.iter().cloned() {
                match t.content_type {
                    ContentType::Filters(Filters::JoinFilter {filter, line }) => {
                        // accumulate join steps
                        new_joins.push(ContentStep {
                            filename: t.filename,
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
                // keep join-free content in this traversal, preserving the actual node
                self.processed_content[depth] = Some(plain_content);
                self.join_width[depth] = 0;
            }
        }

    }

    fn joined_dict(&self, depth: usize) -> Result<HashMap<ParamKey, ParamVal>> {
        let dicts = self.join_dicts.get(depth).and_then(Option::as_ref).ok_or_else(||
            Error::value(format!("Join dicts not initialized at depth {depth}")))?;
        let mut d: HashMap<ParamKey, ParamVal> = HashMap::new();
        let mut name = String::new();
        let mut shortname = String::new();
        for di in dicts.iter().flatten() {
            if name.is_empty() {
                name = di.get_str("name").map(|v| Cow::from(v).into()).unwrap_or_default();
                shortname = di.get_str("shortname").map(|v| Cow::from(v).into()).unwrap_or_default();
            } else {
                let other_name: String = di.get_str("name").map(|v| Cow::from(v).into()).unwrap_or_default();
                let other_short: String = di.get_str("shortname").map(|v| Cow::from(v).into()).unwrap_or_default();
                name = Node::join_names(&name, &other_name);
                shortname = Node::join_names(&shortname, &other_short);
            }
            // update combined map d with di entries
            for (k, v) in di {
                d.insert(k.clone(), v.clone());
            }
        }
        d.insert_str("name", name.into());
        d.insert_str("shortname", shortname.into());
        Ok(d)
    }

    /*
    Advance traversal until a dictionary, a join operation, or an exchaustion.

    Advance across a plain branch of AST nodes up to a returned dictionary or
    up to a blocking request for another joined pre-dictionary or up to termination.
    */
    fn advance(&mut self) -> Result<TraversalStep> {
        if self.joins.len() != self.branch.len()
            || self.join_dicts.len() != self.branch.len()
            || self.join_pre_dicts.len() != self.branch.len()
            || self.route.len() != self.branch.len()
            || self.join_width.len() != self.branch.len()
            || self.processed_content.len() != self.branch.len()
            || self._ctx.len().checked_sub(1) != Some(self.branch.len())
            || self._shortname.len().checked_sub(1) != Some(self.branch.len())
            || self._dep.len().checked_sub(1) != Some(self.branch.len())
            || self._content.len().checked_sub(1) != Some(self.branch.len())
            || self._ctx_content.len().checked_sub(1) != Some(self.branch.len())
        {
            return Err(Error::value("Inconsistent pre-dictionary state"));
        }
        loop {
            let Some(depth) = self.branch.len().checked_sub(1) else {
                return Err(Error::runtime("Pre-dictionary needs at least one node"));
            };
            if self.joins[depth].is_some() != self.join_dicts[depth].is_some()
                || self.joins[depth].is_some() != self.join_pre_dicts[depth].is_some()
            {
                return Err(Error::value("Inconsistent pre-dictionary join state"));
            }
            if self.route[depth].is_none() {
                self.prepare_joins(depth);
            }
            if let Some(joins) = &self.joins[depth] {
                let dicts = self.join_dicts.get(depth).ok_or_else(||
                    Error::value(format!("Join dicts index out of bounds: {depth}")))?.as_ref().ok_or_else(||
                    Error::value(format!("Join dicts not initialized at depth {depth}")))?;
                let pre_dicts = self.join_pre_dicts.get(depth).ok_or_else(||
                    Error::value(format!("Join pre-dicts index out of bounds: {depth}")))?.as_ref().ok_or_else(||
                    Error::value(format!("Join pre-dicts not initialized at depth {depth}")))?;
                if dicts.len() != joins.len() || pre_dicts.len() != joins.len() {
                    return Err(Error::value("Inconsistent pre-dictionary join state"));
                }
                let width = self.join_width[depth];
                if width > joins.len() {
                    return Err(Error::value("Inconsistent pre-dictionary join state"));
                }
                if width < joins.len() {
                    return Ok(TraversalStep::Join(depth, width));
                }
                self.route[depth] = Some(self.branch[depth].children.len());
            }

            if self.route[depth].is_none() {
                self.route[depth] = Some(0);
                if self.branch[depth].children.is_empty() {
                    let mut dict = self.get_dict()?;
                    apply_suffix_bounds(&mut dict);
                    return Ok(TraversalStep::Dict(dict));
                }
            }
            let Some(child_index) = self.route[depth] else {
                return Err(Error::value("Pre-dictionary route not initialized"));
            };
            if child_index < self.branch[depth].children.len() {
                let child = Rc::clone(&self.branch[depth].children[child_index]);
                let updated = self.update_from_node(child, None)?;
                let parent = &self.branch[depth];
                let skip_default = self.defaults
                    && !self.expand_defaults.contains(&parent.var_name.iter()
                        .map(|l| l.to_string()).collect::<Vec<_>>().join("."))
                    && parent.children.iter().any(|c| c.default)
                    && !parent.children[child_index].default;
                if !updated || skip_default {
                    self.reset_from_last_node();
                    self.route[depth] = Some(child_index + 1);
                }
                continue;
            }
            if depth == 0 {
                return Ok(TraversalStep::Done);
            }
            self.reset_from_last_node();
            let Some(child_index) = self.route.last_mut().and_then(Option::as_mut) else {
                return Err(Error::value("Pre-dictionary parent route not initialized"));
            };
            *child_index = child_index.checked_add(1).ok_or_else(||
                Error::value("Pre-dictionary parent route out of bounds"))?;
        }
    }

    /*
    Return one complete dictionary, exhaustion, or an error via iterative evaluator calls.

    This function is the main traversal driver that routes new dictionary generation
    through a stack of nested possibly partial joins.

    The extended in-line documentation within the driver is crucial to understand the
    complete routing and rellocation process.
    */
    fn next_dict(&mut self) -> Result<Option<HashMap<ParamKey, ParamVal>>> {
        // each stack entry owns a join pre-dict and its location (AST depth, join width) in its parent
        // which is in a previous stack frame (or self if outside the stack)
        let mut stack: Vec<(PreDict, usize, usize)> = Vec::new();
        // the inferred type is Option<Result<Option<HashMap<ParamKey, ParamVal>>>> where:
        // 1) None means advance the active evaluator (while Some carries a completion to its parent)
        // 2) Some(Ok(Some(dict))) delivers a dictionary
        // 3) Some(Ok(None)) delivers exhaustion or rejection
        // 4) Some(Err(err)) delivers an error without advancing any more evaluators
        let mut result = None;
        loop {
            if result.is_none() {
                // advance the deepest active join pre-dict or self when no child is on the stack
                let current = stack.last_mut().map(|frame| &mut frame.0).unwrap_or(self);
                match current.advance() {
                    // advancing requests this joined dict but leaves its evaluation to the driver
                    Ok(TraversalStep::Join(depth, width)) => {
                        let Some(join) = current.joins.get(depth).and_then(Option::as_ref)
                            .and_then(|joins| joins.get(width)) else {
                            result = Some(Err(Error::value("Inconsistent pre-dictionary join state")));
                            continue;
                        };
                        let Some(child_slot) = current.join_pre_dicts.get_mut(depth).and_then(Option::as_mut)
                            .and_then(|pre_dicts| pre_dicts.get_mut(width)) else {
                            result = Some(Err(Error::value("Inconsistent pre-dictionary join state")));
                            continue;
                        };
                        // take moves a saved evaluator out of its slot, temporarily leaving None without cloning it
                        // while unwrap_or_else creates a new evaluator only when no saved continuation exists
                        // (moved evaluators must be restored before returning, even on errors)
                        let mut child = child_slot.take()
                            .unwrap_or_else(|| Self::from_frames(
                                // accumulated state from frame zero as the seed excluding the current node at depth which contributes frame depth + 1
                                &current._ctx[..depth + 1], &current._content[..depth + 1],
                                &current._ctx_content[..depth + 1], &current._shortname[..depth + 1],
                                &current._dep[..depth + 1], current.defaults, &current.expand_defaults,
                            ));
                        // every joined variant starts from this shared join node under its own only-filter
                        let node = &current.branch[depth];
                        // an existing node with this ID on the child's branch means initialization happened
                        let initialized = if child.branch.contains(node) {
                            // preserve the saved continuation instead of entering the same node again
                            Ok(true)
                        } else {
                            // new entry needs an owned content vector using the join-free override when present
                            let mut content = current.processed_content[depth].as_ref().unwrap_or(&node.content).clone();
                            // prepared joins made each join operand an only-filter to restrict with here
                            content.push(join.clone());
                            // shares the node returning accepted, rejected, or an error for initialization
                            child.update_from_node(Rc::clone(node), Some(content))
                        };
                        // push before handling rejection or failure so the taken evaluator always reaches restoration
                        // (propagating an error with ? here would leave its parent slot empty)
                        stack.push((child, depth, width));
                        match initialized {
                            // the next iteration advances the newly pushed child (reusing the joined predict)
                            Ok(true) => continue,
                            // rejected entry produces no dictionary for this join variant
                            Ok(false) => result = Some(Ok(None)),
                            Err(err) => result = Some(Err(err)),
                        }
                    }
                    // these outcomes complete only the active evaluator while enclosing joins may still have work
                    Ok(TraversalStep::Dict(dict)) => result = Some(Ok(Some(dict))),
                    Ok(TraversalStep::Done) => result = Some(Ok(None)),
                    Err(err) => result = Some(Err(err)),
                }
            }
            // result is always completed at this point - take() moves that completion out and clears result so
            // traversal can resume unless another result is queued (error or else)
            let completed = result.take().unwrap_or_else(||
                Err(Error::runtime("Pre-dictionary traversal did not complete")));
            let Some((child, depth, width)) = stack.pop() else {
                // no child remains on the temporary stack of nested joins so completion belongs to self and its caller
                return completed;
            };
            // after popping the child its parent is the new stack top or self for the outermost nested join
            let parent = stack.last_mut().map(|frame| &mut frame.0).unwrap_or(self);
            // restore the evaluator's ownership and continuation before processing its completion
            let Some(child_slot) = parent.join_pre_dicts.get_mut(depth).and_then(Option::as_mut)
                .and_then(|pre_dicts| pre_dicts.get_mut(width)) else {
                result = Some(Err(Error::value("Inconsistent pre-dictionary join state")));
                continue;
            };
            let child = child_slot.insert(child);
            match completed {
                // forward the error while restoring remaining ancestors without advancing them (result is no longer none)
                Err(err) => result = Some(Err(err)),
                // forward a dictionary or termination to the parent join evaluator
                Ok(dict) => {
                    // dict is an optional joined result so remember exhaustion before moving it into the cache
                    let exhausted = dict.is_none();
                    // count refers to join operands and not the AST node's children
                    let Some(joins) = parent.joins.get(depth).and_then(Option::as_ref) else {
                        result = Some(Err(Error::value("Inconsistent pre-dictionary join state")));
                        continue;
                    };
                    let count = joins.len();
                    // here Some replaces the cached candidate while None clears it when that join pre-dict is exhausted
                    let Some(dict_slot) = parent.join_dicts.get_mut(depth).and_then(Option::as_mut)
                        .and_then(|dicts| dicts.get_mut(width)) else {
                        result = Some(Err(Error::value("Inconsistent pre-dictionary join state")));
                        continue;
                    };
                    *dict_slot = dict;
                    if exhausted {
                        // normal exhaustion has backtracked to the entry node - pop it so the join pre-dict can restart
                        // while the preceding seed frames remain available for another combination
                        if !child.branch.is_empty() {
                            child.reset_from_last_node();
                        }
                        // advance an earlier join pre-dict where exhausting the first joined pre-dict would set to
                        // count in order to make the advancement treat this as a normal exhausted pre-dict
                        parent.join_width[depth] = if width == 0 { count } else { width - 1 };
                    } else if width + 1 == count {
                        // all join variants now have a dictionary so merge their caches and names in order and leave
                        // width on the final operand so the next call advances its join variant before earlier operands
                        result = Some(parent.joined_dict(depth).map(Some));
                    } else {
                        // obtain the next joined dictionary before emitting a combination (result stays None to resume the parent)
                        parent.join_width[depth] = width + 1;
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_apply_dict() {
        let mut node = Node::new();
        let lexer = Lexer::new(Some(""), None).expect("Failed to create lexer");
        let mut dict: HashMap<ParamKey, ParamVal> = HashMap::new();
        dict.insert_str("key", "value".to_string().into());

        // apply_dict should add an LApplyDict content step and clear dict
        node.apply_dict(&lexer, dict);
        let content = node.get_content();
        assert_eq!(content.len(), 1);

        // content type should be Tokens::LApplyDict and contains our key/value
        match &content[0].content_type {
            ContentType::Tokens(Tokens::LApplyDict(_, map)) => {
                assert_eq!(map.get_str("key"), Some(&"value".to_string().into()));
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
        let _ = lexer.get_next_token(Some(&[TokenKind::LIndent]), None).expect("get_next_token indent");
        let _ = lexer.get_next_token(Some(&[TokenKind::LInclude]), None).expect("get_next_token include");

        let mut node = Node::new();
        let mut dict: HashMap<ParamKey, ParamVal> = HashMap::new();
        dict.insert_str("key1", "value1".to_string().into());

        // apply_include should parse the included file and return a node with a child named "test"
        let returned = node.apply_include(&mut lexer, dict).expect("apply_include failed");
        let children = returned.get_children();
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
        let _ = lexer.get_next_token(Some(&[TokenKind::LIndent]), None).expect("indent token");
        let token = lexer.get_next_token(Some(&[TokenKind::LIdentifier]), None).expect("identifier token");
        // get identifier tokens up to '=' (no_white = true)
        let identifier = lexer.get_until(&[TokenKind::LSet], None, Some(true)).expect("get_until identifier");

        let mut node = Node::new();
        let mut dict: HashMap<ParamKey, ParamVal> = HashMap::new();
        dict.insert_str("key1", "value1".to_string().into());

        let error = node.apply_operator(Vec::new(), token.clone(), &mut lexer, HashMap::new())
            .expect_err("Expected an empty identifier error");
        assert!(matches!(error, Error::Value(_)));

        // apply_operator should add key2 to dict directly (optimized path)
        dict = node.apply_operator(identifier, token, &mut lexer, dict).expect("apply_operator failed");
        // dict now should contain both key1 and key2, and node content should be empty
        assert_eq!(dict.get_str("key1"), Some(&"value1".to_string().into()));
        assert_eq!(dict.get_str("key2"), Some(&"value2".to_string().into()));
        let content = node.get_content();
        assert_eq!(content.len(), 0);
    }

    #[test]
    fn test_apply_operator_append_safe() {
        // content: "key1 += &value2"
        let mut lexer = Lexer::new(Some("key1 += &value2"), None).expect("Failed to create lexer");
        let _ = lexer.get_next_token(Some(&[TokenKind::LIndent]), None).expect("indent token");
        let token = lexer.get_next_token(Some(&[TokenKind::LIdentifier]), None).expect("identifier token");
        let identifier = lexer.get_until(&[TokenKind::LAppend], None, Some(true)).expect("get_until identifier");

        let mut node = Node::new();
        let mut dict: HashMap<ParamKey, ParamVal> = HashMap::new();
        dict.insert_str("key1", "value1".to_string().into());

        dict = node.apply_operator(identifier, token, &mut lexer, dict).expect("apply_operator failed");

        // dict should be updated
        assert_eq!(dict.get_str("key1"), Some(&"value1&value2".to_string().into()));
        // node content should be empty
        let content = node.get_content();
        assert_eq!(content.len(), 0);
    }

    #[test]
    fn test_apply_operator_append_unsafe() {
        // content: "key2 += &value2" (key2 not present in dict therefore flush)
        let mut lexer = Lexer::new(Some("key2 += &value2"), None).expect("Failed to create lexer");
        let _ = lexer.get_next_token(Some(&[TokenKind::LIndent]), None).expect("indent token");
        let token = lexer.get_next_token(Some(&[TokenKind::LIdentifier]), None).expect("identifier token");
        let identifier = lexer.get_until(&[TokenKind::LAppend], None, Some(true)).expect("get_until identifier");

        let mut node = Node::new();
        let mut dict: HashMap<ParamKey, ParamVal> = HashMap::new();
        dict.insert_str("key1", "value1".to_string().into());

        node.apply_operator(identifier, token, &mut lexer, dict).expect("apply_operator failed");

        // node content should be extended with append operation step
        let content = node.get_content();
        assert_eq!(content.len(), 2);
        // first should be an LApplyDict, second an LAppend
        match &content[0].content_type {
            ContentType::Tokens(Tokens::LApplyDict(_, map)) => {
                assert_eq!(map.get_str("key1"), Some(&"value1".to_string().into()));
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
        let _ = lexer.get_next_token(Some(&[TokenKind::LIndent]), None).expect("indent");
        let _ = lexer.get_next_token(Some(&[TokenKind::LDel]), None).expect("del");

        let mut node = Node::new();
        let mut dict: HashMap<ParamKey, ParamVal> = HashMap::new();
        dict.insert_str("key1", "value1".to_string().into());

        node.apply_deletion(&mut lexer, dict).expect("apply_deletion failed");

        // node content should be extended with delete operation step
        let content = node.get_content();
        assert_eq!(content.len(), 2);
        match &content[0].content_type {
            ContentType::Tokens(Tokens::LApplyDict(_, map)) => {
                assert_eq!(map.get_str("key1"), Some(&"value1".to_string().into()));
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
        let _ = lexer.get_next_token(Some(&[TokenKind::LIndent]), None).expect("indent");
        let token = lexer.get_next_token(Some(&[TokenKind::LIdentifier]), None).expect("identifier");
        let identifier = lexer.get_until(&[TokenKind::LColon], None, Some(true)).expect("get_until");

        let mut node = Node::new();
        let mut dict: HashMap<ParamKey, ParamVal> = HashMap::new();
        dict.insert_str("key1", "value1".to_string().into());

        node.apply_condition(identifier, token, &mut lexer, dict, 0).expect("apply_condition failed");

        let content = node.get_content();
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
        let _ = lexer.get_next_token(Some(&[TokenKind::LIndent]), None).expect("indent");
        let _ = lexer.get_next_token(Some(&[TokenKind::LNotCond]), None).expect("notcond");

        let mut node = Node::new();
        let mut dict: HashMap<ParamKey, ParamVal> = HashMap::new();
        dict.insert_str("key1", "value1".to_string().into());

        node.apply_notcondition(&mut lexer, dict, 0).expect("apply_notcondition failed");

        let content = node.get_content();
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
        let _ = lexer.get_next_token(Some(&[TokenKind::LIndent]), None).expect("indent");
        let _ = lexer.get_next_token(Some(&[TokenKind::LVariants]), None).expect("variants");

        let node = Node::new();
        let (variant_name, meta) = node.apply_variants(&mut lexer).expect("apply_variants failed");

        // variant name should be "test" and meta empty
        assert_eq!(variant_name, "test");
        assert!(meta.is_empty());
        // content should be empty
        let content = node.get_content();
        assert_eq!(content.len(), 0);
    }

    #[test]
    fn test_apply_variants_meta() {
        // content: "variants test [meta1] [meta2=val2] [ meta3 ] [ meta4 = val4 val5 ]:"
        let txt = r#"variants test [meta1] [meta2=val2] [ meta3 ] [ meta4 = val4 val5 ]:"#;
        let mut lexer = Lexer::new(Some(txt), None).expect("Failed to create lexer");
        let _ = lexer.get_next_token(Some(&[TokenKind::LIndent]), None).expect("indent");
        let _ = lexer.get_next_token(Some(&[TokenKind::LVariants]), None).expect("variants");

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
        let content = node.get_content();
        assert_eq!(content.len(), 0);
    }

    #[test]
    fn test_apply_variant() {
        // content: "- test:"
        let mut lexer = Lexer::new(Some("- test:"), None).expect("Failed to create lexer");
        let _ = lexer.get_next_token(Some(&[TokenKind::LIndent]), None).expect("indent");
        let _ = lexer.get_next_token(Some(&[TokenKind::LVariant]), None).expect("dash");

        let mut node = Node::new();
        let mut dict: HashMap<ParamKey, ParamVal> = HashMap::new();
        dict.insert_str("key1", "value1".to_string().into());
        let mut meta = HashMap::new();

        let grandparent_node = node.apply_variant(
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
        let content = node.get_content();
        assert!(!content.is_empty());
        match &content[0].content_type {
            ContentType::Tokens(Tokens::LApplyDict(_, map)) => {
                assert_eq!(map.get_str("key1"), Some(&"value1".to_string().into()));
            }
            other => panic!("Unexpected parent content: {:?}", other),
        }

        // grandparent node should have one child (the variant) whose name is "test"
        let parents = grandparent_node.get_children();
        assert_eq!(parents.len(), 1);
        let parent_node = &parents[0];
        assert_eq!(parent_node.name.len(), 1);
        assert!(matches!(parent_node.name[0], Label { .. }));
        assert_eq!(parent_node.name[0].name, "test".to_string());

        // child should include the original grand child as its child
        let children = parent_node.get_children();
        assert_eq!(children.len(), 1);
        assert_eq!(children[0], node);
    }

    #[test]
    fn test_advance_stops_at_dict() {
        let root = parse_string(
            "variants:\n    - first:\n    - rejected:\n        only missing\n    - last:\n".to_string(),
            Node::new(), None, false, None,
        ).unwrap();
        let mut pre_dict = PreDict::default();
        assert!(pre_dict.update_from_node(Rc::new(root), None).unwrap());
        for name in ["first", "last"] {
            let TraversalStep::Dict(dict) = pre_dict.advance().unwrap() else {
                panic!("Expected a plain dictionary");
            };
            assert_eq!(dict.get_str("name"), Some(&name.to_string().into()));
        }
        assert!(matches!(pre_dict.advance().unwrap(), TraversalStep::Done));
        assert!(matches!(pre_dict.advance().unwrap(), TraversalStep::Done));
    }

    #[test]
    fn test_advance_stops_at_join() {
        let root = parse_string(
            "variants:\n    - a:\n    - b:\njoin a b\n".to_string(),
            Node::new(), None, false, None,
        ).unwrap();
        let mut pre_dict = PreDict::default();
        assert!(pre_dict.update_from_node(Rc::new(root), None).unwrap());
        assert!(matches!(pre_dict.advance().unwrap(), TraversalStep::Join(0, 0)));
        assert_eq!(pre_dict.route, vec![None]);
        assert!(matches!(pre_dict.join_dicts[0].as_deref(), Some([None, None])));
        assert!(matches!(pre_dict.join_pre_dicts[0].as_deref(), Some([None, None])));
    }

    #[test]
    fn test_advance_continues_and_exhausts_join() {
        let root = parse_string(
            "variants:\n    - a:\n    - b:\nvariants:\n    - joined:\n        join a b\n    - plain:\n".to_string(),
            Node::new(), None, false, None,
        ).unwrap();
        let mut pre_dict = PreDict::default();
        assert!(pre_dict.update_from_node(Rc::new(root), None).unwrap());
        let TraversalStep::Join(depth, 0) = pre_dict.advance().unwrap() else {
            panic!("Expected the first join pre-dict");
        };
        let joined_node = Rc::clone(&pre_dict.branch[depth]);
        let mut invalid = pre_dict.clone();
        invalid.join_width[depth] = usize::MAX;
        assert!(invalid.advance().is_err());
        // further advance calls for the same join location repeats the traversal step
        let TraversalStep::Join(depth, 0) = pre_dict.advance().unwrap() else {
            panic!("Expected the first join pre-dict");
        };

        // check that advance requests the second pre-dict
        pre_dict.join_width[depth] = 1;
        assert!(matches!(pre_dict.advance().unwrap(), TraversalStep::Join(d, 1) if d == depth));
        assert!(matches!(pre_dict.join_dicts[depth].as_deref(), Some([None, None])));
        assert!(matches!(pre_dict.join_pre_dicts[depth].as_deref(), Some([None, None])));

        // check that advance skips the exhausted join and backtracks to the next sibling
        pre_dict.join_width[depth] = pre_dict.joins[depth].as_ref().unwrap().len();
        for name in ["plain.a", "plain.b"] {
            let TraversalStep::Dict(dict) = pre_dict.advance().unwrap() else {
                panic!("Expected a dictionary from the next (plain) sibling");
            };
            assert_eq!(dict.get_str("name"), Some(&name.to_string().into()));
            // the joined node should no longer be in the branch
            assert!(!pre_dict.branch.iter().any(|node| Rc::ptr_eq(node, &joined_node)));
        }

        assert!(matches!(pre_dict.advance().unwrap(), TraversalStep::Done));
    }

    #[test]
    fn test_advance_propagates_errors() {
        // malformed content as "no condition" conditional node inside another
        let mut child = Node::new();
        child.add_content("<test>".to_string(), Some(1), ContentType::Node(Rc::new(Node::new())));
        let mut root = Node::new();
        root.append_child(child);
        let root = Rc::new(root);
        let mut pre_dict = PreDict::default();
        // seeding the valid root succeeds
        assert!(pre_dict.update_from_node(Rc::clone(&root), None).unwrap());

        let invalidations: [fn(&mut PreDict); 11] = [
            |p| p.route.clear(), |p| p.join_width.clear(), |p| p.processed_content.clear(),
            |p| p.joins.clear(), |p| p.join_dicts.clear(), |p| p.join_pre_dicts.clear(),
            |p| p._ctx.clear(), |p| p._shortname.clear(), |p| p._dep.clear(),
            |p| p._content.clear(), |p| p._ctx_content.clear(),
        ];
        for invalidate in invalidations {
            let mut invalid = pre_dict.clone();
            invalidate(&mut invalid);
            assert!(invalid.advance().is_err_and(|err| matches!(err, Error::Value(_))));
        }

        // error raised by traversing into the invalid node must reach the caller
        let error = pre_dict.advance().err().expect("Expected a child-entry error");
        assert!(matches!(error, Error::Type(_)));
        // the unsuccessfully entered child was not appended to branch
        assert_eq!(pre_dict.branch.len(), 1);
        assert!(Rc::ptr_eq(&pre_dict.branch[0], &root));

        // backtracking must also report a missing parent route instead of panicking
        let mut root = Node::new();
        root.append_child(Node::new());
        let mut pre_dict = PreDict::default();
        assert!(pre_dict.update_from_node(Rc::new(root), None).unwrap());
        assert!(matches!(pre_dict.advance().unwrap(), TraversalStep::Dict(_)));
        for route in [None, Some(usize::MAX)] {
            let mut invalid = pre_dict.clone();
            invalid.route[0] = route;
            assert!(invalid.advance().is_err_and(|err| matches!(err, Error::Value(_))));
        }
    }

    #[test]
    fn test_next_dict_reuses_join_predicts() {
        // two join pre-dicts with two alternatives each exercise cache reuse and replacement
        let root = parse_string(
            "variants:\n    - x:\n    - y:\nvariants:\n    - a:\n    - b:\njoin a b\n".to_string(),
            Node::new(), None, false, None,
        ).unwrap();
        let mut pre_dict = PreDict::default();
        assert!(pre_dict.update_from_node(Rc::new(root), None).unwrap());

        // the first result leaves the second pre-dict active and the first dictionary cached
        assert_eq!(pre_dict.next_dict().unwrap().unwrap().get_str("name"), Some(&"a.x.b.x".to_string().into()));
        assert_eq!(pre_dict.join_width[0], 1);
        // recomputing the first pre-dict would lose this marker from its cached dictionary
        pre_dict.join_dicts[0].as_mut().unwrap()[0].as_mut().unwrap()
            .insert_str("key_of_cached_predict", "retained".to_string().into());

        // advancing the second pre-dict must reuse the first dictionary and leave its route unchanged
        let first_route = pre_dict.join_pre_dicts[0].as_ref().unwrap()[0].as_ref().unwrap().route.clone();
        let dict = pre_dict.next_dict().unwrap().unwrap();
        assert_eq!(dict.get_str("name"), Some(&"a.x.b.y".to_string().into()));
        assert_eq!(dict.get_str("key_of_cached_predict"), Some(&"retained".to_string().into()));
        assert_eq!(pre_dict.join_pre_dicts[0].as_ref().unwrap()[0].as_ref().unwrap().route, first_route);
        assert_eq!(pre_dict.join_width[0], 1);

        // after exhausting the second pre-dict, advancing the first replaces its marked dictionary
        for name in ["a.y.b.x", "a.y.b.y"] {
            let dict = pre_dict.next_dict().unwrap().unwrap();
            assert_eq!(dict.get_str("name"), Some(&name.to_string().into()));
            assert!(dict.get_str("key_of_cached_predict").is_none());
        }
        // exhausting the join records its end position and subsequent calls remain exhausted
        assert!(pre_dict.next_dict().unwrap().is_none());
        assert_eq!(pre_dict.join_width[0], 2);
        assert!(pre_dict.next_dict().unwrap().is_none());
    }

    #[test]
    fn test_next_dict_restores_predict_after_error() {
        // merging must report a missing or uninitialized join cache
        let mut pre_dict = PreDict::default();
        for dicts in [Vec::new(), vec![None]] {
            pre_dict.join_dicts = dicts;
            let error = pre_dict.joined_dict(0).expect_err("Expected an uninitialized join cache error");
            assert!(matches!(error, Error::Value(_)));
        }

        // a missing condition fails during initialization; string content fails during dictionary evaluation
        for (content_type, initialization_error) in [
            (ContentType::Node(Rc::new(Node::new())), true),
            (ContentType::String("invalid dictionary content".to_string()), false),
        ] {
            let root = parse_string(
                "variants:\n    - a:\n    - b:\njoin a b\n".to_string(),
                Node::new(), None, false, None,
            ).unwrap();
            let root = Rc::new(root);
            let mut pre_dict = PreDict::default();
            // seeding the valid root succeeds before malformed content is added to the prepared join
            assert!(pre_dict.update_from_node(Rc::clone(&root), None).unwrap());
            pre_dict.prepare_joins(0);
            // inject the invalid content step
            pre_dict.processed_content[0].as_mut().unwrap().push(ContentStep {
                filename: "<test>".to_string(),
                linenum: Some(1),
                content_type,
            });

            // either error must reach the caller after the driver restores its evaluator
            let error = pre_dict.next_dict().expect_err("Expected a pre-dict initialization or evaluation error");
            if initialization_error {
                assert!(matches!(error, Error::Type(_)));
            } else {
                assert!(matches!(error, Error::Runtime(_)));
            }

            // the parent keeps its root and join position, with no pre-dict results cached
            assert_eq!(pre_dict.branch.len(), 1);
            assert!(Rc::ptr_eq(&pre_dict.branch[0], &root));
            assert_eq!(pre_dict.join_width[0], 0);
            assert!(matches!(pre_dict.join_dicts[0].as_deref(), Some([None, None])));
            let predicts = pre_dict.join_pre_dicts[0].as_ref().unwrap();
            // initialization fails before entering the root while evaluation fails after (nonempty branch)
            assert_eq!(predicts[0].as_ref().unwrap().branch.is_empty(), initialization_error);
            // the second pre-dict was never initialized
            assert!(predicts[1].is_none());
        }
    }

    #[test]
    fn test_pre_dict_failed_case_reuse() {
        let node = Rc::new(parse_string(
            "only test2\n".to_string(), Node::new(), None, false, None,
        ).unwrap());
        let mut failed = PreDict::default();
        assert!(!failed.update_from_node(Rc::clone(&node), None).unwrap());
        assert_eq!(failed.failed_cases[&node.id], VecDeque::from([
            (Vec::new(), Vec::new(), vec![node.content.last().unwrap().clone()]),
        ]));

        // revisiting the same failure reuses its record and still pushes a frame
        failed.reset_from_last_node();
        assert!(!failed.update_from_node(Rc::clone(&node), None).unwrap());
        assert_eq!(failed.failed_cases[&node.id].len(), 1);
        assert_eq!(failed.branch.len(), 1);
        assert_eq!(failed.route, vec![None]);

        // another evaluator can accept the same node with a different context
        let mut passing = PreDict::new(
            Some(vec![Label::new("test2".to_string(), None)]),
            None, None, None, None, None,
        );
        assert!(passing.failed_cases.is_empty());
        assert!(passing.update_from_node(node, None).unwrap());
        let dict = passing.get_dicts(false, true).unwrap().unwrap();
        assert_eq!(dict.get_str("name"), Some(&ParamVal::String("test2".to_string())));
        assert!(passing.failed_cases.is_empty());
    }

    #[test]
    fn test_tree_traversal_and_further_parsing() {
        let ast = Tree::new(None).parse_string(
            "variants:\n    - a:\n    - b:\n".to_string(), false, None,
        ).unwrap();
        let common = &ast.root.children[0].children[0];
        assert!(Rc::ptr_eq(common, &ast.root.children[1].children[0]));
        let mut pre_dict = PreDict::default();
        assert!(pre_dict.update_from_tree(&ast).unwrap());
        assert!(Rc::ptr_eq(&ast.root, &pre_dict.branch[0]));
        assert!(pre_dict.get_dicts(false, true).unwrap().is_some());
        for pair in pre_dict.branch.windows(2) {
            assert!(pair[0].children.iter().any(|child| Rc::ptr_eq(child, &pair[1])));
        }
        let copied = pre_dict.clone();
        for (original, shared) in pre_dict.branch.iter().zip(&copied.branch) {
            assert!(Rc::ptr_eq(original, shared));
        }
        let extended = ast.parse_string("extra = value\n".to_string(), false, None).unwrap();
        assert!(!Rc::ptr_eq(&ast.root, &extended.root));
        assert!(Rc::ptr_eq(&ast.root.children[0], &extended.root.children[0]));
    }
}
