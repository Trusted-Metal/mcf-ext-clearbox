import { describe, expect, it } from 'vitest';
import { EXTENSION_NAME, REQUIRES_CONTRACT, pack, peekSchemaVersion, supportedSchemaVersions, unpack, type ClearBoxConfig, type ClearBoxUnit } from '../src/index.js';
import { freshTree } from './helpers/fixtures.js';
import { treeToJson } from './helpers/treeJson.js';

function grid(shape: [number, number, number], seed: number) {
  const data = new Float64Array(shape[0] * shape[1] * shape[2]);
  data.forEach((_, i) => (data[i] = i % 11 === 0 ? NaN : i * 0.001 + seed));
  return { shape, data };
}

function richUnit(trainId: string): ClearBoxUnit {
  return {
    trainId,
    ipAddress: '10.1.2.3',
    serialNumber: 'SN-9',
    dataPort: 5001,
    serverPort: 20101,
    actualTimingOffset: -8,
    commandedTimingOffset: 50,
    manufacturer: 'Trusted Metal',
    model: 'CB-2',
    firmwareVersion: '2.4.1',
    legacy: { selectedCamera: 'Cam', customVideoFormat: 'MP4', videoOutput: 'HDMI', showConsole: true, correctionGridDomainShape: '5x5', inverseGridDomainShape: '6x6' },
    correctionData: grid([257, 257, 2], 1),
    inverseCorrectionData: grid([257, 257, 2], 2),
    synchronousSensors: [
      {
        label: 'A',
        enabled: true,
        sensorName: 'n',
        sensorOutputRangeLow: -1.5,
        sensorOutputRangeHigh: 6,
        portId: 3,
        calibrationVerified: false,
        samplePeriod: 0.25,
        derivationEquationConstants: [{ name: 'a', value: 1 }],
        calibrationPoints: [{ inputValue: 4, outputValue: 0.5 }],
        extra: { Note: { dtype: 'utf8', value: 'kept' } },
      },
      { label: 'B', derivationEquationConstants: [], calibrationPoints: [], extra: {} },
    ],
    powerCharacterization: {
      algorithmType: 'POLYNOMIAL',
      algorithmEquation: 'W = c0 + c1*V',
      inputType: 'Volts',
      unitsDerivedQuantity: 'Watts',
      derivationEquationConstants: [
        { name: 'c0', value: 0.5 },
        { name: 'c1', value: 2 },
      ],
      characterizationPoints: [{ inputValue: 1, outputValue: 2.5 }],
      extra: { Scale: { dtype: 'f64', value: 3.5 } },
    },
    extra: { Port: { dtype: 'i32', value: 7 }, Big: { dtype: 'i64', value: 9007199254740993n }, List: { dtype: 'u8', value: [1, 2, 3] } },
  };
}

function richConfig(): ClearBoxConfig {
  return {
    outputPath: '/out/',
    softwareTriggerDelay: 44,
    units: [richUnit('Optical_Train_01'), richUnit('Optical_Train_02')],
    extra: { Root_Note: { dtype: 'utf8', value: 'r' } },
  };
}

describe('round trip', () => {
  it('unpack(pack(model)) is the model, including 257x257x2 grids with NaN', () => {
    const model = richConfig();
    const packed = pack(model);
    if (!packed.ok) throw packed.error;
    const back = unpack(packed.value);
    if (!back.ok) throw back.error;
    expect(back.value).toEqual(model);
  });

  it('pack(unpack(tree)) is the tree', () => {
    const first = pack(richConfig());
    if (!first.ok) throw first.error;
    const model = unpack(first.value);
    if (!model.ok) throw model.error;
    const second = pack(model.value);
    if (!second.ok) throw second.error;
    expect(treeToJson(second.value)).toEqual(treeToJson(first.value));
  });

  it('sorts units and sensors by name', () => {
    const model = richConfig();
    model.units.reverse();
    model.units[1].synchronousSensors.reverse();
    const packed = pack(model);
    if (!packed.ok) throw packed.error;
    const back = unpack(packed.value);
    if (!back.ok) throw back.error;
    expect(back.value.units.map((u) => u.trainId)).toEqual(['Optical_Train_01', 'Optical_Train_02']);
    expect(back.value.units[0].synchronousSensors.map((s) => s.label)).toEqual(['A', 'B']);
  });

  it('writes an absent number or flag as an empty string and reads it back absent', () => {
    const model: ClearBoxConfig = {
      units: [{ trainId: 'Optical_Train_01', ipAddress: '1.1.1.1', synchronousSensors: [], extra: {} }],
      extra: {},
    };
    const packed = pack(model);
    if (!packed.ok) throw packed.error;
    const u = packed.value.children['Optical_Train_01'];
    expect(u.attrs['Data_Port']).toEqual({ dtype: 'utf8', value: '' });
    expect(packed.value.attrs['Software_Trigger_Delay']).toEqual({ dtype: 'utf8', value: '' });
    expect(u.attrs['Firmware_Version']).toEqual({ dtype: 'utf8', value: '' });
    expect(u.attrs['Show_Console']).toBeUndefined();
    expect(u.children['Power_Characterization']).toBeUndefined();
    expect(u.datasets).toEqual({});
    const back = unpack(packed.value);
    if (!back.ok) throw back.error;
    expect(back.value).toEqual({ ...model, outputPath: undefined, softwareTriggerDelay: undefined, units: [{ ...model.units[0], dataPort: undefined }] });
  });

  it('writes integers as i32, flags as 0/1 and grid attributes the way core did', () => {
    const packed = pack(richConfig());
    if (!packed.ok) throw packed.error;
    const u = packed.value.children['Optical_Train_01'];
    expect(u.attrs['Data_Port']).toEqual({ dtype: 'i32', value: 5001 });
    expect(u.attrs['Show_Console']).toEqual({ dtype: 'i32', value: 1 });
    expect(u.datasets['Correction_Data'].attrs).toEqual({
      dimensions: { dtype: 'utf8', value: 'H,W,D' },
      dtype: { dtype: 'utf8', value: 'float64' },
      shape: { dtype: 'utf8', value: '257x257x2' },
    });
    expect(u.tables).toBeUndefined();
    expect(u.children['Power_Characterization'].tables!['Derivation_Equation_Constants'].fields).toEqual([
      { name: 'name', dtype: 'utf8', size: 64 },
      { name: 'value', dtype: 'f64' },
    ]);
  });

  it('does not alias the input tree or the model', () => {
    const tree = freshTree('legacy/reference_config');
    const r = unpack(tree);
    if (!r.ok) throw r.error;
    r.value.units[0].correctionData!.data[0] = 123;
    expect(tree.children['Optical_Train_01'].datasets['Correction_Data'].data[0]).not.toBe(123);
  });
});

describe('facade', () => {
  it('exposes the extension name, contract range and versions', () => {
    expect(EXTENSION_NAME).toBe('ClearBox');
    expect(REQUIRES_CONTRACT).toBe('^0.1.1');
    expect(supportedSchemaVersions()).toEqual([]);
    expect(peekSchemaVersion(freshTree('legacy/reference_config'))).toBe('legacy');
    const t = freshTree('legacy/reference_config');
    t.attrs['Schema_Version'] = { dtype: 'utf8', value: '2.0' };
    expect(peekSchemaVersion(t)).toBe('2.0');
  });
});
