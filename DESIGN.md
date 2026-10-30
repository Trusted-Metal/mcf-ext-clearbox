# mcf-ext-clearbox - design

`mcf-ext-clearbox` packs and unpacks the ClearBox extension (`ClearBox`) of a machine configuration
file. It is a pure codec: it never opens a file and has no HDF5 dependency. The Machine Config
Library (MCF core) owns all reads and writes; since core stopped modelling ClearBox it carries the
data as an opaque extension tree and knows nothing about what is inside.

The extension contract (tree, dtypes, tables, canonical digest) and the rules every extension
follows are in core's `docs/EXTENSION_CONTRACT.md` and in `mcf-ext-opcua/DESIGN.md` sections 2-3.
This document only covers what is specific to ClearBox.

Status: Node and Rust implemented against contract 0.1.1 (compound tables are required, which is why
`REQUIRES_CONTRACT` is `^0.1.1`). Python, Go and C++ ports are not started.

## 1. Why this exists

Core's typed `ClearBox` model (`OpticalTrain.optional_components.clearbox` and the get/set facade
calls) was removed so that rdt-core and clearbox-tauri consume ClearBox through one versioned codec
instead of each depending on core's internal model. A lot of behavior lived in that model and in
core's v1.0 and v1.1 readers and writers; it is preserved here, and the preservation is tested
against real files (section 6).

## 2. The tree core hands to the codec

Identical for File_Version 1.0 and 1.1 files: core builds it from each train's
`Optional_Components/ClearBox` for 1.0 and reads `Extensions/ClearBox` for 1.1.

```text
ClearBox/                              attrs: Output_Path, Software_Trigger_Delay   (machine-wide)
  Optical_Train_NN/                    attrs: Ip_Address, Serial_Number, Data_Port, Server_Port,
                                              Actual_Timing_Offset, Commanded_Timing_Offset,
                                              Manufacturer, Model
                                              1.1 only: Firmware_Version
                                              1.0 only: Selected_Camera, Custom_Video_Format,
                                                        Video_Output, Show_Console,
                                                        Correction_Grid_Domain_Shape,
                                                        Inverse_Grid_Domain_Shape
    Correction_Data                    f64 [h, w, 2]; attrs dimensions="H,W,D", dtype, shape="HxWx2"
    Inverse_Correction_Data            same
    Synchronous_Sensors/<label>/       optional; attrs below; tables Derivation_Equation_Constants,
                                       Calibration_Points
    Power_Characterization/            attrs Algorithm_Type, Algorithm_Equation, Input_Type,
                                       Units_Derived_Quantity; tables Derivation_Equation_Constants
                                       {name utf8(64), value f64}, Characterization_Points
                                       {input_value, output_value}
```

`Output_Path` and `Software_Trigger_Delay` are machine-wide. ClearBox reads them from the first
optical train of an older file; having a ClearBox group (and these values) in every train was a
design mistake that let them drift. Core keeps the first train's values when it builds the tree.

Real files store integers as `i32`. An absent number or flag is stored as an empty `utf8` string.

## 3. Domain model

Pack always writes the layout above; unpack always returns the model. Identical in Node and Rust
(Node `camelCase`, Rust `snake_case`; the fixtures' `model.json` is the shared JSON form).

```text
ClearBoxConfig { outputPath?, softwareTriggerDelay?, units: ClearBoxUnit[], extra }
ClearBoxUnit   { trainId, ipAddress, serialNumber?, dataPort?, serverPort?, actualTimingOffset?,
                 commandedTimingOffset?, manufacturer?, model?, firmwareVersion?,
                 legacy?: { selectedCamera?, customVideoFormat?, videoOutput?, showConsole?,
                            correctionGridDomainShape?, inverseGridDomainShape? },
                 correctionData?: Grid, inverseCorrectionData?: Grid,
                 synchronousSensors: SynchronousSensor[], powerCharacterization?, extra }
Grid           { shape: [h, w, 2], data: f64[] }        row-major, NaN outside the correction field
PowerCharacterization { algorithmType?, algorithmEquation?, inputType?, unitsDerivedQuantity?,
                        derivationEquationConstants: {name, value}[],
                        characterizationPoints: {inputValue, outputValue}[], extra }
SynchronousSensor { label, enabled?, sensorName?, ..., derivationEquationConstants,
                    calibrationPoints, extra }
```

| On disk | Model | Rule |
| --- | --- | --- |
| `""` or whitespace in an optional text attribute | absent | read trimmed; written as `""` |
| number or flag stored as `""` | absent | written as `""` when absent, as the typed writers did |
| integer attribute, any integer dtype | number | also accepts an integral float or numeric text; written as `i32` (out of range is an error) |
| flag `0` / `1` | `bool` | any other value is an error; written as `i32` 0/1 |
| `Ip_Address` | `ipAddress` | required (missing is an error), kept as stored, may be empty |
| numeric attribute where text is expected | its decimal text | e.g. `Serial_Number` stored as an integer |
| `Correction_Data` / `Inverse_Correction_Data` | `Grid` | f64, rank 3; NaN kept as NaN; absent stays absent (never zero-filled) |
| attributes core 1.0 groups have and 1.1 groups do not | `legacy` | present iff any of the six was stored; written only when present, so a 1.1 tree is not given empty 1.0 attributes |
| `Firmware_Version` (1.1) | `firmwareVersion` | always written for a unit without a `legacy` group (the 1.1 shape, empty when absent); never added to a 1.0-shaped unit |
| coefficient order in `Derivation_Equation_Constants` | list order | row order is significant, not sorted |
| units and sensors | sorted by name | order on disk is not significant |
| attributes not listed above | `extra` | kept with their dtype at every level; a name the codec writes itself is an error on pack |
| a group, dataset or table not listed above | error | it cannot be preserved, so it is rejected with its path instead of being dropped |
| empty `Synchronous_Sensors` group | no sensors | not written back |

Errors carry a code (`Validation`, `UnsupportedSchemaVersion`, `LossyDowngrade`) and a path
(`Optical_Train_01/Power_Characterization/Derivation_Equation_Constants`, attributes prefixed `@`).

### Where this deliberately differs from core's old typed model

- `Ip_Address` missing is an error (the typed reader defaulted to `""`).
- Grids are a flat `Float64Array` / `Vec<f64>`, not nested lists with `null` for NaN, and an absent
  grid is omitted (the typed writers wrote zeros).
- `Output_Path` and `Software_Trigger_Delay` are machine-wide, not per train.
- Integers are written as `i32`, as real files have them (the typed writers wrote `i64`).
- Unknown groups, datasets and tables are rejected; the typed model dropped them silently.

## 4. Schema versions

Core's files carry no schema version for ClearBox, so today there is exactly one layout, the
unversioned one (`peekSchemaVersion` returns `'legacy'`), and `supportedSchemaVersions()` is empty.
A `Schema_Version` attribute on the root is reserved: `unpack` rejects it with
`UnsupportedSchemaVersion`, and `pack` accepts only `targetSchema: 'legacy'`.

When the first layout-breaking change arrives it becomes `1.0` of the versioned scheme described in
`mcf-ext-opcua/DESIGN.md` section 3, and core's 1.0 writer needs a rule for the extra root attribute
(it currently rejects every root attribute except the two shared values, so a non-empty root `extra`
also cannot be written to a 1.0 file).

## 5. Public interface

Same shape as `mcf-ext-opcua`:

```ts
export const EXTENSION_NAME = 'ClearBox';
export const REQUIRES_CONTRACT = '^0.1.1';
export function supportedSchemaVersions(): string[];
export function peekSchemaVersion(tree: ExtensionTree): string | 'legacy';
export function unpack(tree: ExtensionTree): Result<ClearBoxConfig, ExtensionError>;
export function pack(config: ClearBoxConfig, opts?: { targetSchema?: string }): Result<ExtensionTree, ExtensionError>;
```

The tree types are a structural copy of the contract (including `tables`); core is never imported and
Node has no runtime dependencies. The Rust crate depends on `serde` and `serde_json` (for the
contract's tree JSON form, which is also how a host in another language hands trees over).

## 6. Testing

- `fixtures/legacy/<case>/{tree.json, model.json}`: trees extracted from core's reference files by
  `tools/extract_fixture.py` (grids replaced by a 3x3x2 stand-in with NaN so fixtures stay small),
  covering a 1.0 file, a 1.1 file and a 1.0 file with a synchronous sensor. Both languages assert
  `unpack(tree) == model` and `pack(model) == tree`, so the two codecs cannot drift.
- Behavior, error and round-trip tests in each language, including a real-size 257x257x2 grid with NaN.
- **Parity with the old typed model.** `tools/prepare_parity.py` runs the last core version that
  still had the typed ClearBox and the current one over a directory of real machine configs;
  `nodejs/tests/parity.test.ts` (set `MCF_PARITY_DIR`) asserts every field, sensor, power
  characterization and grid cell the old model returned is what the codec returns. 35 real machine
  configs (Aconity MIDI and MINI, URSA, Trumpf, Rowan, OWEGO, GMPPU) matched exactly; the only
  differences allowed are drifted machine-wide values, which are reported.
- **Node and Rust on real trees.** `node tools/unpack_all.mjs <dir>` then `cargo test --test corpus`
  (set `MCF_PARITY_DIR`): all 37 real ClearBox trees unpack to the same model in both languages, and
  `pack(unpack(tree))` is stable.
- **Core integration (dev only).** `tools/core_integration.py` writes the packed tree through core in
  the file's own File_Version, reads it back and compares it with core's round trip of the original
  tree (empty attributes ignored). All 37 real trees pass for both 1.0 and 1.1 files. Core is not a
  runtime dependency.
- **Core integration suites.** With core checked out next to this repo, real reader/writer calls
  and this codec run in one process, per language:
  `MCF_CORE_DIR=<core> npm test -- core.integration` (Node, needs core's `nodejs/dist` built) and
  `cargo test --manifest-path integration/core-rust/Cargo.toml` (Rust, needs HDF5 on the path).
  They cover every core fixture that carries ClearBox, mock configs, editing a model and writing it
  back, adding ClearBox to a file without one, trees the codec rejects (core must keep them
  verbatim), and 1.0 <-> 1.1 conversion. Writing to File_Version 1.0 is lossy by design
  (`Firmware_Version`, `Input_Type`, `Units_Derived_Quantity` and `Characterization_Points` have no
  1.0 location); the tests assert that exact loss and nothing more. CI runs both on ubuntu against
  the public core repo (`.github/workflows/core-integration.yml`, ref from the `core_ref` dispatch
input, default `main`).

## 7. Repository and release

Same as `mcf-ext-opcua`: `nodejs/` package and `rust/` crate on one lockstep version, semantic-release
with `rc` pre-releases from `main`, Node distributed as a release-asset tarball and Rust as the git
tag or `.crate` asset. No dependency on core or on `mcf-extension-contract`.

## 8. Open questions

- The Rust crate declares its own copy of the tree types, so a host that already holds core's
  `mcf_extension_contract::ExtensionTree` (clearbox-tauri) must convert; the tree JSON form is the
  bridge today. Depending on `mcf-extension-contract` instead would remove the copy.
- `Ip_Address` is strict (missing is an error). If real files turn up without it, relax it to `""`.
- Trimming optional text drops significant whitespace; it matches how core read these attributes.
- Python, Go and C++ ports are deferred, as for `mcf-ext-opcua`.
- `LightSource.power_characterization` stays in core; only ClearBox's own copy is handled here.
