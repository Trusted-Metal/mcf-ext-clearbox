import { describe, expect, it } from 'vitest';
import { pack, unpack, type ClearBoxConfig, type ClearBoxUnit, type ExtensionTree } from '../src/index.js';
import { freshTree, group } from './helpers/fixtures.js';

function unpackError(tree: ExtensionTree) {
  const r = unpack(tree);
  if (r.ok) throw new Error('expected an error');
  return r.error;
}

function packError(config: ClearBoxConfig) {
  const r = pack(config);
  if (r.ok) throw new Error('expected an error');
  return r.error;
}

function baseConfig(): ClearBoxConfig {
  const r = unpack(freshTree('legacy/reference_config'));
  if (!r.ok) throw r.error;
  return r.value;
}

const unit = (c: ClearBoxConfig): ClearBoxUnit => c.units[0];
const REF = 'legacy/reference_config';

describe('unpack errors', () => {
  it('requires the IP address', () => {
    const tree = freshTree(REF);
    delete group(tree, 'Optical_Train_01').attrs['Ip_Address'];
    const e = unpackError(tree);
    expect([e.code, e.path]).toEqual(['Validation', 'Optical_Train_01/@Ip_Address']);
  });

  it('rejects an array where a scalar is expected', () => {
    const tree = freshTree(REF);
    group(tree, 'Optical_Train_01').attrs['Data_Port'] = { dtype: 'i32', value: [1, 2] };
    expect(unpackError(tree).path).toBe('Optical_Train_01/@Data_Port');
  });

  it('rejects a flag other than 0 or 1', () => {
    const tree = freshTree(REF);
    group(tree, 'Optical_Train_01').attrs['Show_Console'] = { dtype: 'i32', value: 2 };
    expect(unpackError(tree).path).toBe('Optical_Train_01/@Show_Console');
  });

  it('rejects non-numeric text for a number', () => {
    const tree = freshTree(REF);
    group(tree, 'Optical_Train_01').attrs['Data_Port'] = { dtype: 'utf8', value: 'abc' };
    expect(unpackError(tree).path).toBe('Optical_Train_01/@Data_Port');
  });

  it('rejects a non-integral float for an integer', () => {
    const tree = freshTree(REF);
    group(tree, 'Optical_Train_01').attrs['Data_Port'] = { dtype: 'f64', value: 1.5 };
    expect(unpackError(tree).path).toBe('Optical_Train_01/@Data_Port');
  });

  it('rejects groups, datasets and tables it cannot preserve', () => {
    const child = freshTree(REF);
    group(child, 'Optical_Train_01').children['Surprise'] = { attrs: {}, children: {}, datasets: {} };
    expect(unpackError(child).path).toBe('Optical_Train_01/Surprise');

    const ds = freshTree(REF);
    group(ds, 'Optical_Train_01').datasets['Extra_Data'] = { dtype: 'f64', shape: [1], data: new Float64Array(1), attrs: {} };
    expect(unpackError(ds).path).toBe('Optical_Train_01/Extra_Data');

    const rootDs = freshTree(REF);
    rootDs.datasets['Stray'] = { dtype: 'f64', shape: [1], data: new Float64Array(1), attrs: {} };
    expect(unpackError(rootDs).path).toBe('Stray');

    const table = freshTree(REF);
    group(table, 'Optical_Train_01', 'Power_Characterization').tables!['Extra_Table'] = {
      fields: [{ name: 'x', dtype: 'f64' }],
      rows: 0,
      columns: { x: new Float64Array(0) },
      attrs: {},
    };
    expect(unpackError(table).path).toBe('Optical_Train_01/Power_Characterization/Extra_Table');
  });

  it('checks grid dtype, rank and attributes', () => {
    const f32 = freshTree(REF);
    group(f32, 'Optical_Train_01').datasets['Correction_Data'].dtype = 'f32';
    expect(unpackError(f32).path).toBe('Optical_Train_01/Correction_Data');

    const flat = freshTree(REF);
    const ds = group(flat, 'Optical_Train_01').datasets['Correction_Data'];
    ds.shape = [18];
    expect(unpackError(flat).path).toBe('Optical_Train_01/Correction_Data');

    const attr = freshTree(REF);
    group(attr, 'Optical_Train_01').datasets['Correction_Data'].attrs['units'] = { dtype: 'utf8', value: 'mm' };
    expect(unpackError(attr).path).toBe('Optical_Train_01/Correction_Data/@units');
  });

  it('checks table fields and attributes', () => {
    const fields = freshTree(REF);
    const t = group(fields, 'Optical_Train_01', 'Power_Characterization').tables!['Derivation_Equation_Constants'];
    t.fields[1] = { name: 'value', dtype: 'f32' };
    const e = unpackError(fields);
    expect([e.code, e.path]).toEqual(['Validation', 'Optical_Train_01/Power_Characterization/Derivation_Equation_Constants']);

    const attrs = freshTree(REF);
    group(attrs, 'Optical_Train_01', 'Power_Characterization').tables!['Characterization_Points'].attrs['note'] = { dtype: 'utf8', value: 'x' };
    expect(unpackError(attrs).path).toBe('Optical_Train_01/Power_Characterization/Characterization_Points');
  });

  it('rejects a stored schema version', () => {
    const tree = freshTree(REF);
    tree.attrs['Schema_Version'] = { dtype: 'utf8', value: '1.0' };
    const e = unpackError(tree);
    expect([e.code, e.path]).toEqual(['UnsupportedSchemaVersion', '@Schema_Version']);
  });
});

describe('pack errors', () => {
  it('rejects duplicate train ids and bad names', () => {
    const dup = baseConfig();
    dup.units.push({ ...unit(dup) });
    expect(packError(dup).path).toBe('Optical_Train_01');

    const slash = baseConfig();
    unit(slash).trainId = 'a/b';
    expect(packError(slash).path).toBe('a/b');
  });

  it('rejects an integer outside the i32 range or a non-integer', () => {
    const big = baseConfig();
    unit(big).dataPort = 2 ** 31;
    expect(packError(big).path).toBe('Optical_Train_01/@Data_Port');

    const frac = baseConfig();
    frac.softwareTriggerDelay = 1.5;
    expect(packError(frac).path).toBe('@Software_Trigger_Delay');
  });

  it('rejects an extra attribute that collides with a modelled one', () => {
    const c = baseConfig();
    unit(c).extra['Ip_Address'] = { dtype: 'utf8', value: 'x' };
    expect(packError(c).path).toBe('Optical_Train_01/@Ip_Address');
  });

  it('rejects a grid whose data does not match its shape', () => {
    const c = baseConfig();
    unit(c).correctionData = { shape: [3, 3, 2], data: new Float64Array(5) };
    expect(packError(c).path).toBe('Optical_Train_01/Correction_Data');
  });

  it('rejects a constant name that does not fit the fixed-width column', () => {
    const c = baseConfig();
    unit(c).powerCharacterization!.derivationEquationConstants[0].name = 'x'.repeat(65);
    expect(packError(c).path).toBe('Optical_Train_01/Power_Characterization/Derivation_Equation_Constants[0]/name');
  });

  it('rejects duplicate sensor labels', () => {
    const c = unpack(freshTree('legacy/synchronous_sensors'));
    if (!c.ok) throw c.error;
    const u = unit(c.value);
    u.synchronousSensors.push({ ...u.synchronousSensors[0] });
    expect(packError(c.value).path).toBe('Optical_Train_01/Synchronous_Sensors/Oxygen Sensor');
  });

  it('rejects an unknown target schema', () => {
    const r = pack(baseConfig(), { targetSchema: '1.0' });
    if (r.ok) throw new Error('expected an error');
    expect(r.error.code).toBe('UnsupportedSchemaVersion');
  });
});
