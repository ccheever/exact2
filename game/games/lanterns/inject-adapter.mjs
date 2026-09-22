import {readFileSync, writeFileSync} from 'node:fs';
import {resolve} from 'node:path';
import {publicFileCards} from '../../../host/web/serve.mjs';

const TAG = '<script type="module" src="./assets/lanterns-adapter.mjs"></script>';

export function installAdapter(dist) {
  const path = resolve(dist, 'index.html');
  const html = readFileSync(path, 'utf8');
  if (!html.includes(TAG)) writeFileSync(path, `${html.trimEnd()}\n${TAG}\n`);
  // The consumer's deterministic post-build step is part of its complete web
  // artifact, so refresh the completion marker after injecting the one script.
  const markerPath = resolve(dist, '.exact-build.json');
  const marker = JSON.parse(readFileSync(markerPath, 'utf8'));
  marker.files = publicFileCards(dist);
  writeFileSync(markerPath, JSON.stringify(marker) + '\n');
}

if (import.meta.main) installAdapter(process.argv[2]);
