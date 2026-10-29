# mcf-ext-clearbox

Pack/unpack codec for the **ClearBox** extension of a machine configuration file. It is a pure
library: it never opens a file and has no HDF5 dependency. The
[Machine Config Library](https://github.com/Trusted-Metal/Machine_Config_Library) (MCF core) reads and
writes the file and hands this library the opaque `ClearBox` extension tree
(`config.extensions["ClearBox"]`); this library turns it into a typed model and back.

| Package | Directory | Docs |
| --- | --- | --- |
| Node.js (`mcf-ext-clearbox`) | [`nodejs/`](nodejs) | [`nodejs/README.md`](nodejs/README.md) |
| Rust crate (`mcf-ext-clearbox`) | [`rust/`](rust) | `cargo doc --open` |

The design, the tree layout, the domain model and what is preserved from core's old typed ClearBox
model are in [`DESIGN.md`](DESIGN.md).

```ts
import { unpack, pack } from 'mcf-ext-clearbox';

const r = unpack(config.extensions.ClearBox);          // core's tree, either file version
if (!r.ok) throw r.error;                              // { code, path, message }
const clearBox = r.value;                              // ClearBoxConfig
clearBox.units[0].correctionData;                      // { shape: [257, 257, 2], data: Float64Array }

const packed = pack(clearBox);                         // back to a tree core can write
if (!packed.ok) throw packed.error;
config.extensions.ClearBox = packed.value;
```

```rust
use mcf_ext_clearbox::{pack, unpack, PackOptions};

let clear_box = unpack(&tree)?;                        // &ExtensionTree -> ClearBoxConfig
let packed = pack(&clear_box, &PackOptions::default())?;
```

## Development

```text
cd nodejs && npm ci && npm test          # vitest; npm run typecheck
cd rust   && cargo test                  # cargo fmt --check, cargo clippy --all-targets -- -D warnings
```

`fixtures/legacy/*` are trees extracted from core's reference files (`tools/extract_fixture.py`) with
their expected models; both languages read the same files. Checks against real machine configs
(parity with the old typed model, Node vs Rust, core write/read) are described in
[`DESIGN.md`](DESIGN.md) section 6 and run from `tools/`.

Commits follow Conventional Commits; releases are cut by semantic-release (`rc` pre-releases from
`main`). Both packages share one version.

## License

MIT
