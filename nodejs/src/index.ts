import { packLegacy, unpackLegacy } from './adapters/legacy.js';
import { type ExtensionTree } from './contract.js';
import { attempt, ExtensionError, type Result } from './errors.js';
import type { ClearBoxConfig } from './model.js';

export { EXTENSION_NAME, REQUIRES_CONTRACT, emptyTree } from './contract.js';
export type {
  ExtensionAttr,
  ExtensionDataset,
  ExtensionDtype,
  ExtensionNumericArray,
  ExtensionScalar,
  ExtensionTable,
  ExtensionTableField,
  ExtensionTree,
} from './contract.js';
export { ExtensionError } from './errors.js';
export type { ExtensionErrorCode, Result } from './errors.js';
export type * from './model.js';

/** The only layout today is the unversioned one core writes; a stored version attribute is rejected. */
const SCHEMA_VERSION_ATTR = 'Schema_Version';

/** Schema versions `pack` can write. Empty: the unversioned (`'legacy'`) layout is the only one. */
export function supportedSchemaVersions(): string[] {
  return [];
}

/** The root version attribute as stored; `'legacy'` when it is absent. */
export function peekSchemaVersion(tree: ExtensionTree): string | 'legacy' {
  const a = tree.attrs[SCHEMA_VERSION_ATTR];
  if (a === undefined) return 'legacy';
  return Array.isArray(a.value) ? a.value.map(String).join(',') : String(a.value);
}

export function unpack(tree: ExtensionTree): Result<ClearBoxConfig, ExtensionError> {
  return attempt(() => {
    if (tree.attrs[SCHEMA_VERSION_ATTR] !== undefined) {
      const v = peekSchemaVersion(tree);
      throw new ExtensionError('UnsupportedSchemaVersion', `@${SCHEMA_VERSION_ATTR}`, `unsupported schema version "${v}"`);
    }
    return unpackLegacy(tree);
  });
}

/** Writes the unversioned layout; any other `targetSchema` is `UnsupportedSchemaVersion`. */
export function pack(config: ClearBoxConfig, opts: { targetSchema?: string } = {}): Result<ExtensionTree, ExtensionError> {
  return attempt(() => {
    const target = opts.targetSchema ?? 'legacy';
    if (target !== 'legacy') {
      throw new ExtensionError('UnsupportedSchemaVersion', '', `cannot pack schema version "${target}"; supported: legacy`);
    }
    return packLegacy(config);
  });
}
