# mcf-ext-clearbox (Node.js)

Pack/unpack codec for the ClearBox extension of a machine configuration file. Pure functions, no
runtime dependencies; the extension tree types are a structural copy of the extension contract
(`^0.1.1`), so the Machine Config Library is never imported.

```ts
import { unpack, pack, EXTENSION_NAME, REQUIRES_CONTRACT } from 'mcf-ext-clearbox';

const r = unpack(config.extensions[EXTENSION_NAME]);
if (!r.ok) throw r.error;                       // ExtensionError { code, path, message }

const unit = r.value.units[0];
unit.ipAddress;                                  // required, as stored
unit.powerCharacterization?.derivationEquationConstants;
unit.correctionData;                             // { shape: [h, w, 2], data: Float64Array } | undefined

const packed = pack(r.value);
if (!packed.ok) throw packed.error;
config.extensions[EXTENSION_NAME] = packed.value;
```

- `unpack` accepts the layout core writes for File_Version 1.0 and 1.1 files; a `Schema_Version`
  attribute is rejected as `UnsupportedSchemaVersion`.
- `pack` writes that layout back. Absent numbers and flags are written as empty strings, integers as
  `i32`, and every group, dataset or table the model does not cover is an error on `unpack` rather
  than being dropped.
- Errors carry a `code` (`Validation`, `UnsupportedSchemaVersion`, `LossyDowngrade`) and a `path`
  such as `Optical_Train_01/@Data_Port`.

See [`DESIGN.md`](../DESIGN.md) for the tree layout, the model and the mapping rules.

## Scripts

```text
npm test          # vitest
npm run typecheck # src and tests
npm run build     # dist/
```

Set `MCF_PARITY_DIR` to run `tests/parity.test.ts` against real configs (see `tools/prepare_parity.py`).
