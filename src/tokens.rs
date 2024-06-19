use std::fmt;

use pyo3::prelude::*;

#[derive(Eq)]
struct Identifiable {
    identifier: String,
}

impl PartialEq for Identifiable {
    fn eq(&self, other: &Self) -> bool {
        self.identifier == other.identifier
    }
}

impl fmt::Display for Identifiable {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.identifier)
    }
}

impl fmt::Debug for Identifiable {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self.identifier)
    }
}

macro_rules! wrap_identifiable {
    {$name:ty} => {
        impl PartialEq for $name {
            fn eq(&self, other: &Self) -> bool {
                self.identifiable == other.identifiable
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                self.identifiable.fmt(f)
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                self.identifiable.fmt(f)
            }
        }
    }
}

#[pyclass]
#[derive(Eq)]
pub struct Token {
    identifiable: Identifiable,
}
wrap_identifiable!(Token);
impl Token {
    fn new() -> Self {
        Token {
            identifiable: Identifiable { identifier: "".to_string() },
        }
    }
}

#[pyclass]
#[derive(Eq)]
pub struct LIndent {
    identifiable: Identifiable,
    length: i32,
}
wrap_identifiable!(LIndent);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_display() {
        let t1 = Identifiable {
            identifier: "abc".to_string(),
        };
        assert_eq!(format!("{}", t1), "abc");
    }

    #[test]
    fn test_debug() {
        let t1 = Identifiable {
            identifier: "xyz".to_string(),
        };
        assert_eq!(format!("{:?}", t1), format!("{:?}", "xyz"));
    }

    #[test]
    fn test_equality() {
        let t1 = Identifiable {
            identifier: "abc".to_string(),
        };
        let t2 = Identifiable {
            identifier: "abc".to_string(),
        };
        let t3 = Identifiable {
            identifier: "abc".to_string(),
        };
        let t4 = Identifiable {
            identifier: "def".to_string(),
        };
        let t5 = Identifiable {
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
