use hashbrown::HashMap;
use pyo3::prelude::*;

mod tokens;
mod lexer;
mod filters;
mod parser;

#[cfg(test)]
mod tests {
    use super::*; // bring the module under test into scope
    use crate::tokens::ParamKeyHashMapExt;

    #[test]
    fn test_parse_dicts() {
        let dicts = parse_dicts(
            "".to_string(), "param = val\n".to_string(), -1, false, None
        ).unwrap();
        assert_eq!(dicts.len(), 1);
        assert_eq!(dicts[0].len(), 4);
        assert_ne!(dicts[0].get_str("name"), None);
        assert_ne!(dicts[0].get_str("shortname"), None);
        assert_ne!(dicts[0].get_str("dep"), None);
        assert_eq!(dicts[0].get_str("param"), Some(&tokens::ParamVal::String("val".to_string())));
    }
}

/// Parse parametric dictionaries from a configuration string.
#[pyfunction]
fn parse_dicts(
    cfgfile: String,
    cfgstr: String,
    prev_indent: isize,
    defaults: bool,
    expand_defaults: Option<Vec<String>>,
) -> PyResult<Vec<HashMap<tokens::ParamKey, tokens::ParamVal>>> {
    use crate::parser::{Tree, PreDict};
    use crate::tokens::{ParamKey, ParamVal};

    let mut tree = Tree::new()?;
    if !cfgfile.is_empty() {
        tree.parse_file(
            cfgfile, prev_indent, defaults, expand_defaults.clone()
        )?;
    }
    if !cfgstr.is_empty() {
        tree.parse_string(
            cfgstr, prev_indent, defaults, expand_defaults
        )?;
    }
    let mut pre_dict = PreDict::default();
    if !pre_dict.update_from_node(tree.borrow_root_mut()?.clone())? {
        return Err(PyErr::new::<parser::ParserError, _>((
            "Failed to generate PreDict from Node".to_string(),
            "",
            Some("<string>".to_string()),
            0,
        )));
    }
    let mut dicts = Vec::<HashMap<ParamKey, ParamVal>>::new();
    while let Some(dict) = pre_dict.get_dicts(true, true)? {
        dicts.push(dict);
    }
    Ok(dicts)
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
    parser_module.add_class::<parser::Tree>()?;
    parser_module.add_class::<parser::PreDict>()?;
    parser_module.add("ParserError", m.py().get_type::<parser::ParserError>())?;

    m.add_submodule(&tokens_module)?;
    m.add_submodule(&lexer_module)?;
    m.add_submodule(&filters_module)?;
    m.add_submodule(&parser_module)?;
    m.add_function(wrap_pyfunction!(parse_dicts, m)?)?;

    Ok(())
}
