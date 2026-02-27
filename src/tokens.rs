use std::borrow::Cow;
use std::cmp;
use std::fmt;
use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::sync::LazyLock;
use regex::Regex;

use pyo3::prelude::*;
use pyo3::types::{PyDict, PyTuple};
use pyo3::exceptions::{PyAttributeError, PyValueError};

const RESERVED_KEYS: &[&str] = &[
    "name",
    "shortname",
    "dep",
    "_short_name_map_file",
    "_name_map_file",
];

#[derive(Eq, PartialEq, Clone, Hash)]
pub enum ParamKey {
    String(String),
    Tuple(Vec<String>),
}
impl From<String> for ParamKey {
    fn from(s: String) -> Self {
        ParamKey::String(s)
    }
}
impl<'a> From<&'a ParamKey> for Cow<'a, str> {
    fn from(p: &'a ParamKey) -> Self {
        match p {
            ParamKey::String(s) => Cow::Borrowed(s.as_str()),
            ParamKey::Tuple(v) => {
                let concatenated = v.iter().fold(String::new(), |mut acc, item| {
                    acc.push_str(item);
                    acc
                });
                Cow::Owned(concatenated)
            }
        }
    }
}
impl fmt::Display for ParamKey {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ParamKey::String(key) => {
                f.write_str(key)?;
            }
            ParamKey::Tuple(key) => {
                for value in key {
                    f.write_str(value)?;
                }
            }
        }
        Ok(())
    }
}
impl fmt::Debug for ParamKey {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self)
    }
}
impl<'py> IntoPyObject<'py> for ParamKey {
    type Target = PyAny;
    type Output = Bound<'py, Self::Target>;
    type Error = PyErr;

    fn into_pyobject(self, py: Python<'py>) -> Result<Self::Output, Self::Error> {
        use pyo3::IntoPyObjectExt;

        match self {
            ParamKey::String(s) => s.into_bound_py_any(py),
            ParamKey::Tuple(v) => {
                let py_tuple = PyTuple::new(py, v)?;
                py_tuple.into_bound_py_any(py)
            }
        }
    }
}
impl<'py> FromPyObject<'_, 'py> for ParamKey {
    type Error = PyErr;

    fn extract(object: Borrowed<'_, 'py, PyAny>) -> Result<Self, Self::Error> {
        if let Ok(s) = object.extract::<String>() {
            Ok(ParamKey::String(s))
        }
        else {
            let py_tuple = object.cast::<PyTuple>()?;
            let tuple = py_tuple.as_slice();
            let vector = tuple.iter().filter_map(|item| {
                item.extract::<String>().ok()
            }).collect();
            Ok(ParamKey::Tuple(vector))
        }
    }
}
#[derive(PartialEq, Clone)]
pub enum ParamVal {
    String(String),
    List(Vec<String>),
    Dict(HashMap<String, String>),
}
impl From<String> for ParamVal {
    fn from(s: String) -> Self {
        ParamVal::String(s)
    }
}
impl<'a> From<&'a ParamVal> for Cow<'a, str> {
    fn from(p: &'a ParamVal) -> Self {
        match p {
            ParamVal::String(s) => Cow::Borrowed(s.as_str()),
            ParamVal::List(v) => {
                let concatenated = v.iter().fold(String::new(), |mut acc, item| {
                    acc.push_str(item);
                    acc
                });
                Cow::Owned(concatenated)
            }
            ParamVal::Dict(h) => {
                let concatenated = h.iter().fold(String::new(), |mut acc, item| {
                    acc.push_str(item.0);
                    acc.push('=');
                    acc.push_str(item.1);
                    acc.push(';');
                    acc
                });
                Cow::Owned(concatenated)
            }
        }
    }
}
impl fmt::Display for ParamVal {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ParamVal::String(value) => {
                f.write_str(value)?;
            }
            ParamVal::List(values) => {
                for value in values {
                    f.write_str(value)?;
                    f.write_str(",")?;
                }
            }
            ParamVal::Dict(hash) => {
                for (key, value) in hash {
                    f.write_str(key)?;
                    f.write_str("=")?;
                    f.write_str(value)?;
                    f.write_str(";")?;
                }
            }
        }
        Ok(())
    }
}
impl fmt::Debug for ParamVal {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self)
    }
}
impl<'py> IntoPyObject<'py> for ParamVal {
    type Target = PyAny;
    type Output = Bound<'py, Self::Target>;
    type Error = PyErr;

    fn into_pyobject(self, py: Python<'py>) -> Result<Self::Output, Self::Error> {
        use pyo3::IntoPyObjectExt;

        match self {
            ParamVal::String(s) => s.into_bound_py_any(py),
            ParamVal::List(v) => v.into_bound_py_any(py),
            ParamVal::Dict(h) => h.into_bound_py_any(py),
        }
    }
}
impl<'py> FromPyObject<'_, 'py> for ParamVal {
    type Error = PyErr;

    fn extract(object: Borrowed<'_, 'py, PyAny>) -> Result<Self, Self::Error> {
        if let Ok(s) = object.extract::<String>() {
            Ok(ParamVal::String(s))
        }
        else if let Ok(v) = object.extract::<Vec<String>>() {
            Ok(ParamVal::List(v))
        }
        else if let Ok(h) = object.extract::<HashMap<String, String>>() {
            Ok(ParamVal::Dict(h))
        } else {
            let s: String = object.extract()?;
            Ok(ParamVal::String(s))
        }
    }
}

// Define an enum for the different types of tokens
#[pyclass(eq)]
#[derive(Debug, PartialEq, Clone)]
pub enum Tokens {
    LIndent(isize),
    LEndL(),
    LEndBlock(isize),
    LIdentifier(String),
    LWhite(String),
    LString(String),
    LColon(),
    LVariants(),
    LDot(),
    LVariant(),
    LDefault(),
    LOnly(),
    LSuffix(),
    LJoin(),
    LNo(),
    LCond(),
    LNotCond(),
    LOr(),
    LAnd(),
    LCoc(),
    LComa(),
    LLBracket(),
    LRBracket(),
    LLRBracket(),
    LRRBracket(),
    LRegExpStart(),
    LRegExpStop(),
    LInclude(),
    LSet(String, String),
    LAppend(String, String),
    LPrepend(String, String),
    LLazySet(String, String),
    LRegExpSet(String, String),
    LRegExpAppend(String, String),
    LRegExpPrepend(String, String),
    LDel(String, String),
    LApplyDict(String, HashMap<ParamKey, ParamVal>),
    LUpdateFileMap(String, String, String),
    Suffix(String, String),
}
// Implement the Display trait for Tokens
impl fmt::Display for Tokens {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Tokens::LIndent(length) => write!(f, "indent {length}"),
            Tokens::LEndL() => write!(f, "endl"),
            Tokens::LEndBlock(length) => write!(f, "endb {length}"),
            Tokens::LIdentifier(string) => write!(f, "Identifier re([A-Za-z0-9][A-Za-z0-9_-]*) \"{string}\""),
            Tokens::LWhite(string) => write!(f, "WhiteSpace re(\\s) \"{string}\""),
            Tokens::LString(string) => write!(f, "String re(.+) \"{string}\""),
            Tokens::LColon() => write!(f, ":"),
            Tokens::LVariants() => write!(f, "variants"),
            Tokens::LDot() => write!(f, "."),
            Tokens::LVariant() => write!(f, "-"),
            Tokens::LDefault() => write!(f, "@"),
            Tokens::LOnly() => write!(f, "only"),
            Tokens::LSuffix() => write!(f, "suffix"),
            Tokens::LJoin() => write!(f, "join"),
            Tokens::LNo() => write!(f, "no"),
            Tokens::LCond() => write!(f, "?"),
            Tokens::LNotCond() => write!(f, "!"),
            Tokens::LOr() => write!(f, ","),
            Tokens::LAnd() => write!(f, ".."),
            Tokens::LCoc() => write!(f, "."),
            Tokens::LComa() => write!(f, ","),
            Tokens::LLBracket() => write!(f, "["),
            Tokens::LRBracket() => write!(f, "]"),
            Tokens::LLRBracket() => write!(f, "("),
            Tokens::LRRBracket() => write!(f, ")"),
            Tokens::LRegExpStart() => write!(f, "${{"),
            Tokens::LRegExpStop() => write!(f, "}}"),
            Tokens::LInclude() => write!(f, "include"),
            Tokens::LSet(_name, _value) => write!(f, "="),
            Tokens::LAppend(_name, _value) => write!(f, "+="),
            Tokens::LPrepend(_name, _value) => write!(f, "<="),
            Tokens::LLazySet(_name, _value) => write!(f, "~="),
            Tokens::LRegExpSet(_name, _value) => write!(f, "?="),
            Tokens::LRegExpAppend(_name, _value) => write!(f, "?+="),
            Tokens::LRegExpPrepend(_name, _value) => write!(f, "?<="),
            Tokens::LDel(_name, _value) => write!(f, "del"),
            Tokens::LApplyDict(_name, value) => write!(f, "apply_dict {value:?}"),
            Tokens::LUpdateFileMap(_filename, _name, _value) => write!(f, "update_file_map"),
            Tokens::Suffix(_name, value) => write!(f, "suffix {value}"),
        }
    }
}

#[pymethods]
impl Tokens {
    #[getter]
    fn identifier(&self) -> String {
        self.to_string()
    }

    fn __str__(&self) -> String {
        self.to_string()
    }

    fn __repr__(&self) -> String {
        let s = self.__str__();
        format!("'{s}'")
    }

    #[staticmethod]
    pub fn default(identifier: &str) -> Self {
        // NOTE: based on identifier string and thus a static method instead
        // of a simpler class method due to rust introspection limitations,
        // same reason why we don't pass the types as arguments to the lexer
        match identifier {
            // fast exact matches
            "endl" => Tokens::LEndL(),
            ":" => Tokens::LColon(),
            "variants" => Tokens::LVariants(),
            "." => Tokens::LDot(),
            "-" => Tokens::LVariant(),
            "@" => Tokens::LDefault(),
            "only" => Tokens::LOnly(),
            "suffix" => Tokens::LSuffix(),
            "join" => Tokens::LJoin(),
            "no" => Tokens::LNo(),
            "?" => Tokens::LCond(),
            "!" => Tokens::LNotCond(),
            "," => Tokens::LComa(),
            ".." => Tokens::LAnd(),
            "[" => Tokens::LLBracket(),
            "]" => Tokens::LRBracket(),
            "(" => Tokens::LLRBracket(),
            ")" => Tokens::LRRBracket(),
            "${" | "${{" => Tokens::LRegExpStart(),
            "}}" => Tokens::LRegExpStop(),
            "include" => Tokens::LInclude(),
            "=" => Tokens::LSet(String::new(), String::new()),
            "+=" => Tokens::LAppend(String::new(), String::new()),
            "<=" => Tokens::LPrepend(String::new(), String::new()),
            "~=" => Tokens::LLazySet(String::new(), String::new()),
            "?=" => Tokens::LRegExpSet(String::new(), String::new()),
            "?+=" => Tokens::LRegExpAppend(String::new(), String::new()),
            "?<=" => Tokens::LRegExpPrepend(String::new(), String::new()),
            "del" => Tokens::LDel(String::new(), String::new()),
            "update_file_map" => Tokens::LUpdateFileMap(String::new(), String::new(), String::new()),
            other => {
                // continue to heuristic matches below
                let id = other.to_string();
                if id.starts_with("indent") { return Tokens::LIndent(0); }
                if id.starts_with("endb") { return Tokens::LEndBlock(0); }
                if id.starts_with("Identifier") { return Tokens::LIdentifier(String::new()); }
                if id.starts_with("WhiteSpace") { return Tokens::LWhite(String::new()); }
                if id.starts_with("String") { return Tokens::LString(String::new()); }
                if id.starts_with("apply_dict") {
                    return Tokens::LApplyDict(String::new(), HashMap::new());
                }
                if id.starts_with("suffix") { return Tokens::Suffix(String::new(), String::new()); }
                // fallback: return as identifier token
                Tokens::LIdentifier(id)
            }
        }
    }

    pub fn like(&self, name: String, value: String) -> PyResult<Self> {
        match self {
            Tokens::LSet(_name, _value) => Ok(Tokens::LSet(name, value)),
            Tokens::LAppend(_name, _value) => Ok(Tokens::LAppend(name, value)),
            Tokens::LPrepend(_name, _value) => Ok(Tokens::LPrepend(name, value)),
            Tokens::LLazySet(_name, _value) => Ok(Tokens::LLazySet(name, value)),
            Tokens::LRegExpSet(_name, _value) => Ok(Tokens::LRegExpSet(name, value)),
            Tokens::LRegExpAppend(_name, _value) => Ok(Tokens::LRegExpAppend(name, value)),
            Tokens::LRegExpPrepend(_name, _value) => Ok(Tokens::LRegExpPrepend(name, value)),
            Tokens::LDel(_name, _value) => Ok(Tokens::LDel(name, value)),
            _ => Err(PyAttributeError::new_err("like is not a valid attribute for this token")),
        }
    }

    #[getter]
    pub fn length(&self) -> PyResult<isize> {
        match self {
            Tokens::LIndent(length) => Ok(*length),
            Tokens::LEndBlock(length) => Ok(*length),
            _ => Err(PyAttributeError::new_err("length is not a valid attribute for this token")),
        }
    }

    #[getter]
    pub fn string(&self) -> PyResult<String> {
        match self {
            Tokens::LIdentifier(string) => Ok(string.clone()),
            Tokens::LWhite(string) => Ok(string.clone()),
            Tokens::LString(string) => Ok(string.clone()),
            _ => Err(PyAttributeError::new_err("string is not a valid attribute for this token")),
        }
    }

    #[getter]
    pub fn name(&self) -> PyResult<String> {
        match self {
            Tokens::LSet(name, _value) => Ok(name.clone()),
            Tokens::LAppend(name, _value) => Ok(name.clone()),
            Tokens::LPrepend(name, _value) => Ok(name.clone()),
            Tokens::LLazySet(name, _value) => Ok(name.clone()),
            Tokens::LRegExpSet(name, _value) => Ok(name.clone()),
            Tokens::LRegExpAppend(name, _value) => Ok(name.clone()),
            Tokens::LRegExpPrepend(name, _value) => Ok(name.clone()),
            Tokens::LDel(name, _value) => Ok(name.clone()),
            Tokens::LApplyDict(name, _value) => Ok(name.clone()),
            Tokens::LUpdateFileMap(_filename, name, _value) => Ok(name.clone()),
            Tokens::Suffix(name, _value) => Ok(name.clone()),
            _ => Err(PyAttributeError::new_err("name is not a valid attribute for this token")),
        }
    }

    #[getter]
    fn value(&self) -> PyResult<String> {
        match self {
            Tokens::LSet(_name, value) => Ok(value.clone()),
            Tokens::LAppend(_name, value) => Ok(value.clone()),
            Tokens::LPrepend(_name, value) => Ok(value.clone()),
            Tokens::LLazySet(_name, value) => Ok(value.clone()),
            Tokens::LRegExpSet(_name, value) => Ok(value.clone()),
            Tokens::LRegExpAppend(_name, value) => Ok(value.clone()),
            Tokens::LRegExpPrepend(_name, value) => Ok(value.clone()),
            Tokens::LDel(_name, value) => Ok(value.clone()),
            Tokens::LUpdateFileMap(_filename, _name, value) => Ok(value.clone()),
            Tokens::Suffix(_name, value) => Ok(value.clone()),
            _ => Err(PyAttributeError::new_err("value is not a valid attribute for this token")),
        }
    }

    #[getter]
    fn filename(&self) -> PyResult<String> {
        match self {
            Tokens::LUpdateFileMap(filename, _name, _value) => Ok(filename.clone()),
            _ => Err(PyAttributeError::new_err("filename is not a valid attribute for this token")),
        }
    }

    #[pyo3(name = "apply_to_dict")]
    fn apply_to_pydict(&self, py_dict: &Bound<'_, PyDict>) -> PyResult<()> {
        let mut dict = py_dict.extract::<HashMap<ParamKey, ParamVal>>()?;
        self.clone().apply_to_dict(&mut dict)?;

        py_dict.clear();
        for (key, value) in dict.iter() {
            py_dict.set_item(key.clone(), value.clone())?;
        }

        Ok(())
    }
}
impl Tokens {
    pub fn apply_to_dict(self, dict: &mut HashMap<ParamKey, ParamVal>) -> PyResult<()> {
        match self {
            Tokens::LSet(name, value) => {
                if !RESERVED_KEYS.contains(&name.as_str()) {
                    let current_key = ParamKey::from(name);
                    let substituted = substitution(value, dict)?;
                    dict.insert(current_key, substituted.into());
                }
                Ok(())
            }
            Tokens::LAppend(name, value) => {
                if !RESERVED_KEYS.contains(&name.as_str()) {
                    let current_key = ParamKey::from(name);
                    let substituted = substitution(value, dict)?;
                    dict.entry(current_key)
                        .and_modify(|existing| {
                            *existing = format!("{existing}{substituted}").into();
                        })
                        .or_insert_with(|| substituted.into());
                }
                Ok(())
            }
            Tokens::LPrepend(name, value) => {
                if !RESERVED_KEYS.contains(&name.as_str()) {
                    let current_key = ParamKey::from(name);
                    let substituted = substitution(value, dict)?;
                    dict.entry(current_key)
                        .and_modify(|existing| {
                            *existing = format!("{substituted}{existing}").into();
                        })
                        .or_insert_with(|| substituted.into());
                }
                Ok(())
            }
            Tokens::LLazySet(name, value) => {
                if !RESERVED_KEYS.contains(&name.as_str()) {
                    let current_key = ParamKey::from(name);
                    if !dict.contains_key(&current_key) {
                        let substituted = substitution(value, dict)?;
                        dict.insert(current_key, substituted.into());
                    }
                }
                Ok(())
            }
            Tokens::LRegExpSet(name, value) => {
                let exp = Regex::new(&format!(r"^{name}$"))
                   .map_err(|e| PyErr::new::<PyValueError, _>(e.to_string()))?;
                let substituted = substitution(value, dict)?;
                let substituted_val = ParamVal::from(substituted);
                for (key, val) in dict.iter_mut() {
                    let key_str = Cow::from(key);
                    if !RESERVED_KEYS.contains(&key_str.as_ref()) && exp.is_match(&key_str) {
                        *val = substituted_val.clone();
                    }
                }
                Ok(())
            }
            Tokens::LRegExpAppend(name, value) => {
                let exp = Regex::new(&format!(r"^{name}$"))
                   .map_err(|e| PyErr::new::<PyValueError, _>(e.to_string()))?;
                let substituted = substitution(value, dict)?;
                for (key, val) in dict.iter_mut() {
                    let key_str = Cow::from(key);
                    if !RESERVED_KEYS.contains(&key_str.as_ref()) && exp.is_match(&key_str) {
                        *val = format!("{val}{substituted}").into();
                    }
                }
                Ok(())
            }
            Tokens::LRegExpPrepend(name, value) => {
                let exp = Regex::new(&format!(r"^{name}$"))
                   .map_err(|e| PyErr::new::<PyValueError, _>(e.to_string()))?;
                let substituted = substitution(value, dict)?;
                for (key, val) in dict.iter_mut() {
                    let key_str = Cow::from(key);
                    if !RESERVED_KEYS.contains(&key_str.as_ref()) && exp.is_match(&key_str) {
                        *val = format!("{substituted}{val}").into();
                    }
                }
                Ok(())
            }
            Tokens::LDel(name, _val) => {
                let exp = Regex::new(&format!(r"^{name}$"))
                    .map_err(|e| PyErr::new::<PyValueError, _>(e.to_string()))?;
                dict.retain(|key, _value| {
                    let key_str = Cow::from(key);
                    RESERVED_KEYS.contains(&key_str.as_ref()) || !exp.is_match(&key_str)
                });
                Ok(())
            }
            Tokens::LApplyDict(_, value_map) => {
                dict.extend(value_map);
                Ok(())
            }
            Tokens::LUpdateFileMap(filename, name, value) => {
                let shortname = if filename == "<string>" {
                    Cow::Owned(filename)
                } else {
                    Cow::Borrowed(
                        std::path::Path::new(&filename)
                            .file_name()
                            .and_then(|os_str| os_str.to_str())
                            .unwrap_or(&filename)
                    )
                };

                let dest_key = ParamKey::from(value);

                // Use entry API to handle the dest_key dict creation and access
                match dict.entry(dest_key) {
                    Entry::Occupied(mut entry) => {
                        match entry.get_mut() {
                            ParamVal::Dict(inner_dict) => {
                                // Process the inner dict in place
                                match inner_dict.entry(shortname.into_owned()) {
                                    Entry::Occupied(mut inner_entry) => {
                                        let old_name = inner_entry.get();
                                        *inner_entry.get_mut() = format!("{name}.{old_name}");
                                    }
                                    Entry::Vacant(inner_vacant) => {
                                        inner_vacant.insert(name);
                                    }
                                }
                            }
                            _ => {
                                // Create new dict if key exists but is not a dict
                                let mut new_dict = HashMap::new();
                                new_dict.insert(shortname.into_owned(), name);
                                *entry.get_mut() = ParamVal::Dict(new_dict);
                            }
                        }
                    }
                    Entry::Vacant(vacant) => {
                        // Ensure destination key exists as a dict
                        let mut new_dict = HashMap::new();
                        new_dict.insert(shortname.into_owned(), name);
                        vacant.insert(ParamVal::Dict(new_dict));
                    }
                }

                Ok(())
            }
            Tokens::Suffix(_, value) => {
                // Drain the entire map and rebuild with new keys
                let old_entries: Vec<_> = dict.drain().collect();

                // Create suffixed mapping: turn string keys into tuple keys where possible
                for (k, v) in old_entries {
                    match k {
                        ParamKey::String(s) => {
                            if RESERVED_KEYS.contains(&s.as_str()) {
                                dict.insert(ParamKey::String(s), v);
                            } else {
                                dict.insert(ParamKey::Tuple(vec![s, value.clone()]), v);
                            }
                        }
                        ParamKey::Tuple(mut vec) => {
                            vec.push(value.clone());
                            dict.insert(ParamKey::Tuple(vec), v);
                        }
                    }
                }
                Ok(())
            }
            _ => Err(PyAttributeError::new_err("apply_to_dict is not a valid attribute for this token")),
        }
    }
}

fn convert_data_size(size: &str, default_suffix: &str) -> Result<i64, String> {
    let orders: HashMap<&str, i64> = [
        ("B", 1),
        ("K", 1024),
        ("M", 1024 * 1024),
        ("G", 1024 * 1024 * 1024),
        ("T", 1024 * 1024 * 1024 * 1024),
    ]
    .iter()
    .cloned()
    .collect();

    let (number_part, suffix) = if let Some(last_char) = size.chars().last() {
        if "BbKkMmGgTt".contains(last_char) {
            (&size[..size.len() - 1], last_char.to_uppercase().to_string())
        } else {
            (size, default_suffix.to_string())
        }
    } else {
        (size, default_suffix.to_string())
    };

    let number: f64 = number_part
        .parse()
        .map_err(|e: std::num::ParseFloatError| e.to_string())?;
    let multiplier = orders.get(suffix.as_str()).copied().unwrap_or(1);

    Ok((number * multiplier as f64) as i64)
}

fn compare_data_size(a: &str, b: &str) -> cmp::Ordering {
    // Check if either string contains a size suffix
    let has_suffix1 = a
        .chars()
        .last()
        .is_some_and(|c| "BbKkMmGgTt".contains(c));
    let has_suffix2 = b
        .chars()
        .last()
        .is_some_and(|c| "BbKkMmGgTt".contains(c));

    if has_suffix1 || has_suffix2 {
        match (
            convert_data_size(a, "M"),
            convert_data_size(b, "M"),
        ) {
            (Ok(v1), Ok(v2)) => v1.cmp(&v2),
            _ => a.cmp(b),
        }
    } else {
        match (a.parse::<i64>(), b.parse::<i64>()) {
            (Ok(v1), Ok(v2)) => v1.cmp(&v2),
            _ => a.cmp(b),
        }
    }
}

pub fn apply_suffix_bounds(dict: &mut HashMap<ParamKey, ParamVal>) {
    for key in dict.keys().cloned().collect::<Vec<_>>() {
        match key {
            ParamKey::Tuple(_) => {
                // Skip tuple keys as they are generated from suffixes and should not be processed for bounds
            }
            ParamKey::String(ref key_str) if key_str.ends_with("_max") => {
                let tmp_key = key_str.trim_end_matches("_max").to_string();
                if !dict.contains_key(&ParamKey::String(tmp_key.clone())) ||
                    compare_data_size(
                        &dict[&ParamKey::String(tmp_key.clone())].to_string(),
                        &dict[&key].to_string()
                    ) > cmp::Ordering::Equal {
                    dict.insert(ParamKey::String(tmp_key), dict[&key].clone());
                }
            }
            ParamKey::String(ref key_str) if key_str.ends_with("_min") => {
                let tmp_key = key_str.trim_end_matches("_min").to_string();
                if !dict.contains_key(&ParamKey::String(tmp_key.clone())) ||
                    compare_data_size(
                        &dict[&ParamKey::String(tmp_key.clone())].to_string(),
                        &dict[&key].to_string()
                    ) < cmp::Ordering::Equal {
                    dict.insert(ParamKey::String(tmp_key), dict[&key].clone());
                }
            }
            ParamKey::String(ref key_str) if key_str.ends_with("_fixed") => {
                let tmp_key = key_str.trim_end_matches("_fixed").to_string();
                dict.insert(ParamKey::String(tmp_key), dict[&key].clone());
            }
            _ => {}
        }
    }
}

pub fn drop_suffixes(dict: &HashMap<ParamKey, ParamVal>, skipdups: bool) -> PyResult<HashMap<ParamKey, ParamVal>> {
    let d_flat: HashMap<ParamKey, ParamVal> = dict
        .iter()
        .filter_map(|(key, value)| {
            match key {
                ParamKey::String(_) => Some((key.clone(), value.clone())),
                ParamKey::Tuple(key_vec) => {
                    let gen_key_str = key_vec.first()?.clone();
                    let gen_key = ParamKey::String(gen_key_str.clone());
                    let mut can_drop_all_suffixes = true;

                    if skipdups {
                        let value_str = Cow::from(value);
                        if let Some(gen_value) = dict.get(&gen_key) {
                            let gen_value_str = Cow::from(gen_value);
                            if gen_value_str == value_str {
                                return None; // Skip duplicate suffixes
                            } else {
                                can_drop_all_suffixes = false;
                            }
                        }

                        if can_drop_all_suffixes {
                            can_drop_all_suffixes = dict
                                .iter()
                                .filter_map(|(other_key, other_value)| {
                                    match other_key {
                                        ParamKey::Tuple(other_vec) => {
                                            other_vec.first()
                                                .filter(|k| *k == &gen_key_str)
                                                .map(|_| Cow::from(other_value))
                                        }
                                        _ => None,
                                    }
                                })
                                .all(|other_value_str| other_value_str == value_str);
                        }
                    }

                    let new_key = if skipdups && can_drop_all_suffixes {
                        gen_key_str
                    } else {
                        let mut suffix_parts = key_vec[1..].to_vec();
                        suffix_parts.reverse();
                        format!("{}{}", key_vec[0], suffix_parts.join(""))
                    };

                    Some((new_key.into(), value.clone()))
                }
            }
        })
        .collect();

    Ok(d_flat)
}

static MATCH_SUBSTITUTE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\$\{(.+?)\}")
    .expect("Invalid MATCH_SUBSTITUTE pattern")
});

pub fn substitution(value: String, dict: &HashMap<ParamKey, ParamVal>) -> PyResult<String> {
    if !value.contains('$') {
        return Ok(value);
    }
    let mut start = 0;
    let mut result = String::with_capacity(value.len());

    let d = drop_suffixes(dict, true)?;

    while let Some(captures) = MATCH_SUBSTITUTE.captures(&value[start..]) {
        if let Some(matched) = captures.get(0) {
            let key = captures.get(1).map_or("", |m| m.as_str());
            if let Some(val) = d.get(&key.to_string().into()) {
                result.push_str(&value[start..start + matched.start()]);
                result.push_str(&val.to_string());
                start += matched.end();
            } else {
                break;
            }
        }
    }
    result.push_str(&value[start..]);
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_display() {
        // all tokens are tested via python tests to have end-to-end coverage
        let t1 = Tokens::LIndent(42);
        assert_eq!(format!("{t1}"), "indent 42");
    }

    #[test]
    fn test_debug() {
        // all tokens are tested via python tests to have end-to-end coverage
        let t1 = Tokens::LIndent(42);
        assert_eq!(format!("{t1:?}"), "LIndent(42)");
    }

    #[test]
    fn test_equality() {
        let t1 = Tokens::LIndent(42);
        let t2 = Tokens::LIndent(42);
        let t3 = Tokens::LIndent(2);
        // reflexivity of Eq
        assert!(t1 == t1);
        // commutativity of Eq
        assert!(t1 == t2);
        assert!(t2 == t1);
        // inequality
        assert!(t1 != t3);
        assert!(t2 != t3);
    }

    #[test]
    fn test_convert_data_size() {
        assert_eq!(convert_data_size("1B", "B"), Ok(1));
        assert_eq!(convert_data_size("1K", "B"), Ok(1024));
        assert_eq!(convert_data_size("1M", "B"), Ok(1024 * 1024));
        assert_eq!(convert_data_size("1G", "B"), Ok(1024 * 1024 * 1024));
        assert_eq!(convert_data_size("1T", "B"), Ok(1024 * 1024 * 1024 * 1024));
        assert_eq!(convert_data_size("1", "B"), Ok(1));
        assert_eq!(convert_data_size("1", "K"), Ok(1024));
    }

    #[test]
    fn test_compare_data_size() {
        assert_eq!(compare_data_size("1B", "1B"), cmp::Ordering::Equal);
        assert_eq!(compare_data_size("1K", "1B"), cmp::Ordering::Greater);
        assert_eq!(compare_data_size("1B", "1K"), cmp::Ordering::Less);
        assert_eq!(compare_data_size("1M", "1024K"), cmp::Ordering::Equal);
        assert_eq!(compare_data_size("1G", "1024M"), cmp::Ordering::Equal);
        assert_eq!(compare_data_size("1T", "1024G"), cmp::Ordering::Equal);
        assert_eq!(compare_data_size("1", "1"), cmp::Ordering::Equal);
        assert_eq!(compare_data_size("2", "1"), cmp::Ordering::Greater);
        assert_eq!(compare_data_size("1", "2"), cmp::Ordering::Less);
        assert_eq!(compare_data_size("1.5G", "1.5G"), cmp::Ordering::Equal);
        assert_eq!(compare_data_size("2G", "1.5G"), cmp::Ordering::Greater);
        assert_eq!(compare_data_size("1.5G", "2G"), cmp::Ordering::Less);
    }

    #[test]
    fn test_apply_suffix_bounds() {
        let mut d: HashMap<ParamKey, ParamVal> = [
            (ParamKey::String("size_max".to_string()), ParamVal::String("2G".to_string())),
            (ParamKey::String("size_min".to_string()), ParamVal::String("1G".to_string())),
            (ParamKey::String("size".to_string()), ParamVal::String("2.5G".to_string())),
            (ParamKey::String("speed_fixed".to_string()), ParamVal::String("100M".to_string())),
            (ParamKey::String("speed".to_string()), ParamVal::String("50M".to_string())),
        ].iter().cloned().collect();
        apply_suffix_bounds(&mut d);
        assert_eq!(d.get(&ParamKey::String("size".to_string())), Some(&ParamVal::String("2G".to_string())));
        assert_eq!(d.get(&ParamKey::String("speed".to_string())), Some(&ParamVal::String("100M".to_string())));

        d.insert(ParamKey::String("size".to_string()), ParamVal::String("0.5G".to_string()));
        apply_suffix_bounds(&mut d);
        assert_eq!(d.get(&ParamKey::String("size".to_string())), Some(&ParamVal::String("1G".to_string())));

        d.insert(ParamKey::String("size".to_string()), ParamVal::String("1.5G".to_string()));
        apply_suffix_bounds(&mut d);
        assert_eq!(d.get(&ParamKey::String("size".to_string())), Some(&ParamVal::String("1.5G".to_string())));
    }

    #[test]
    fn test_substitution_with_placeholders() {
        let dict: HashMap<ParamKey, ParamVal> = [
            (ParamKey::String("key1".to_string()), ParamVal::String("value1".to_string())),
            (ParamKey::String("key2".to_string()), ParamVal::String("value2".to_string())),
        ].iter().cloned().collect();
        let result = substitution("This is ${key1} and ${key2}.".to_string(), &dict).unwrap();
        assert_eq!(result, "This is value1 and value2.");
    }

    #[test]
    fn test_substitution_missing_placeholder() {
        let dict: HashMap<ParamKey, ParamVal> = [
            (ParamKey::String("key1".to_string()), ParamVal::String("value1".to_string())),
        ].iter().cloned().collect();
        let result = substitution("This is ${key1} and ${key2}.".to_string(), &dict).unwrap();
        assert_eq!(result, "This is value1 and ${key2}.");
    }

    #[test]
    fn test_substitution_no_placeholders() {
        let dict: HashMap<ParamKey, ParamVal> = [
            (ParamKey::String("key1".to_string()), ParamVal::String("value1".to_string())),
            (ParamKey::String("key2".to_string()), ParamVal::String("value2".to_string())),
        ].iter().cloned().collect();
        let result = substitution("no placeholders here".to_string(), &dict).unwrap();
        assert_eq!(result, "no placeholders here");
    }

    #[test]
    fn test_substitution_empty_dict() {
        let dict: HashMap<ParamKey, ParamVal> = HashMap::new();
        let result = substitution("This is ${key1}.".to_string(), &dict).unwrap();
        assert_eq!(result, "This is ${key1}.");
    }

    #[test]
    fn test_substitution_with_dollar_sign_but_no_placeholders() {
        let dict: HashMap<ParamKey, ParamVal> = [
            (ParamKey::String("key1".to_string()), ParamVal::String("value1".to_string())),
        ].iter().cloned().collect();
        let result = substitution("This costs $5.".to_string(), &dict).unwrap();
        assert_eq!(result, "This costs $5.");
    }

    #[test]
    fn test_substitution_with_suffixed_dict() {
        let dict: HashMap<ParamKey, ParamVal> = [
            (ParamKey::Tuple(vec!["key1".to_string(), "_s1".to_string()]), ParamVal::String("value1".to_string())),
            (ParamKey::Tuple(vec!["key2".to_string(), "_s1".to_string()]), ParamVal::String("value2".to_string())),
            (ParamKey::Tuple(vec!["key2".to_string(), "_s2".to_string()]), ParamVal::String("value3".to_string())),
        ].iter().cloned().collect();
        let result = substitution("This is ${key1} and ${key2_s1}.".to_string(), &dict).unwrap();
        assert_eq!(result, "This is value1 and value2.");
    }

    #[test]
    fn test_drop_suffixes_with_simple_keys() {
        let dict: HashMap<ParamKey, ParamVal> = [
            (ParamKey::String("key1".to_string()), ParamVal::String("value1".to_string())),
            (ParamKey::String("key2".to_string()), ParamVal::String("value2".to_string())),
        ].iter().cloned().collect();
        let result = drop_suffixes(&dict, true).unwrap();
        assert_eq!(result.get(&"key1".to_string().into()), Some(&"value1".to_string().into()), "key1 is preserved");
        assert_eq!(result.get(&"key2".to_string().into()), Some(&"value2".to_string().into()), "key2 is preserved");
    }

    #[test]
    fn test_drop_suffixes_with_tuple_keys() {
        let dict: HashMap<ParamKey, ParamVal> = [
            (ParamKey::Tuple(vec!["key1".to_string(), "_s1".to_string()]), ParamVal::String("value1".to_string())),
            (ParamKey::Tuple(vec!["key1".to_string(), "_s2".to_string()]), ParamVal::String("value1".to_string())),
            (ParamKey::Tuple(vec!["key2".to_string(), "_s1".to_string()]), ParamVal::String("value2".to_string())),
            (ParamKey::Tuple(vec!["key2".to_string(), "_s2".to_string()]), ParamVal::String("value22".to_string())),
            (ParamKey::Tuple(vec!["key3".to_string(), "_sX".to_string()]), ParamVal::String("value3".to_string())),
        ].iter().cloned().collect();
        let result = drop_suffixes(&dict, true).unwrap();
        assert_eq!(result.get(&"key1".to_string().into()), Some(&"value1".to_string().into()), "single general key remains");
        assert_eq!(result.get(&"key1_s1".to_string().into()), None, "duplicate suffix is skipped");
        assert_eq!(result.get(&"key1_s2".to_string().into()), None, "duplicate suffix is skipped");
        assert_eq!(result.get(&"key2".to_string().into()), None, "no general key is created for different suffix values");
        assert_eq!(result.get(&"key2_s1".to_string().into()), Some(&"value2".to_string().into()), "nonduplicate suffix is preserved");
        assert_eq!(result.get(&"key2_s2".to_string().into()), Some(&"value22".to_string().into()), "nonduplicate suffix is preserved");
        assert_eq!(result.get(&"key3".to_string().into()), Some(&ParamVal::String("value3".to_string())), "single suffix is converted to general key");
    }

    #[test]
    fn test_drop_suffixes_with_mixed_keys() {
        let mut dict: HashMap<ParamKey, ParamVal> = [
            (ParamKey::Tuple(vec!["key1".to_string(), "_s2".to_string()]), ParamVal::String("value1".to_string())),
            (ParamKey::Tuple(vec!["key2".to_string(), "_s2".to_string()]), ParamVal::String("value22".to_string())),
            (ParamKey::Tuple(vec!["key3".to_string(), "_sX".to_string()]), ParamVal::String("value3".to_string())),
        ].iter().cloned().collect();

        // Add mixed entries (general keys and multi-suffix tuple)
        dict.insert(ParamKey::String("key1".to_string()), ParamVal::String("value1".to_string()));
        dict.insert(ParamKey::Tuple(vec!["key1".to_string(), "_sY".to_string(), "_sZ".to_string()]), ParamVal::String("value1".to_string()));
        dict.insert(ParamKey::String("key2".to_string()), ParamVal::String("value2".to_string()));
        dict.insert(ParamKey::Tuple(vec!["key2".to_string(), "_sY".to_string(), "_sZ".to_string()]), ParamVal::String("value222".to_string()));
        dict.insert(ParamKey::String("key4".to_string()), ParamVal::String("value4".to_string()));
        dict.insert(ParamKey::Tuple(vec!["key5".to_string(), "_sY".to_string(), "_sZ".to_string()]), ParamVal::String("value5".to_string()));

        let result = drop_suffixes(&dict, true).unwrap();
        assert_eq!(result.get(&"key1".to_string().into()), Some(&ParamVal::String("value1".to_string())), "single general key remains");
        assert_eq!(result.get(&"key1_s2".to_string().into()), None, "duplicate suffix is skipped");
        assert_eq!(result.get(&"key1_sZ_sY".to_string().into()), None, "duplicate double suffix is skipped");
        assert_eq!(result.get(&"key2".to_string().into()), Some(&ParamVal::String("value2".to_string())), "general key is preserved");
        assert_eq!(result.get(&"key2_s2".to_string().into()), Some(&ParamVal::String("value22".to_string())), "single suffix is preserved together with general key");
        assert_eq!(result.get(&"key2_sZ_sY".to_string().into()), Some(&ParamVal::String("value222".to_string())), "duplicate double suffix is preserved together with general key");
        assert_eq!(result.get(&"key3".to_string().into()), Some(&ParamVal::String("value3".to_string())), "single suffix is converted to general key");
        assert_eq!(result.get(&"key4".to_string().into()), Some(&ParamVal::String("value4".to_string())), "single general key is preserved");
        assert_eq!(result.get(&"key5".to_string().into()), Some(&ParamVal::String("value5".to_string())), "single general key is preserved");
    }

    #[test]
    fn test_drop_suffixes_with_skipdups_false() {
        let mut dict: HashMap<ParamKey, ParamVal> = [
            (ParamKey::Tuple(vec!["key1".to_string(), "_s1".to_string()]), ParamVal::String("value1".to_string())),
            (ParamKey::Tuple(vec!["key1".to_string(), "_s2".to_string()]), ParamVal::String("value1".to_string())),
        ].iter().cloned().collect();
        dict.insert(ParamKey::String("key1".to_string()), ParamVal::String("value1".to_string()));
        let result = drop_suffixes(&dict, false).unwrap();
        assert_eq!(result.get(&"key1".to_string().into()), Some(&ParamVal::String("value1".to_string())), "general key is preserved");
        assert_eq!(result.get(&"key1_s1".to_string().into()), Some(&ParamVal::String("value1".to_string())), "duplicate suffix is preserved");
        assert_eq!(result.get(&"key1_s2".to_string().into()), Some(&ParamVal::String("value1".to_string())), "duplicate suffix is preserved");
    }

    #[test]
    fn test_drop_suffixes_with_reserved_keys() {
        let mut dict: HashMap<ParamKey, ParamVal> = [
            (ParamKey::Tuple(vec!["key1".to_string(), "_s1".to_string()]), ParamVal::String("value1".to_string())),
        ].iter().cloned().collect();
        for key in RESERVED_KEYS.iter() {
            dict.insert(ParamKey::String(key.to_string()), ParamVal::String("reserved_value".to_string()));
        }
        let result = drop_suffixes(&dict, true).unwrap();
        assert_eq!(result.get(&"key1".to_string().into()), Some(&ParamVal::String("value1".to_string())), "suffixed key is reduced as usual");
        for key in RESERVED_KEYS.iter() {
            assert_eq!(result.get(&key.to_string().into()), Some(&ParamVal::String("reserved_value".to_string())), "reserved key is preserved");
        }
    }
}
