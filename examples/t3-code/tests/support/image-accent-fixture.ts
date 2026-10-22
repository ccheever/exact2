// The image chip accent's fixtures (image-accent.test.ts; not a test file). Each `scaled` PNG is what macOS's
// `storage.fs.compressImage(from, to, {maxDimension: 128, type: 'image/png'})` wrote for its source (exact-data's
// image::compress, 2026-10-22), and `native` the accent the app's former Swift module (T3ImageAccent.swift) read from
// the source itself: the picture drawn into 16×16 by Core Graphics and averaged by alpha. `memoryFiles` stands in for
// the app's files.
import { adoptAppFiles } from '../../src/media/save-file';

export interface AccentFixture { name: string; source: string; native: string; scaled: string }

export const ACCENT_FIXTURES: AccentFixture[] = [
  { name: 'photo', source: 'photo.png below, 320×200', native: '129 140 168',
    scaled: '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' },
  { name: 'halves', source: '32×32, left half opaque red, right half transparent', native: '255 0 0',
    scaled: 'iVBORw0KGgoAAAANSUhEUgAAACAAAAAgCAYAAABzenr0AAAAAXNSR0IArs4c6QAAAERlWElmTU0AKgAAAAgAAYdpAAQAAAABAAAAGgAAAAAAA6' +
      'ABAAMAAAABAAEAAKACAAQAAAABAAAAIKADAAQAAAABAAAAIAAAAACshmLzAAAAOElEQVRYCe3QwQkAMAwCQNP9d24zg/R54leQm5ts+0yy7XP6' +
      '6Z+lAwQIECBAgAABAgQIECBAgMADUJ4CQCuNqrEAAAAASUVORK5CYII=' },
  { name: 'split', source: '32×32, left half red, right half blue', native: '128 0 128',
    scaled: 'iVBORw0KGgoAAAANSUhEUgAAACAAAAAgCAYAAABzenr0AAAAAXNSR0IArs4c6QAAAERlWElmTU0AKgAAAAgAAYdpAAQAAAABAAAAGgAAAAAAA6' +
      'ABAAMAAAABAAEAAKACAAQAAAABAAAAIKADAAQAAAABAAAAIAAAAACshmLzAAAAOElEQVRYCe3QsQ0AMAgDsND/f4beQDs6yhqBXJ3c7lNv85z9' +
      '6T9LDxAgQIAAAQIECBAgQIAAAQIDDtUDPk0xSUAAAAAASUVORK5CYII=' },
  { name: 'alpha-ramp', source: 'a 120×40 ramp, red to blue, alpha 0 to 1', native: '171 153 84',
    scaled: 'iVBORw0KGgoAAAANSUhEUgAAAHgAAAAoCAYAAAA16j4lAAAAAXNSR0IArs4c6QAAAERlWElmTU0AKgAAAAgAAYdpAAQAAAABAAAAGgAAAAAAA6' +
      'ABAAMAAAABAAEAAKACAAQAAAABAAAAeKADAAQAAAABAAAAKAAAAAAw8ZolAAABoklEQVR4Ae1TQW4DIRADD9tEyq3qf/q1Pja39l6lWaAeNlXz' +
      'BssrbVgY8GA7riWfj4nyXlA++V6vUfqGt9dA3wOjA+MCXHpF//6K8+DaGTiNG8YAXk5AH3e0nz22ecfYgJgdbe6IxrP8xn6LmADaYI2Hci04lo' +
      'HKJhvXam7IcU5s/K7EKqNH4zxriVdmQYt97cHYickaiFUKeGlE5cvvmjXOS80ePMM+XGRtLMzsm/McWyFeZe9cGyTHeTzmiZG1HLNWV49jbxT2' +
      'Zr/jXOc57vubP+FTQN7lv1/uj9WbZycxyYEirLstrLxn6lCIydrBifcg/uJA7fhN7jz/4HBgLjzWBmvP/ZbD/lFVgH9wP8oK2GBld8nNBttgcQ' +
      'XE6TnBNlhcAXF6TrANFldAnJ4TbIPFFRCn5wTbYHEFxOk5wTZYXAFxek6wDRZXQJyeE2yDxRUQp+cE22BxBcTpOcE2WFwBcXpOsA0WV0CcnhNs' +
      'g8UVEKfnBNtgcQXE6TnBNlhcAXF6TrANFldAnJ4TbIPFFRCn5wTbYHEFxOk5wTZYXAFxer8F+pznHKvC5AAAAABJRU5ErkJggg==' },
  { name: 'caltrain', source: 'apps/caltrain/assets/caltrain.png (text on transparency)', native: '167 100 98',
    scaled: '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' },
  { name: 'srgb-16bit', source: 'apps/color-gallery/assets/srgb-16bit.png (16-bit RGB)', native: '102 112 115',
    scaled: 'iVBORw0KGgoAAAANSUhEUgAAAIAAAAAgCAYAAADaInAlAAAAAXNSR0IArs4c6QAAAERlWElmTU0AKgAAAAgAAYdpAAQAAAABAAAAGgAAAAAAA6' +
      'ABAAMAAAABAAEAAKACAAQAAAABAAAAgKADAAQAAAABAAAAIAAAAAA4qZvhAAABD0lEQVR4Ae2UMQrCQAAE9zSCGmLpDwRrfYHPEGyt9RNin9q/' +
      '+AJtLBQEX2AXbAwkpkljlaSdPQIhyd2F2Rs2lFJ1dR/Li3RddFxf9KT5Q3rOOm0Q9b/arbeaxG+VZb/VHkVZaDROtD+eNBzFrdbWk7MsU5qmyv' +
      'NcIYT6daN7qP6fRYnO01Wj+f+TQjTQ53XT/bD5/9TquToBD3ICFoB8+hW7BbAA8ATg+G4ACwBPAI7vBrAA8ATg+G4ACwBPAI7vBrAA8ATg+G4A' +
      'CwBPAI7vBrAA8ATg+G4ACwBPAI7vBrAA8ATg+G4ACwBPAI7vBrAA8ATg+G4ACwBPAI7vBrAA8ATg+G4ACwBPAI7vBoAL8AOO+iE/zdwOjAAAAA' +
      'BJRU5ErkJggg==' },
  { name: 'p3-camera', source: 'apps/color-gallery/assets/p3-camera.jpg (Display P3 JPEG)', native: '96 112 115',
    scaled: 'iVBORw0KGgoAAAANSUhEUgAAAIAAAAAgCAYAAADaInAlAAAAAXNSR0IArs4c6QAAAERlWElmTU0AKgAAAAgAAYdpAAQAAAABAAAAGgAAAAAAA6' +
      'ABAAMAAAABAAEAAKACAAQAAAABAAAAgKADAAQAAAABAAAAIAAAAAA4qZvhAAAA7UlEQVR4Ae2SsRHCMADEHNbIEOxBxxBZgiouWIJBWA88Q1Tp' +
      'Tr60cvz/2n5jrO/62RC9/gsveB+P649f5OvzRfycE/Hn/kT8OO6IvyE6WN9AAugnZAESgPWnpxNAPyELkACsPz2dAPoJWYAEYP3p6QTQT8gCJA' +
      'DrT08ngH5CFiABWH96OgH0E7IACcD609MJoJ+QBUgA1p+eTgD9hCxAArD+9HQC6CdkARKA9aenE0A/IQuQAKw/PZ0A+glZgARg/enpBNBPyAIk' +
      'AOtPTyeAfkIWIAFYf3o6AfQTsgAJwPrT0wmgn5AFSADWn57+A5nTCj9Hox8EAAAAAElFTkSuQmCC' },
  { name: 'adobe-rgb', source: 'apps/color-gallery/assets/adobe-rgb.jpg (Adobe RGB JPEG)', native: '97 112 115',
    scaled: 'iVBORw0KGgoAAAANSUhEUgAAAIAAAAAgCAYAAADaInAlAAAAAXNSR0IArs4c6QAAAERlWElmTU0AKgAAAAgAAYdpAAQAAAABAAAAGgAAAAAAA6' +
      'ABAAMAAAABAAEAAKACAAQAAAABAAAAgKADAAQAAAABAAAAIAAAAAA4qZvhAAAA7klEQVR4Ae2SwQ3CMADEUqboGsyBxBgwQd9IebIBgzAfZIb6' +
      'ZclRv05zd95+Y6zv/NkQvf4LL3g/bucfv8jj80X8nBPxr/2O+PG8Iv6C6GB9Awmgn5AFSADWn55OAP2ELEACsP70dALoJ2QBEoD1p6cTQD8hC5' +
      'AArD89nQD6CVmABGD96ekE0E/IAiQA609PJ4B+QhYgAVh/ejoB9BOyAAnA+tPTCaCfkAVIANafnk4A/YQsQAKw/vR0AugnZAESgPWnpxNAPyEL' +
      'kACsPz2dAPoJWYAEYP3p6QTQT8gCJADrT08ngH5CFiABWH96+g+bUwo/XHvYwQAAAABJRU5ErkJggg==' },
  { name: 'transparent', source: '32×32, fully transparent', native: '',
    scaled: 'iVBORw0KGgoAAAANSUhEUgAAACAAAAAgCAYAAABzenr0AAAAAXNSR0IArs4c6QAAAERlWElmTU0AKgAAAAgAAYdpAAQAAAABAAAAGgAAAAAAA6' +
      'ABAAMAAAABAAEAAKACAAQAAAABAAAAIKADAAQAAAABAAAAIAAAAACshmLzAAAAKElEQVRYCe3QgQAAAADDoPlTH+SFUGHAgAEDBgwYMGDAgAED' +
      'Bgy8DwwQIAABziq1xwAAAABJRU5ErkJggg==' },
];

/** photo.png, the attachment fixture: a blue card with a yellow square and a red circle. Chrome's averageImageColor
 *  reads it as rgb(129 140 168). */
export const PHOTO_PNG = 'iVBORw0KGgoAAAANSUhEUgAAAUAAAADICAIAAAAWZq/8AAAD2UlEQVR4nO3cwW1TQRRA0YCozSuKYD1FsEgRWVMEq9RCMSyMLIEQRIg/M/f/c9' +
  'ZJPO/LV29kS3l3e/72BDS9X30A4N8JGMIEDGEChjABQ5iAIUzAECZgCBMwhAkYwgQMYQKGMAFDmIAhTMAQJmAIEzCECRjCBAxhAoYwAUOYgCFM' +
  'wBAmYAgTMIQJGMIEDGEChjABQ5iAIUzAECZgCBMwhAkYwgQMYQKGMAFDmIAhTMAQ9mHVC3+9fVr10rzdx9cvq4/An9jAECZgCBMwhAkYwgQMYQ' +
  'KGMAFDmIAhTMAQJmAIEzCECRjCBAxhAoYwAUOYgCFMwBAmYAgTMIQt+59Y8N+9vH5+y4+N2/PTWQiY8xf7599K9yxgrtLtX/9asWQBc9Fuz1Gy' +
  'gAk4Ot3fvlwiYwGztcnp5jIWMJtamG4oY98Ds6NN6t32PA82MHvZNpWXLVexDcxGtq132xMKmF3s1kbinK7QrLdVEq3rtA3MYrl6tzq5gLl6A+' +
  'nzC5jrvvtPMIWAWeMc9S6fRcAQJmAWONP6XTuRgJntfPUunEvATHXWeldNJ2AIEzDznHv9LplRwExyhXrnTypgCBMwM1xn/U6eV8AQJmAOd7X1' +
  'O3NqAUOYgDnWNdfvtNkFDGEChjABc6Ar35/nPAEBQ5iAIUzAHMX9ecJzEDCECRjCBAxhAoYwAXMIn2DNeRoChjABQ5iAIUzAECZgCBMwhAkYwg' +
  'QMYQKGMAFDmIAhTMAQJmAOMW7Pq49wiachYAgTMIQJGMIEDGEC5ig+x5rwHAQMYQKGMAFzILfocfATEDCECRjCBMyxrnyLHsfPLmAIEzCHu+YS' +
  'HlOmFjCECZgZrraEx6x5BQxhAmaS6yzhMXFSATPPFRoec2cUMIQJmKnOvYTH9OkEzGxnbXismEvALHC+hseiiQQMYQJmjTMt4bFuFgGzzDkaHk' +
  'unEDAr1Rseq88vYK7eQPrkH1YfAH6U8PL6+SlibJDunQ3MLvapInROAbORrdpInNAVmr1se50em6V7ZwOzo91qGZud58EGZlObrOKxa7p3AmZr' +
  'CzMee6d7J2ACJmc8CuneCZiMR1cHlTw63T4ImKuXPILdPgiYsPFze2/sOV3sL97dnr+tPgPwj3wPDGEChjABQ5iAIUzAECZgCBMwhAkYwgQMYQ' +
  'KGMAFDmIAhTMAQJmAIEzCECRjCBAxhAoYwAUOYgCFMwBAmYAgTMIQJGMIEDGEChjABQ5iAIUzAECZgCBMwhAkYwgQMYQKGMAFDmIAhTMAQJmAI' +
  'EzCECRjCBAxhAoYwAUOYgCFMwBAmYAgTMIQJGJ66vgMQpb77qSizrgAAAABJRU5ErkJggg==';

/** The app's files in memory; compressImage "scales" by writing `scaled`, or fails with `failure` when it is null. */
export function memoryFiles(scaled: Uint8Array | null, failure = 'undecodable') {
  const files = new Map<string, Uint8Array>(), scaledFrom: string[] = [];
  adoptAppFiles({ fs: {
    async readFile(path: string) { const found = files.get(path); if (!found) throw new Error('ENOENT'); return found.slice().buffer; },
    async writeFile(path: string, data: Uint8Array) { files.set(path, new Uint8Array(data)); },
    async mkdir() {},
    async rm(path: string) { files.delete(path); },
    async compressImage(from: string, to: string, options: { maxDimension: number; type: string }) {
      if (!files.has(from)) throw new Error('ENOENT');
      scaledFrom.push(`${options.maxDimension} ${options.type}`);
      if (!scaled) throw Object.assign(new Error(`compressImage: ${failure}`), { code: failure });
      files.set(to, scaled);
      return { path: to, type: options.type, size: scaled.length, width: 128, height: 80 };
    },
  } });
  return { files, scaledFrom };
}

/** The scaled photo.png, whose accent is "129 140 168". */
export const scaledPhoto = () => new Uint8Array(Buffer.from(ACCENT_FIXTURES[0]!.scaled, 'base64'));
