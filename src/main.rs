//! Main entry point for the cartconf binary.
//! This provides a command-line interface to parse configuration files
//! similar to the parse.py Python script.

use std::collections::HashMap;

use clap::{Arg, ArgAction, Command};

use crate::parser::{parse_file, parse_string, Node, PreDict};
use crate::tokens::{ParamKey, ParamVal};

mod filters;
mod lexer;
mod parser;
mod tokens;

/// Print dictionaries in the default mode
fn print_dicts_default(
    fullname: bool,
    contents: bool,
    dicts: &[HashMap<ParamKey, ParamVal>],
) {
    for (count, d) in dicts.iter().enumerate() {
        let name = if fullname {
            d.get(&ParamKey::from("name".to_string()))
                .map(|v| v.to_string())
                .unwrap_or_default()
        } else {
            d.get(&ParamKey::from("shortname".to_string()))
                .map(|v| v.to_string())
                .unwrap_or_default()
        };
        println!("dict {:4}:  {}", count + 1, name);

        if contents {
            let mut keys: Vec<&ParamKey> = d.keys().collect();
            keys.sort_by_key(|a| a.to_string());
            for key in keys {
                if let Some(value) = d.get(key) {
                    println!("    {} = {}", key, value);
                }
            }
        }
    }
}

fn py_quote_string(value: &str) -> String {
    let escaped = value
        .replace('\\', "\\\\")
        .replace('\n', "\\n")
        .replace('"', "\\\"");
    format!("\"{}\"", escaped)
}

fn py_repr_key(key: &ParamKey) -> String {
    match key {
        ParamKey::String(s) => py_quote_string(s),
        ParamKey::Tuple(parts) => {
            let items: Vec<String> = parts.iter().map(|part| py_quote_string(part)).collect();
            format!("({})", items.join(", "))
        }
    }
}

fn py_repr_val(value: &ParamVal) -> String {
    match value {
        ParamVal::String(s) => py_quote_string(s),
        ParamVal::List(items) => {
            let items: Vec<String> = items.iter().map(|item| py_quote_string(item)).collect();
            format!("[{}]", items.join(", "))
        }
        ParamVal::Dict(map) => {
            let mut parts: Vec<String> = map
                .iter()
                .map(|(k, v)| format!("{}: {}", py_quote_string(k), py_quote_string(v)))
                .collect();
            parts.sort();
            format!("{{{}}}", parts.join(", "))
        }
    }
}

/// Print dictionaries in repr mode (Python format)
fn print_dicts_repr(dicts: &[HashMap<ParamKey, ParamVal>]) {
    println!("[");
    for d in dicts {
        let mut items: Vec<String> = d
            .iter()
            .map(|(k, v)| format!("{}: {}", py_repr_key(k), py_repr_val(v)))
            .collect();
        items.sort();
        println!("    {{{}}},", items.join(", "));
    }
    println!("]");
}

fn main() {
    let matches = Command::new("cartconf")
        .version("0.1.0")
        .author("Cartconf Team")
        .about("Parse configuration files and output dictionaries")
        .override_usage(
            "usage: cartconf [options] filename [extra code] ...\n\nExample:\n\n    cartconf tests.cfg \"only my_set\" \"no qcow2\"",
        )
        .arg(
            Arg::new("filename")
                .help("Configuration file to parse")
                .required(true)
                .index(1),
        )
        .arg(
            Arg::new("extra")
                .help("Extra code to parse after the file")
                .num_args(1..)
                .last(true)
                .index(2),
        )
        .arg(
            Arg::new("verbose")
                .short('v')
                .long("verbose")
                .help("include debug messages in console output")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("fullname")
                .short('f')
                .long("fullname")
                .help("show full dict names instead of short names")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("contents")
                .short('c')
                .long("contents")
                .help("show dict contents")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("repr")
                .short('r')
                .long("repr")
                .help("output parsing results Python format")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("defaults")
                .short('d')
                .long("defaults")
                .help("use only default variant of variants if there is some")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("expand")
                .short('e')
                .long("expand")
                .help("list of variants which should be expanded when defaults is enabled. \"name, name, name\"")
                .value_name("EXPAND")
                .num_args(1),
        )
        .arg(
            Arg::new("skip-dups")
                .short('s')
                .long("skip-dups")
                .help("Don't drop variables with different suffixes and same val")
                .action(ArgAction::SetFalse),
        )
        .get_matches();

    let verbose = matches.get_flag("verbose");
    let fullname = matches.get_flag("fullname");
    let contents = matches.get_flag("contents");
    let repr_mode = matches.get_flag("repr");
    let defaults = matches.get_flag("defaults");
    let skipdups = matches.get_flag("skip-dups");

    let expand: Vec<String> = if let Some(expand_str) = matches.get_one::<String>("expand") {
        expand_str.split(',').map(|s| s.trim().to_string()).collect()
    } else {
        Vec::new()
    };

    let filename = matches.get_one::<String>("filename").unwrap();
    let extra_strings: Vec<String> = if let Some(vals) = matches.get_many::<String>("extra") {
        vals.cloned().collect()
    } else {
        Vec::new()
    };

    if let Err(err) = run_parser(
        filename,
        &extra_strings,
        defaults,
        &expand,
        verbose,
        skipdups,
        fullname,
        contents,
        repr_mode,
    ) {
        eprintln!("Error: {}", err);
        std::process::exit(1);
    }
}

#[allow(clippy::too_many_arguments)]
fn run_parser(
    filename: &str,
    extra_strings: &[String],
    defaults: bool,
    expand_defaults: &[String],
    debug: bool,
    skipdups: bool,
    fullname: bool,
    contents: bool,
    repr_mode: bool,
) -> Result<(), String> {
    let mut node = Node::default();

    node = parse_file(
        filename.to_string(),
        node,
        -1,
        defaults,
        if expand_defaults.is_empty() {
            None
        } else {
            Some(expand_defaults.to_vec())
        },
    )
    .map_err(|e| e.to_string())?;

    for extra in extra_strings {
        node = parse_string(
            extra.clone(),
            node,
            -1,
            defaults,
            if expand_defaults.is_empty() {
                None
            } else {
                Some(expand_defaults.to_vec())
            },
        )
        .map_err(|e| e.to_string())?;
    }

    if debug {
        println!("{}", node.dump(0, true));
    }

    let mut pre_dict = PreDict::default();
    if !pre_dict.update_from_node(node).map_err(|e| e.to_string())? {
        return Err("Failed to generate PreDict from Node".to_string());
    }

    let mut dicts = Vec::<HashMap<ParamKey, ParamVal>>::new();
    while let Some(dict) = pre_dict.get_dicts(true, skipdups).map_err(|e| e.to_string())? {
        dicts.push(dict);
    }

    if repr_mode {
        print_dicts_repr(&dicts);
    } else {
        print_dicts_default(fullname, contents, &dicts);
    }

    Ok(())
}
