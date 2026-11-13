use std::io::Write;
use std::process::Command;

use cartconf::{Error, ParamKeyHashMapExt, ParamVal, PreDict, Tree, parse_dicts};

#[test]
fn test_library_and_cli() {
    let mut config = tempfile::NamedTempFile::new().unwrap();
    config.write_all(b"variants:\n    - a:\n        value = one\n    - b:\n        value = two\n").unwrap();
    let filename = config.path().to_str().unwrap();
    let tree = Tree::new(None).parse_file(filename.to_string(), false, None).unwrap();
    let tree = tree.parse_string("only b\n".to_string(), false, None).unwrap();
    let mut pre_dict = PreDict::default();
    assert!(pre_dict.update_from_tree(&tree).unwrap());
    let dict = pre_dict.get_dicts(true, true).unwrap().unwrap();
    assert_eq!(dict.get_str("value"), Some(&ParamVal::String("two".to_string())));
    assert!(pre_dict.get_dicts(true, true).unwrap().is_none());

    let output = Command::new(env!("CARGO_BIN_EXE_cartconf"))
        .args(["-r", filename, "--", "only b"])
        .output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"value\": \"two\""));
    assert!(!stdout.contains("\"value\": \"one\""));
}

#[test]
fn test_native_errors() {
    let config = tempfile::tempdir().unwrap();
    let missing = config.path().join("missing.cfg");
    let err = Tree::new(None).parse_file(missing.to_str().unwrap().to_string(), false, None).unwrap_err();
    assert!(matches!(err, Error::Io(_)));
    assert!(std::error::Error::source(&err).is_some());

    let err = parse_dicts(String::new(), "only .bad\n".to_string(), None, false, None).unwrap_err();
    assert!(matches!(err, Error::Parser(_)));
    assert!(err.to_string().contains("Filter cannot start with '.'"));
}
