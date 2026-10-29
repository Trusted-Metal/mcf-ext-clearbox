// Copy of core's tree JSON form (i64/u64 as decimal strings, non-finite floats as strings), so tests never import core.
import { readFileSync } from 'node:fs';
import {
  emptyTree,
  type ExtensionAttr,
  type ExtensionDataset,
  type ExtensionDtype,
  type ExtensionNumericArray,
  type ExtensionScalar,
  type ExtensionTable,
  type ExtensionTree,
} from '../../src/contract.js';

type JsonScalar = string | number;
interface AttrJson {
  dtype: ExtensionDtype;
  value: JsonScalar | JsonScalar[];
}
interface DatasetJson {
  dtype: Exclude<ExtensionDtype, 'utf8'>;
  shape: number[];
  data: JsonScalar[];
  attrs: Record<string, AttrJson>;
}
interface TableJson {
  fields: Array<{ name: string; dtype: ExtensionDtype; size?: number }>;
  rows: number;
  columns: Record<string, JsonScalar[]>;
  attrs: Record<string, AttrJson>;
}
export interface TreeJson {
  attrs: Record<string, AttrJson>;
  children: Record<string, TreeJson>;
  datasets?: Record<string, DatasetJson>;
  tables?: Record<string, TableJson>;
}

function scalarToJson(v: ExtensionScalar): JsonScalar {
  if (typeof v === 'bigint') return v.toString();
  if (typeof v === 'number' && !Number.isFinite(v)) return Number.isNaN(v) ? 'NaN' : v > 0 ? 'Infinity' : '-Infinity';
  return v;
}

function scalarFromJson(dtype: ExtensionDtype, v: JsonScalar): ExtensionScalar {
  if (dtype === 'i64' || dtype === 'u64') return BigInt(v as string);
  if (typeof v === 'string' && dtype !== 'utf8') return v === 'NaN' ? NaN : v === 'Infinity' ? Infinity : -Infinity;
  return v;
}

const ARRAYS: Record<string, new (n: number) => ExtensionNumericArray> = {
  i8: Int8Array,
  u8: Uint8Array,
  i16: Int16Array,
  u16: Uint16Array,
  i32: Int32Array,
  u32: Uint32Array,
  i64: BigInt64Array,
  u64: BigUint64Array,
  f32: Float32Array,
  f64: Float64Array,
};

function arrayFromJson(dtype: ExtensionDtype, items: JsonScalar[]): ExtensionNumericArray {
  const out = new ARRAYS[dtype](items.length);
  items.forEach((v, i) => {
    (out as unknown as Record<number, ExtensionScalar>)[i] = scalarFromJson(dtype, v);
  });
  return out;
}

export function attrToJson(a: ExtensionAttr): AttrJson {
  return { dtype: a.dtype, value: Array.isArray(a.value) ? a.value.map(scalarToJson) : scalarToJson(a.value) };
}

export function attrFromJson(a: AttrJson): ExtensionAttr {
  return {
    dtype: a.dtype,
    value: Array.isArray(a.value) ? a.value.map((v) => scalarFromJson(a.dtype, v)) : scalarFromJson(a.dtype, a.value),
  };
}

const attrsToJson = (attrs: Record<string, ExtensionAttr>) => Object.fromEntries(Object.entries(attrs).map(([k, a]) => [k, attrToJson(a)]));
const attrsFromJson = (attrs: Record<string, AttrJson> | undefined) =>
  Object.fromEntries(Object.entries(attrs ?? {}).map(([k, a]) => [k, attrFromJson(a)]));

export function treeToJson(tree: ExtensionTree): TreeJson {
  const out: TreeJson = {
    attrs: attrsToJson(tree.attrs),
    children: Object.fromEntries(Object.entries(tree.children).map(([k, c]) => [k, treeToJson(c)])),
    datasets: Object.fromEntries(
      Object.entries(tree.datasets).map(([k, d]) => [
        k,
        { dtype: d.dtype, shape: d.shape, data: Array.from(d.data as ArrayLike<ExtensionScalar>, scalarToJson), attrs: attrsToJson(d.attrs) },
      ]),
    ),
  };
  if (tree.tables !== undefined && Object.keys(tree.tables).length > 0) {
    out.tables = Object.fromEntries(
      Object.entries(tree.tables).map(([k, t]) => [
        k,
        {
          fields: t.fields.map((f) => ({ ...f })),
          rows: t.rows,
          columns: Object.fromEntries(
            t.fields.map((f) => [f.name, Array.from(t.columns[f.name] as ArrayLike<ExtensionScalar>, scalarToJson)]),
          ),
          attrs: attrsToJson(t.attrs),
        },
      ]),
    );
  }
  return out;
}

export function treeFromJson(json: TreeJson): ExtensionTree {
  const tree = emptyTree();
  tree.attrs = attrsFromJson(json.attrs);
  for (const [k, c] of Object.entries(json.children)) tree.children[k] = treeFromJson(c);
  for (const [k, d] of Object.entries(json.datasets ?? {})) {
    const ds: ExtensionDataset = { dtype: d.dtype, shape: [...d.shape], data: arrayFromJson(d.dtype, d.data), attrs: attrsFromJson(d.attrs) };
    tree.datasets[k] = ds;
  }
  for (const [k, t] of Object.entries(json.tables ?? {})) {
    tree.tables ??= {};
    const columns: ExtensionTable['columns'] = {};
    for (const f of t.fields) {
      columns[f.name] = f.dtype === 'utf8' ? (t.columns[f.name] as string[]).slice() : arrayFromJson(f.dtype, t.columns[f.name]);
    }
    tree.tables[k] = { fields: t.fields.map((f) => ({ ...f })), rows: t.rows, columns, attrs: attrsFromJson(t.attrs) };
  }
  return tree;
}

export function loadTree(path: string): ExtensionTree {
  return treeFromJson(JSON.parse(readFileSync(path, 'utf8')) as TreeJson);
}

export function cloneTree(tree: ExtensionTree): ExtensionTree {
  return treeFromJson(treeToJson(tree));
}

/** Model fields that are numbers; a non-finite value is a string in model JSON. */
const NUMERIC_KEYS = new Set(['value', 'inputValue', 'outputValue', 'sensorOutputRangeLow', 'sensorOutputRangeHigh', 'samplePeriod']);

const f64 = (v: number): JsonScalar => scalarToJson(v) as JsonScalar;

/** Model JSON: grids are `{shape, data}` with `"NaN"` strings; `extra` maps hold attributes (i64/u64 as strings). */
export function modelToJson(model: unknown): unknown {
  const walk = (node: unknown, key?: string): unknown => {
    if (node instanceof Float64Array) return Array.from(node, f64);
    if (Array.isArray(node)) return node.map((n) => walk(n));
    if (node !== null && typeof node === 'object') {
      if (key === 'extra') return attrsToJson(node as Record<string, ExtensionAttr>);
      return Object.fromEntries(Object.entries(node).map(([k, v]) => [k, walk(v, k)]));
    }
    if (typeof node === 'number') return f64(node);
    return node;
  };
  return walk(model);
}

export function modelFromJson<T>(json: unknown): T {
  const walk = (node: unknown, key?: string, parent?: string): unknown => {
    if (key === 'data' && Array.isArray(node) && parent !== undefined && /^(inverse)?[cC]orrectionData$/.test(parent)) {
      return Float64Array.from(node as JsonScalar[], (v) => scalarFromJson('f64', v) as number);
    }
    if (Array.isArray(node)) return node.map((n) => walk(n));
    if (node !== null && typeof node === 'object') {
      if (key === 'extra') return attrsFromJson(node as Record<string, AttrJson>);
      return Object.fromEntries(Object.entries(node).map(([k, v]) => [k, walk(v, k, key)]));
    }
    if (typeof node === 'string' && key !== undefined && NUMERIC_KEYS.has(key)) return scalarFromJson('f64', node);
    return node;
  };
  return walk(json) as T;
}
