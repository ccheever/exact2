import {mkdirSync, writeFileSync} from 'node:fs';
import {dirname, resolve} from 'node:path';

export function writeFixtureFile(root, file, contents) {
  const path = resolve(root, file);
  mkdirSync(dirname(path), {recursive:true});
  writeFileSync(path, contents);
  return path;
}
