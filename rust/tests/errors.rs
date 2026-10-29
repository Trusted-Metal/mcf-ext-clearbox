mod common;

use common::*;
use mcf_ext_clearbox::{
    pack, unpack, Attr, ClearBoxConfig, ColumnData, Dataset, DatasetData, Dtype, ErrorCode,
    ExtensionError, ExtensionTree, PackOptions, Table, TableField,
};

const REF: &str = "legacy/reference_config";

fn unpack_err(tree: &ExtensionTree) -> ExtensionError {
    unpack(tree).unwrap_err()
}

fn pack_err(c: &ClearBoxConfig) -> ExtensionError {
    pack(c, &PackOptions::default()).unwrap_err()
}

fn assert_at(e: &ExtensionError, code: ErrorCode, path: &str) {
    assert_eq!((e.code, e.path.as_str()), (code, path), "{e}");
}

fn ref_tree() -> ExtensionTree {
    fixture_tree(REF)
}

fn f64_dataset() -> Dataset {
    Dataset {
        shape: vec![1],
        data: DatasetData::F64(vec![0.0]),
        attrs: Default::default(),
    }
}

#[test]
fn unpack_requires_the_ip_address() {
    let mut t = ref_tree();
    group(&mut t, &["Optical_Train_01"])
        .attrs
        .remove("Ip_Address");
    assert_at(
        &unpack_err(&t),
        ErrorCode::Validation,
        "Optical_Train_01/@Ip_Address",
    );
}

#[test]
fn unpack_rejects_bad_scalars() {
    let cases: Vec<(&str, Attr)> = vec![
        (
            "Data_Port",
            Attr {
                dtype: Dtype::I32,
                value: mcf_ext_clearbox::AttrValue::Array(vec![1i32.into(), 2i32.into()]),
            },
        ),
        ("Show_Console", Attr::scalar(2i32)),
        ("Data_Port", Attr::scalar("abc")),
        ("Data_Port", Attr::scalar(1.5f64)),
    ];
    for (name, attr) in cases {
        let mut t = ref_tree();
        group(&mut t, &["Optical_Train_01"])
            .attrs
            .insert(name.into(), attr);
        assert_at(
            &unpack_err(&t),
            ErrorCode::Validation,
            &format!("Optical_Train_01/@{name}"),
        );
    }
}

#[test]
fn unpack_rejects_what_it_cannot_preserve() {
    let mut t = ref_tree();
    group(&mut t, &["Optical_Train_01"])
        .children
        .insert("Surprise".into(), ExtensionTree::default());
    assert_at(
        &unpack_err(&t),
        ErrorCode::Validation,
        "Optical_Train_01/Surprise",
    );

    let mut t = ref_tree();
    group(&mut t, &["Optical_Train_01"])
        .datasets
        .insert("Extra_Data".into(), f64_dataset());
    assert_at(
        &unpack_err(&t),
        ErrorCode::Validation,
        "Optical_Train_01/Extra_Data",
    );

    let mut t = ref_tree();
    t.datasets.insert("Stray".into(), f64_dataset());
    assert_at(&unpack_err(&t), ErrorCode::Validation, "Stray");

    let mut t = ref_tree();
    let table = Table {
        fields: vec![TableField {
            name: "x".into(),
            dtype: Dtype::F64,
            size: None,
        }],
        rows: 0,
        columns: vec![ColumnData::F64(vec![])],
        attrs: Default::default(),
    };
    group(&mut t, &["Optical_Train_01", "Power_Characterization"])
        .tables
        .insert("Extra_Table".into(), table);
    assert_at(
        &unpack_err(&t),
        ErrorCode::Validation,
        "Optical_Train_01/Power_Characterization/Extra_Table",
    );
}

#[test]
fn unpack_checks_grids() {
    let mut t = ref_tree();
    group(&mut t, &["Optical_Train_01"])
        .datasets
        .get_mut("Correction_Data")
        .unwrap()
        .data = DatasetData::F32(vec![0.0; 18]);
    assert_at(
        &unpack_err(&t),
        ErrorCode::Validation,
        "Optical_Train_01/Correction_Data",
    );

    let mut t = ref_tree();
    group(&mut t, &["Optical_Train_01"])
        .datasets
        .get_mut("Correction_Data")
        .unwrap()
        .shape = vec![18];
    assert_at(
        &unpack_err(&t),
        ErrorCode::Validation,
        "Optical_Train_01/Correction_Data",
    );

    let mut t = ref_tree();
    group(&mut t, &["Optical_Train_01"])
        .datasets
        .get_mut("Correction_Data")
        .unwrap()
        .attrs
        .insert("units".into(), Attr::scalar("mm"));
    assert_at(
        &unpack_err(&t),
        ErrorCode::Validation,
        "Optical_Train_01/Correction_Data/@units",
    );
}

#[test]
fn unpack_checks_table_fields_and_attrs() {
    let mut t = ref_tree();
    let pc = group(&mut t, &["Optical_Train_01", "Power_Characterization"]);
    pc.tables
        .get_mut("Derivation_Equation_Constants")
        .unwrap()
        .fields[1]
        .dtype = Dtype::F32;
    assert_at(
        &unpack_err(&t),
        ErrorCode::Validation,
        "Optical_Train_01/Power_Characterization/Derivation_Equation_Constants",
    );

    let mut t = ref_tree();
    let pc = group(&mut t, &["Optical_Train_01", "Power_Characterization"]);
    pc.tables
        .get_mut("Characterization_Points")
        .unwrap()
        .attrs
        .insert("note".into(), Attr::scalar("x"));
    assert_at(
        &unpack_err(&t),
        ErrorCode::Validation,
        "Optical_Train_01/Power_Characterization/Characterization_Points",
    );
}

#[test]
fn unpack_rejects_a_stored_schema_version() {
    let mut t = ref_tree();
    t.attrs.insert("Schema_Version".into(), Attr::scalar("1.0"));
    assert_at(
        &unpack_err(&t),
        ErrorCode::UnsupportedSchemaVersion,
        "@Schema_Version",
    );
}

#[test]
fn pack_rejects_duplicate_ids_and_bad_names() {
    let mut c = ref_config();
    let dup = c.units[0].clone();
    c.units.push(dup);
    assert_at(&pack_err(&c), ErrorCode::Validation, "Optical_Train_01");

    let mut c = ref_config();
    c.units[0].train_id = "a/b".into();
    assert_at(&pack_err(&c), ErrorCode::Validation, "a/b");
}

#[test]
fn pack_rejects_out_of_range_integers() {
    let mut c = ref_config();
    c.units[0].data_port = Some(1 << 31);
    assert_at(
        &pack_err(&c),
        ErrorCode::Validation,
        "Optical_Train_01/@Data_Port",
    );
}

#[test]
fn pack_rejects_colliding_extra_attributes_and_bad_grids() {
    let mut c = ref_config();
    c.units[0]
        .extra
        .insert("Ip_Address".into(), Attr::scalar("x"));
    assert_at(
        &pack_err(&c),
        ErrorCode::Validation,
        "Optical_Train_01/@Ip_Address",
    );

    let mut c = ref_config();
    c.units[0]
        .correction_data
        .as_mut()
        .unwrap()
        .data
        .truncate(5);
    assert_at(
        &pack_err(&c),
        ErrorCode::Validation,
        "Optical_Train_01/Correction_Data",
    );
}

#[test]
fn pack_rejects_a_constant_name_wider_than_the_column() {
    let mut c = ref_config();
    c.units[0]
        .power_characterization
        .as_mut()
        .unwrap()
        .derivation_equation_constants[0]
        .name = "x".repeat(65);
    assert_at(
        &pack_err(&c),
        ErrorCode::Validation,
        "Optical_Train_01/Power_Characterization/Derivation_Equation_Constants[0]/name",
    );
}

#[test]
fn pack_rejects_duplicate_sensor_labels() {
    let mut c = unpack_ok(&fixture_tree("legacy/synchronous_sensors"));
    let dup = c.units[0].synchronous_sensors[0].clone();
    c.units[0].synchronous_sensors.push(dup);
    assert_at(
        &pack_err(&c),
        ErrorCode::Validation,
        "Optical_Train_01/Synchronous_Sensors/Oxygen Sensor",
    );
}

#[test]
fn pack_rejects_an_unknown_target_schema() {
    let e = pack(
        &ref_config(),
        &PackOptions {
            target_schema: Some("1.0".into()),
        },
    )
    .unwrap_err();
    assert_eq!(e.code, ErrorCode::UnsupportedSchemaVersion);
}
