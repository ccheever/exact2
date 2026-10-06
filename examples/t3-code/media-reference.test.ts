// Ported from T3 Code 1e2ecbd975 packages/client-runtime/src/mediaReference.test.ts (MIT; see LICENSE-T3).
import { describe, expect, it } from 'bun:test';
import { mediaFileReference, mediaReferenceFileName, mediaUrlReference, type MediaReference } from './media-reference';

describe('mediaFileReference', () => {
  it.each([
    ['/work/project/./media/../clip.mp4', '/work/project/', 'clip.mp4'],
    ['/work/project/../outside.mp4', '/work/project', undefined],
    ['/work/project-other/clip.mp4', '/work/project', undefined],
    ['/work/Project/clip.mp4', '/work/project', undefined],
    ['/work/project/a\\b.mp4', '/work/project', 'a\\b.mp4'],
    ['/work/clip.mp4', '/work/other/..', 'clip.mp4'],
    ['/clip.mp4', '/', 'clip.mp4'],
    ['C:\\WORK\\project\\media\\..\\Clip.mp4', 'c:/work/project/', 'Clip.mp4'],
    ['D:\\work\\clip.mp4', 'C:\\work', undefined],
    ['\\\\Server\\Share\\Project\\Clip.mp4', '//server/share/project', 'Clip.mp4'],
    ['\\\\server\\other\\clip.mp4', '\\\\server\\share', undefined],
    ['../clip.mp4', '/work/project', undefined],
  ] as const)('preserves %s and only labels paths inside %s as relative', (path, root, relativePath) => {
    expect(mediaFileReference(path, root)).toEqual({ kind: 'file', path, ...(relativePath === undefined ? {} : { relativePath }) });
  });
});

describe('mediaReferenceFileName', () => {
  it.each([
    [{ kind: 'file', path: '/tmp/take\\one%20.mp4' }, 'take\\one%20.mp4'],
    [{ kind: 'file', path: 'C:\\clips/take\\one.mp4' }, 'one.mp4'],
    [{ kind: 'file', path: '\\\\server\\share\\one.mp4' }, 'one.mp4'],
    [{ kind: 'url', url: '//cdn.example/clip%20one%2Emp4?sig=a+b#t=2' }, 'clip one.mp4'],
    [{ kind: 'url', url: 'https://cdn.example/clip%2520.mp4' }, 'clip%20.mp4'],
    [{ kind: 'url', url: 'https://cdn.example/clip%20%oops.mp4' }, 'clip%20%oops.mp4'],
  ] as const)('preserves filename semantics for %j', (reference, name) => {
    expect(mediaReferenceFileName(reference as MediaReference)).toBe(name);
  });
});

describe('mediaUrlReference', () => {
  it('keeps http(s) and protocol-relative URLs as authored and refuses the rest', () => {
    expect(mediaUrlReference('https://cdn.example/a.png')).toEqual({ kind: 'url', url: 'https://cdn.example/a.png' });
    expect(mediaUrlReference('//cdn.example/a.png')).toEqual({ kind: 'url', url: '//cdn.example/a.png' });
    expect(mediaUrlReference('data:image/png;base64,AAAA')).toBeUndefined();
    expect(mediaUrlReference('/tmp/a.png')).toBeUndefined();
  });
});
