// Behavior parity with the typed ClearBox model that used to live in core (see tools/prepare_parity.py).
// Skipped unless MCF_PARITY_DIR points at the prepared directory.
import { existsSync, readdirSync, readFileSync } from 'node:fs';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';
import { unpack, type ClearBoxUnit } from '../src/index.js';
import { treeFromJson, type TreeJson } from './helpers/treeJson.js';

const dir = process.env.MCF_PARITY_DIR;

type Old = Record<string, unknown>;

const nul = <T>(v: T | undefined): T | null => (v === undefined ? null : v);

function sameNumber(a: unknown, b: unknown): boolean {
  return a === b || (typeof a === 'number' && typeof b === 'number' && Object.is(a, b));
}

function compareUnit(u: ClearBoxUnit, cb: Old, root: { outputPath?: string; softwareTriggerDelay?: number }, drift: string[], where: string): string[] {
  const bad: string[] = [];
  const eq = (field: string, got: unknown, want: unknown) => {
    if (!sameNumber(got, want)) bad.push(`${where} ${field}: codec ${JSON.stringify(got)} vs old ${JSON.stringify(want)}`);
  };
  eq('ip_address', u.ipAddress, cb.ip_address);
  eq('serial_number', nul(u.serialNumber), cb.serial_number ?? null);
  eq('data_port', nul(u.dataPort), cb.data_port ?? null);
  eq('server_port', nul(u.serverPort), cb.server_port ?? null);
  eq('actual_timing_offset', nul(u.actualTimingOffset), cb.actual_timing_offset ?? null);
  eq('commanded_timing_offset', nul(u.commandedTimingOffset), cb.commanded_timing_offset ?? null);
  eq('manufacturer', nul(u.manufacturer), cb.manufacturer ?? null);
  eq('model', nul(u.model), cb.model ?? null);
  eq('firmware_version', nul(u.firmwareVersion), cb.firmware_version ?? null);
  eq('selected_camera', nul(u.legacy?.selectedCamera), cb.selected_camera ?? null);
  eq('custom_video_format', nul(u.legacy?.customVideoFormat), cb.custom_video_format ?? null);
  eq('video_output', nul(u.legacy?.videoOutput), cb.video_output ?? null);
  eq('show_console', nul(u.legacy?.showConsole), cb.show_console ?? null);
  eq('correction_grid_domain_shape', nul(u.legacy?.correctionGridDomainShape), cb.correction_grid_domain_shape ?? null);
  eq('inverse_grid_domain_shape', nul(u.legacy?.inverseGridDomainShape), cb.inverse_grid_domain_shape ?? null);

  // Machine-wide values: any difference is drift between trains in the source file, not a codec bug.
  if (nul(root.outputPath) !== (cb.output_path ?? null)) drift.push(`${where} output_path ${JSON.stringify(cb.output_path)} vs shared ${JSON.stringify(root.outputPath)}`);
  if (nul(root.softwareTriggerDelay) !== (cb.software_trigger_delay ?? null)) {
    drift.push(`${where} software_trigger_delay ${JSON.stringify(cb.software_trigger_delay)} vs shared ${JSON.stringify(root.softwareTriggerDelay)}`);
  }

  const pc = cb.power_characterization as Old | null | undefined;
  if (pc == null) {
    if (u.powerCharacterization !== undefined) bad.push(`${where} unexpected power_characterization`);
  } else if (u.powerCharacterization === undefined) {
    bad.push(`${where} missing power_characterization`);
  } else {
    const p = u.powerCharacterization;
    eq('pc.algorithm_type', nul(p.algorithmType), pc.algorithm_type ?? null);
    eq('pc.algorithm_equation', nul(p.algorithmEquation), pc.algorithm_equation ?? null);
    eq('pc.input_type', nul(p.inputType), pc.input_type ?? null);
    eq('pc.units_derived_quantity', nul(p.unitsDerivedQuantity), pc.units_derived_quantity ?? null);
    expect(p.derivationEquationConstants).toEqual(pc.derivation_equation_constants);
    expect(p.characterizationPoints.map((c) => ({ input_value: c.inputValue, output_value: c.outputValue }))).toEqual(pc.characterization_points);
  }

  const sensors = (cb.synchronous_sensors ?? {}) as Record<string, Old>;
  eq('sensor labels', u.synchronousSensors.map((s) => s.label).join('|'), Object.keys(sensors).sort().join('|'));
  for (const s of u.synchronousSensors) {
    const o = sensors[s.label];
    if (o === undefined) continue;
    const pairs: Array<[string, unknown, unknown]> = [
      ['enabled', nul(s.enabled), o.enabled ?? null],
      ['sensor_name', nul(s.sensorName), o.sensor_name ?? null],
      ['sensor_output_range_low', nul(s.sensorOutputRangeLow), o.sensor_output_range_low ?? null],
      ['sensor_output_range_high', nul(s.sensorOutputRangeHigh), o.sensor_output_range_high ?? null],
      ['sensor_output_space', nul(s.sensorOutputSpace), o.sensor_output_space ?? null],
      ['sensor_model', nul(s.sensorModel), o.sensor_model ?? null],
      ['sensor_manufacturer', nul(s.sensorManufacturer), o.sensor_manufacturer ?? null],
      ['sensor_scope', nul(s.sensorScope), o.sensor_scope ?? null],
      ['units_derived_quantity', nul(s.unitsDerivedQuantity), o.units_derived_quantity ?? null],
      ['port_id', nul(s.portId), o.port_id ?? null],
      ['sensor_type', nul(s.sensorType), o.sensor_type ?? null],
      ['input_type', nul(s.inputType), o.input_type ?? null],
      ['algorithm_type', nul(s.algorithmType), o.algorithm_type ?? null],
      ['algorithm_equation', nul(s.algorithmEquation), o.algorithm_equation ?? null],
      ['calibration_source', nul(s.calibrationSource), o.calibration_source ?? null],
      ['calibration_verified', nul(s.calibrationVerified), o.calibration_verified ?? null],
      ['sample_period', nul(s.samplePeriod), o.sample_period ?? null],
      ['metadata', nul(s.metadata), o.metadata ?? null],
    ];
    for (const [f, got, want] of pairs) eq(`sensor ${s.label} ${f}`, got, want);
    expect(s.derivationEquationConstants).toEqual(o.derivation_equation_constants);
    expect(s.calibrationPoints.map((c) => ({ input_value: c.inputValue, output_value: c.outputValue }))).toEqual(o.calibration_points);
  }

  for (const [field, grid] of [
    ['correction_data', u.correctionData],
    ['inverse_correction_data', u.inverseCorrectionData],
  ] as const) {
    const old = cb[field] as number[][][] | null | undefined;
    if (old == null) {
      if (grid !== undefined) bad.push(`${where} unexpected ${field}`);
      continue;
    }
    if (grid === undefined) {
      bad.push(`${where} missing ${field}`);
      continue;
    }
    const [a, b, c] = grid.shape;
    let mismatches = 0;
    for (let i = 0; i < a; i++) {
      for (let j = 0; j < b; j++) {
        for (let k = 0; k < c; k++) {
          const want = old[i][j][k];
          const got = grid.data[(i * b + j) * c + k];
          if (want === null ? !Number.isNaN(got) : got !== want) mismatches++;
        }
      }
    }
    if (mismatches > 0) bad.push(`${where} ${field}: ${mismatches} differing cells`);
  }
  return bad;
}

describe.skipIf(!dir || !existsSync(dir))('parity with the old typed ClearBox model', () => {
  const cases = dir && existsSync(dir) ? readdirSync(dir).sort() : [];
  const allDrift: string[] = [];

  it.each(cases)('file %s', (id) => {
    const base = join(dir!, id);
    const name = readFileSync(join(base, 'name.txt'), 'utf8');
    const old = JSON.parse(readFileSync(join(base, 'old.json'), 'utf8')) as { trains: Array<Old | null> };
    const newJson = JSON.parse(readFileSync(join(base, 'new.json'), 'utf8')) as TreeJson | null;
    const present = old.trains.map((t, i) => [t, i] as const).filter(([t]) => t !== null);

    if (newJson === null) {
      expect(present, `${name}: old core had ClearBox but the new tree is absent`).toHaveLength(0);
      return;
    }
    const r = unpack(treeFromJson(newJson));
    if (!r.ok) throw new Error(`${name}: ${r.error.path}: ${r.error.message}`);
    const cfg = r.value;
    expect(cfg.units.map((u) => u.trainId), name).toEqual(present.map(([, i]) => `Optical_Train_${String(i + 1).padStart(2, '0')}`));
    const drift: string[] = [];
    const bad: string[] = [];
    for (const [cb, i] of present) {
      const u = cfg.units.find((x) => x.trainId === `Optical_Train_${String(i + 1).padStart(2, '0')}`)!;
      bad.push(...compareUnit(u, cb as Old, cfg, drift, `${name} train ${i + 1}`));
    }
    allDrift.push(...drift);
    expect(bad).toEqual([]);
  });

  it('reports machine-wide drift between trains', () => {
    if (allDrift.length > 0) console.log(`drift (second train differs from the first):\n  ${allDrift.join('\n  ')}`);
    expect(true).toBe(true);
  });
});
