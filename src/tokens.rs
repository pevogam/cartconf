use std::fmt;

use pyo3::prelude::*;

// Define an enum for the different types of tokens
#[pyclass(eq)]
#[derive(Debug, PartialEq)]
pub enum Tokens {
    LIndent(i32),
    LEndL(),
    LEndBlock(i32),
    LIdentifier(),
    LWhite(),
    LString(),
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
            Tokens::LIdentifier() => write!(f, "Identifier re([A-Za-z0-9][A-Za-z0-9_-]*)"),
            Tokens::LWhite() => write!(f, "WhiteSpace re(\\s)"),
            Tokens::LString() => write!(f, "String re(.+)"),
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
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
