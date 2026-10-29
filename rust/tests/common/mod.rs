#![allow(dead_code)]

use std::fs;
use std::path::{Path, PathBuf};

use mcf_ext_clearbox::{tree_from_json, unpack, ClearBoxConfig, ExtensionTree};
use serde_json::Value;

pub fn fixtures_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../fixtures")
}

pub fn fixture_names(group: &str) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(fixtures_dir().join(group))
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

pub fn read_json(path: &Path) -> Value {
    serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap()
}

pub fn fixture_tree_json(rel: &str) -> Value {
    read_json(&fixtures_dir().join(rel).join("tree.json"))
}

pub fn fixture_tree(rel: &str) -> ExtensionTree {
    tree_from_json(&fixture_tree_json(rel)).unwrap()
}

pub fn fixture_model_json(rel: &str) -> Value {
    read_json(&fixtures_dir().join(rel).join("model.json"))
}

pub fn unpack_ok(tree: &ExtensionTree) -> ClearBoxConfig {
    unpack(tree).unwrap()
}

pub fn ref_config() -> ClearBoxConfig {
    unpack_ok(&fixture_tree("legacy/reference_config"))
}

/// JSON with integral floats turned into integers, so `5.0` and `5` compare equal.
pub fn canon(v: Value) -> Value {
    match v {
        Value::Number(n) => match n.as_f64() {
            Some(f) if n.is_f64() && f.fract() == 0.0 && f.abs() < 9e15 => Value::from(f as i64),
            _ => Value::Number(n),
        },
        Value::Array(a) => Value::Array(a.into_iter().map(canon).collect()),
        Value::Object(o) => Value::Object(o.into_iter().map(|(k, v)| (k, canon(v))).collect()),
        other => other,
    }
}

pub fn model_json(c: &ClearBoxConfig) -> Value {
    canon(serde_json::to_value(c).unwrap())
}

pub fn group<'a>(tree: &'a mut ExtensionTree, names: &[&str]) -> &'a mut ExtensionTree {
    let mut g = tree;
    for n in names {
        g = g.children.get_mut(*n).unwrap();
    }
    g
}
