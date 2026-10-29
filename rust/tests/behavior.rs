mod common;

use common::*;
use mcf_ext_clearbox::{Attr, AttrValue, ClearBoxConfig, Dtype, Scalar};

fn with_unit_attr(name: &str, attr: Attr) -> ClearBoxConfig {
    let mut tree = fixture_tree("legacy/reference_config");
    group(&mut tree, &["Optical_Train_01"])
        .attrs
        .insert(name.into(), attr);
    unpack_ok(&tree)
}

fn utf8(s: &str) -> Attr {
    Attr::scalar(s)
}

#[test]
fn reference_config_reads_machine_wide_values_and_units() {
    let cfg = ref_config();
    assert_eq!(cfg.output_path.as_deref(), Some("/recordings/"));
    assert_eq!(cfg.software_trigger_delay, Some(3000));
    let ids: Vec<&str> = cfg.units.iter().map(|u| u.train_id.as_str()).collect();
    assert_eq!(ids, ["Optical_Train_01", "Optical_Train_02"]);
    let u = &cfg.units[0];
    assert_eq!(u.ip_address, "192.168.1.10");
    assert_eq!(u.serial_number.as_deref(), Some("001"));
    assert_eq!(
        (
            u.data_port,
            u.server_port,
            u.actual_timing_offset,
            u.commanded_timing_offset
        ),
        (Some(5001), Some(20101), Some(-8), Some(50))
    );
}

#[test]
fn an_empty_string_is_absent_and_legacy_fields_are_grouped() {
    let u = &ref_config().units[0];
    assert!(u.manufacturer.is_none() && u.model.is_none() && u.firmware_version.is_none());
    let l = u.legacy.as_ref().unwrap();
    assert_eq!(l.selected_camera.as_deref(), Some("Default"));
    assert_eq!(l.show_console, Some(false));
    assert!(l.correction_grid_domain_shape.is_none());
}

#[test]
fn power_characterization_keeps_coefficient_order() {
    let pc = ref_config().units[0]
        .power_characterization
        .clone()
        .unwrap();
    assert_eq!(pc.algorithm_type.as_deref(), Some("LINEAR"));
    assert_eq!(pc.algorithm_equation.as_deref(), Some("W = a*V + b"));
    assert!(pc.input_type.is_none() && pc.units_derived_quantity.is_none());
    let names: Vec<&str> = pc
        .derivation_equation_constants
        .iter()
        .map(|c| c.name.as_str())
        .collect();
    assert_eq!(names, ["b", "a"]);
    assert!(pc.characterization_points.is_empty());
}

#[test]
fn grids_are_row_major_with_nan_preserved() {
    let g = ref_config().units[0].correction_data.clone().unwrap();
    assert_eq!(g.shape, [3, 3, 2]);
    assert!(g.data[0].is_nan());
    assert_eq!(g.data[1], -2.5);
}

#[test]
fn v1_1_origin_has_firmware_and_no_legacy_group() {
    let cfg = unpack_ok(&fixture_tree("legacy/v1_1"));
    assert_eq!(cfg.units[0].firmware_version.as_deref(), Some("2.4.1"));
    assert!(cfg.units[0].legacy.is_none());
}

#[test]
fn synchronous_sensors_read_every_attribute_and_table() {
    let cfg = unpack_ok(&fixture_tree("legacy/synchronous_sensors"));
    let s = &cfg.units[0].synchronous_sensors;
    assert_eq!(s.len(), 1);
    assert_eq!(s[0].label, "Oxygen Sensor");
    assert_eq!(
        (s[0].enabled, s[0].calibration_verified, s[0].port_id),
        (Some(true), Some(false), Some(5))
    );
    assert_eq!(
        (
            s[0].sensor_output_range_low,
            s[0].sensor_output_range_high,
            s[0].sample_period
        ),
        (Some(-1.0), Some(6.0), Some(5.0))
    );
    assert_eq!(s[0].derivation_equation_constants.len(), 2);
    assert_eq!(s[0].calibration_points.len(), 2);
}

#[test]
fn lenient_number_reads_match_how_core_read_legacy_files() {
    assert_eq!(
        with_unit_attr("Data_Port", utf8("")).units[0].data_port,
        None
    );
    assert_eq!(
        with_unit_attr("Data_Port", utf8(" 5002 ")).units[0].data_port,
        Some(5002)
    );
    assert_eq!(
        with_unit_attr("Data_Port", Attr::scalar(7i64)).units[0].data_port,
        Some(7)
    );
    assert_eq!(
        with_unit_attr("Data_Port", Attr::scalar(9u8)).units[0].data_port,
        Some(9)
    );
    assert_eq!(
        with_unit_attr("Data_Port", Attr::scalar(11.0f64)).units[0].data_port,
        Some(11)
    );
}

#[test]
fn text_is_trimmed_except_the_required_ip_address() {
    assert_eq!(
        with_unit_attr("Serial_Number", utf8("  X1  ")).units[0]
            .serial_number
            .as_deref(),
        Some("X1")
    );
    assert_eq!(
        with_unit_attr("Ip_Address", utf8(" 10.0.0.1")).units[0].ip_address,
        " 10.0.0.1"
    );
    assert_eq!(
        with_unit_attr("Serial_Number", Attr::scalar(123i32)).units[0]
            .serial_number
            .as_deref(),
        Some("123")
    );
}

#[test]
fn unmodelled_attributes_stay_in_extra_with_their_dtype() {
    let u = with_unit_attr("Port", Attr::scalar(5001i32))
        .units
        .remove(0);
    let a = u.extra.get("Port").unwrap();
    assert_eq!(a.dtype, Dtype::I32);
    assert_eq!(a.value, AttrValue::Scalar(Scalar::I32(5001)));
}
