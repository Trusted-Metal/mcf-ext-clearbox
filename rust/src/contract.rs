//! Structural copy of core's extension contract 0.1.1 (types only; core is never a dependency).
//!
//! The serde form is the contract's tree JSON, the same as `fixtures/**/tree.json`: `i64`/`u64` are
//! decimal strings and non-finite floats are `"NaN"`, `"Infinity"`, `"-Infinity"`.

use std::collections::BTreeMap;

use serde::{de::Error as _, Deserialize, Deserializer, Serialize, Serializer};
use serde_json::{json, Map, Number, Value};

use crate::errors::{join_path, validation, ExtensionError};

pub const EXTENSION_NAME: &str = "ClearBox";
/// Contract range this library is written against; tables need 0.1.1.
pub const REQUIRES_CONTRACT: &str = "^0.1.1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Dtype {
    Utf8,
    I8,
    U8,
    I16,
    U16,
    I32,
    U32,
    I64,
    U64,
    F32,
    F64,
}

impl Dtype {
    pub const ALL: [Dtype; 11] = [
        Dtype::Utf8,
        Dtype::I8,
        Dtype::U8,
        Dtype::I16,
        Dtype::U16,
        Dtype::I32,
        Dtype::U32,
        Dtype::I64,
        Dtype::U64,
        Dtype::F32,
        Dtype::F64,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Dtype::Utf8 => "utf8",
            Dtype::I8 => "i8",
            Dtype::U8 => "u8",
            Dtype::I16 => "i16",
            Dtype::U16 => "u16",
            Dtype::I32 => "i32",
            Dtype::U32 => "u32",
            Dtype::I64 => "i64",
            Dtype::U64 => "u64",
            Dtype::F32 => "f32",
            Dtype::F64 => "f64",
        }
    }

    pub fn from_name(name: &str) -> Option<Dtype> {
        Dtype::ALL.into_iter().find(|d| d.name() == name)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Scalar {
    Utf8(String),
    I8(i8),
    U8(u8),
    I16(i16),
    U16(u16),
    I32(i32),
    U32(u32),
    I64(i64),
    U64(u64),
    F32(f32),
    F64(f64),
}

impl Scalar {
    pub fn dtype(&self) -> Dtype {
        match self {
            Scalar::Utf8(_) => Dtype::Utf8,
            Scalar::I8(_) => Dtype::I8,
            Scalar::U8(_) => Dtype::U8,
            Scalar::I16(_) => Dtype::I16,
            Scalar::U16(_) => Dtype::U16,
            Scalar::I32(_) => Dtype::I32,
            Scalar::U32(_) => Dtype::U32,
            Scalar::I64(_) => Dtype::I64,
            Scalar::U64(_) => Dtype::U64,
            Scalar::F32(_) => Dtype::F32,
            Scalar::F64(_) => Dtype::F64,
        }
    }
}

/// A scalar or a 1-D array.
#[derive(Debug, Clone, PartialEq)]
pub enum AttrValue {
    Scalar(Scalar),
    /// Every element's dtype must equal the owning [`Attr::dtype`].
    Array(Vec<Scalar>),
}

/// A group or dataset attribute: a scalar or a 1-D array of one dtype.
#[derive(Debug, Clone, PartialEq)]
pub struct Attr {
    pub dtype: Dtype,
    pub value: AttrValue,
}

impl Attr {
    pub fn scalar(value: impl Into<Scalar>) -> Self {
        let value = value.into();
        Attr {
            dtype: value.dtype(),
            value: AttrValue::Scalar(value),
        }
    }
}

macro_rules! scalar_from {
    ($($ty:ty => $variant:ident),* $(,)?) => {
        $(impl From<$ty> for Scalar {
            fn from(v: $ty) -> Self { Scalar::$variant(v) }
        })*
    };
}

scalar_from!(
    String => Utf8, i8 => I8, u8 => U8, i16 => I16, u16 => U16, i32 => I32, u32 => U32,
    i64 => I64, u64 => U64, f32 => F32, f64 => F64,
);

impl From<&str> for Scalar {
    fn from(v: &str) -> Self {
        Scalar::Utf8(v.to_string())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum DatasetData {
    I8(Vec<i8>),
    U8(Vec<u8>),
    I16(Vec<i16>),
    U16(Vec<u16>),
    I32(Vec<i32>),
    U32(Vec<u32>),
    I64(Vec<i64>),
    U64(Vec<u64>),
    F32(Vec<f32>),
    F64(Vec<f64>),
}

impl DatasetData {
    pub fn dtype(&self) -> Dtype {
        match self {
            DatasetData::I8(_) => Dtype::I8,
            DatasetData::U8(_) => Dtype::U8,
            DatasetData::I16(_) => Dtype::I16,
            DatasetData::U16(_) => Dtype::U16,
            DatasetData::I32(_) => Dtype::I32,
            DatasetData::U32(_) => Dtype::U32,
            DatasetData::I64(_) => Dtype::I64,
            DatasetData::U64(_) => Dtype::U64,
            DatasetData::F32(_) => Dtype::F32,
            DatasetData::F64(_) => Dtype::F64,
        }
    }
}

/// N-d numeric array stored row-major in `data`; `data.len()` equals the product of `shape`.
#[derive(Debug, Clone, PartialEq)]
pub struct Dataset {
    pub shape: Vec<usize>,
    pub data: DatasetData,
    pub attrs: BTreeMap<String, Attr>,
}

impl Dataset {
    pub fn dtype(&self) -> Dtype {
        self.data.dtype()
    }
}

/// One column of a [`Table`]. `size` is the fixed byte length of a `utf8` column.
#[derive(Debug, Clone, PartialEq)]
pub struct TableField {
    pub name: String,
    pub dtype: Dtype,
    pub size: Option<usize>,
}

/// The values of one table column; the variant must match the field's dtype.
#[derive(Debug, Clone, PartialEq)]
pub enum ColumnData {
    Utf8(Vec<String>),
    I8(Vec<i8>),
    U8(Vec<u8>),
    I16(Vec<i16>),
    U16(Vec<u16>),
    I32(Vec<i32>),
    U32(Vec<u32>),
    I64(Vec<i64>),
    U64(Vec<u64>),
    F32(Vec<f32>),
    F64(Vec<f64>),
}

impl ColumnData {
    pub fn dtype(&self) -> Dtype {
        match self {
            ColumnData::Utf8(_) => Dtype::Utf8,
            ColumnData::I8(_) => Dtype::I8,
            ColumnData::U8(_) => Dtype::U8,
            ColumnData::I16(_) => Dtype::I16,
            ColumnData::U16(_) => Dtype::U16,
            ColumnData::I32(_) => Dtype::I32,
            ColumnData::U32(_) => Dtype::U32,
            ColumnData::I64(_) => Dtype::I64,
            ColumnData::U64(_) => Dtype::U64,
            ColumnData::F32(_) => Dtype::F32,
            ColumnData::F64(_) => Dtype::F64,
        }
    }
}

/// 1-D compound dataset: `fields` order is significant; `columns[i]` holds the values of `fields[i]`.
#[derive(Debug, Clone, PartialEq)]
pub struct Table {
    pub fields: Vec<TableField>,
    pub rows: usize,
    pub columns: Vec<ColumnData>,
    pub attrs: BTreeMap<String, Attr>,
}

/// Order of attrs/children/datasets/tables is not significant; the maps sort by UTF-8 bytes.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ExtensionTree {
    pub attrs: BTreeMap<String, Attr>,
    pub children: BTreeMap<String, ExtensionTree>,
    pub datasets: BTreeMap<String, Dataset>,
    pub tables: BTreeMap<String, Table>,
}

// ------------------------------------------------------------------ JSON form

pub(crate) fn float_to_json(v: f64) -> Value {
    if v.is_nan() {
        Value::String("NaN".into())
    } else if v == f64::INFINITY {
        Value::String("Infinity".into())
    } else if v == f64::NEG_INFINITY {
        Value::String("-Infinity".into())
    } else {
        Value::Number(Number::from_f64(v).expect("finite"))
    }
}

pub(crate) fn float_from_json(v: &Value, path: &str) -> Result<f64, ExtensionError> {
    match v {
        Value::Number(n) => n
            .as_f64()
            .ok_or_else(|| validation(path, "invalid float JSON value")),
        Value::String(s) => match s.as_str() {
            "NaN" => Ok(f64::NAN),
            "Infinity" => Ok(f64::INFINITY),
            "-Infinity" => Ok(f64::NEG_INFINITY),
            other => Err(validation(
                path,
                format!("invalid float JSON value \"{other}\""),
            )),
        },
        _ => Err(validation(path, "float JSON value must be a number")),
    }
}

fn scalar_to_json(v: &Scalar) -> Value {
    match v {
        Scalar::Utf8(s) => Value::String(s.clone()),
        Scalar::I8(x) => json!(x),
        Scalar::U8(x) => json!(x),
        Scalar::I16(x) => json!(x),
        Scalar::U16(x) => json!(x),
        Scalar::I32(x) => json!(x),
        Scalar::U32(x) => json!(x),
        Scalar::I64(x) => Value::String(x.to_string()),
        Scalar::U64(x) => Value::String(x.to_string()),
        Scalar::F32(x) => float_to_json(f64::from(*x)),
        Scalar::F64(x) => float_to_json(*x),
    }
}

pub(crate) fn attrs_to_json(attrs: &BTreeMap<String, Attr>) -> Value {
    let mut out = Map::new();
    for (name, attr) in attrs {
        let value = match &attr.value {
            AttrValue::Scalar(v) => scalar_to_json(v),
            AttrValue::Array(values) => Value::Array(values.iter().map(scalar_to_json).collect()),
        };
        out.insert(
            name.clone(),
            json!({ "dtype": attr.dtype.name(), "value": value }),
        );
    }
    Value::Object(out)
}

macro_rules! each_numeric {
    ($v:expr, $conv:expr) => {
        match $v {
            Numeric::I8(x) => x.iter().map(|x| $conv(Scalar::I8(*x))).collect(),
            Numeric::U8(x) => x.iter().map(|x| $conv(Scalar::U8(*x))).collect(),
            Numeric::I16(x) => x.iter().map(|x| $conv(Scalar::I16(*x))).collect(),
            Numeric::U16(x) => x.iter().map(|x| $conv(Scalar::U16(*x))).collect(),
            Numeric::I32(x) => x.iter().map(|x| $conv(Scalar::I32(*x))).collect(),
            Numeric::U32(x) => x.iter().map(|x| $conv(Scalar::U32(*x))).collect(),
            Numeric::I64(x) => x.iter().map(|x| $conv(Scalar::I64(*x))).collect(),
            Numeric::U64(x) => x.iter().map(|x| $conv(Scalar::U64(*x))).collect(),
            Numeric::F32(x) => x.iter().map(|x| $conv(Scalar::F32(*x))).collect(),
            Numeric::F64(x) => x.iter().map(|x| $conv(Scalar::F64(*x))).collect(),
        }
    };
}

/// Borrowed view shared by dataset data and numeric table columns.
enum Numeric<'a> {
    I8(&'a [i8]),
    U8(&'a [u8]),
    I16(&'a [i16]),
    U16(&'a [u16]),
    I32(&'a [i32]),
    U32(&'a [u32]),
    I64(&'a [i64]),
    U64(&'a [u64]),
    F32(&'a [f32]),
    F64(&'a [f64]),
}

fn numeric_of_data(d: &DatasetData) -> Numeric<'_> {
    match d {
        DatasetData::I8(v) => Numeric::I8(v),
        DatasetData::U8(v) => Numeric::U8(v),
        DatasetData::I16(v) => Numeric::I16(v),
        DatasetData::U16(v) => Numeric::U16(v),
        DatasetData::I32(v) => Numeric::I32(v),
        DatasetData::U32(v) => Numeric::U32(v),
        DatasetData::I64(v) => Numeric::I64(v),
        DatasetData::U64(v) => Numeric::U64(v),
        DatasetData::F32(v) => Numeric::F32(v),
        DatasetData::F64(v) => Numeric::F64(v),
    }
}

fn column_to_json(c: &ColumnData) -> Value {
    let items: Vec<Value> = match c {
        ColumnData::Utf8(v) => v.iter().map(|s| Value::String(s.clone())).collect(),
        ColumnData::I8(v) => each_numeric!(Numeric::I8(v), |s| scalar_to_json(&s)),
        ColumnData::U8(v) => each_numeric!(Numeric::U8(v), |s| scalar_to_json(&s)),
        ColumnData::I16(v) => each_numeric!(Numeric::I16(v), |s| scalar_to_json(&s)),
        ColumnData::U16(v) => each_numeric!(Numeric::U16(v), |s| scalar_to_json(&s)),
        ColumnData::I32(v) => each_numeric!(Numeric::I32(v), |s| scalar_to_json(&s)),
        ColumnData::U32(v) => each_numeric!(Numeric::U32(v), |s| scalar_to_json(&s)),
        ColumnData::I64(v) => each_numeric!(Numeric::I64(v), |s| scalar_to_json(&s)),
        ColumnData::U64(v) => each_numeric!(Numeric::U64(v), |s| scalar_to_json(&s)),
        ColumnData::F32(v) => each_numeric!(Numeric::F32(v), |s| scalar_to_json(&s)),
        ColumnData::F64(v) => each_numeric!(Numeric::F64(v), |s| scalar_to_json(&s)),
    };
    Value::Array(items)
}

fn table_to_json(table: &Table) -> Value {
    let fields: Vec<Value> = table
        .fields
        .iter()
        .map(|f| match f.size {
            Some(size) => json!({ "name": f.name, "dtype": f.dtype.name(), "size": size }),
            None => json!({ "name": f.name, "dtype": f.dtype.name() }),
        })
        .collect();
    let columns: Map<String, Value> = table
        .fields
        .iter()
        .zip(&table.columns)
        .map(|(f, c)| (f.name.clone(), column_to_json(c)))
        .collect();
    json!({ "fields": fields, "rows": table.rows, "columns": columns, "attrs": attrs_to_json(&table.attrs) })
}

pub fn tree_to_json(tree: &ExtensionTree) -> Value {
    let children: Map<String, Value> = tree
        .children
        .iter()
        .map(|(n, c)| (n.clone(), tree_to_json(c)))
        .collect();
    let datasets: Map<String, Value> = tree
        .datasets
        .iter()
        .map(|(n, ds)| {
            let data: Vec<Value> = each_numeric!(numeric_of_data(&ds.data), |s| scalar_to_json(&s));
            (
                n.clone(),
                json!({ "dtype": ds.dtype().name(), "shape": ds.shape, "data": data, "attrs": attrs_to_json(&ds.attrs) }),
            )
        })
        .collect();
    let mut out =
        json!({ "attrs": attrs_to_json(&tree.attrs), "children": children, "datasets": datasets });
    if !tree.tables.is_empty() {
        let tables: Map<String, Value> = tree
            .tables
            .iter()
            .map(|(n, t)| (n.clone(), table_to_json(t)))
            .collect();
        out["tables"] = Value::Object(tables);
    }
    out
}

fn scalar_from_json(dtype: Dtype, v: &Value, path: &str) -> Result<Scalar, ExtensionError> {
    let int_range = |lo: i64, hi: i64| -> Result<i64, ExtensionError> {
        let n = v.as_i64().ok_or_else(|| {
            validation(
                path,
                format!("{} JSON value must be an integer", dtype.name()),
            )
        })?;
        if n < lo || n > hi {
            return Err(validation(
                path,
                format!("{} value {n} is out of range", dtype.name()),
            ));
        }
        Ok(n)
    };
    Ok(match dtype {
        Dtype::Utf8 => Scalar::Utf8(
            v.as_str()
                .ok_or_else(|| validation(path, "utf8 JSON value must be a string"))?
                .to_string(),
        ),
        Dtype::I8 => Scalar::I8(int_range(i8::MIN.into(), i8::MAX.into())? as i8),
        Dtype::U8 => Scalar::U8(int_range(0, u8::MAX.into())? as u8),
        Dtype::I16 => Scalar::I16(int_range(i16::MIN.into(), i16::MAX.into())? as i16),
        Dtype::U16 => Scalar::U16(int_range(0, u16::MAX.into())? as u16),
        Dtype::I32 => Scalar::I32(int_range(i32::MIN.into(), i32::MAX.into())? as i32),
        Dtype::U32 => Scalar::U32(int_range(0, u32::MAX.into())? as u32),
        Dtype::I64 => Scalar::I64(
            v.as_str()
                .and_then(|s| s.parse::<i64>().ok())
                .ok_or_else(|| validation(path, "i64 JSON value must be a decimal string"))?,
        ),
        Dtype::U64 => Scalar::U64(
            v.as_str()
                .and_then(|s| s.parse::<u64>().ok())
                .ok_or_else(|| validation(path, "u64 JSON value must be a decimal string"))?,
        ),
        Dtype::F32 => Scalar::F32(float_from_json(v, path)? as f32),
        Dtype::F64 => Scalar::F64(float_from_json(v, path)?),
    })
}

fn dtype_from_json(v: Option<&Value>, path: &str) -> Result<Dtype, ExtensionError> {
    let name = v
        .and_then(Value::as_str)
        .ok_or_else(|| validation(path, "missing dtype"))?;
    Dtype::from_name(name).ok_or_else(|| validation(path, format!("unknown dtype \"{name}\"")))
}

pub(crate) fn attrs_from_json(
    v: Option<&Value>,
    path: &str,
) -> Result<BTreeMap<String, Attr>, ExtensionError> {
    let mut out = BTreeMap::new();
    let Some(Value::Object(map)) = v else {
        return Ok(out);
    };
    for (name, a) in map {
        let attr_path = join_path(path, &format!("@{name}"));
        let dtype = dtype_from_json(a.get("dtype"), &attr_path)?;
        let raw = a
            .get("value")
            .ok_or_else(|| validation(&attr_path, "missing value"))?;
        let value = match raw {
            Value::Array(items) => AttrValue::Array(
                items
                    .iter()
                    .enumerate()
                    .map(|(i, item)| scalar_from_json(dtype, item, &format!("{attr_path}[{i}]")))
                    .collect::<Result<_, _>>()?,
            ),
            other => AttrValue::Scalar(scalar_from_json(dtype, other, &attr_path)?),
        };
        out.insert(name.clone(), Attr { dtype, value });
    }
    Ok(out)
}

macro_rules! collect_numeric {
    ($variant:ident, $ctor:path, $dtype:expr, $items:expr, $path:expr) => {{
        let mut out = Vec::with_capacity($items.len());
        for (i, item) in $items.iter().enumerate() {
            match scalar_from_json($dtype, item, &format!("{}[{}]", $path, i))? {
                Scalar::$variant(x) => out.push(x),
                _ => unreachable!("scalar_from_json returns the requested dtype"),
            }
        }
        $ctor(out)
    }};
}

fn data_from_json(
    dtype: Dtype,
    items: &[Value],
    path: &str,
) -> Result<DatasetData, ExtensionError> {
    Ok(match dtype {
        Dtype::I8 => collect_numeric!(I8, DatasetData::I8, dtype, items, path),
        Dtype::U8 => collect_numeric!(U8, DatasetData::U8, dtype, items, path),
        Dtype::I16 => collect_numeric!(I16, DatasetData::I16, dtype, items, path),
        Dtype::U16 => collect_numeric!(U16, DatasetData::U16, dtype, items, path),
        Dtype::I32 => collect_numeric!(I32, DatasetData::I32, dtype, items, path),
        Dtype::U32 => collect_numeric!(U32, DatasetData::U32, dtype, items, path),
        Dtype::I64 => collect_numeric!(I64, DatasetData::I64, dtype, items, path),
        Dtype::U64 => collect_numeric!(U64, DatasetData::U64, dtype, items, path),
        Dtype::F32 => collect_numeric!(F32, DatasetData::F32, dtype, items, path),
        Dtype::F64 => collect_numeric!(F64, DatasetData::F64, dtype, items, path),
        Dtype::Utf8 => {
            return Err(validation(
                path,
                "dataset dtype must be numeric, got \"utf8\"",
            ))
        }
    })
}

fn column_from_json(
    dtype: Dtype,
    items: &[Value],
    path: &str,
) -> Result<ColumnData, ExtensionError> {
    if dtype == Dtype::Utf8 {
        return items
            .iter()
            .enumerate()
            .map(|(i, v)| {
                v.as_str().map(str::to_string).ok_or_else(|| {
                    validation(format!("{path}[{i}]"), "utf8 JSON value must be a string")
                })
            })
            .collect::<Result<_, _>>()
            .map(ColumnData::Utf8);
    }
    Ok(match data_from_json(dtype, items, path)? {
        DatasetData::I8(v) => ColumnData::I8(v),
        DatasetData::U8(v) => ColumnData::U8(v),
        DatasetData::I16(v) => ColumnData::I16(v),
        DatasetData::U16(v) => ColumnData::U16(v),
        DatasetData::I32(v) => ColumnData::I32(v),
        DatasetData::U32(v) => ColumnData::U32(v),
        DatasetData::I64(v) => ColumnData::I64(v),
        DatasetData::U64(v) => ColumnData::U64(v),
        DatasetData::F32(v) => ColumnData::F32(v),
        DatasetData::F64(v) => ColumnData::F64(v),
    })
}

fn table_from_json(json: &Value, path: &str) -> Result<Table, ExtensionError> {
    let field_values = json
        .get("fields")
        .and_then(Value::as_array)
        .ok_or_else(|| validation(path, "missing fields"))?;
    let rows = json
        .get("rows")
        .and_then(Value::as_u64)
        .ok_or_else(|| validation(path, "missing rows"))? as usize;
    let mut fields = Vec::with_capacity(field_values.len());
    let mut columns = Vec::with_capacity(field_values.len());
    for f in field_values {
        let name = f
            .get("name")
            .and_then(Value::as_str)
            .ok_or_else(|| validation(path, "field is missing a name"))?;
        let field_path = join_path(path, name);
        let dtype = dtype_from_json(f.get("dtype"), &field_path)?;
        let size = f.get("size").and_then(Value::as_u64).map(|n| n as usize);
        let items = json
            .get("columns")
            .and_then(|c| c.get(name))
            .and_then(Value::as_array)
            .ok_or_else(|| validation(&field_path, "missing column"))?;
        columns.push(column_from_json(dtype, items, &field_path)?);
        fields.push(TableField {
            name: name.to_string(),
            dtype,
            size,
        });
    }
    Ok(Table {
        fields,
        rows,
        columns,
        attrs: attrs_from_json(json.get("attrs"), path)?,
    })
}

fn tree_from_json_at(json: &Value, path: &str) -> Result<ExtensionTree, ExtensionError> {
    let mut tree = ExtensionTree {
        attrs: attrs_from_json(json.get("attrs"), path)?,
        ..Default::default()
    };
    if let Some(Value::Object(children)) = json.get("children") {
        for (name, child) in children {
            tree.children.insert(
                name.clone(),
                tree_from_json_at(child, &join_path(path, name))?,
            );
        }
    }
    if let Some(Value::Object(datasets)) = json.get("datasets") {
        for (name, ds) in datasets {
            let ds_path = join_path(path, name);
            let dtype = dtype_from_json(ds.get("dtype"), &ds_path)?;
            let shape = ds
                .get("shape")
                .and_then(Value::as_array)
                .ok_or_else(|| validation(&ds_path, "missing shape"))?
                .iter()
                .map(|d| {
                    d.as_u64()
                        .map(|n| n as usize)
                        .ok_or_else(|| validation(&ds_path, "invalid shape"))
                })
                .collect::<Result<Vec<_>, _>>()?;
            let items = ds
                .get("data")
                .and_then(Value::as_array)
                .ok_or_else(|| validation(&ds_path, "missing data"))?;
            tree.datasets.insert(
                name.clone(),
                Dataset {
                    shape,
                    data: data_from_json(dtype, items, &ds_path)?,
                    attrs: attrs_from_json(ds.get("attrs"), &ds_path)?,
                },
            );
        }
    }
    if let Some(Value::Object(tables)) = json.get("tables") {
        for (name, table) in tables {
            tree.tables.insert(
                name.clone(),
                table_from_json(table, &join_path(path, name))?,
            );
        }
    }
    Ok(tree)
}

/// Inverse of [`tree_to_json`].
pub fn tree_from_json(json: &Value) -> Result<ExtensionTree, ExtensionError> {
    tree_from_json_at(json, "")
}

impl Serialize for ExtensionTree {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        tree_to_json(self).serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for ExtensionTree {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = Value::deserialize(deserializer)?;
        tree_from_json(&value).map_err(D::Error::custom)
    }
}
