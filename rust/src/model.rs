use std::collections::BTreeMap;

use serde::{de::Error as _, Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;

use crate::contract::{attrs_from_json, attrs_to_json, float_from_json, float_to_json, Attr};

/// Attributes the codec does not model, kept verbatim (name -> attribute with its stored dtype).
pub type Extra = BTreeMap<String, Attr>;

mod f64_json {
    use super::*;

    pub fn serialize<S: Serializer>(v: &f64, s: S) -> Result<S::Ok, S::Error> {
        float_to_json(*v).serialize(s)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<f64, D::Error> {
        let v = Value::deserialize(d)?;
        float_from_json(&v, "").map_err(D::Error::custom)
    }
}

mod opt_f64_json {
    use super::*;

    pub fn serialize<S: Serializer>(v: &Option<f64>, s: S) -> Result<S::Ok, S::Error> {
        match v {
            Some(v) => float_to_json(*v).serialize(s),
            None => s.serialize_none(),
        }
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Option<f64>, D::Error> {
        match Option::<Value>::deserialize(d)? {
            None | Some(Value::Null) => Ok(None),
            Some(v) => float_from_json(&v, "").map(Some).map_err(D::Error::custom),
        }
    }
}

mod vec_f64_json {
    use super::*;

    pub fn serialize<S: Serializer>(v: &[f64], s: S) -> Result<S::Ok, S::Error> {
        Value::Array(v.iter().map(|x| float_to_json(*x)).collect()).serialize(s)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<f64>, D::Error> {
        Vec::<Value>::deserialize(d)?
            .iter()
            .map(|v| float_from_json(v, "").map_err(D::Error::custom))
            .collect()
    }
}

mod extra_json {
    use super::*;

    pub fn serialize<S: Serializer>(v: &Extra, s: S) -> Result<S::Ok, S::Error> {
        attrs_to_json(v).serialize(s)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Extra, D::Error> {
        let v = Value::deserialize(d)?;
        attrs_from_json(Some(&v), "").map_err(D::Error::custom)
    }
}

/// Row-major f64 grid; `NaN` marks points outside the correction field.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Grid {
    pub shape: [usize; 3],
    #[serde(with = "vec_f64_json")]
    pub data: Vec<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EquationConstant {
    pub name: String,
    #[serde(with = "f64_json")]
    pub value: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CalibrationPoint {
    #[serde(with = "f64_json")]
    pub input_value: f64,
    #[serde(with = "f64_json")]
    pub output_value: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PowerCharacterization {
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub algorithm_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub algorithm_equation: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub input_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub units_derived_quantity: Option<String>,
    /// Row order is significant (coefficient position).
    pub derivation_equation_constants: Vec<EquationConstant>,
    pub characterization_points: Vec<CalibrationPoint>,
    #[serde(with = "extra_json", default)]
    pub extra: Extra,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SynchronousSensor {
    /// Free-form group name under `Synchronous_Sensors/`.
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub sensor_name: Option<String>,
    #[serde(
        skip_serializing_if = "Option::is_none",
        default,
        with = "opt_f64_json"
    )]
    pub sensor_output_range_low: Option<f64>,
    #[serde(
        skip_serializing_if = "Option::is_none",
        default,
        with = "opt_f64_json"
    )]
    pub sensor_output_range_high: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub sensor_output_space: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub sensor_model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub sensor_manufacturer: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub sensor_scope: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub units_derived_quantity: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub port_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub sensor_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub input_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub algorithm_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub algorithm_equation: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub calibration_source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub calibration_verified: Option<bool>,
    #[serde(
        skip_serializing_if = "Option::is_none",
        default,
        with = "opt_f64_json"
    )]
    pub sample_period: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub metadata: Option<String>,
    pub derivation_equation_constants: Vec<EquationConstant>,
    pub calibration_points: Vec<CalibrationPoint>,
    #[serde(with = "extra_json", default)]
    pub extra: Extra,
}

/// Per-train attributes that only File_Version 1.0 ClearBox groups have. Present iff any was stored.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LegacyUnitFields {
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub selected_camera: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub custom_video_format: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub video_output: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub show_console: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub correction_grid_domain_shape: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub inverse_grid_domain_shape: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClearBoxUnit {
    /// Group name under `ClearBox/`, e.g. `Optical_Train_01`.
    pub train_id: String,
    pub ip_address: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub serial_number: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub data_port: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub server_port: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub actual_timing_offset: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub commanded_timing_offset: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub manufacturer: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub model: Option<String>,
    /// File_Version 1.1 attribute.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub firmware_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub legacy: Option<LegacyUnitFields>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub correction_data: Option<Grid>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub inverse_correction_data: Option<Grid>,
    /// Sorted by label.
    pub synchronous_sensors: Vec<SynchronousSensor>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub power_characterization: Option<PowerCharacterization>,
    #[serde(with = "extra_json", default)]
    pub extra: Extra,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClearBoxConfig {
    /// Machine-wide: ClearBox reads it from the first optical train of a 1.0 file.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub output_path: Option<String>,
    /// Machine-wide, milliseconds.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub software_trigger_delay: Option<i64>,
    /// Sorted by `train_id`.
    pub units: Vec<ClearBoxUnit>,
    #[serde(with = "extra_json", default)]
    pub extra: Extra,
}
