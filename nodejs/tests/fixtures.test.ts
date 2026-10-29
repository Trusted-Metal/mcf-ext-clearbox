import { describe, expect, it } from 'vitest';
import { pack, unpack } from '../src/index.js';
import { fixtureModel, fixtureNames, fixtureTree, hasModel, writeModel } from './helpers/fixtures.js';
import { treeToJson } from './helpers/treeJson.js';

describe.each(fixtureNames('legacy'))('legacy fixture %s', (name) => {
  const rel = `legacy/${name}`;

  it('unpacks to the stored model', () => {
    const r = unpack(fixtureTree(rel));
    if (!r.ok) throw r.error;
    if (process.env.UPDATE_FIXTURES && !hasModel(rel)) writeModel(rel, r.value);
    expect(r.value).toEqual(fixtureModel(rel));
  });

  it('packs the stored model back to the same tree', () => {
    const r = pack(fixtureModel(rel));
    if (!r.ok) throw r.error;
    expect(treeToJson(r.value)).toEqual(treeToJson(fixtureTree(rel)));
  });
});
