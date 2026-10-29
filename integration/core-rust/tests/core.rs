// Core (Rust) reads and writes the files, this codec packs and unpacks the ClearBox tree.
// Needs the HDF5 library on the path, same as core. Run: cargo test --manifest-path integration/core-rust/Cargo.toml
use std::path::{Path, PathBuf};

use machine_config::{
    canonical_digest, extension_contract, ExtensionTree as CoreTree, MachineConfig,
    MachineConfigReader, MachineConfigWriter, MockConfigBuilder, EXTENSION_CONTRACT_VERSION,
};
use mcf_ext_clearbox::{
    pack, tree_from_json, tree_to_json, unpack, ClearBoxConfig, ErrorCode, PackOptions,
    EXTENSION_NAME, REQUIRES_CONTRACT,
};
use serde_json::{json, Value};

const VERSIONS: [&str; 2] = ["1.0", "1.1"];

fn core_fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../Machine_Config_Library/fixtures")
}

/// The two crates keep separate copies of the tree types; the contract JSON form is the bridge.
fn to_ext(t: &CoreTree) -> mcf_ext_clearbox::ExtensionTree {
    tree_from_json(&extension_contract::tree_to_json(t)).unwrap()
}

fn to_core(t: &mcf_ext_clearbox::ExtensionTree) -> CoreTree {
    extension_contract::tree_from_json(&tree_to_json(t)).unwrap()
}

/// Present-but-empty and absent attributes mean the same to every reader, so digests ignore them.
fn strip_empty(v: &mut Value) {
    if let Some(attrs) = v.get_mut("attrs").and_then(Value::as_object_mut) {
        attrs.retain(|_, a| !(a["dtype"] == "utf8" && a["value"] == ""));
    }
    if let Some(children) = v.get_mut("children").and_then(Value::as_object_mut) {
        children.values_mut().for_each(strip_empty);
    }
}

fn digest(t: &CoreTree) -> String {
    let mut v = extension_contract::tree_to_json(t);
    strip_empty(&mut v);
    canonical_digest(&extension_contract::tree_from_json(&v).unwrap()).unwrap()
}

/// Model as JSON: `f64::NAN` grid points compare equal there, unlike through `PartialEq`.
fn model_json(m: &ClearBoxConfig) -> Value {
    serde_json::to_value(m).unwrap()
}

fn unpack_core(t: &CoreTree) -> ClearBoxConfig {
    unpack(&to_ext(t)).unwrap_or_else(|e| panic!("unpack failed: {e:?}"))
}

fn pack_core(m: &ClearBoxConfig) -> CoreTree {
    to_core(&pack(m, &PackOptions::default()).unwrap_or_else(|e| panic!("pack failed: {e:?}")))
}

fn clearbox(cfg: &MachineConfig) -> &CoreTree {
    cfg.extensions
        .get(EXTENSION_NAME)
        .expect("no ClearBox extension")
}

fn with_clearbox(cfg: &MachineConfig, tree: CoreTree) -> MachineConfig {
    let mut c = cfg.clone();
    c.extensions.insert(EXTENSION_NAME.into(), tree);
    c
}

fn through(cfg: &MachineConfig, version: &str, dir: &Path, name: &str) -> MachineConfig {
    let out = dir.join(format!("{name}_{version}.h5"));
    MachineConfigWriter::with_target_version(cfg, version)
        .write(&out)
        .unwrap();
    MachineConfigReader::open(&out).unwrap().parse().unwrap()
}

fn mock(laser_count: usize, version: &str, dir: &Path) -> MachineConfig {
    let cfg = MockConfigBuilder::new(laser_count).build();
    through(&cfg, version, dir, &format!("mock{laser_count}"))
}

/// What a File_Version 1.0 file keeps of a model: no firmware version, and power characterization only as constants.
fn v1_0_view(m: &ClearBoxConfig) -> ClearBoxConfig {
    let mut m = m.clone();
    for u in &mut m.units {
        u.firmware_version = None;
        if let Some(pc) = &mut u.power_characterization {
            pc.input_type = None;
            pc.units_derived_quantity = None;
            pc.characterization_points.clear();
        }
    }
    m
}

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("mcf-clearbox-it-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

#[test]
fn core_contract_version_is_accepted() {
    let req = REQUIRES_CONTRACT.trim_start_matches('^');
    let parse = |s: &str| {
        s.split('.')
            .map(|p| p.parse::<u32>().unwrap())
            .collect::<Vec<_>>()
    };
    let (r, c) = (parse(req), parse(EXTENSION_CONTRACT_VERSION));
    assert_eq!(c[0], r[0]);
    if r[0] == 0 {
        assert_eq!(c[1], r[1], "caret on 0.x pins the minor");
    } else {
        assert!(c[1] >= r[1]);
    }
    if c[1] == r[1] {
        assert!(c[2] >= r[2]);
    }
}

#[test]
fn every_core_fixture_with_clearbox_round_trips() {
    let dir = tmp("fixtures");
    let mut names: Vec<_> = std::fs::read_dir(core_fixtures())
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "h5"))
        .collect();
    names.sort();

    let mut seen = 0;
    for path in names {
        let name = path.file_stem().unwrap().to_string_lossy().into_owned();
        let cfg = MachineConfigReader::open(&path).unwrap().parse().unwrap();
        let Some(tree) = cfg.extensions.get(EXTENSION_NAME) else {
            continue;
        };
        seen += 1;

        let model = unpack_core(tree);
        // Not digest-equal to `tree` in general: integers are normalized to i32 on write (a sample file stores i64).
        let repacked = pack_core(&model);
        assert_eq!(
            model_json(&unpack_core(&repacked)),
            model_json(&model),
            "{name}: unpack(pack(model)) changed the model"
        );
        assert_eq!(
            digest(&pack_core(&unpack_core(&repacked))),
            digest(&repacked),
            "{name}: pack is not a fixpoint"
        );

        for v in VERSIONS {
            // File_Version 1.0 has no Extensions group, so ClearBox is the only extension it can carry.
            let mut target = with_clearbox(&cfg, repacked.clone());
            if v == "1.0" {
                target.extensions.retain(|k, _| k == EXTENSION_NAME);
            }
            let back = through(&target, v, &dir, &name);
            assert_eq!(back.meta.file_version, v);
            for (other, t) in &target.extensions {
                if other != EXTENSION_NAME {
                    assert_eq!(
                        digest(&back.extensions[other]),
                        digest(t),
                        "{name}: {other} changed"
                    );
                }
            }
            let back_tree = clearbox(&back);
            // A 1.0 layout has no place for the 1.1-only fields, so only a 1.1 target (or a 1.0 source) is lossless.
            if v == "1.1" || cfg.meta.file_version == "1.0" {
                assert_eq!(
                    digest(back_tree),
                    digest(&repacked),
                    "{name} -> {v}: core changed the packed tree"
                );
                assert_eq!(
                    model_json(&unpack_core(back_tree)),
                    model_json(&model),
                    "{name} -> {v}: model changed"
                );
            } else {
                assert_eq!(
                    model_json(&unpack_core(back_tree)),
                    model_json(&v1_0_view(&model)),
                    "{name} -> {v}: lost more than the 1.1-only fields"
                );
            }
        }
    }
    assert!(seen > 0);
}

#[test]
fn core_mock_config_round_trips_through_the_codec() {
    let dir = tmp("mock");
    for v in VERSIONS {
        let cfg = mock(3, v, &dir);
        let model = unpack_core(clearbox(&cfg));
        let ids: Vec<_> = model.units.iter().map(|u| u.train_id.clone()).collect();
        let train_ids: Vec<_> = cfg
            .optical_trains
            .iter()
            .map(|t| t.train_id.clone())
            .collect();
        assert_eq!(ids, train_ids);

        let back = through(&with_clearbox(&cfg, pack_core(&model)), v, &dir, "mock_rt");
        assert_eq!(
            model_json(&unpack_core(clearbox(&back))),
            model_json(&model),
            "File_Version {v}"
        );
    }
}

#[test]
fn editing_the_model_and_writing_it_back_keeps_the_edit_and_the_rest_of_the_file() {
    let dir = tmp("edit");
    for v in VERSIONS {
        let cfg = mock(2, v, &dir);
        let mut edited = unpack_core(clearbox(&cfg));
        edited.units[0].ip_address = "192.168.77.5".into();
        edited.units[0].data_port = Some(6123);
        edited.units[1].synchronous_sensors.clear();
        edited.software_trigger_delay = Some(42);

        let back = through(&with_clearbox(&cfg, pack_core(&edited)), v, &dir, "edit");
        assert_eq!(
            model_json(&unpack_core(clearbox(&back))),
            model_json(&edited),
            "File_Version {v}"
        );
        assert_eq!(
            serde_json::to_value(&back.machine).unwrap(),
            serde_json::to_value(&cfg.machine).unwrap()
        );
        let ids = |c: &MachineConfig| {
            c.optical_trains
                .iter()
                .map(|t| t.train_id.clone())
                .collect::<Vec<_>>()
        };
        assert_eq!(ids(&back), ids(&cfg));
    }
}

#[test]
fn a_config_without_clearbox_gains_one_from_pack() {
    let dir = tmp("bare");
    let mut b = MockConfigBuilder::new(2);
    b.include_clearbox = false;
    let bare = b.build();
    assert!(!bare.extensions.contains_key(EXTENSION_NAME));

    let model = unpack_core(clearbox(&MockConfigBuilder::new(2).build()));
    for v in VERSIONS {
        let back = through(&with_clearbox(&bare, pack_core(&model)), v, &dir, "bare");
        assert_eq!(
            model_json(&unpack_core(clearbox(&back))),
            model_json(&model),
            "File_Version {v}"
        );

        let none = through(&bare, v, &dir, "none");
        assert!(
            !none.extensions.contains_key(EXTENSION_NAME),
            "File_Version {v}"
        );
    }
}

#[test]
fn core_keeps_a_tree_the_codec_rejects_verbatim() {
    let dir = tmp("future");
    let cfg = mock(2, "1.1", &dir);

    let mut j = extension_contract::tree_to_json(clearbox(&cfg));
    j["attrs"]["Schema_Version"] = json!({"dtype": "utf8", "value": "2.0"});
    let tree = extension_contract::tree_from_json(&j).unwrap();
    let back = through(&with_clearbox(&cfg, tree.clone()), "1.1", &dir, "future");
    assert_eq!(digest(clearbox(&back)), digest(&tree));
    let e = unpack(&to_ext(clearbox(&back))).unwrap_err();
    assert_eq!(e.code, ErrorCode::UnsupportedSchemaVersion);

    j["attrs"].as_object_mut().unwrap().remove("Schema_Version");
    j["children"]["Optical_Train_01"]["children"]["Future_Group"] = json!({"attrs": {"Note": {"dtype": "utf8", "value": "future"}}, "children": {}, "datasets": {}});
    let tree = extension_contract::tree_from_json(&j).unwrap();
    let back = through(&with_clearbox(&cfg, tree.clone()), "1.1", &dir, "future2");
    assert_eq!(digest(clearbox(&back)), digest(&tree));
    let e = unpack(&to_ext(clearbox(&back))).unwrap_err();
    assert_eq!(e.code, ErrorCode::Validation);
    assert!(e.path.contains("Future_Group"), "{}", e.path);
}

#[test]
fn a_1_0_config_written_as_1_1_and_back_keeps_the_model() {
    let dir = tmp("cross");
    let cfg = mock(2, "1.0", &dir);
    let model = unpack_core(clearbox(&cfg));
    let as11 = through(&cfg, "1.1", &dir, "up");
    let as10 = through(&as11, "1.0", &dir, "down");
    assert_eq!(
        model_json(&unpack_core(clearbox(&as11))),
        model_json(&model)
    );
    assert_eq!(
        model_json(&unpack_core(clearbox(&as10))),
        model_json(&model)
    );
}
