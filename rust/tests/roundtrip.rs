mod common;

use common::*;
use mcf_ext_clearbox::{
    pack, peek_schema_version, supported_schema_versions, tree_to_json, unpack, Attr, AttrValue,
    CalibrationPoint, ClearBoxConfig, ClearBoxUnit, Dtype, EquationConstant, Grid,
    LegacyUnitFields, PackOptions, PowerCharacterization, Scalar, SynchronousSensor,
    EXTENSION_NAME, REQUIRES_CONTRACT,
};

fn grid(shape: [usize; 3], seed: f64) -> Grid {
    let n = shape[0] * shape[1] * shape[2];
    Grid {
        shape,
        data: (0..n)
            .map(|i| {
                if i % 11 == 0 {
                    f64::NAN
                } else {
                    i as f64 * 0.001 + seed
                }
            })
            .collect(),
    }
}

fn rich_unit(id: &str) -> ClearBoxUnit {
    let mut extra = std::collections::BTreeMap::new();
    extra.insert("Port".to_string(), Attr::scalar(7i32));
    extra.insert("Big".to_string(), Attr::scalar(9_007_199_254_740_993i64));
    extra.insert(
        "List".to_string(),
        Attr {
            dtype: Dtype::U8,
            value: AttrValue::Array(vec![Scalar::U8(1), Scalar::U8(2), Scalar::U8(3)]),
        },
    );
    let mut sensor_extra = std::collections::BTreeMap::new();
    sensor_extra.insert("Note".to_string(), Attr::scalar("kept"));
    let mut pc_extra = std::collections::BTreeMap::new();
    pc_extra.insert("Scale".to_string(), Attr::scalar(3.5f64));
    ClearBoxUnit {
        train_id: id.into(),
        ip_address: "10.1.2.3".into(),
        serial_number: Some("SN-9".into()),
        data_port: Some(5001),
        server_port: Some(20101),
        actual_timing_offset: Some(-8),
        commanded_timing_offset: Some(50),
        manufacturer: Some("Trusted Metal".into()),
        model: Some("CB-2".into()),
        firmware_version: Some("2.4.1".into()),
        legacy: Some(LegacyUnitFields {
            selected_camera: Some("Cam".into()),
            custom_video_format: Some("MP4".into()),
            video_output: Some("HDMI".into()),
            show_console: Some(true),
            correction_grid_domain_shape: Some("5x5".into()),
            inverse_grid_domain_shape: Some("6x6".into()),
        }),
        correction_data: Some(grid([257, 257, 2], 1.0)),
        inverse_correction_data: Some(grid([257, 257, 2], 2.0)),
        synchronous_sensors: vec![
            SynchronousSensor {
                label: "A".into(),
                enabled: Some(true),
                sensor_name: Some("n".into()),
                sensor_output_range_low: Some(-1.5),
                sensor_output_range_high: Some(6.0),
                sensor_output_space: None,
                sensor_model: None,
                sensor_manufacturer: None,
                sensor_scope: None,
                units_derived_quantity: None,
                port_id: Some(3),
                sensor_type: None,
                input_type: None,
                algorithm_type: None,
                algorithm_equation: None,
                calibration_source: None,
                calibration_verified: Some(false),
                sample_period: Some(0.25),
                metadata: None,
                derivation_equation_constants: vec![EquationConstant {
                    name: "a".into(),
                    value: 1.0,
                }],
                calibration_points: vec![CalibrationPoint {
                    input_value: 4.0,
                    output_value: 0.5,
                }],
                extra: sensor_extra,
            },
            SynchronousSensor {
                label: "B".into(),
                enabled: None,
                sensor_name: None,
                sensor_output_range_low: None,
                sensor_output_range_high: None,
                sensor_output_space: None,
                sensor_model: None,
                sensor_manufacturer: None,
                sensor_scope: None,
                units_derived_quantity: None,
                port_id: None,
                sensor_type: None,
                input_type: None,
                algorithm_type: None,
                algorithm_equation: None,
                calibration_source: None,
                calibration_verified: None,
                sample_period: None,
                metadata: None,
                derivation_equation_constants: vec![],
                calibration_points: vec![],
                extra: Default::default(),
            },
        ],
        power_characterization: Some(PowerCharacterization {
            algorithm_type: Some("POLYNOMIAL".into()),
            algorithm_equation: Some("W = c0 + c1*V".into()),
            input_type: Some("Volts".into()),
            units_derived_quantity: Some("Watts".into()),
            derivation_equation_constants: vec![
                EquationConstant {
                    name: "c0".into(),
                    value: 0.5,
                },
                EquationConstant {
                    name: "c1".into(),
                    value: 2.0,
                },
            ],
            characterization_points: vec![CalibrationPoint {
                input_value: 1.0,
                output_value: 2.5,
            }],
            extra: pc_extra,
        }),
        extra,
    }
}

fn rich_config() -> ClearBoxConfig {
    let mut extra = std::collections::BTreeMap::new();
    extra.insert("Root_Note".to_string(), Attr::scalar("r"));
    ClearBoxConfig {
        output_path: Some("/out/".into()),
        software_trigger_delay: Some(44),
        units: vec![rich_unit("Optical_Train_01"), rich_unit("Optical_Train_02")],
        extra,
    }
}

fn packed(c: &ClearBoxConfig) -> mcf_ext_clearbox::ExtensionTree {
    pack(c, &PackOptions::default()).unwrap()
}

#[test]
fn unpack_of_pack_is_the_model_including_real_size_grids_with_nan() {
    let model = rich_config();
    let back = unpack(&packed(&model)).unwrap();
    assert_eq!(model_json(&back), model_json(&model));
}

#[test]
fn pack_of_unpack_is_the_tree() {
    let first = packed(&rich_config());
    let second = packed(&unpack(&first).unwrap());
    assert_eq!(canon(tree_to_json(&second)), canon(tree_to_json(&first)));
}

#[test]
fn units_and_sensors_come_back_sorted() {
    let mut model = rich_config();
    model.units.reverse();
    model.units[1].synchronous_sensors.reverse();
    let back = unpack(&packed(&model)).unwrap();
    let ids: Vec<&str> = back.units.iter().map(|u| u.train_id.as_str()).collect();
    assert_eq!(ids, ["Optical_Train_01", "Optical_Train_02"]);
    let labels: Vec<&str> = back.units[0]
        .synchronous_sensors
        .iter()
        .map(|s| s.label.as_str())
        .collect();
    assert_eq!(labels, ["A", "B"]);
}

#[test]
fn an_absent_number_or_flag_is_an_empty_string_and_reads_back_absent() {
    let model = ClearBoxConfig {
        output_path: None,
        software_trigger_delay: None,
        units: vec![ClearBoxUnit {
            train_id: "Optical_Train_01".into(),
            ip_address: "1.1.1.1".into(),
            serial_number: None,
            data_port: None,
            server_port: None,
            actual_timing_offset: None,
            commanded_timing_offset: None,
            manufacturer: None,
            model: None,
            firmware_version: None,
            legacy: None,
            correction_data: None,
            inverse_correction_data: None,
            synchronous_sensors: vec![],
            power_characterization: None,
            extra: Default::default(),
        }],
        extra: Default::default(),
    };
    let tree = packed(&model);
    let u = &tree.children["Optical_Train_01"];
    assert_eq!(u.attrs["Data_Port"], Attr::scalar(""));
    assert_eq!(tree.attrs["Software_Trigger_Delay"], Attr::scalar(""));
    assert_eq!(u.attrs["Firmware_Version"], Attr::scalar(""));
    assert!(!u.attrs.contains_key("Show_Console"));
    assert!(u.children.is_empty() && u.datasets.is_empty());
    assert_eq!(model_json(&unpack(&tree).unwrap()), model_json(&model));
}

#[test]
fn integers_are_i32_flags_are_0_or_1_and_grids_carry_core_attributes() {
    let tree = packed(&rich_config());
    let u = &tree.children["Optical_Train_01"];
    assert_eq!(u.attrs["Data_Port"], Attr::scalar(5001i32));
    assert_eq!(u.attrs["Show_Console"], Attr::scalar(1i32));
    let attrs = &u.datasets["Correction_Data"].attrs;
    assert_eq!(attrs["dimensions"], Attr::scalar("H,W,D"));
    assert_eq!(attrs["dtype"], Attr::scalar("float64"));
    assert_eq!(attrs["shape"], Attr::scalar("257x257x2"));
    let fields =
        &u.children["Power_Characterization"].tables["Derivation_Equation_Constants"].fields;
    assert_eq!(
        (
            fields[0].dtype,
            fields[0].size,
            fields[1].dtype,
            fields[1].size
        ),
        (Dtype::Utf8, Some(64), Dtype::F64, None)
    );
}

#[test]
fn facade_reports_name_contract_and_versions() {
    assert_eq!(EXTENSION_NAME, "ClearBox");
    assert_eq!(REQUIRES_CONTRACT, "^0.1.1");
    assert!(supported_schema_versions().is_empty());
    let mut t = fixture_tree("legacy/reference_config");
    assert_eq!(peek_schema_version(&t), "legacy");
    t.attrs.insert("Schema_Version".into(), Attr::scalar("2.0"));
    assert_eq!(peek_schema_version(&t), "2.0");
}
