// Integration with MCF core: files go through core's reader/writer, the ClearBox tree through this codec.
// Skipped unless MCF_CORE_DIR points at a built core checkout (needs <dir>/nodejs/dist and its node_modules).
//   MCF_CORE_DIR=C:/repo/Machine_Config_Library npm test -- core.integration
import { existsSync, readdirSync, rmSync, mkdtempSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { pathToFileURL } from 'node:url';
import { afterAll, beforeAll, describe, expect, it } from 'vitest';
import {
  EXTENSION_NAME,
  REQUIRES_CONTRACT,
  pack,
  unpack,
  type ClearBoxConfig,
  type ExtensionTree,
} from '../src/index.js';

const coreDir = process.env.MCF_CORE_DIR;
const enabled = !!coreDir && existsSync(join(coreDir, 'nodejs', 'dist', 'index.js'));

// Loose view of the parts of core used here; the real types live in core.
interface Core {
  MachineConfigReader: new (path: string) => { parse(): Promise<Cfg> };
  MachineConfigWriter: new (cfg: Cfg, targetVersion?: string) => { write(path: string): Promise<void> };
  MockConfigBuilder: new (o?: Record<string, unknown>) => { build(): Cfg };
  canonicalDigest(tree: ExtensionTree): string;
  EXTENSION_CONTRACT_VERSION: string;
  emptyExtensionTree(): ExtensionTree;
}
interface Cfg {
  meta: { file_version: string };
  machine: unknown;
  optical_trains: Array<{ train_id: string }>;
  extensions?: Record<string, ExtensionTree>;
}

let core: Core;
let tmp: string;
const VERSIONS = ['1.0', '1.1'];

function must<T>(r: { ok: true; value: T } | { ok: false; error: Error }): T {
  if (!r.ok) throw r.error;
  return r.value;
}

/** Present-but-empty and absent attributes mean the same to every reader, so digests ignore them. */
function stripEmpty(tree: ExtensionTree): ExtensionTree {
  const attrs = Object.fromEntries(Object.entries(tree.attrs).filter(([, a]) => !(a.dtype === 'utf8' && a.value === '')));
  const children = Object.fromEntries(Object.entries(tree.children).map(([k, c]) => [k, stripEmpty(c)]));
  return { ...tree, attrs, children };
}

const digest = (t: ExtensionTree) => core.canonicalDigest(stripEmpty(t));

async function through(cfg: Cfg, version: string, name: string): Promise<Cfg> {
  const out = join(tmp, `${name}_${version}_${Math.random().toString(36).slice(2)}.h5`);
  await new core.MachineConfigWriter(cfg, version).write(out);
  return new core.MachineConfigReader(out).parse();
}

/** What a File_Version 1.0 file keeps of a model: no firmware version, and power characterization only as constants. */
function v1_0View(model: ClearBoxConfig): ClearBoxConfig {
  const m = structuredClone(model);
  for (const u of m.units) {
    delete u.firmwareVersion;
    if (u.powerCharacterization) {
      delete u.powerCharacterization.inputType;
      delete u.powerCharacterization.unitsDerivedQuantity;
      u.powerCharacterization.characterizationPoints = [];
    }
  }
  return m;
}

function withClearBox(cfg: Cfg, tree: ExtensionTree): Cfg {
  return { ...cfg, extensions: { ...(cfg.extensions ?? {}), [EXTENSION_NAME]: tree } };
}

function clearBoxOf(cfg: Cfg): ExtensionTree {
  const t = cfg.extensions?.[EXTENSION_NAME];
  if (t === undefined) throw new Error('no ClearBox extension');
  return t;
}

describe.skipIf(!enabled)('core integration', () => {
  const fixtures: Array<[string, string]> = [];

  beforeAll(async () => {
    core = (await import(pathToFileURL(join(coreDir!, 'nodejs', 'dist', 'index.js')).href)) as Core;
    tmp = mkdtempSync(join(tmpdir(), 'mcf-clearbox-it-'));
    for (const f of readdirSync(join(coreDir!, 'fixtures')).filter((n) => n.endsWith('.h5')).sort()) {
      fixtures.push([f, join(coreDir!, 'fixtures', f)]);
    }
  }, 60_000);

  afterAll(() => {
    if (tmp) rmSync(tmp, { recursive: true, force: true });
  });

  it('core exposes a contract version this codec accepts', () => {
    const m = /^\^(\d+)\.(\d+)\.(\d+)$/.exec(REQUIRES_CONTRACT)!;
    const c = core.EXTENSION_CONTRACT_VERSION.split('.').map(Number);
    const [maj, min, pat] = [Number(m[1]), Number(m[2]), Number(m[3])];
    expect(c[0]).toBe(maj);
    // caret on 0.x pins the minor
    if (maj === 0) expect(c[1]).toBe(min);
    else expect(c[1]).toBeGreaterThanOrEqual(min);
    if (c[1] === min) expect(c[2]).toBeGreaterThanOrEqual(pat);
  });

  it('every core fixture that carries ClearBox unpacks, repacks and survives a core round trip in both file versions', async () => {
    let seen = 0;
    for (const [name, path] of fixtures) {
      const cfg = await new core.MachineConfigReader(path).parse();
      const tree = cfg.extensions?.[EXTENSION_NAME];
      if (tree === undefined) continue;
      seen++;

      const model = must(unpack(tree));
      // Not digest-equal to `tree` in general: integers are normalized to i32 on write (a sample file stores i64).
      const repacked = must(pack(model));
      expect(must(unpack(repacked)), `${name}: unpack(pack(model)) changed the model`).toEqual(model);
      expect(digest(must(pack(must(unpack(repacked))))), `${name}: pack is not a fixpoint`).toBe(digest(repacked));

      for (const v of VERSIONS) {
        // File_Version 1.0 has no Extensions group, so ClearBox is the only extension it can carry.
        const target = v === '1.0' ? { ...cfg, extensions: { [EXTENSION_NAME]: repacked } } : withClearBox(cfg, repacked);
        const back = await through(target, v, name.replace('.h5', ''));
        expect(back.meta.file_version).toBe(v);
        for (const [other, t] of Object.entries(target.extensions ?? {})) {
          if (other !== EXTENSION_NAME) expect(digest(back.extensions![other]), `${name}: ${other} changed`).toBe(digest(t));
        }
        const backTree = clearBoxOf(back);
        // A 1.0 layout has no place for the 1.1-only fields, so only a 1.1 target (or a 1.0 source) is lossless.
        const lossless = v === '1.1' || cfg.meta.file_version === '1.0';
        if (lossless) {
          expect(digest(backTree), `${name} -> ${v}: core changed the packed tree`).toBe(digest(repacked));
          expect(must(unpack(backTree)), `${name} -> ${v}: model changed`).toEqual(model);
        } else {
          expect(must(unpack(backTree)), `${name} -> ${v}: lost more than the 1.1-only fields`).toEqual(v1_0View(model));
        }
      }
    }
    expect(seen).toBeGreaterThan(0);
  }, 120_000);

  it.each(VERSIONS)('core mock config (File_Version %s) round trips through the codec', async (version) => {
    const cfg = new core.MockConfigBuilder({ fileVersion: version, nLasers: 3 }).build();
    const model = must(unpack(clearBoxOf(cfg)));
    expect(model.units.map((u) => u.trainId)).toEqual(cfg.optical_trains.map((t) => t.train_id));

    const back = await through(withClearBox(cfg, must(pack(model))), version, 'mock');
    expect(must(unpack(clearBoxOf(back)))).toEqual(model);
  });

  it.each(VERSIONS)('editing the model and writing it back through core (File_Version %s) keeps the edit and the rest of the file', async (version) => {
    const cfg = new core.MockConfigBuilder({ fileVersion: version, nLasers: 2 }).build();
    const model = must(unpack(clearBoxOf(cfg)));

    const edited: ClearBoxConfig = structuredClone(model);
    edited.units[0].ipAddress = '192.168.77.5';
    edited.units[0].dataPort = 6123;
    edited.units[1].synchronousSensors = [];
    edited.softwareTriggerDelay = 42;

    const back = await through(withClearBox(cfg, must(pack(edited))), version, 'edit');
    expect(must(unpack(clearBoxOf(back)))).toEqual(edited);
    expect(back.machine).toEqual(cfg.machine);
    expect(back.optical_trains.map((t) => t.train_id)).toEqual(cfg.optical_trains.map((t) => t.train_id));
  });

  it('a config without ClearBox gains one from pack and loses nothing else', async () => {
    const bare = new core.MockConfigBuilder({ includeClearbox: false, nLasers: 2 }).build();
    expect(bare.extensions?.[EXTENSION_NAME]).toBeUndefined();

    const donor = new core.MockConfigBuilder({ nLasers: 2 }).build();
    const model = must(unpack(clearBoxOf(donor)));
    for (const v of VERSIONS) {
      const back = await through(withClearBox(bare, must(pack(model))), v, 'bare');
      expect(must(unpack(clearBoxOf(back)))).toEqual(model);
    }
  });

  it('a file written without ClearBox reads back without it', async () => {
    const bare = new core.MockConfigBuilder({ includeClearbox: false }).build();
    for (const v of VERSIONS) {
      const back = await through(bare, v, 'noclearbox');
      expect(back.extensions?.[EXTENSION_NAME]).toBeUndefined();
    }
  });

  it('core keeps a tree the codec rejects verbatim, so a newer ClearBox layout is not lost by an older reader', async () => {
    const cfg = new core.MockConfigBuilder({ fileVersion: '1.1' }).build();
    const tree = structuredClone(clearBoxOf(cfg));
    tree.attrs['Schema_Version'] = { dtype: 'utf8', value: '2.0' };
    const unknown = core.emptyExtensionTree();
    unknown.attrs['Note'] = { dtype: 'utf8', value: 'future' };
    tree.children['Optical_Train_01'].children['Future_Group'] = unknown;

    const back = await through(withClearBox(cfg, tree), '1.1', 'future');
    const backTree = clearBoxOf(back);
    expect(digest(backTree)).toBe(digest(tree));

    const r = unpack(backTree);
    expect(r.ok).toBe(false);
    if (!r.ok) expect(r.error.code).toBe('UnsupportedSchemaVersion');

    delete tree.attrs['Schema_Version'];
    const r2 = unpack(tree);
    expect(r2.ok).toBe(false);
    if (!r2.ok) {
      expect(r2.error.code).toBe('Validation');
      expect(r2.error.path).toContain('Future_Group');
    }
  });

  it('cross-version: a 1.0-shaped config written as 1.1 and back as 1.0 keeps the model', async () => {
    const cfg = new core.MockConfigBuilder({ fileVersion: '1.0', nLasers: 2 }).build();
    const model = must(unpack(clearBoxOf(cfg)));
    const as11 = await through(cfg, '1.1', 'up');
    const as10 = await through(as11, '1.0', 'down');
    expect(must(unpack(clearBoxOf(as11)))).toEqual(model);
    expect(must(unpack(clearBoxOf(as10)))).toEqual(model);
  });
});
