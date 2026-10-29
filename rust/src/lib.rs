//! Pack/unpack codec for the ClearBox extension of a machine configuration file.
//!
//! Pure functions, no I/O. The extension tree types are declared structurally in [`contract`];
//! machine-config core is never a dependency.
//!
//! - [`unpack`] reads the unversioned layout core writes today into a [`ClearBoxConfig`].
//! - [`pack`] writes that layout back.

mod adapters;
pub mod contract;
mod errors;
mod fields;
mod model;

pub use contract::{
    tree_from_json, tree_to_json, Attr, AttrValue, ColumnData, Dataset, DatasetData, Dtype,
    ExtensionTree, Scalar, Table, TableField, EXTENSION_NAME, REQUIRES_CONTRACT,
};
pub use errors::{ErrorCode, ExtensionError, Result};
pub use model::*;

use adapters::legacy::{pack_legacy, unpack_legacy};

/// The only layout today is the unversioned one core writes; a stored version attribute is rejected.
const SCHEMA_VERSION_ATTR: &str = "Schema_Version";

#[derive(Debug, Clone, Default)]
pub struct PackOptions {
    /// Only `"legacy"` (the default) is supported.
    pub target_schema: Option<String>,
}

/// Schema versions `pack` can write. Empty: the unversioned (`"legacy"`) layout is the only one.
pub fn supported_schema_versions() -> Vec<&'static str> {
    Vec::new()
}

fn scalar_text(s: &Scalar) -> String {
    match s {
        Scalar::Utf8(v) => v.clone(),
        Scalar::I8(v) => v.to_string(),
        Scalar::U8(v) => v.to_string(),
        Scalar::I16(v) => v.to_string(),
        Scalar::U16(v) => v.to_string(),
        Scalar::I32(v) => v.to_string(),
        Scalar::U32(v) => v.to_string(),
        Scalar::I64(v) => v.to_string(),
        Scalar::U64(v) => v.to_string(),
        Scalar::F32(v) => v.to_string(),
        Scalar::F64(v) => v.to_string(),
    }
}

/// The root version attribute as stored; `"legacy"` when it is absent.
pub fn peek_schema_version(tree: &ExtensionTree) -> String {
    match tree.attrs.get(SCHEMA_VERSION_ATTR) {
        None => "legacy".to_string(),
        Some(a) => match &a.value {
            AttrValue::Scalar(s) => scalar_text(s),
            AttrValue::Array(items) => items.iter().map(scalar_text).collect::<Vec<_>>().join(","),
        },
    }
}

pub fn unpack(tree: &ExtensionTree) -> Result<ClearBoxConfig> {
    if tree.attrs.contains_key(SCHEMA_VERSION_ATTR) {
        return Err(ExtensionError::new(
            ErrorCode::UnsupportedSchemaVersion,
            format!("@{SCHEMA_VERSION_ATTR}"),
            format!(
                "unsupported schema version \"{}\"",
                peek_schema_version(tree)
            ),
        ));
    }
    unpack_legacy(tree)
}

/// Writes the unversioned layout; any other `target_schema` is `UnsupportedSchemaVersion`.
pub fn pack(config: &ClearBoxConfig, opts: &PackOptions) -> Result<ExtensionTree> {
    let target = opts.target_schema.as_deref().unwrap_or("legacy");
    if target != "legacy" {
        return Err(ExtensionError::new(
            ErrorCode::UnsupportedSchemaVersion,
            "",
            format!("cannot pack schema version \"{target}\"; supported: legacy"),
        ));
    }
    pack_legacy(config)
}
