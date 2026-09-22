import { test } from 'bun:test';
import assert from 'node:assert/strict';
import { mkdtempSync, mkdirSync, writeFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';
import { productCandidates, builtProduct, macTriple } from './build.mjs';

// SwiftPM's native build system nests a product under the triple; Xcode's classic
// one writes it into `release/`. A toolchain switch leaves both: the build must
// never hand over the stale one.
test('a built product is resolved from exactly one SwiftPM layout', () => {
  const root = mkdtempSync(resolve(tmpdir(), 'apple-products-'));
  try {
    const [nested, flat] = productCandidates('ExactMac', macTriple, root);
    assert.equal(nested, resolve(root, macTriple, 'release', 'ExactMac'));
    assert.equal(flat, resolve(root, 'release', 'ExactMac'));
    assert.throws(() => builtProduct('ExactMac', macTriple, root), /0 copies/);
    mkdirSync(resolve(flat, '..'), { recursive: true }); writeFileSync(flat, 'classic');
    assert.equal(builtProduct('ExactMac', macTriple, root), flat);
    mkdirSync(resolve(nested, '..'), { recursive: true }); writeFileSync(nested, 'native');
    assert.throws(() => builtProduct('ExactMac', macTriple, root), /2 copies/);
    rmSync(flat);
    assert.equal(builtProduct('ExactMac', macTriple, root), nested);
  } finally { rmSync(root, { recursive: true, force: true }); }
});
