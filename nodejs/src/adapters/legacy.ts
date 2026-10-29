// The layout core writes today: `ClearBox/` with shared attributes, one `Optical_Train_NN/` group per
// train, and no schema-version attribute. Core builds the same shape from File_Version 1.0 and 1.1 files.
import {
  emptyTree,
  type ExtensionAttr,
  type ExtensionDataset,
  type ExtensionTable,
  type ExtensionTree,
} from '../contract.js';
import { joinPath, validation } from '../errors.js';
import {
  boolAttr,
  compareBytes,
  floatAttr,
  intAttr,
  nodeName,
  putExtra,
  readOptBool,
  readOptFloat,
  readOptInt,
  readOptString,
  readRequiredString,
  strAttr,
  takeExtra,
} from '../fields.js';
import type {
  CalibrationPoint,
  ClearBoxConfig,
  ClearBoxUnit,
  EquationConstant,
  Grid,
  LegacyUnitFields,
  PowerCharacterization,
  SynchronousSensor,
} from '../model.js';

const ROOT_ATTRS = new Set(['Output_Path', 'Software_Trigger_Delay']);
const LEGACY_ATTRS = [
  'Selected_Camera',
  'Custom_Video_Format',
  'Video_Output',
  'Show_Console',
  'Correction_Grid_Domain_Shape',
  'Inverse_Grid_Domain_Shape',
];
const UNIT_ATTRS = new Set([
  'Ip_Address',
  'Serial_Number',
  'Data_Port',
  'Server_Port',
  'Actual_Timing_Offset',
  'Commanded_Timing_Offset',
  'Manufacturer',
  'Model',
  'Firmware_Version',
  ...LEGACY_ATTRS,
]);
const PC_ATTRS = new Set(['Algorithm_Type', 'Algorithm_Equation', 'Input_Type', 'Units_Derived_Quantity']);
const SENSOR_ATTRS = new Set([
  'Enabled',
  'Sensor_Name',
  'Sensor_Output_Range_Low',
  'Sensor_Output_Range_High',
  'Sensor_Output_Space',
  'Sensor_Model',
  'Sensor_Manufacturer',
  'Sensor_Scope',
  'Units_Derived_Quantity',
  'Port_ID',
  'Sensor_Type',
  'Input_Type',
  'Algorithm_Type',
  'Algorithm_Equation',
  'Calibration_Source',
  'Calibration_Verified',
  'Sample_Period',
  'Metadata',
]);

const SENSORS_GROUP = 'Synchronous_Sensors';
const PC_GROUP = 'Power_Characterization';
const CORRECTION = 'Correction_Data';
const INVERSE_CORRECTION = 'Inverse_Correction_Data';
const CONSTANTS_TABLE = 'Derivation_Equation_Constants';
const POINTS_TABLE = 'Characterization_Points';
const SENSOR_POINTS_TABLE = 'Calibration_Points';
/** Byte width of the `name` column of `Derivation_Equation_Constants`. */
const CONSTANT_NAME_BYTES = 64;

function rejectUnknown(tree: ExtensionTree, path: string, children: readonly string[], datasets: readonly string[], tables: readonly string[]): void {
  for (const n of Object.keys(tree.children)) {
    if (!children.includes(n)) throw validation(joinPath(path, n), 'unexpected child group');
  }
  for (const n of Object.keys(tree.datasets)) {
    if (!datasets.includes(n)) throw validation(joinPath(path, n), 'unexpected dataset');
  }
  for (const n of Object.keys(tree.tables ?? {})) {
    if (!tables.includes(n)) throw validation(joinPath(path, n), 'unexpected table');
  }
}

// ---------------------------------------------------------------- tables

function expectFields(table: ExtensionTable, path: string, expected: ReadonlyArray<readonly [string, string]>): void {
  const same =
    table.fields.length === expected.length && table.fields.every((f, i) => f.name === expected[i][0] && f.dtype === expected[i][1]);
  if (!same) {
    const want = expected.map(([n, d]) => `${n}:${d}`).join(', ');
    throw validation(path, `unexpected table fields; expected ${want}`);
  }
  if (Object.keys(table.attrs).length > 0) throw validation(path, 'table attributes are not supported');
}

function readConstants(table: ExtensionTable | undefined, path: string): EquationConstant[] {
  if (table === undefined) return [];
  expectFields(table, path, [
    ['name', 'utf8'],
    ['value', 'f64'],
  ]);
  const names = table.columns['name'] as string[];
  const values = table.columns['value'] as Float64Array;
  return names.map((name, i) => ({ name, value: values[i] }));
}

function readPoints(table: ExtensionTable | undefined, path: string): CalibrationPoint[] {
  if (table === undefined) return [];
  expectFields(table, path, [
    ['input_value', 'f64'],
    ['output_value', 'f64'],
  ]);
  const input = table.columns['input_value'] as Float64Array;
  const output = table.columns['output_value'] as Float64Array;
  return Array.from(input, (inputValue, i) => ({ inputValue, outputValue: output[i] }));
}

function constantsTable(rows: EquationConstant[], path: string): ExtensionTable {
  const encoder = new TextEncoder();
  rows.forEach((c, i) => {
    const p = `${path}[${i}]/name`;
    if (c.name.includes('\0')) throw validation(p, 'constant name contains NUL');
    if (encoder.encode(c.name).length > CONSTANT_NAME_BYTES) {
      throw validation(p, `constant name "${c.name}" exceeds ${CONSTANT_NAME_BYTES} UTF-8 bytes`);
    }
  });
  return {
    fields: [
      { name: 'name', dtype: 'utf8', size: CONSTANT_NAME_BYTES },
      { name: 'value', dtype: 'f64' },
    ],
    rows: rows.length,
    columns: { name: rows.map((c) => c.name), value: Float64Array.from(rows, (c) => c.value) },
    attrs: {},
  };
}

function pointsTable(rows: CalibrationPoint[]): ExtensionTable {
  return {
    fields: [
      { name: 'input_value', dtype: 'f64' },
      { name: 'output_value', dtype: 'f64' },
    ],
    rows: rows.length,
    columns: {
      input_value: Float64Array.from(rows, (p) => p.inputValue),
      output_value: Float64Array.from(rows, (p) => p.outputValue),
    },
    attrs: {},
  };
}

// ---------------------------------------------------------------- grids

const GRID_ATTRS = new Set(['dimensions', 'dtype', 'shape']);

function readGrid(ds: ExtensionDataset | undefined, path: string): Grid | undefined {
  if (ds === undefined) return undefined;
  if (ds.dtype !== 'f64') throw validation(path, `expected an f64 dataset, got ${ds.dtype}`);
  if (ds.shape.length !== 3) throw validation(path, `expected a 3-D dataset, got ${ds.shape.length}-D`);
  for (const name of Object.keys(ds.attrs)) {
    if (!GRID_ATTRS.has(name)) throw validation(joinPath(path, `@${name}`), 'unexpected dataset attribute');
  }
  return { shape: [ds.shape[0], ds.shape[1], ds.shape[2]], data: (ds.data as Float64Array).slice() };
}

function gridDataset(grid: Grid, path: string): ExtensionDataset {
  const [a, b, c] = grid.shape;
  if (![a, b, c].every((n) => Number.isInteger(n) && n >= 1)) throw validation(path, `invalid grid shape ${grid.shape.join('x')}`);
  if (!(grid.data instanceof Float64Array)) throw validation(path, 'grid data must be a Float64Array');
  if (grid.data.length !== a * b * c) throw validation(path, `grid has ${grid.data.length} values for shape ${a}x${b}x${c}`);
  return {
    dtype: 'f64',
    shape: [a, b, c],
    data: grid.data.slice(),
    attrs: { dimensions: strAttr('H,W,D'), dtype: strAttr('float64'), shape: strAttr(`${a}x${b}x${c}`) },
  };
}

// ---------------------------------------------------------------- power characterization

function unpackPowerCharacterization(tree: ExtensionTree, path: string): PowerCharacterization {
  rejectUnknown(tree, path, [], [], [CONSTANTS_TABLE, POINTS_TABLE]);
  return {
    algorithmType: readOptString(tree.attrs, 'Algorithm_Type', path),
    algorithmEquation: readOptString(tree.attrs, 'Algorithm_Equation', path),
    inputType: readOptString(tree.attrs, 'Input_Type', path),
    unitsDerivedQuantity: readOptString(tree.attrs, 'Units_Derived_Quantity', path),
    derivationEquationConstants: readConstants(tree.tables?.[CONSTANTS_TABLE], joinPath(path, CONSTANTS_TABLE)),
    characterizationPoints: readPoints(tree.tables?.[POINTS_TABLE], joinPath(path, POINTS_TABLE)),
    extra: takeExtra(tree.attrs, PC_ATTRS),
  };
}

function packPowerCharacterization(pc: PowerCharacterization, path: string): ExtensionTree {
  const tree = emptyTree();
  tree.attrs['Algorithm_Type'] = strAttr(pc.algorithmType);
  tree.attrs['Algorithm_Equation'] = strAttr(pc.algorithmEquation);
  tree.attrs['Input_Type'] = strAttr(pc.inputType);
  tree.attrs['Units_Derived_Quantity'] = strAttr(pc.unitsDerivedQuantity);
  putExtra(tree.attrs, pc.extra, PC_ATTRS, path);
  tree.tables = {
    [CONSTANTS_TABLE]: constantsTable(pc.derivationEquationConstants, joinPath(path, CONSTANTS_TABLE)),
    [POINTS_TABLE]: pointsTable(pc.characterizationPoints),
  };
  return tree;
}

// ---------------------------------------------------------------- synchronous sensors

function unpackSensor(label: string, tree: ExtensionTree, path: string): SynchronousSensor {
  rejectUnknown(tree, path, [], [], [CONSTANTS_TABLE, SENSOR_POINTS_TABLE]);
  const a = tree.attrs;
  return {
    label,
    enabled: readOptBool(a, 'Enabled', path),
    sensorName: readOptString(a, 'Sensor_Name', path),
    sensorOutputRangeLow: readOptFloat(a, 'Sensor_Output_Range_Low', path),
    sensorOutputRangeHigh: readOptFloat(a, 'Sensor_Output_Range_High', path),
    sensorOutputSpace: readOptString(a, 'Sensor_Output_Space', path),
    sensorModel: readOptString(a, 'Sensor_Model', path),
    sensorManufacturer: readOptString(a, 'Sensor_Manufacturer', path),
    sensorScope: readOptString(a, 'Sensor_Scope', path),
    unitsDerivedQuantity: readOptString(a, 'Units_Derived_Quantity', path),
    portId: readOptInt(a, 'Port_ID', path),
    sensorType: readOptString(a, 'Sensor_Type', path),
    inputType: readOptString(a, 'Input_Type', path),
    algorithmType: readOptString(a, 'Algorithm_Type', path),
    algorithmEquation: readOptString(a, 'Algorithm_Equation', path),
    calibrationSource: readOptString(a, 'Calibration_Source', path),
    calibrationVerified: readOptBool(a, 'Calibration_Verified', path),
    samplePeriod: readOptFloat(a, 'Sample_Period', path),
    metadata: readOptString(a, 'Metadata', path),
    derivationEquationConstants: readConstants(tree.tables?.[CONSTANTS_TABLE], joinPath(path, CONSTANTS_TABLE)),
    calibrationPoints: readPoints(tree.tables?.[SENSOR_POINTS_TABLE], joinPath(path, SENSOR_POINTS_TABLE)),
    extra: takeExtra(a, SENSOR_ATTRS),
  };
}

function packSensor(s: SynchronousSensor, path: string): ExtensionTree {
  const tree = emptyTree();
  const a = tree.attrs;
  a['Enabled'] = boolAttr(s.enabled);
  a['Sensor_Name'] = strAttr(s.sensorName);
  a['Sensor_Output_Range_Low'] = floatAttr(s.sensorOutputRangeLow);
  a['Sensor_Output_Range_High'] = floatAttr(s.sensorOutputRangeHigh);
  a['Sensor_Output_Space'] = strAttr(s.sensorOutputSpace);
  a['Sensor_Model'] = strAttr(s.sensorModel);
  a['Sensor_Manufacturer'] = strAttr(s.sensorManufacturer);
  a['Sensor_Scope'] = strAttr(s.sensorScope);
  a['Units_Derived_Quantity'] = strAttr(s.unitsDerivedQuantity);
  a['Port_ID'] = intAttr(s.portId, joinPath(path, '@Port_ID'));
  a['Sensor_Type'] = strAttr(s.sensorType);
  a['Input_Type'] = strAttr(s.inputType);
  a['Algorithm_Type'] = strAttr(s.algorithmType);
  a['Algorithm_Equation'] = strAttr(s.algorithmEquation);
  a['Calibration_Source'] = strAttr(s.calibrationSource);
  a['Calibration_Verified'] = boolAttr(s.calibrationVerified);
  a['Sample_Period'] = floatAttr(s.samplePeriod);
  a['Metadata'] = strAttr(s.metadata);
  putExtra(a, s.extra, SENSOR_ATTRS, path);
  tree.tables = {
    [CONSTANTS_TABLE]: constantsTable(s.derivationEquationConstants, joinPath(path, CONSTANTS_TABLE)),
    [SENSOR_POINTS_TABLE]: pointsTable(s.calibrationPoints),
  };
  return tree;
}

// ---------------------------------------------------------------- units

function unpackLegacyFields(attrs: Record<string, ExtensionAttr>, path: string): LegacyUnitFields | undefined {
  if (!LEGACY_ATTRS.some((n) => attrs[n] !== undefined)) return undefined;
  return {
    selectedCamera: readOptString(attrs, 'Selected_Camera', path),
    customVideoFormat: readOptString(attrs, 'Custom_Video_Format', path),
    videoOutput: readOptString(attrs, 'Video_Output', path),
    showConsole: readOptBool(attrs, 'Show_Console', path),
    correctionGridDomainShape: readOptString(attrs, 'Correction_Grid_Domain_Shape', path),
    inverseGridDomainShape: readOptString(attrs, 'Inverse_Grid_Domain_Shape', path),
  };
}

function unpackUnit(trainId: string, tree: ExtensionTree): ClearBoxUnit {
  const path = trainId;
  rejectUnknown(tree, path, [SENSORS_GROUP, PC_GROUP], [CORRECTION, INVERSE_CORRECTION], []);
  const a = tree.attrs;

  const sensors: SynchronousSensor[] = [];
  const sensorsTree = tree.children[SENSORS_GROUP];
  if (sensorsTree !== undefined) {
    const sp = joinPath(path, SENSORS_GROUP);
    if (Object.keys(sensorsTree.attrs).length > 0) throw validation(sp, 'attributes on the sensors group are not supported');
    rejectUnknown(sensorsTree, sp, Object.keys(sensorsTree.children), [], []);
    for (const label of Object.keys(sensorsTree.children).sort(compareBytes)) {
      sensors.push(unpackSensor(label, sensorsTree.children[label], joinPath(sp, label)));
    }
  }
  const pcTree = tree.children[PC_GROUP];

  return {
    trainId,
    ipAddress: readRequiredString(a, 'Ip_Address', path),
    serialNumber: readOptString(a, 'Serial_Number', path),
    dataPort: readOptInt(a, 'Data_Port', path),
    serverPort: readOptInt(a, 'Server_Port', path),
    actualTimingOffset: readOptInt(a, 'Actual_Timing_Offset', path),
    commandedTimingOffset: readOptInt(a, 'Commanded_Timing_Offset', path),
    manufacturer: readOptString(a, 'Manufacturer', path),
    model: readOptString(a, 'Model', path),
    firmwareVersion: readOptString(a, 'Firmware_Version', path),
    legacy: unpackLegacyFields(a, path),
    correctionData: readGrid(tree.datasets[CORRECTION], joinPath(path, CORRECTION)),
    inverseCorrectionData: readGrid(tree.datasets[INVERSE_CORRECTION], joinPath(path, INVERSE_CORRECTION)),
    synchronousSensors: sensors,
    powerCharacterization: pcTree === undefined ? undefined : unpackPowerCharacterization(pcTree, joinPath(path, PC_GROUP)),
    extra: takeExtra(a, UNIT_ATTRS),
  };
}

function packUnit(u: ClearBoxUnit): ExtensionTree {
  const path = u.trainId;
  nodeName(u.trainId, path);
  const tree = emptyTree();
  const a = tree.attrs;
  a['Ip_Address'] = strAttr(u.ipAddress);
  a['Serial_Number'] = strAttr(u.serialNumber);
  a['Data_Port'] = intAttr(u.dataPort, joinPath(path, '@Data_Port'));
  a['Server_Port'] = intAttr(u.serverPort, joinPath(path, '@Server_Port'));
  a['Actual_Timing_Offset'] = intAttr(u.actualTimingOffset, joinPath(path, '@Actual_Timing_Offset'));
  a['Commanded_Timing_Offset'] = intAttr(u.commandedTimingOffset, joinPath(path, '@Commanded_Timing_Offset'));
  a['Manufacturer'] = strAttr(u.manufacturer);
  a['Model'] = strAttr(u.model);
  // File_Version 1.1 always has the attribute; the 1.0 shape (a `legacy` group) never does.
  if (u.firmwareVersion !== undefined || u.legacy === undefined) a['Firmware_Version'] = strAttr(u.firmwareVersion);
  if (u.legacy !== undefined) {
    a['Selected_Camera'] = strAttr(u.legacy.selectedCamera);
    a['Custom_Video_Format'] = strAttr(u.legacy.customVideoFormat);
    a['Video_Output'] = strAttr(u.legacy.videoOutput);
    a['Show_Console'] = boolAttr(u.legacy.showConsole);
    a['Correction_Grid_Domain_Shape'] = strAttr(u.legacy.correctionGridDomainShape);
    a['Inverse_Grid_Domain_Shape'] = strAttr(u.legacy.inverseGridDomainShape);
  }
  putExtra(a, u.extra, UNIT_ATTRS, path);

  if (u.correctionData !== undefined) tree.datasets[CORRECTION] = gridDataset(u.correctionData, joinPath(path, CORRECTION));
  if (u.inverseCorrectionData !== undefined) {
    tree.datasets[INVERSE_CORRECTION] = gridDataset(u.inverseCorrectionData, joinPath(path, INVERSE_CORRECTION));
  }

  if (u.synchronousSensors.length > 0) {
    const sp = joinPath(path, SENSORS_GROUP);
    const group = emptyTree();
    for (const s of u.synchronousSensors) {
      const sensorPath = joinPath(sp, s.label);
      nodeName(s.label, sensorPath);
      if (group.children[s.label] !== undefined) throw validation(sensorPath, 'duplicate sensor label');
      group.children[s.label] = packSensor(s, sensorPath);
    }
    tree.children[SENSORS_GROUP] = group;
  }
  if (u.powerCharacterization !== undefined) {
    tree.children[PC_GROUP] = packPowerCharacterization(u.powerCharacterization, joinPath(path, PC_GROUP));
  }
  return tree;
}

// ---------------------------------------------------------------- root

export function unpackLegacy(tree: ExtensionTree): ClearBoxConfig {
  rejectUnknown(tree, '', Object.keys(tree.children), [], []);
  const units = Object.keys(tree.children)
    .sort(compareBytes)
    .map((trainId) => unpackUnit(trainId, tree.children[trainId]));
  return {
    outputPath: readOptString(tree.attrs, 'Output_Path', ''),
    softwareTriggerDelay: readOptInt(tree.attrs, 'Software_Trigger_Delay', ''),
    units,
    extra: takeExtra(tree.attrs, ROOT_ATTRS),
  };
}

export function packLegacy(config: ClearBoxConfig): ExtensionTree {
  const tree = emptyTree();
  tree.attrs['Output_Path'] = strAttr(config.outputPath);
  tree.attrs['Software_Trigger_Delay'] = intAttr(config.softwareTriggerDelay, '@Software_Trigger_Delay');
  putExtra(tree.attrs, config.extra, ROOT_ATTRS, '');
  for (const unit of config.units) {
    if (tree.children[unit.trainId] !== undefined) throw validation(unit.trainId, 'duplicate train id');
    tree.children[unit.trainId] = packUnit(unit);
  }
  return tree;
}
