import type { ExtensionAttr, ExtensionScalar } from './contract.js';
import { joinPath, validation } from './errors.js';
import type { Extra } from './model.js';

type Attrs = Record<string, ExtensionAttr>;

const INT_DTYPES = new Set(['i8', 'u8', 'i16', 'u16', 'i32', 'u32', 'i64', 'u64']);
const FLOAT_DTYPES = new Set(['f32', 'f64']);

const I32_MIN = -2147483648;
const I32_MAX = 2147483647;

function attrPath(path: string, name: string): string {
  return joinPath(path, `@${name}`);
}

function scalarOf(a: ExtensionAttr, p: string): ExtensionScalar {
  if (Array.isArray(a.value)) throw validation(p, 'expected a scalar attribute, got an array');
  return a.value;
}

function textOf(v: ExtensionScalar): string {
  return typeof v === 'string' ? v : String(v);
}

/** Optional text: trimmed, empty means absent. Numeric dtypes are read as their decimal text. */
export function readOptString(attrs: Attrs, name: string, path: string): string | undefined {
  const a = attrs[name];
  if (a === undefined) return undefined;
  const t = textOf(scalarOf(a, attrPath(path, name))).trim();
  return t === '' ? undefined : t;
}

/** Required text, kept as stored (not trimmed); the attribute must exist. */
export function readRequiredString(attrs: Attrs, name: string, path: string): string {
  const a = attrs[name];
  const p = attrPath(path, name);
  if (a === undefined) throw validation(p, 'required attribute is missing');
  return textOf(scalarOf(a, p));
}

function toSafeNumber(v: ExtensionScalar, p: string): number {
  if (typeof v === 'bigint') {
    if (v > BigInt(Number.MAX_SAFE_INTEGER) || v < BigInt(Number.MIN_SAFE_INTEGER)) {
      throw validation(p, `integer ${v} does not fit in a JS number`);
    }
    return Number(v);
  }
  return v as number;
}

/** Optional integer: any integer dtype, an integral float, or numeric text; empty text means absent. */
export function readOptInt(attrs: Attrs, name: string, path: string): number | undefined {
  const a = attrs[name];
  if (a === undefined) return undefined;
  const p = attrPath(path, name);
  const v = scalarOf(a, p);
  if (INT_DTYPES.has(a.dtype)) return toSafeNumber(v, p);
  if (FLOAT_DTYPES.has(a.dtype)) {
    const f = v as number;
    if (!Number.isInteger(f)) throw validation(p, `expected an integer, got ${f}`);
    return f;
  }
  const t = (v as string).trim();
  if (t === '') return undefined;
  if (!/^[+-]?\d+$/.test(t)) throw validation(p, `expected an integer, got "${t}"`);
  const n = Number(t);
  if (!Number.isSafeInteger(n)) throw validation(p, `integer "${t}" does not fit in a JS number`);
  return n;
}

/** Optional float: any numeric dtype or numeric text; empty text means absent. */
export function readOptFloat(attrs: Attrs, name: string, path: string): number | undefined {
  const a = attrs[name];
  if (a === undefined) return undefined;
  const p = attrPath(path, name);
  const v = scalarOf(a, p);
  if (a.dtype !== 'utf8') return toSafeNumber(v, p);
  const t = (v as string).trim();
  if (t === '') return undefined;
  const n = Number(t);
  if (!Number.isFinite(n)) throw validation(p, `expected a number, got "${t}"`);
  return n;
}

/** Optional flag: stored as integer 0 or 1; anything else is an error. */
export function readOptBool(attrs: Attrs, name: string, path: string): boolean | undefined {
  const n = readOptInt(attrs, name, path);
  if (n === undefined) return undefined;
  if (n !== 0 && n !== 1) throw validation(attrPath(path, name), `expected 0 or 1, got ${n}`);
  return n === 1;
}

/** Every attribute not in `known`, copied so the model never aliases the input tree. */
export function takeExtra(attrs: Attrs, known: ReadonlySet<string>): Extra {
  const out: Extra = {};
  for (const [name, a] of Object.entries(attrs)) {
    if (known.has(name)) continue;
    out[name] = { dtype: a.dtype, value: Array.isArray(a.value) ? [...a.value] : a.value };
  }
  return out;
}

// --- writers: an absent number or flag is stored as an empty utf8 string, as core's typed writers did ---

export function strAttr(v: string | undefined): ExtensionAttr {
  return { dtype: 'utf8', value: v ?? '' };
}

export function intAttr(v: number | undefined, path: string): ExtensionAttr {
  if (v === undefined) return strAttr(undefined);
  if (!Number.isInteger(v) || v < I32_MIN || v > I32_MAX) {
    throw validation(path, `${v} is not an integer in the i32 range`);
  }
  return { dtype: 'i32', value: v };
}

export function floatAttr(v: number | undefined): ExtensionAttr {
  return v === undefined ? strAttr(undefined) : { dtype: 'f64', value: v };
}

export function boolAttr(v: boolean | undefined): ExtensionAttr {
  return v === undefined ? strAttr(undefined) : { dtype: 'i32', value: v ? 1 : 0 };
}

/** Adds `extra` to `attrs`; a name the codec itself writes is an error rather than an override. */
export function putExtra(attrs: Attrs, extra: Extra, known: ReadonlySet<string>, path: string): void {
  for (const [name, a] of Object.entries(extra)) {
    if (known.has(name)) throw validation(attrPath(path, name), 'extra attribute collides with a modelled attribute');
    attrs[name] = { dtype: a.dtype, value: Array.isArray(a.value) ? [...a.value] : a.value };
  }
}

export function nodeName(name: string, path: string): void {
  if (name === '' || name.includes('/') || name.includes('\0')) {
    throw validation(path, `invalid name "${name}": must be non-empty and contain no "/" or NUL`);
  }
}

/** Byte-order comparison, which is also the contract's canonical order. */
export function compareBytes(a: string, b: string): number {
  return Buffer.compare(Buffer.from(a, 'utf8'), Buffer.from(b, 'utf8'));
}
