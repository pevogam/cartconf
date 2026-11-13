use std::{fmt, io};

use crate::lexer::LexerError;
use crate::parser::ParserError;

/// Errors returned by configuration parsing and dictionary generation.
#[derive(Debug)]
pub enum Error {
    Io(io::Error),
    Lexer(LexerError),
    Parser(ParserError),
    Value(String),
    Type(String),
    Attribute(String),
    Runtime(String),
}

pub type Result<T> = std::result::Result<T, Error>;

impl Error {
    pub(crate) fn value(message: impl Into<String>) -> Self {
        Self::Value(message.into())
    }

    pub(crate) fn type_error(message: impl Into<String>) -> Self {
        Self::Type(message.into())
    }

    pub(crate) fn attribute(message: impl Into<String>) -> Self {
        Self::Attribute(message.into())
    }

    pub(crate) fn runtime(message: impl Into<String>) -> Self {
        Self::Runtime(message.into())
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(err) => err.fmt(f),
            Self::Lexer(err) => err.fmt(f),
            Self::Parser(err) => err.fmt(f),
            Self::Value(message) | Self::Type(message) | Self::Attribute(message)
                | Self::Runtime(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(err) => Some(err),
            Self::Lexer(err) => Some(err),
            Self::Parser(err) => Some(err),
            _ => None,
        }
    }
}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Self {
        Self::Io(err)
    }
}

impl From<LexerError> for Error {
    fn from(err: LexerError) -> Self {
        Self::Lexer(err)
    }
}

#[cfg(feature = "python")]
impl From<Error> for pyo3::PyErr {
    fn from(err: Error) -> Self {
        use pyo3::exceptions::{PyAttributeError, PyRuntimeError, PyTypeError, PyValueError};

        match err {
            Error::Io(err) => err.into(),
            Error::Lexer(err) => Self::new::<LexerError, _>(err.into_parts()),
            Error::Parser(err) => Self::new::<ParserError, _>(err.lexer_error.into_parts()),
            Error::Value(message) => PyValueError::new_err(message),
            Error::Type(message) => PyTypeError::new_err(message),
            Error::Attribute(message) => PyAttributeError::new_err(message),
            Error::Runtime(message) => PyRuntimeError::new_err(message),
        }
    }
}
