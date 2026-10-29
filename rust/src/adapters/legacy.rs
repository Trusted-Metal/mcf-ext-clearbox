//! The layout core writes today: `ClearBox/` with shared attributes, one `Optical_Train_NN/` group
//! per train, and no schema-version attribute. Core builds the same shape from File_Version 1.0 and
//! 1.1 files.

use std::collections::BTreeMap;

use crate::contract::{ColumnData, Dataset, DatasetData, Dtype, ExtensionTree, Table, TableField};
use crate::errors::{join_path, validation, ExtensionError, Result};
use crate::fields::{
    bool_attr, float_attr, int_attr, node_name, put_extra, read_opt_bool, read_opt_float,
    read_opt_int, read_opt_string, read_required_string, str_attr, take_extra,
};
use crate::model::{
    CalibrationPoint, ClearBoxConfig, ClearBoxUnit, EquationConstant, Grid, LegacyUnitFields,
    PowerCharacterization, SynchronousSensor,
};

const ROOT_ATTRS: &[&str] = &["Output_Path", "Software_Trigger_Delay"];
const LEGACY_ATTRS: &[&str] = &[
    "Selected_Camera",
    "Custom_Video_Format",
    "Video_Output",
    "Show_Console",
    "Correction_Grid_Domain_Shape",
    "Inverse_Grid_Domain_Shape",
];
const UNIT_ATTRS: &[&str] = &[
    "Ip_Address",
    "Serial_Number",
    "Data_Port",
    "Server_Port",
    "Actual_Timing_Offset",
    "Commanded_Timing_Offset",
    "Manufacturer",
    "Model",
    "Firmware_Version",
    "Selected_Camera",
    "Custom_Video_Format",
    "Video_Output",
    "Show_Console",
    "Correction_Grid_Domain_Shape",
    "Inverse_Grid_Domain_Shape",
];
const PC_ATTRS: &[&str] = &[
    "Algorithm_Type",
    "Algorithm_Equation",
    "Input_Type",
    "Units_Derived_Quantity",
];
const SENSOR_ATTRS: &[&str] = &[
    "Enabled",
    "Sensor_Name",
    "Sensor_Output_Range_Low",
    "Sensor_Output_Range_High",
    "Sensor_Output_Space",
    "Sensor_Model",
    "Sensor_Manufacturer",
    "Sensor_Scope",
    "Units_Derived_Quantity",
    "Port_ID",
    "Sensor_Type",
    "Input_Type",
    "Algorithm_Type",
    "Algorithm_Equation",
    "Calibration_Source",
    "Calibration_Verified",
    "Sample_Period",
    "Metadata",
];

const SENSORS_GROUP: &str = "Synchronous_Sensors";
const PC_GROUP: &str = "Power_Characterization";
const CORRECTION: &str = "Correction_Data";
const INVERSE_CORRECTION: &str = "Inverse_Correction_Data";
const CONSTANTS_TABLE: &str = "Derivation_Equation_Constants";
const POINTS_TABLE: &str = "Characterization_Points";
const SENSOR_POINTS_TABLE: &str = "Calibration_Points";
/// Byte width of the `name` column of `Derivation_Equation_Constants`.
const CONSTANT_NAME_BYTES: usize = 64;
const GRID_ATTRS: &[&str] = &["dimensions", "dtype", "shape"];

fn reject_unknown(
    tree: &ExtensionTree,
    path: &str,
    children: &[&str],
    datasets: &[&str],
    tables: &[&str],
) -> Result<()> {
    if let Some(n) = tree
        .children
        .keys()
        .find(|n| !children.contains(&n.as_str()))
    {
        return Err(validation(join_path(path, n), "unexpected child group"));
    }
    if let Some(n) = tree
        .datasets
        .keys()
        .find(|n| !datasets.contains(&n.as_str()))
    {
        return Err(validation(join_path(path, n), "unexpected dataset"));
    }
    if let Some(n) = tree.tables.keys().find(|n| !tables.contains(&n.as_str())) {
        return Err(validation(join_path(path, n), "unexpected table"));
    }
    Ok(())
}

// ---------------------------------------------------------------- tables

fn expect_fields(table: &Table, path: &str, expected: &[(&str, Dtype)]) -> Result<()> {
    let same = table.fields.len() == expected.len()
        && table
            .fields
            .iter()
            .zip(expected)
            .all(|(f, (n, d))| f.name == *n && f.dtype == *d);
    if !same {
        let want: Vec<String> = expected
            .iter()
            .map(|(n, d)| format!("{n}:{}", d.name()))
            .collect();
        return Err(validation(
            path,
            format!("unexpected table fields; expected {}", want.join(", ")),
        ));
    }
    if !table.attrs.is_empty() {
        return Err(validation(path, "table attributes are not supported"));
    }
    Ok(())
}

fn f64_column(table: &Table, index: usize, path: &str) -> Result<Vec<f64>> {
    match &table.columns[index] {
        ColumnData::F64(v) => Ok(v.clone()),
        _ => Err(validation(path, "expected an f64 column")),
    }
}

fn read_constants(table: Option<&Table>, path: &str) -> Result<Vec<EquationConstant>> {
    let Some(table) = table else {
        return Ok(Vec::new());
    };
    expect_fields(table, path, &[("name", Dtype::Utf8), ("value", Dtype::F64)])?;
    let ColumnData::Utf8(names) = &table.columns[0] else {
        return Err(validation(path, "expected a utf8 column"));
    };
    let values = f64_column(table, 1, path)?;
    Ok(names
        .iter()
        .zip(values)
        .map(|(name, value)| EquationConstant {
            name: name.clone(),
            value,
        })
        .collect())
}

fn read_points(table: Option<&Table>, path: &str) -> Result<Vec<CalibrationPoint>> {
    let Some(table) = table else {
        return Ok(Vec::new());
    };
    expect_fields(
        table,
        path,
        &[("input_value", Dtype::F64), ("output_value", Dtype::F64)],
    )?;
    let input = f64_column(table, 0, path)?;
    let output = f64_column(table, 1, path)?;
    Ok(input
        .into_iter()
        .zip(output)
        .map(|(input_value, output_value)| CalibrationPoint {
            input_value,
            output_value,
        })
        .collect())
}

fn constants_table(rows: &[EquationConstant], path: &str) -> Result<Table> {
    for (i, c) in rows.iter().enumerate() {
        let p = format!("{path}[{i}]/name");
        if c.name.contains('\0') {
            return Err(validation(p, "constant name contains NUL"));
        }
        if c.name.len() > CONSTANT_NAME_BYTES {
            return Err(validation(
                p,
                format!(
                    "constant name \"{}\" exceeds {CONSTANT_NAME_BYTES} UTF-8 bytes",
                    c.name
                ),
            ));
        }
    }
    Ok(Table {
        fields: vec![
            TableField {
                name: "name".into(),
                dtype: Dtype::Utf8,
                size: Some(CONSTANT_NAME_BYTES),
            },
            TableField {
                name: "value".into(),
                dtype: Dtype::F64,
                size: None,
            },
        ],
        rows: rows.len(),
        columns: vec![
            ColumnData::Utf8(rows.iter().map(|c| c.name.clone()).collect()),
            ColumnData::F64(rows.iter().map(|c| c.value).collect()),
        ],
        attrs: BTreeMap::new(),
    })
}

fn points_table(rows: &[CalibrationPoint]) -> Table {
    Table {
        fields: vec![
            TableField {
                name: "input_value".into(),
                dtype: Dtype::F64,
                size: None,
            },
            TableField {
                name: "output_value".into(),
                dtype: Dtype::F64,
                size: None,
            },
        ],
        rows: rows.len(),
        columns: vec![
            ColumnData::F64(rows.iter().map(|p| p.input_value).collect()),
            ColumnData::F64(rows.iter().map(|p| p.output_value).collect()),
        ],
        attrs: BTreeMap::new(),
    }
}

// ---------------------------------------------------------------- grids

fn read_grid(ds: Option<&Dataset>, path: &str) -> Result<Option<Grid>> {
    let Some(ds) = ds else { return Ok(None) };
    let DatasetData::F64(data) = &ds.data else {
        return Err(validation(
            path,
            format!("expected an f64 dataset, got {}", ds.dtype().name()),
        ));
    };
    let [a, b, c] = ds.shape[..] else {
        return Err(validation(
            path,
            format!("expected a 3-D dataset, got {}-D", ds.shape.len()),
        ));
    };
    if let Some(name) = ds.attrs.keys().find(|n| !GRID_ATTRS.contains(&n.as_str())) {
        return Err(validation(
            join_path(path, &format!("@{name}")),
            "unexpected dataset attribute",
        ));
    }
    Ok(Some(Grid {
        shape: [a, b, c],
        data: data.clone(),
    }))
}

fn grid_dataset(grid: &Grid, path: &str) -> Result<Dataset> {
    let [a, b, c] = grid.shape;
    if [a, b, c].contains(&0) {
        return Err(validation(path, format!("invalid grid shape {a}x{b}x{c}")));
    }
    if grid.data.len() != a * b * c {
        return Err(validation(
            path,
            format!("grid has {} values for shape {a}x{b}x{c}", grid.data.len()),
        ));
    }
    let mut attrs = BTreeMap::new();
    attrs.insert("dimensions".to_string(), str_attr(Some("H,W,D")));
    attrs.insert("dtype".to_string(), str_attr(Some("float64")));
    attrs.insert("shape".to_string(), str_attr(Some(&format!("{a}x{b}x{c}"))));
    Ok(Dataset {
        shape: vec![a, b, c],
        data: DatasetData::F64(grid.data.clone()),
        attrs,
    })
}

// ---------------------------------------------------------------- power characterization

fn unpack_power_characterization(
    tree: &ExtensionTree,
    path: &str,
) -> Result<PowerCharacterization> {
    reject_unknown(tree, path, &[], &[], &[CONSTANTS_TABLE, POINTS_TABLE])?;
    let a = &tree.attrs;
    Ok(PowerCharacterization {
        algorithm_type: read_opt_string(a, "Algorithm_Type", path)?,
        algorithm_equation: read_opt_string(a, "Algorithm_Equation", path)?,
        input_type: read_opt_string(a, "Input_Type", path)?,
        units_derived_quantity: read_opt_string(a, "Units_Derived_Quantity", path)?,
        derivation_equation_constants: read_constants(
            tree.tables.get(CONSTANTS_TABLE),
            &join_path(path, CONSTANTS_TABLE),
        )?,
        characterization_points: read_points(
            tree.tables.get(POINTS_TABLE),
            &join_path(path, POINTS_TABLE),
        )?,
        extra: take_extra(a, PC_ATTRS),
    })
}

fn pack_power_characterization(pc: &PowerCharacterization, path: &str) -> Result<ExtensionTree> {
    let mut tree = ExtensionTree::default();
    tree.attrs.insert(
        "Algorithm_Type".into(),
        str_attr(pc.algorithm_type.as_deref()),
    );
    tree.attrs.insert(
        "Algorithm_Equation".into(),
        str_attr(pc.algorithm_equation.as_deref()),
    );
    tree.attrs
        .insert("Input_Type".into(), str_attr(pc.input_type.as_deref()));
    tree.attrs.insert(
        "Units_Derived_Quantity".into(),
        str_attr(pc.units_derived_quantity.as_deref()),
    );
    put_extra(&mut tree.attrs, &pc.extra, PC_ATTRS, path)?;
    tree.tables.insert(
        CONSTANTS_TABLE.into(),
        constants_table(
            &pc.derivation_equation_constants,
            &join_path(path, CONSTANTS_TABLE),
        )?,
    );
    tree.tables.insert(
        POINTS_TABLE.into(),
        points_table(&pc.characterization_points),
    );
    Ok(tree)
}

// ---------------------------------------------------------------- synchronous sensors

fn unpack_sensor(label: &str, tree: &ExtensionTree, path: &str) -> Result<SynchronousSensor> {
    reject_unknown(
        tree,
        path,
        &[],
        &[],
        &[CONSTANTS_TABLE, SENSOR_POINTS_TABLE],
    )?;
    let a = &tree.attrs;
    Ok(SynchronousSensor {
        label: label.to_string(),
        enabled: read_opt_bool(a, "Enabled", path)?,
        sensor_name: read_opt_string(a, "Sensor_Name", path)?,
        sensor_output_range_low: read_opt_float(a, "Sensor_Output_Range_Low", path)?,
        sensor_output_range_high: read_opt_float(a, "Sensor_Output_Range_High", path)?,
        sensor_output_space: read_opt_string(a, "Sensor_Output_Space", path)?,
        sensor_model: read_opt_string(a, "Sensor_Model", path)?,
        sensor_manufacturer: read_opt_string(a, "Sensor_Manufacturer", path)?,
        sensor_scope: read_opt_string(a, "Sensor_Scope", path)?,
        units_derived_quantity: read_opt_string(a, "Units_Derived_Quantity", path)?,
        port_id: read_opt_int(a, "Port_ID", path)?,
        sensor_type: read_opt_string(a, "Sensor_Type", path)?,
        input_type: read_opt_string(a, "Input_Type", path)?,
        algorithm_type: read_opt_string(a, "Algorithm_Type", path)?,
        algorithm_equation: read_opt_string(a, "Algorithm_Equation", path)?,
        calibration_source: read_opt_string(a, "Calibration_Source", path)?,
        calibration_verified: read_opt_bool(a, "Calibration_Verified", path)?,
        sample_period: read_opt_float(a, "Sample_Period", path)?,
        metadata: read_opt_string(a, "Metadata", path)?,
        derivation_equation_constants: read_constants(
            tree.tables.get(CONSTANTS_TABLE),
            &join_path(path, CONSTANTS_TABLE),
        )?,
        calibration_points: read_points(
            tree.tables.get(SENSOR_POINTS_TABLE),
            &join_path(path, SENSOR_POINTS_TABLE),
        )?,
        extra: take_extra(a, SENSOR_ATTRS),
    })
}

fn pack_sensor(s: &SynchronousSensor, path: &str) -> Result<ExtensionTree> {
    let mut tree = ExtensionTree::default();
    let a = &mut tree.attrs;
    a.insert("Enabled".into(), bool_attr(s.enabled));
    a.insert("Sensor_Name".into(), str_attr(s.sensor_name.as_deref()));
    a.insert(
        "Sensor_Output_Range_Low".into(),
        float_attr(s.sensor_output_range_low),
    );
    a.insert(
        "Sensor_Output_Range_High".into(),
        float_attr(s.sensor_output_range_high),
    );
    a.insert(
        "Sensor_Output_Space".into(),
        str_attr(s.sensor_output_space.as_deref()),
    );
    a.insert("Sensor_Model".into(), str_attr(s.sensor_model.as_deref()));
    a.insert(
        "Sensor_Manufacturer".into(),
        str_attr(s.sensor_manufacturer.as_deref()),
    );
    a.insert("Sensor_Scope".into(), str_attr(s.sensor_scope.as_deref()));
    a.insert(
        "Units_Derived_Quantity".into(),
        str_attr(s.units_derived_quantity.as_deref()),
    );
    a.insert(
        "Port_ID".into(),
        int_attr(s.port_id, &join_path(path, "@Port_ID"))?,
    );
    a.insert("Sensor_Type".into(), str_attr(s.sensor_type.as_deref()));
    a.insert("Input_Type".into(), str_attr(s.input_type.as_deref()));
    a.insert(
        "Algorithm_Type".into(),
        str_attr(s.algorithm_type.as_deref()),
    );
    a.insert(
        "Algorithm_Equation".into(),
        str_attr(s.algorithm_equation.as_deref()),
    );
    a.insert(
        "Calibration_Source".into(),
        str_attr(s.calibration_source.as_deref()),
    );
    a.insert(
        "Calibration_Verified".into(),
        bool_attr(s.calibration_verified),
    );
    a.insert("Sample_Period".into(), float_attr(s.sample_period));
    a.insert("Metadata".into(), str_attr(s.metadata.as_deref()));
    put_extra(a, &s.extra, SENSOR_ATTRS, path)?;
    tree.tables.insert(
        CONSTANTS_TABLE.into(),
        constants_table(
            &s.derivation_equation_constants,
            &join_path(path, CONSTANTS_TABLE),
        )?,
    );
    tree.tables.insert(
        SENSOR_POINTS_TABLE.into(),
        points_table(&s.calibration_points),
    );
    Ok(tree)
}

// ---------------------------------------------------------------- units

fn unpack_legacy_fields(tree: &ExtensionTree, path: &str) -> Result<Option<LegacyUnitFields>> {
    let a = &tree.attrs;
    if !LEGACY_ATTRS.iter().any(|n| a.contains_key(*n)) {
        return Ok(None);
    }
    Ok(Some(LegacyUnitFields {
        selected_camera: read_opt_string(a, "Selected_Camera", path)?,
        custom_video_format: read_opt_string(a, "Custom_Video_Format", path)?,
        video_output: read_opt_string(a, "Video_Output", path)?,
        show_console: read_opt_bool(a, "Show_Console", path)?,
        correction_grid_domain_shape: read_opt_string(a, "Correction_Grid_Domain_Shape", path)?,
        inverse_grid_domain_shape: read_opt_string(a, "Inverse_Grid_Domain_Shape", path)?,
    }))
}

fn unpack_unit(train_id: &str, tree: &ExtensionTree) -> Result<ClearBoxUnit> {
    let path = train_id;
    reject_unknown(
        tree,
        path,
        &[SENSORS_GROUP, PC_GROUP],
        &[CORRECTION, INVERSE_CORRECTION],
        &[],
    )?;
    let a = &tree.attrs;

    let mut sensors = Vec::new();
    if let Some(sensors_tree) = tree.children.get(SENSORS_GROUP) {
        let sp = join_path(path, SENSORS_GROUP);
        if !sensors_tree.attrs.is_empty() {
            return Err(validation(
                sp,
                "attributes on the sensors group are not supported",
            ));
        }
        if let Some(n) = sensors_tree.datasets.keys().next() {
            return Err(validation(join_path(&sp, n), "unexpected dataset"));
        }
        if let Some(n) = sensors_tree.tables.keys().next() {
            return Err(validation(join_path(&sp, n), "unexpected table"));
        }
        for (label, child) in &sensors_tree.children {
            sensors.push(unpack_sensor(label, child, &join_path(&sp, label))?);
        }
    }

    Ok(ClearBoxUnit {
        train_id: train_id.to_string(),
        ip_address: read_required_string(a, "Ip_Address", path)?,
        serial_number: read_opt_string(a, "Serial_Number", path)?,
        data_port: read_opt_int(a, "Data_Port", path)?,
        server_port: read_opt_int(a, "Server_Port", path)?,
        actual_timing_offset: read_opt_int(a, "Actual_Timing_Offset", path)?,
        commanded_timing_offset: read_opt_int(a, "Commanded_Timing_Offset", path)?,
        manufacturer: read_opt_string(a, "Manufacturer", path)?,
        model: read_opt_string(a, "Model", path)?,
        firmware_version: read_opt_string(a, "Firmware_Version", path)?,
        legacy: unpack_legacy_fields(tree, path)?,
        correction_data: read_grid(tree.datasets.get(CORRECTION), &join_path(path, CORRECTION))?,
        inverse_correction_data: read_grid(
            tree.datasets.get(INVERSE_CORRECTION),
            &join_path(path, INVERSE_CORRECTION),
        )?,
        synchronous_sensors: sensors,
        power_characterization: match tree.children.get(PC_GROUP) {
            Some(pc) => Some(unpack_power_characterization(
                pc,
                &join_path(path, PC_GROUP),
            )?),
            None => None,
        },
        extra: take_extra(a, UNIT_ATTRS),
    })
}

fn pack_unit(u: &ClearBoxUnit) -> Result<ExtensionTree> {
    let path = u.train_id.as_str();
    node_name(&u.train_id, path)?;
    let mut tree = ExtensionTree::default();
    let a = &mut tree.attrs;
    a.insert("Ip_Address".into(), str_attr(Some(&u.ip_address)));
    a.insert("Serial_Number".into(), str_attr(u.serial_number.as_deref()));
    a.insert(
        "Data_Port".into(),
        int_attr(u.data_port, &join_path(path, "@Data_Port"))?,
    );
    a.insert(
        "Server_Port".into(),
        int_attr(u.server_port, &join_path(path, "@Server_Port"))?,
    );
    a.insert(
        "Actual_Timing_Offset".into(),
        int_attr(
            u.actual_timing_offset,
            &join_path(path, "@Actual_Timing_Offset"),
        )?,
    );
    a.insert(
        "Commanded_Timing_Offset".into(),
        int_attr(
            u.commanded_timing_offset,
            &join_path(path, "@Commanded_Timing_Offset"),
        )?,
    );
    a.insert("Manufacturer".into(), str_attr(u.manufacturer.as_deref()));
    a.insert("Model".into(), str_attr(u.model.as_deref()));
    // File_Version 1.1 always has the attribute; the 1.0 shape (a `legacy` group) never does.
    if u.firmware_version.is_some() || u.legacy.is_none() {
        a.insert(
            "Firmware_Version".into(),
            str_attr(u.firmware_version.as_deref()),
        );
    }
    if let Some(l) = &u.legacy {
        a.insert(
            "Selected_Camera".into(),
            str_attr(l.selected_camera.as_deref()),
        );
        a.insert(
            "Custom_Video_Format".into(),
            str_attr(l.custom_video_format.as_deref()),
        );
        a.insert("Video_Output".into(), str_attr(l.video_output.as_deref()));
        a.insert("Show_Console".into(), bool_attr(l.show_console));
        a.insert(
            "Correction_Grid_Domain_Shape".into(),
            str_attr(l.correction_grid_domain_shape.as_deref()),
        );
        a.insert(
            "Inverse_Grid_Domain_Shape".into(),
            str_attr(l.inverse_grid_domain_shape.as_deref()),
        );
    }
    put_extra(a, &u.extra, UNIT_ATTRS, path)?;

    if let Some(g) = &u.correction_data {
        tree.datasets.insert(
            CORRECTION.into(),
            grid_dataset(g, &join_path(path, CORRECTION))?,
        );
    }
    if let Some(g) = &u.inverse_correction_data {
        tree.datasets.insert(
            INVERSE_CORRECTION.into(),
            grid_dataset(g, &join_path(path, INVERSE_CORRECTION))?,
        );
    }

    if !u.synchronous_sensors.is_empty() {
        let sp = join_path(path, SENSORS_GROUP);
        let mut group = ExtensionTree::default();
        for s in &u.synchronous_sensors {
            let sensor_path = join_path(&sp, &s.label);
            node_name(&s.label, &sensor_path)?;
            if group.children.contains_key(&s.label) {
                return Err(validation(sensor_path, "duplicate sensor label"));
            }
            group
                .children
                .insert(s.label.clone(), pack_sensor(s, &sensor_path)?);
        }
        tree.children.insert(SENSORS_GROUP.into(), group);
    }
    if let Some(pc) = &u.power_characterization {
        tree.children.insert(
            PC_GROUP.into(),
            pack_power_characterization(pc, &join_path(path, PC_GROUP))?,
        );
    }
    Ok(tree)
}

// ---------------------------------------------------------------- root

pub(crate) fn unpack_legacy(tree: &ExtensionTree) -> Result<ClearBoxConfig> {
    if let Some(n) = tree.datasets.keys().next() {
        return Err(validation(n, "unexpected dataset"));
    }
    if let Some(n) = tree.tables.keys().next() {
        return Err(validation(n, "unexpected table"));
    }
    let units = tree
        .children
        .iter()
        .map(|(id, child)| unpack_unit(id, child))
        .collect::<Result<Vec<_>>>()?;
    Ok(ClearBoxConfig {
        output_path: read_opt_string(&tree.attrs, "Output_Path", "")?,
        software_trigger_delay: read_opt_int(&tree.attrs, "Software_Trigger_Delay", "")?,
        units,
        extra: take_extra(&tree.attrs, ROOT_ATTRS),
    })
}

pub(crate) fn pack_legacy(
    config: &ClearBoxConfig,
) -> std::result::Result<ExtensionTree, ExtensionError> {
    let mut tree = ExtensionTree::default();
    tree.attrs.insert(
        "Output_Path".into(),
        str_attr(config.output_path.as_deref()),
    );
    tree.attrs.insert(
        "Software_Trigger_Delay".into(),
        int_attr(config.software_trigger_delay, "@Software_Trigger_Delay")?,
    );
    put_extra(&mut tree.attrs, &config.extra, ROOT_ATTRS, "")?;
    for unit in &config.units {
        if tree.children.contains_key(&unit.train_id) {
            return Err(validation(&unit.train_id, "duplicate train id"));
        }
        tree.children
            .insert(unit.train_id.clone(), pack_unit(unit)?);
    }
    Ok(tree)
}
