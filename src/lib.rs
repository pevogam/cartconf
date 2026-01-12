use pyo3::prelude::*;

mod tokens;
mod lexer;
mod filters;
mod parser;

#[cfg(test)]
mod tests {
    use super::*; // bring the module under test into scope

    #[test]
    fn it_works() {
        let result: usize = add(2, 2);
        assert_eq!(result, 4);
    }
}

fn add(a: usize, b: usize) -> usize {
    a + b
}

/// Formats the sum of two numbers as string.
#[pyfunction]
fn sum_as_string(a: usize, b: usize) -> PyResult<String> {
    Ok((add(a, b)).to_string())
}

/// A Python module implemented in Rust.
#[pymodule]
fn cartconf(m: &Bound<'_, PyModule>) -> PyResult<()> {

    let tokens_module = PyModule::new(m.py(), "tokens")?;
    tokens_module.add_class::<tokens::Tokens>()?;

    let lexer_module = PyModule::new(m.py(), "lexer")?;
    lexer_module.add_class::<lexer::Reader>()?;
    lexer_module.add_class::<lexer::Lexer>()?;
    lexer_module.add("LexerError", m.py().get_type::<lexer::LexerError>())?;

    let filters_module = PyModule::new(m.py(), "filters")?;
    filters_module.add_class::<filters::Filters>()?;

    let parser_module = PyModule::new(m.py(), "parser")?;
    parser_module.add_class::<parser::Label>()?;
    parser_module.add_class::<parser::Node>()?;
    parser_module.add_function(wrap_pyfunction!(parser::parse_string, m)?)?;
    parser_module.add_function(wrap_pyfunction!(parser::parse_file, m)?)?;
    parser_module.add_class::<parser::PreDict>()?;
    parser_module.add("ParserError", m.py().get_type::<parser::ParserError>())?;

    m.add_submodule(&tokens_module)?;
    m.add_submodule(&lexer_module)?;
    m.add_submodule(&filters_module)?;
    m.add_submodule(&parser_module)?;
    m.add_function(wrap_pyfunction!(sum_as_string, m)?)?;

    Ok(())
}
