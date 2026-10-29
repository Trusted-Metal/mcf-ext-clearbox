// Unpacks every <dir>/*/new.json with the built codec, checks pack/unpack stability and writes <dir>/*/model.json
// (the model JSON the Rust corpus test compares against).
// usage: node tools/unpack_all.mjs <dir>
import { readdirSync, readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';

const { unpack, pack } = await import('../nodejs/dist/index.js');
const dir = process.argv[2];

// Minimal contract-JSON -> tree conversion (datasets and tables as typed arrays).
const ARR = { f64: Float64Array, f32: Float32Array, i32: Int32Array, u32: Uint32Array, i16: Int16Array, u16: Uint16Array, i8: Int8Array, u8: Uint8Array, i64: BigInt64Array, u64: BigUint64Array };
const num = (dtype, v) => (dtype === 'i64' || dtype === 'u64' ? BigInt(v) : v === 'NaN' ? NaN : v === 'Infinity' ? Infinity : v === '-Infinity' ? -Infinity : v);
const attrs = (a) => Object.fromEntries(Object.entries(a ?? {}).map(([k, x]) => [k, { dtype: x.dtype, value: Array.isArray(x.value) ? x.value.map((v) => num(x.dtype, v)) : num(x.dtype, x.value) }]));
const arr = (dtype, items) => ARR[dtype].from(items.map((v) => num(dtype, v)));
function tree(j) {
  const t = { attrs: attrs(j.attrs), children: {}, datasets: {} };
  for (const [k, c] of Object.entries(j.children ?? {})) t.children[k] = tree(c);
  for (const [k, d] of Object.entries(j.datasets ?? {})) t.datasets[k] = { dtype: d.dtype, shape: d.shape, data: arr(d.dtype, d.data), attrs: attrs(d.attrs) };
  for (const [k, tb] of Object.entries(j.tables ?? {})) {
    t.tables ??= {};
    t.tables[k] = {
      fields: tb.fields, rows: tb.rows, attrs: attrs(tb.attrs),
      columns: Object.fromEntries(tb.fields.map((f) => [f.name, f.dtype === 'utf8' ? tb.columns[f.name] : arr(f.dtype, tb.columns[f.name])])),
    };
  }
  return t;
}

const jsonNum = (v) => (Number.isFinite(v) ? v : Number.isNaN(v) ? 'NaN' : v > 0 ? 'Infinity' : '-Infinity');
const attrJson = (a) => ({ dtype: a.dtype, value: Array.isArray(a.value) ? a.value.map((v) => (typeof v === 'bigint' ? String(v) : typeof v === 'number' && a.dtype[0] === 'f' ? jsonNum(v) : v)) : typeof a.value === 'bigint' ? String(a.value) : typeof a.value === 'number' && a.dtype[0] === 'f' ? jsonNum(a.value) : a.value });
const NUMERIC = new Set(['value', 'inputValue', 'outputValue', 'sensorOutputRangeLow', 'sensorOutputRangeHigh', 'samplePeriod']);
function modelJson(node, key) {
  if (node instanceof Float64Array) return Array.from(node, jsonNum);
  if (Array.isArray(node)) return node.map((n) => modelJson(n));
  if (node && typeof node === 'object') return Object.fromEntries(Object.entries(node).map(([k, v]) => [k, k === 'extra' ? Object.fromEntries(Object.entries(v).map(([n, a]) => [n, attrJson(a)])) : modelJson(v, k)]));
  if (typeof node === 'number' && NUMERIC.has(key)) return jsonNum(node);
  return node;
}


// Reverse of tree(): contract JSON for a tree built by pack().
const njson = (dtype, v) => (typeof v === 'bigint' ? String(v) : typeof v === 'number' && dtype[0] === 'f' ? jsonNum(v) : v);
const attrsJson = (a) => Object.fromEntries(Object.entries(a).map(([k, x]) => [k, { dtype: x.dtype, value: Array.isArray(x.value) ? x.value.map((v) => njson(x.dtype, v)) : njson(x.dtype, x.value) }]));
function treeJson(t) {
  const out = {
    attrs: attrsJson(t.attrs),
    children: Object.fromEntries(Object.entries(t.children).map(([k, c]) => [k, treeJson(c)])),
    datasets: Object.fromEntries(Object.entries(t.datasets).map(([k, d]) => [k, { dtype: d.dtype, shape: d.shape, data: Array.from(d.data, (v) => njson(d.dtype, v)), attrs: attrsJson(d.attrs) }])),
  };
  if (t.tables) {
    out.tables = Object.fromEntries(Object.entries(t.tables).map(([k, tb]) => [k, {
      fields: tb.fields, rows: tb.rows, attrs: attrsJson(tb.attrs),
      columns: Object.fromEntries(tb.fields.map((f) => [f.name, f.dtype === 'utf8' ? tb.columns[f.name] : Array.from(tb.columns[f.name], (v) => njson(f.dtype, v))])),
    }]));
  }
  return out;
}
let ok = 0, absent = 0, failed = 0, roundTripDiffs = 0;
for (const id of readdirSync(dir).sort()) {
  const p = join(dir, id, 'new.json');
  let j;
  try { j = JSON.parse(readFileSync(p, 'utf8')); } catch { continue; }
  const name = readFileSync(join(dir, id, 'name.txt'), 'utf8');
  if (j === null) { absent++; continue; }
  const t = tree(j);
  const r = unpack(t);
  if (!r.ok) { failed++; console.log(`FAIL ${name}: ${r.error.path}: ${r.error.message}`); continue; }
  ok++;
  writeFileSync(join(dir, id, 'model.json'), JSON.stringify(modelJson(r.value)));
  const back = pack(r.value);
  if (back.ok) writeFileSync(join(dir, id, 'packed.json'), JSON.stringify(treeJson(back.value)));
  if (!back.ok) { failed++; console.log(`PACK FAIL ${name}: ${back.error.path}: ${back.error.message}`); continue; }
  const r2 = unpack(back.value);
  if (JSON.stringify(r2.value, (k, v) => (v instanceof Float64Array ? Array.from(v, String) : typeof v === 'bigint' ? String(v) : v)) !==
      JSON.stringify(r.value, (k, v) => (v instanceof Float64Array ? Array.from(v, String) : typeof v === 'bigint' ? String(v) : v))) { roundTripDiffs++; console.log(`ROUND TRIP DIFF ${name}`); }
}
console.log({ ok, absent, failed, roundTripDiffs });
