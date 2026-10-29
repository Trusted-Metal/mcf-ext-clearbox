import type { ExtensionAttr } from './contract.js';

/** Attributes the codec does not model, kept verbatim (name -> attribute with its stored dtype). */
export type Extra = Record<string, ExtensionAttr>;

/** Row-major f64 grid; `NaN` marks points outside the correction field. */
export interface Grid {
  shape: [number, number, number];
  data: Float64Array;
}

export interface EquationConstant {
  name: string;
  value: number;
}

export interface CalibrationPoint {
  inputValue: number;
  outputValue: number;
}

export interface PowerCharacterization {
  algorithmType?: string;
  algorithmEquation?: string;
  inputType?: string;
  unitsDerivedQuantity?: string;
  /** Row order is significant (coefficient position). */
  derivationEquationConstants: EquationConstant[];
  characterizationPoints: CalibrationPoint[];
  extra: Extra;
}

export interface SynchronousSensor {
  /** Free-form group name under `Synchronous_Sensors/`. */
  label: string;
  enabled?: boolean;
  sensorName?: string;
  sensorOutputRangeLow?: number;
  sensorOutputRangeHigh?: number;
  sensorOutputSpace?: string;
  sensorModel?: string;
  sensorManufacturer?: string;
  sensorScope?: string;
  unitsDerivedQuantity?: string;
  portId?: number;
  sensorType?: string;
  inputType?: string;
  algorithmType?: string;
  algorithmEquation?: string;
  calibrationSource?: string;
  calibrationVerified?: boolean;
  samplePeriod?: number;
  metadata?: string;
  derivationEquationConstants: EquationConstant[];
  calibrationPoints: CalibrationPoint[];
  extra: Extra;
}

/** Per-train attributes that only File_Version 1.0 ClearBox groups have. Present iff any was stored. */
export interface LegacyUnitFields {
  selectedCamera?: string;
  customVideoFormat?: string;
  videoOutput?: string;
  showConsole?: boolean;
  correctionGridDomainShape?: string;
  inverseGridDomainShape?: string;
}

export interface ClearBoxUnit {
  /** Group name under `ClearBox/`, e.g. `Optical_Train_01`. */
  trainId: string;
  ipAddress: string;
  serialNumber?: string;
  dataPort?: number;
  serverPort?: number;
  actualTimingOffset?: number;
  commandedTimingOffset?: number;
  manufacturer?: string;
  model?: string;
  /** File_Version 1.1 attribute. */
  firmwareVersion?: string;
  legacy?: LegacyUnitFields;
  correctionData?: Grid;
  inverseCorrectionData?: Grid;
  /** Sorted by label. */
  synchronousSensors: SynchronousSensor[];
  powerCharacterization?: PowerCharacterization;
  extra: Extra;
}

export interface ClearBoxConfig {
  /** Machine-wide: ClearBox reads it from the first optical train of a 1.0 file. */
  outputPath?: string;
  /** Machine-wide, milliseconds. */
  softwareTriggerDelay?: number;
  /** Sorted by `trainId`. */
  units: ClearBoxUnit[];
  extra: Extra;
}
