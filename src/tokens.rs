use std::fmt;
use std::collections::HashMap;
use std::sync::LazyLock;
use regex::Regex;

use pyo3::{prelude::*, IntoPyObjectExt};
use pyo3::types::{PyDict, PyTuple, PyString, IntoPyDict};
use pyo3::exceptions::{PyAttributeError, PyValueError};

const RESERVED_KEYS: &[&str] = &[
    "name",
    "shortname",
    "dep",
    "_short_name_map_file",
    "_name_map_file",
];

// Define an enum for the different types of tokens
#[pyclass(eq)]
#[derive(Debug, PartialEq, Clone)]
pub enum Tokens {
    LIndent(i32),
    LEndL(),
    LEndBlock(i32),
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
    LApplyPreDict(String, HashMap<String, String>),
    LUpdateFileMap(String, String, String),
    Suffix(String, String),
}
// Implement the Display trait for Tokens
impl fmt::Display for Tokens {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Tokens::LIndent(length) => write!(f, "indent {length}"),
            Tokens::LEndL() => write!(f, "endl"),
            Tokens::LEndBlock(length) => write!(f, "indent {length}"),
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
            Tokens::LCond() => write!(f, ""),
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
            Tokens::LApplyPreDict(_name, value) => write!(f, "apply_pre_dict {value:?}"),
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

    fn __str__(&self) -> PyResult<String> {
        Ok(self.to_string())
    }

    fn __repr__(&self) -> PyResult<String> {
        let s = self.__str__()?;
        Ok(format!("'{s}'"))
    }

    #[getter]
    fn length(&self) -> PyResult<i32> {
        match self {
            Tokens::LIndent(length) => Ok(*length),
            Tokens::LEndBlock(length) => Ok(*length),
            _ => Err(PyAttributeError::new_err("length is not a valid attribute for this token")),
        }
    }

    #[getter]
    fn string(&self) -> PyResult<String> {
        match self {
            Tokens::LIdentifier(string) => Ok(string.to_string()),
            Tokens::LWhite(string) => Ok(string.to_string()),
            Tokens::LString(string) => Ok(string.to_string()),
            _ => Err(PyAttributeError::new_err("string is not a valid attribute for this token")),
        }
    }

    #[getter]
    fn name(&self) -> PyResult<String> {
        match self {
            Tokens::LSet(name, _value) => Ok(name.to_string()),
            Tokens::LAppend(name, _value) => Ok(name.to_string()),
            Tokens::LPrepend(name, _value) => Ok(name.to_string()),
            Tokens::LLazySet(name, _value) => Ok(name.to_string()),
            Tokens::LRegExpSet(name, _value) => Ok(name.to_string()),
            Tokens::LRegExpAppend(name, _value) => Ok(name.to_string()),
            Tokens::LRegExpPrepend(name, _value) => Ok(name.to_string()),
            Tokens::LDel(name, _value) => Ok(name.to_string()),
            Tokens::LApplyPreDict(name, _value) => Ok(name.to_string()),
            Tokens::LUpdateFileMap(_filename, name, _value) => Ok(name.to_string()),
            Tokens::Suffix(name, _value) => Ok(name.to_string()),
            _ => Err(PyAttributeError::new_err("name is not a valid attribute for this token")),
        }
    }

    #[getter]
    fn value(&self) -> PyResult<String> {
        match self {
            Tokens::LSet(_name, value) => Ok(value.to_string()),
            Tokens::LAppend(_name, value) => Ok(value.to_string()),
            Tokens::LPrepend(_name, value) => Ok(value.to_string()),
            Tokens::LLazySet(_name, value) => Ok(value.to_string()),
            Tokens::LRegExpSet(_name, value) => Ok(value.to_string()),
            Tokens::LRegExpAppend(_name, value) => Ok(value.to_string()),
            Tokens::LRegExpPrepend(_name, value) => Ok(value.to_string()),
            Tokens::LDel(_name, value) => Ok(value.to_string()),
            Tokens::LUpdateFileMap(_filename, _name, value) => Ok(value.to_string()),
            Tokens::Suffix(_name, value) => Ok(value.to_string()),
            _ => Err(PyAttributeError::new_err("value is not a valid attribute for this token")),
        }
    }

    #[getter]
    fn filename(&self) -> PyResult<String> {
        match self {
            Tokens::LUpdateFileMap(filename, _name, _value) => Ok(filename.to_string()),
            _ => Err(PyAttributeError::new_err("filename is not a valid attribute for this token")),
        }
    }

    fn apply_to_dict(&self, py_dict: &Bound<'_, PyDict>) -> PyResult<()> {
        match self {
            Tokens::LSet(name, value) => {
                if !RESERVED_KEYS.contains(&name.as_str()) {
                    let substituted_value = substitution(value, py_dict)?;
                    py_dict.set_item(name, substituted_value)?;
                }
                Ok(())
            }
            Tokens::LAppend(name, value) => {
                if !RESERVED_KEYS.contains(&name.as_str()) {
                    let empty_value = PyString::new(py_dict.py(), "");
                    if let Ok(current_value) = py_dict.get_item(name) {
                        let current_value = current_value.unwrap_or(
                            empty_value.into_bound_py_any(py_dict.py())?
                        );
                        let substituted_value = substitution(value, py_dict)?;
                        let new_value = format!("{}{}", current_value.extract::<String>()?, substituted_value);
                        py_dict.set_item(name, new_value)?;
                    }
                }
                Ok(())
            }
            Tokens::LPrepend(name, value) => {
                if !RESERVED_KEYS.contains(&name.as_str()) {
                    let empty_value = PyString::new(py_dict.py(), "");
                    if let Ok(current_value) = py_dict.get_item(name) {
                        let current_value = current_value.unwrap_or(
                            empty_value.into_bound_py_any(py_dict.py())?
                        );
                        let substituted_value = substitution(value, py_dict)?;
                        let new_value = format!("{}{}", substituted_value, current_value.extract::<String>()?);
                        py_dict.set_item(name, new_value)?;
                    }
                }
                Ok(())
            }
            Tokens::LLazySet(name, value) => {
                if !RESERVED_KEYS.contains(&name.as_str()) && !py_dict.contains(name)? {
                    let substituted_value = substitution(value, py_dict)?;
                    py_dict.set_item(name, substituted_value)?;
                }
                Ok(())
            }
            Tokens::LRegExpSet(name, value) => {
                let exp = Regex::new(&format!(r"^{name}$"))
                   .map_err(|e| PyErr::new::<PyValueError, _>(e.to_string()))?;
                let substituted_value = substitution(value, py_dict)?;
                for (key, _) in py_dict.iter() {
                    if let Ok(key_str) = key.extract::<String>() {
                        if !RESERVED_KEYS.contains(&key_str.as_str()) && exp.is_match(&key_str) {
                            py_dict.set_item(key, &substituted_value)?;
                        }
                    } else {
                        let key_tuple = key.cast::<PyTuple>()?.as_slice();
                        let key_str = key_tuple.iter().fold(String::new(), |mut acc, item| {
                            if let Ok(item_str) = item.extract::<String>() {
                                acc.push_str(&item_str);
                            }
                            acc
                        });
                        if exp.is_match(&key_str) {
                            py_dict.set_item(key, &substituted_value)?;
                        }
                    }
                }
                Ok(())
            }
            Tokens::LRegExpAppend(name, value) => {
                let exp = Regex::new(&format!(r"^{name}$"))
                   .map_err(|e| PyErr::new::<PyValueError, _>(e.to_string()))?;
                let substituted_value = substitution(value, py_dict)?;
                for (key, val) in py_dict.iter() {
                    if let Ok(key_str) = key.extract::<String>() {
                        if !RESERVED_KEYS.contains(&key_str.as_str()) && exp.is_match(&key_str) {
                            let current_value = val.extract::<String>().unwrap_or_default();
                            let new_value = format!("{current_value}{substituted_value}");
                            py_dict.set_item(key, new_value)?;
                        }
                    } else {
                        let key_tuple = key.cast::<PyTuple>()?.as_slice();
                        let key_str = key_tuple.iter().fold(String::new(), |mut acc, item| {
                            if let Ok(item_str) = item.extract::<String>() {
                                acc.push_str(&item_str);
                            }
                            acc
                        });
                        if exp.is_match(&key_str) {
                            let current_value = val.extract::<String>().unwrap_or_default();
                            let new_value = format!("{current_value}{substituted_value}");
                            py_dict.set_item(key, new_value)?;
                        }
                    }
                }
                Ok(())
            }
            Tokens::LRegExpPrepend(name, value) => {
                let exp = Regex::new(&format!(r"^{name}$"))
                   .map_err(|e| PyErr::new::<PyValueError, _>(e.to_string()))?;
                let substituted_value = substitution(value, py_dict)?;
                for (key, val) in py_dict.iter() {
                    if let Ok(key_str) = key.extract::<String>() {
                        if !RESERVED_KEYS.contains(&key_str.as_str()) && exp.is_match(&key_str) {
                            let current_value = val.extract::<String>().unwrap_or_default();
                            let new_value = format!("{substituted_value}{current_value}");
                            py_dict.set_item(key, new_value)?;
                        }
                    } else {
                        let key_tuple = key.cast::<PyTuple>()?.as_slice();
                        let key_str = key_tuple.iter().fold(String::new(), |mut acc, item| {
                            if let Ok(item_str) = item.extract::<String>() {
                                acc.push_str(&item_str);
                            }
                            acc
                        });
                        if exp.is_match(&key_str) {
                            let current_value = val.extract::<String>().unwrap_or_default();
                            let new_value = format!("{substituted_value}{current_value}");
                            py_dict.set_item(key, new_value)?;
                        }
                    }
                }
                Ok(())
            }
            Tokens::LDel(name, _) => {
                let exp = Regex::new(&format!(r"^{name}$"))
                   .map_err(|e| PyErr::new::<PyValueError, _>(e.to_string()))?;
                let keys_to_delete: Vec<_> = py_dict
                    .iter()
                    // TODO: using "?" doesn't propagate so now we just ignore errors using "ok()?" - try try_filter_map?
                    .filter_map(|(key, _)| {
                        if let Ok(key_str) = key.extract::<String>() {
                            if !RESERVED_KEYS.contains(&key_str.as_str()) && exp.is_match(&key_str) {
                                return Some(key);
                            }
                        } else {
                            let key_tuple = key.cast::<PyTuple>().ok()?.as_slice();
                            let key_str = key_tuple.iter().fold(String::new(), |mut acc, item| {
                                if let Ok(item_str) = item.extract::<String>() {
                                    acc.push_str(&item_str);
                                }
                                acc
                            });
                            if exp.is_match(&key_str) {
                                return Some(key);
                            }
                        }
                        None
                    })
                    .collect();
                for key in keys_to_delete {
                    py_dict.del_item(key)?;
                }
                Ok(())
            }
            Tokens::LApplyPreDict(_, value) => {
                py_dict.update(value.into_py_dict(py_dict.py())?.as_mapping())?;
                Ok(())
            }
            Tokens::LUpdateFileMap(filename, name, value) => {
                let dest = value;
                let shortname = if filename == "<string>" {
                    filename.clone()
                } else {
                    std::path::Path::new(filename)
                        .file_name()
                        .and_then(|os_str| os_str.to_str())
                        .unwrap_or(filename)
                        .to_string()
                };

                if !py_dict.contains(dest)? {
                    py_dict.set_item(dest, PyDict::new(py_dict.py()))?;
                }


                let dest_dict = if let Ok(Some(dest_any)) = py_dict.get_item(dest) {
                    dest_any
                    .cast::<PyDict>()
                    .map_err(|_| PyAttributeError::new_err(format!("{dest} is not a dict")))?
                    .clone()
                } else {
                    PyDict::new(py_dict.py())
                };
                if let Some(old_name) = dest_dict.get_item(&shortname)? {
                    let old_name_str = old_name.extract::<String>()?;
                    let new_name = format!("{name}.{old_name_str}");
                    dest_dict.set_item(&shortname, new_name)?;
                } else {
                    dest_dict.set_item(&shortname, name)?;
                }
                Ok(())
            }
            Tokens::Suffix(_, value) => {
                let py_value = value.into_bound_py_any(py_dict.py())?;
                let keyvals = py_dict.iter().filter_map(|(key, val)| {
                    let py_value_clone = py_value.clone();
                    let mut items = Vec::new();
                    if let Ok(key_str) = key.extract::<String>() {
                        if RESERVED_KEYS.contains(&key_str.as_str()) {
                            return Some((key, val));
                        } else {
                            items.push(&key);
                        }
                    } else {
                        let key_tuple = key.cast::<PyTuple>().ok()?.as_slice();
                        for item in key_tuple.iter() {
                            items.push(item);
                        }
                    }
                    items.push(&py_value_clone);
                    let new_key = PyTuple::new(py_dict.py(), items).ok()?;
                    Some((new_key.into_bound_py_any(py_dict.py()).ok()?, val))
                }).collect::<Vec<_>>();
                let suffixed_py_dict = PyDict::new(py_dict.py());
                for (k, v) in keyvals {
                    suffixed_py_dict.set_item(k, v)?;
                }
                py_dict.clear();
                py_dict.update(suffixed_py_dict.as_mapping())?;
                Ok(())
            }
            _ => Err(PyAttributeError::new_err("apply_to_dict is not a valid attribute for this token")),
        }
    }
}

fn drop_suffixes(py_dict: &Bound<'_, PyDict>, skipdups: bool) -> PyResult<HashMap<String, String>> {
    let d_flat: HashMap<String, String> = py_dict
        .iter()
        .filter_map(|(key, value)| {
            let value_str = value.extract::<String>().ok()?;
            if let Ok(key_str) = key.extract::<String>() {
                Some((key_str, value_str))
            } else if let Ok(key_tuple) = key.cast::<PyTuple>() {
                let gen_key = key_tuple.get_item(0).ok()?.extract::<String>().ok()?;
                let mut can_drop_all_suffixes = true;

                if skipdups {
                    if let Ok(Some(gen_value)) = py_dict.get_item(&gen_key) &&
                            let Ok(gen_value_str) = gen_value.extract::<String>() {
                        if gen_value_str == value_str {
                            return None; // Skip duplicate suffixes
                        } else {
                            can_drop_all_suffixes = false;
                        }
                    }

                    if can_drop_all_suffixes {
                        can_drop_all_suffixes = py_dict.iter()
                            .filter_map(|(other_key, other_value)| {
                                other_key.cast::<PyTuple>().ok()?
                                    .get_item(0).ok()?
                                    .extract::<String>().ok()
                                    .filter(|k| k == &gen_key)
                                    .and_then(|_| other_value.extract::<String>().ok())
                            })
                            .all(|other_value_str| other_value_str == value_str);
                    }
                }

                let new_key = if skipdups && can_drop_all_suffixes {
                    gen_key
                } else {
                    let key_vec = key_tuple.iter()
                        .map(|item| item.extract::<String>())
                        .collect::<Result<Vec<_>, _>>()
                        .ok()?;
                        let mut suffix_parts = key_vec[1..].to_vec();
                        suffix_parts.reverse();
                        format!("{}{}", key_vec[0], suffix_parts.join(""))
                };

                Some((new_key, value_str))
            } else {
                None
            }
        })
        .collect();

    Ok(d_flat)
}

static MATCH_SUBSTITUTE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\$\{(.+?)\}")
    .expect("Invalid MATCH_SUBSTITUTE pattern")
});

#[pyfunction]
pub fn substitution(value: &str, py_dict: &Bound<'_, PyDict>) -> PyResult<String> {
    if value.contains('$') {
        let mut start = 0;
        let mut result = String::new();

        // Use the Rust `drop_suffixes` function
        let d = drop_suffixes(py_dict, true)?;

        while let Some(captures) = MATCH_SUBSTITUTE.captures(&value[start..]) {
            if let Some(matched) = captures.get(0) {
                let key = captures.get(1).map_or("", |m| m.as_str());
                if let Some(val) = d.get(key) {
                    result.push_str(&value[start..start + matched.start()]);
                    result.push_str(val);
                    start += matched.end();
                } else {
                    break;
                }
            }
        }
        result.push_str(&value[start..]);
        Ok(result)
    } else {
        Ok(value.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn test_substitution_with_placeholders() {
        Python::initialize();
        Python::attach(|py| {
            let py_dict = [("key1", "value1"), ("key2", "value2")].into_py_dict(py).unwrap();
            let result = substitution("This is ${key1} and ${key2}.", &py_dict).unwrap();
            assert_eq!(result, "This is value1 and value2.");
        });
    }

    #[test]
    fn test_substitution_missing_placeholder() {
        Python::initialize();
        Python::attach(|py| {
            let py_dict = [("key1", "value1")].into_py_dict(py).unwrap();
            let result = substitution("This is ${key1} and ${key2}.", &py_dict).unwrap();
            assert_eq!(result, "This is value1 and ${key2}.");
        });
    }

    #[test]
    fn test_substitution_no_placeholders() {
        Python::initialize();
        Python::attach(|py| {
            let py_dict = [("key1", "value1"), ("key2", "value2")].into_py_dict(py).unwrap();
            let result = substitution("no placeholders here", &py_dict).unwrap();
            assert_eq!(result, "no placeholders here");
        });
    }

    #[test]
    fn test_substitution_empty_dict() {
        Python::initialize();
        Python::attach(|py| {
            let py_dict = PyDict::new(py);
            let result = substitution("This is ${key1}.", &py_dict).unwrap();
            assert_eq!(result, "This is ${key1}.");
        });
    }

    #[test]
    fn test_substitution_with_dollar_sign_but_no_placeholders() {
        Python::initialize();
        Python::attach(|py| {
            let py_dict = [("key1", "value1")].into_py_dict(py).unwrap();
            let result = substitution("This costs $5.", &py_dict).unwrap();
            assert_eq!(result, "This costs $5.");
        });
    }

    #[test]
    fn test_substitution_with_suffixed_dict() {
        Python::initialize();
        Python::attach(|py| {
            let py_dict = [
                (("key1", "_s1"), "value1"),
                (("key2", "_s1"), "value2"),
                (("key2", "_s2"), "value3"),
            ].into_py_dict(py).unwrap();
            let result = substitution("This is ${key1} and ${key2_s1}.", &py_dict).unwrap();
            assert_eq!(result, "This is value1 and value2.");
        });
    }

    #[test]
    fn test_drop_suffixes_with_simple_keys() {
        Python::initialize();
        Python::attach(|py| {
            let py_dict = [
                ("key1", "value1"),
                ("key2", "value2")
            ].into_py_dict(py)
            .unwrap();
            let result = drop_suffixes(&py_dict, true).unwrap();
            assert_eq!(result.get("key1"), Some(&"value1".to_string()), "key1 is preserved");
            assert_eq!(result.get("key2"), Some(&"value2".to_string()), "key2 is preserved");
        });
    }

    #[test]
    fn test_drop_suffixes_with_tuple_keys() {
        Python::initialize();
        Python::attach(|py| {
            let py_dict = [
                (("key1", "_s1"), "value1"),
                (("key1", "_s2"), "value1"),
                (("key2", "_s1"), "value2"),
                (("key2", "_s2"), "value22"),
                (("key3", "_sX"), "value3"),
            ]
            .into_py_dict(py)
            .unwrap();
            let result = drop_suffixes(&py_dict, true).unwrap();
            assert_eq!(result.get("key1"), Some(&"value1".to_string()), "single general key remains");
            assert_eq!(result.get("key1_s1"), None, "duplicate suffix is skipped");
            assert_eq!(result.get("key1_s2"), None, "duplicate suffix is skipped");
            assert_eq!(result.get("key2"), None, "no general key is created for different suffix values");
            assert_eq!(result.get("key2_s1"), Some(&"value2".to_string()), "nonduplicate suffix is preserved");
            assert_eq!(result.get("key2_s2"), Some(&"value22".to_string()), "nonduplicate suffix is preserved");
            assert_eq!(result.get("key3"), Some(&"value3".to_string()), "single suffix is converted to general key");
        });
    }

    #[test]
    fn test_drop_suffixes_with_mixed_keys() {
        Python::initialize();
        Python::attach(|py| {
            let py_dict = [
                (("key1", "_s2"), "value1"),
                (("key2", "_s2"), "value22"),
                (("key3", "_sX"), "value3"),
            ]
            .into_py_dict(py)
            .unwrap();
            py_dict.set_item("key1", "value1").unwrap();
            py_dict.set_item(("key1", "_sY", "_sZ"), "value1").unwrap();
            py_dict.set_item("key2", "value2").unwrap();
            py_dict.set_item(("key2", "_sY", "_sZ"), "value222").unwrap();
            py_dict.set_item("key4", "value4").unwrap();
            py_dict.set_item(("key5", "_sY", "_sZ"), "value5").unwrap();
            let result = drop_suffixes(&py_dict, true).unwrap();
            assert_eq!(result.get("key1"), Some(&"value1".to_string()), "single general key remains");
            assert_eq!(result.get("key1_s2"), None, "duplicate suffix is skipped");
            assert_eq!(result.get("key1_sZ_sY"), None, "duplicate double suffix is skipped");
            assert_eq!(result.get("key2"), Some(&"value2".to_string()), "general key is preserved");
            assert_eq!(result.get("key2_s2"), Some(&"value22".to_string()), "single suffix is preserved together with general key");
            assert_eq!(result.get("key2_sZ_sY"), Some(&"value222".to_string()), "duplicate double suffix is preserved together with general key");
            assert_eq!(result.get("key3"), Some(&"value3".to_string()), "single suffix is converted to general key");
            assert_eq!(result.get("key4"), Some(&"value4".to_string()), "single general key is preserved");
            assert_eq!(result.get("key5"), Some(&"value5".to_string()), "single general key is preserved");
        });
    }

    #[test]
    fn test_drop_suffixes_with_skipdups_false() {
        Python::initialize();
        Python::attach(|py| {
            let py_dict = [
                (("key1", "_s1"), "value1"),
                (("key1", "_s2"), "value1"),
            ]
            .into_py_dict(py)
            .unwrap();
            py_dict.set_item("key1", "value1").unwrap();
            let result = drop_suffixes(&py_dict, false).unwrap();
            assert_eq!(result.get("key1"), Some(&"value1".to_string()), "general key is preserved");
            assert_eq!(result.get("key1_s1"), Some(&"value1".to_string()), "duplicate suffix is preserved");
            assert_eq!(result.get("key1_s2"), Some(&"value1".to_string()), "duplicate suffix is preserved");
        });
    }

    #[test]
    fn test_drop_suffixes_with_reserved_keys() {
        Python::initialize();
        Python::attach(|py| {
            let py_dict = [
                (("key1", "_s1"), "value1"),
            ]
            .into_py_dict(py)
            .unwrap();
            for key in RESERVED_KEYS.iter() {
                py_dict.set_item(*key, "reserved_value").unwrap();
            }
            let result = drop_suffixes(&py_dict, true).unwrap();
            assert_eq!(result.get("key1"), Some(&"value1".to_string()), "suffixed key is reduced as usual");
            for key in RESERVED_KEYS.iter() {
                py_dict.set_item(*key, "reserved_value").unwrap();
                assert_eq!(result.get(*key), Some(&"reserved_value".to_string()), "reserved key is preserved");
            }
        });
    }
}
