use std::fmt;
use std::collections::HashMap;
use std::sync::LazyLock;
use regex::Regex;

use pyo3::prelude::*;
use pyo3::types::{PyDict, PyTuple};
use pyo3::exceptions::PyAttributeError;

const RESERVED_KEYS: &[&str] = &[
    "name",
    "shortname",
    "dep",
    "_short_name_map_file",
    "_name_map_file",
];

// Define an enum for the different types of tokens
#[pyclass(eq)]
#[derive(Debug, PartialEq)]
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
    LSet(),
    LAppend(),
    LPrepend(),
    LLazySet(),
    LRegExpSet(),
    LRegExpAppend(),
    LRegExpPrepend(),
    LDel(),
    LApplyPreDict(),
    LUpdateFileMap(),
    Suffix(),
}
// Implement the Display trait for Tokens
impl fmt::Display for Tokens {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Tokens::LIndent(length) => write!(f, "indent {}", length),
            Tokens::LEndL() => write!(f, "endl"),
            Tokens::LEndBlock(length) => write!(f, "indent {}", length),
            Tokens::LIdentifier(string) => write!(f, "Identifier re([A-Za-z0-9][A-Za-z0-9_-]*) \"{}\"", string),
            Tokens::LWhite(string) => write!(f, "WhiteSpace re(\\s) \"{}\"", string),
            Tokens::LString(string) => write!(f, "String re(.+) \"{}\"", string),
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
            Tokens::LSet() => write!(f, "="),
            Tokens::LAppend() => write!(f, "+="),
            Tokens::LPrepend() => write!(f, "<="),
            Tokens::LLazySet() => write!(f, "~="),
            Tokens::LRegExpSet() => write!(f, "?="),
            Tokens::LRegExpAppend() => write!(f, "?+="),
            Tokens::LRegExpPrepend() => write!(f, "?<="),
            Tokens::LDel() => write!(f, "del"),
            Tokens::LApplyPreDict() => write!(f, "apply_pre_dict"),
            Tokens::LUpdateFileMap() => write!(f, "update_file_map"),
            Tokens::Suffix() => write!(f, "apply_suffix"),
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
        Ok(format!("'{}'", s))
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
}

fn drop_suffixes(py_dict: &Bound<'_, PyDict>, skipdups: bool) -> HashMap<String, String> {
    let mut d_flat = HashMap::new();

    for (key, value) in py_dict.iter() {
        let Ok(value_str) = value.extract::<String>() else {
            continue;
        };

        if let Ok(key_str) = key.extract::<String>() {
            if RESERVED_KEYS.contains(&key_str.as_str()) {
                // treating reserved keys as regular string keys here
            }
            d_flat.insert(key_str, value_str);
        } else if let Ok(key_tuple) = key.downcast::<PyTuple>() {
            let gen_key = &key_tuple.get_item(0).unwrap().extract::<String>().unwrap();
            let mut can_drop_all_suffixes = true;

            if skipdups {
                if let Ok(Some(gen_value)) = py_dict.get_item(gen_key) {
                    if let Ok(gen_value_str) = gen_value.extract::<String>() {
                        if gen_value_str == value_str {
                            continue; // Skip duplicate suffixes
                        } else {
                            can_drop_all_suffixes = false;
                        }
                    }
                }

                if can_drop_all_suffixes {
                    for (other_key, other_value) in py_dict.iter() {
                        if let Ok(other_key_tuple) = other_key.downcast::<PyTuple>() {
                            if other_key_tuple.get_item(0).unwrap().extract::<String>().unwrap() == *gen_key {
                                if let Ok(other_value_str) = other_value.extract::<String>() {
                                    if other_value_str != value_str {
                                        can_drop_all_suffixes = false;
                                        break;
                                    }
                                }
                            }
                        }
                    }
                }
            }

            let new_key = if skipdups && can_drop_all_suffixes {
                gen_key.clone()
            } else {
                let key_vec = key_tuple.iter()
                    .map(|item| item.extract::<String>().unwrap())
                    .collect::<Vec<_>>();
                let mut suffix_parts = key_vec[1..].to_vec();
                suffix_parts.reverse();
                format!("{}{}", key_vec[0], suffix_parts.join(""))
            };

            d_flat.insert(new_key, value_str);
        }

    }

    d_flat
}

static MATCH_SUBSTITUTE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\$\{(.+?)\}")
    .expect("Invalid MATCH_SUBSTITUTE pattern")
});

#[pyfunction]
pub fn substitution(value: &str, py_dict: &Bound<'_, PyDict>) -> String {
    if value.contains('$') {
        let mut start = 0;
        let mut result = String::new();

        // Use the Rust `drop_suffixes` function
        let d = drop_suffixes(py_dict, true);

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
        result
    } else {
        value.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pyo3::types::IntoPyDict;

    #[test]
    fn test_display() {
        let t1 = Tokens::LIndent(42);
        assert_eq!(format!("{}", t1), "indent 42");
    }

    #[test]
    fn test_debug() {
        let t1 = Tokens::LIndent(42);
        assert_eq!(format!("{:?}", t1), "LIndent(42)");
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
        pyo3::prepare_freethreaded_python();
        Python::with_gil(|py| {
            let py_dict = [("key1", "value1"), ("key2", "value2")].into_py_dict(py).unwrap();
            let result = substitution("This is ${key1} and ${key2}.", &py_dict);
            assert_eq!(result, "This is value1 and value2.");
        });
    }

    #[test]
    fn test_substitution_missing_placeholder() {
        pyo3::prepare_freethreaded_python();
        Python::with_gil(|py| {
            let py_dict = [("key1", "value1")].into_py_dict(py).unwrap();
            let result = substitution("This is ${key1} and ${key2}.", &py_dict);
            assert_eq!(result, "This is value1 and ${key2}.");
        });
    }

    #[test]
    fn test_substitution_no_placeholders() {
        pyo3::prepare_freethreaded_python();
        Python::with_gil(|py| {
            let py_dict = [("key1", "value1"), ("key2", "value2")].into_py_dict(py).unwrap();
            let result = substitution("no placeholders here", &py_dict);
            assert_eq!(result, "no placeholders here");
        });
    }

    #[test]
    fn test_substitution_empty_dict() {
        pyo3::prepare_freethreaded_python();
        Python::with_gil(|py| {
            let py_dict = PyDict::new(py);
            let result = substitution("This is ${key1}.", &py_dict);
            assert_eq!(result, "This is ${key1}.");
        });
    }

    #[test]
    fn test_substitution_with_dollar_sign_but_no_placeholders() {
        pyo3::prepare_freethreaded_python();
        Python::with_gil(|py| {
            let py_dict = [("key1", "value1")].into_py_dict(py).unwrap();
            let result = substitution("This costs $5.", &py_dict);
            assert_eq!(result, "This costs $5.");
        });
    }

    #[test]
    fn test_substitution_with_suffixed_dict() {
        pyo3::prepare_freethreaded_python();
        Python::with_gil(|py| {
            let py_dict = [
                (("key1", "_s1"), "value1"),
                (("key2", "_s1"), "value2"),
                (("key2", "_s2"), "value3"),
            ].into_py_dict(py).unwrap();
            let result = substitution("This is ${key1} and ${key2_s1}.", &py_dict);
            assert_eq!(result, "This is value1 and value2.");
        });
    }

    #[test]
    fn test_drop_suffixes_with_simple_keys() {
        pyo3::prepare_freethreaded_python();
        Python::with_gil(|py| {
            let py_dict = [
                ("key1", "value1"),
                ("key2", "value2")
            ].into_py_dict(py)
            .unwrap();
            let result = drop_suffixes(&py_dict, true);
            assert_eq!(result.get("key1"), Some(&"value1".to_string()), "key1 is preserved");
            assert_eq!(result.get("key2"), Some(&"value2".to_string()), "key2 is preserved");
        });
    }

    #[test]
    fn test_drop_suffixes_with_tuple_keys() {
        pyo3::prepare_freethreaded_python();
        Python::with_gil(|py| {
            let py_dict = [
                (("key1", "_s1"), "value1"),
                (("key1", "_s2"), "value1"),
                (("key2", "_s1"), "value2"),
                (("key2", "_s2"), "value22"),
                (("key3", "_sX"), "value3"),
            ]
            .into_py_dict(py)
            .unwrap();
            let result = drop_suffixes(&py_dict, true);
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
        pyo3::prepare_freethreaded_python();
        Python::with_gil(|py| {
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
            let result = drop_suffixes(&py_dict, true);
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
        pyo3::prepare_freethreaded_python();
        Python::with_gil(|py| {
            let py_dict = [
                (("key1", "_s1"), "value1"),
                (("key1", "_s2"), "value1"),
            ]
            .into_py_dict(py)
            .unwrap();
            py_dict.set_item("key1", "value1").unwrap();
            let result = drop_suffixes(&py_dict, false);
            assert_eq!(result.get("key1"), Some(&"value1".to_string()), "general key is preserved");
            assert_eq!(result.get("key1_s1"), Some(&"value1".to_string()), "duplicate suffix is preserved");
            assert_eq!(result.get("key1_s2"), Some(&"value1".to_string()), "duplicate suffix is preserved");
        });
    }

    #[test]
    fn test_drop_suffixes_with_reserved_keys() {
        pyo3::prepare_freethreaded_python();
        Python::with_gil(|py| {
            let py_dict = [
                (("key1", "_s1"), "value1"),
            ]
            .into_py_dict(py)
            .unwrap();
            for key in RESERVED_KEYS.iter() {
                py_dict.set_item(*key, "reserved_value").unwrap();
            }
            let result = drop_suffixes(&py_dict, true);
            assert_eq!(result.get("key1"), Some(&"value1".to_string()), "suffixed key is reduced as usual");
            for key in RESERVED_KEYS.iter() {
                py_dict.set_item(*key, "reserved_value").unwrap();
                assert_eq!(result.get(*key), Some(&"reserved_value".to_string()), "reserved key is preserved");
            }
        });
    }
}
