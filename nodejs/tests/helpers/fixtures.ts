import { existsSync, readdirSync, readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import type { ExtensionTree } from '../../src/contract.js';
import type { ClearBoxConfig } from '../../src/model.js';
import { cloneTree, loadTree, modelFromJson, modelToJson } from './treeJson.js';

export const FIXTURES = join(fileURLToPath(new URL('.', import.meta.url)), '../../../fixtures');

export function fixtureNames(group: string): string[] {
  return readdirSync(join(FIXTURES, group), { withFileTypes: true })
    .filter((d) => d.isDirectory())
    .map((d) => d.name)
    .sort();
}

export function fixtureTree(rel: string): ExtensionTree {
  return loadTree(join(FIXTURES, rel, 'tree.json'));
}

export function fixtureModel(rel: string): ClearBoxConfig {
  return modelFromJson<ClearBoxConfig>(JSON.parse(readFileSync(join(FIXTURES, rel, 'model.json'), 'utf8')));
}

export function hasModel(rel: string): boolean {
  return existsSync(join(FIXTURES, rel, 'model.json'));
}

export function writeModel(rel: string, model: ClearBoxConfig): void {
  writeFileSync(join(FIXTURES, rel, 'model.json'), JSON.stringify(modelToJson(model), null, 2) + '\n');
}

/** A fresh copy of a fixture tree to mutate in error tests. */
export function freshTree(rel: string): ExtensionTree {
  return cloneTree(fixtureTree(rel));
}

export function group(tree: ExtensionTree, ...names: string[]): ExtensionTree {
  let g = tree;
  for (const n of names) g = g.children[n];
  return g;
}
