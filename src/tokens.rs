use std::fmt;

use pyo3::prelude::*;

trait HasIdentifier {
    fn identifier(&self) -> &str;
}

// PartialEq +
trait TokenEq :  HasIdentifier {
    fn eq(&self, other: &Self) -> bool {
        self.identifier() == other.identifier()
    }
}

#[pyclass]
pub struct Token {
    identifier: String,
}

impl Token {
    fn new() -> Self {
        Token {
            identifier: "".to_string(),
        }
    }
}

impl HasIdentifier for Token {
    fn identifier(&self) -> &str {
        &self.identifier
    }
}

impl TokenEq for Token {}
//impl Eq for Token {}

impl fmt::Display for Token {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.identifier)
    }
}

impl fmt::Debug for Token {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self.identifier)
    }
}

#[pyclass]
pub struct LIndent {
    identifier: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_display() {
        let t1 = Token {
            identifier: "abc".to_string(),
        };
        assert_eq!(format!("{}", t1), "abc");
    }

    #[test]
    fn test_debug() {
        let t1 = Token {
            identifier: "xyz".to_string(),
        };
        assert_eq!(format!("{:?}", t1), format!("{:?}", "xyz"));
    }

    #[test]
    fn test_equality() {
        let t1 = Token {
            identifier: "abc".to_string(),
        };
        let t2 = Token {
            identifier: "abc".to_string(),
        };
        let t3 = Token {
            identifier: "abc".to_string(),
        };
        let t4 = Token {
            identifier: "def".to_string(),
        };
        let t5 = Token {
            identifier: "abe".to_string(),
        };
        // reflexivity of Eq
        assert!(t1 == t1);
        // commutativity of Eq
        assert!(t1 == t2);
        assert!(t2 == t1);
        // transitivity of Eq
        assert!(t2 == t3);
        assert!(t1 == t3);
        // complement of Eq
        assert!(t1 != t4);
        assert!(t1 != t5);
    }
}
