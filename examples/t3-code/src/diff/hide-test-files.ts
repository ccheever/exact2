// The Changes panel's and a pull request Code tab's one "Hide test files" choice (T3 Code 6d450a4284, MIT, see
// LICENSE-T3: components/diffs/useHideTestFiles.ts, local storage "t3code.diffHideTestFiles", default off), kept in
// the client's preference file as `diffHideTestFiles`. Hiding only changes what shows: a file asked for (a reveal, a
// turn diff's named file) stays listed.
import type { Obj } from '../client/domain';
import { isTestChangedFile } from './changed-file-kind';

type Holder = { local: object };
type Stored = { diffHideTestFiles?: boolean };

export const hideTestFiles = (client: Holder): boolean => (client.local as Stored).diffHideTestFiles === true;
export function setHideTestFiles(client: Holder, hidden: boolean): void { (client.local as Stored).diffHideTestFiles = hidden; }
/** load(): the saved choice, off unless it was saved on. */
export function adoptHideTestFiles(next: object, saved: Obj): void { if (saved.diffHideTestFiles === true) (next as Stored).diffHideTestFiles = true; }

/** The files a list shows: all of them, or, while tests are hidden, all but the tests other than `kept`. */
export function shownDiffFiles<T>(files: readonly T[], pathOf: (file: T) => string, hidden: boolean, kept: string): T[] {
  return hidden ? files.filter(file => { const path = pathOf(file); return path === kept || !isTestChangedFile(path); }) : [...files];
}
export const testFileCount = <T>(files: readonly T[], pathOf: (file: T) => string): number => files.filter(file => isTestChangedFile(pathOf(file))).length;
