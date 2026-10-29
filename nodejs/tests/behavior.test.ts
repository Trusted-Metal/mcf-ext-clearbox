import { describe, expect, it } from 'vitest';
import { unpack, type ClearBoxConfig, type ExtensionTree } from '../src/index.js';
import { freshTree, group } from './helpers/fixtures.js';

function ok(tree: ExtensionTree): ClearBoxConfig {
  const r = unpack(tree);
  if (!r.ok) throw r.error;
  return r.value;
}

describe('reference_config (File_Version 1.0 origin)', () => {
  const cfg = ok(freshTree('legacy/reference_config'));

  it('reads the machine-wide values from the root', () => {
    expect(cfg.outputPath).toBe('/recordings/');
    expect(cfg.softwareTriggerDelay).toBe(3000);
  });

  it('reads units sorted by train id with their scalars', () => {
    expect(cfg.units.map((u) => u.trainId)).toEqual(['Optical_Train_01', 'Optical_Train_02']);
    const u = cfg.units[0];
    expect(u.ipAddress).toBe('192.168.1.10');
    expect(u.serialNumber).toBe('001');
    expect([u.dataPort, u.serverPort, u.actualTimingOffset, u.commandedTimingOffset]).toEqual([5001, 20101, -8, 50]);
  });

  it('treats an empty string as absent', () => {
    const u = cfg.units[0];
    expect(u.manufacturer).toBeUndefined();
    expect(u.model).toBeUndefined();
    expect(u.firmwareVersion).toBeUndefined();
    expect(u.legacy?.correctionGridDomainShape).toBeUndefined();
  });

  it('groups the File_Version 1.0 only attributes under `legacy`', () => {
    expect(cfg.units[0].legacy).toEqual({
      selectedCamera: 'Default',
      customVideoFormat: 'MP4',
      videoOutput: 'HDMI',
      showConsole: false,
      correctionGridDomainShape: undefined,
      inverseGridDomainShape: undefined,
    });
  });

  it('keeps coefficient order and leaves blank power fields absent', () => {
    const pc = cfg.units[0].powerCharacterization!;
    expect(pc.algorithmType).toBe('LINEAR');
    expect(pc.algorithmEquation).toBe('W = a*V + b');
    expect(pc.inputType).toBeUndefined();
    expect(pc.unitsDerivedQuantity).toBeUndefined();
    expect(pc.derivationEquationConstants).toEqual([
      { name: 'b', value: 50.5 },
      { name: 'a', value: 107.5 },
    ]);
    expect(pc.characterizationPoints).toEqual([]);
  });

  it('reads grids row-major with NaN preserved', () => {
    const g = cfg.units[0].correctionData!;
    expect(g.shape).toEqual([3, 3, 2]);
    expect(g.data).toBeInstanceOf(Float64Array);
    expect(Number.isNaN(g.data[0])).toBe(true);
    expect(g.data[1]).toBe(-2.5);
  });
});

describe('File_Version 1.1 origin', () => {
  const cfg = ok(freshTree('legacy/v1_1'));

  it('reads the firmware version and has no legacy group', () => {
    expect(cfg.units[0].firmwareVersion).toBe('2.4.1');
    expect(cfg.units[0].legacy).toBeUndefined();
  });
});

describe('synchronous sensors', () => {
  const sensors = ok(freshTree('legacy/synchronous_sensors')).units[0].synchronousSensors;

  it('reads every attribute and both tables', () => {
    expect(sensors).toHaveLength(1);
    const s = sensors[0];
    expect(s.label).toBe('Oxygen Sensor');
    expect(s.enabled).toBe(true);
    expect(s.calibrationVerified).toBe(false);
    expect(s.portId).toBe(5);
    expect(s.samplePeriod).toBe(5);
    expect([s.sensorOutputRangeLow, s.sensorOutputRangeHigh]).toEqual([-1, 6]);
    expect(s.algorithmType).toBe('Log-Linear');
    expect(s.derivationEquationConstants.map((c) => c.name)).toEqual(['a', 'b']);
    expect(s.calibrationPoints).toHaveLength(2);
  });
});

describe('lenient reads (how core read legacy files)', () => {
  function withUnitAttr(name: string, attr: { dtype: 'utf8' | 'i32' | 'i64' | 'f64' | 'u8'; value: string | number | bigint | Array<string | number | bigint> }): ClearBoxConfig {
    const tree = freshTree('legacy/reference_config');
    group(tree, 'Optical_Train_01').attrs[name] = attr;
    return ok(tree);
  }

  it('reads an empty string number as absent and numeric text as a number', () => {
    expect(withUnitAttr('Data_Port', { dtype: 'utf8', value: '' }).units[0].dataPort).toBeUndefined();
    expect(withUnitAttr('Data_Port', { dtype: 'utf8', value: ' 5002 ' }).units[0].dataPort).toBe(5002);
  });

  it('accepts any integer dtype, including i64', () => {
    expect(withUnitAttr('Data_Port', { dtype: 'i64', value: 7n }).units[0].dataPort).toBe(7);
    expect(withUnitAttr('Data_Port', { dtype: 'u8', value: 9 }).units[0].dataPort).toBe(9);
    expect(withUnitAttr('Data_Port', { dtype: 'f64', value: 11 }).units[0].dataPort).toBe(11);
  });

  it('trims optional text but keeps the required IP address as stored', () => {
    expect(withUnitAttr('Serial_Number', { dtype: 'utf8', value: '  X1  ' }).units[0].serialNumber).toBe('X1');
    expect(withUnitAttr('Ip_Address', { dtype: 'utf8', value: ' 10.0.0.1' }).units[0].ipAddress).toBe(' 10.0.0.1');
  });

  it('reads a numeric text attribute as its decimal text', () => {
    expect(withUnitAttr('Serial_Number', { dtype: 'i32', value: 123 }).units[0].serialNumber).toBe('123');
  });

  it('keeps unmodelled attributes in `extra` with their dtype', () => {
    const u = withUnitAttr('Port', { dtype: 'i32', value: 5001 }).units[0];
    expect(u.extra).toEqual({ Port: { dtype: 'i32', value: 5001 } });
  });
});
