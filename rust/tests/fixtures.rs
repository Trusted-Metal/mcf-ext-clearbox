mod common;

use common::*;
use mcf_ext_clearbox::{pack, tree_to_json, ClearBoxConfig, PackOptions};

#[test]
fn every_legacy_fixture_unpacks_to_its_model_and_packs_back() {
    let names = fixture_names("legacy");
    assert!(names.len() >= 3);
    for name in names {
        let rel = format!("legacy/{name}");
        let tree = fixture_tree(&rel);

        let config = unpack_ok(&tree);
        assert_eq!(
            model_json(&config),
            canon(fixture_model_json(&rel)),
            "{rel}: unpack"
        );

        let model: ClearBoxConfig = serde_json::from_value(fixture_model_json(&rel)).unwrap();
        let packed = pack(&model, &PackOptions::default()).unwrap();
        assert_eq!(
            canon(tree_to_json(&packed)),
            canon(fixture_tree_json(&rel)),
            "{rel}: pack"
        );
    }
}
