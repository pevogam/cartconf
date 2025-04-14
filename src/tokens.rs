use std::fmt;
use std::collections::HashMap;
use std::sync::LazyLock;
use regex::Regex;

use pyo3::prelude::*;
use pyo3::types::PyDict;
use pyo3::exceptions::PyAttributeError;

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

static MATCH_SUBSTITUTE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\$\{(.+?)\}")
    .expect("Invalid MATCH_SUBSTITUTE pattern")
});

#[pyfunction]
pub fn substitution(value: &str, py_dict: &Bound<'_, PyDict>) -> String {
    if value.contains('$') {
        let mut start = 0;
        let mut result = String::new();

        // Call the Python `drop_suffixes` function from the `utils` module
        let d: HashMap<String, String> = Python::with_gil(|py| {
            let utils = PyModule::import(py, "cartconf.utils").expect("Failed to import utils module");
            let drop_suffixes = utils.getattr("drop_suffixes").expect("Failed to get drop_suffixes function");
            let result = drop_suffixes
            .call1((py_dict, true))
            .expect("Failed to call drop_suffixes");
            let d_flat = result.downcast::<PyDict>().unwrap();
            // Convert the flattened PyDict to HashMap<String, String> to guarantee string values
            d_flat
                .iter()
                .filter_map(|(key, value)| {
                    let key = key.extract::<String>().ok()?;
                    // ignore deps, mapped files, or other list-like key values
                    let value = value.extract::<String>().ok()?;
                    Some((key, value))
                })
                .collect()
            });

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
}
