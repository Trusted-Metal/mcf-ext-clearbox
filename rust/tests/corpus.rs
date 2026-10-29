//! Node-vs-Rust parity over real configs: skipped unless MCF_PARITY_DIR points at a directory prepared
//! by `tools/prepare_parity.py` and `node tools/unpack_all.mjs` (each case has new.json + model.json).

mod common;

use std::fs;

use common::*;
use mcf_ext_clearbox::{pack, tree_from_json, tree_to_json, unpack, PackOptions};

#[test]
fn rust_unpacks_every_real_tree_to_the_same_model_as_node() {
    let Ok(dir) = std::env::var("MCF_PARITY_DIR") else {
        return;
    };
    let mut checked = 0;
    for entry in fs::read_dir(&dir).unwrap().filter_map(|e| e.ok()) {
        let base = entry.path();
        let (tree_path, model_path) = (base.join("new.json"), base.join("model.json"));
        if !model_path.exists() {
            continue;
        }
        let name = fs::read_to_string(base.join("name.txt")).unwrap();
        let tree = tree_from_json(&read_json(&tree_path)).unwrap();
        let config = unpack(&tree).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(model_json(&config), canon(read_json(&model_path)), "{name}");

        let packed = pack(&config, &PackOptions::default()).unwrap();
        let again = unpack(&packed).unwrap();
        assert_eq!(
            model_json(&again),
            model_json(&config),
            "{name}: pack/unpack"
        );
        assert_eq!(
            canon(tree_to_json(
                &pack(&again, &PackOptions::default()).unwrap()
            )),
            canon(tree_to_json(&packed)),
            "{name}"
        );
        checked += 1;
    }
    assert!(checked > 0, "no cases with model.json in {dir}");
}
