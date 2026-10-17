// Mobile Review adapter for T3 Code365aa87982 reviewModel.ts (MIT, LICENSE-T3).
// Boundary/name/recovery semantics adapted from @pierre/diffs1.5.2
// utils/parsePatchFiles and parseQuotedDiffFileName, Apache-2.0, license below.
// MODIFIED: handwritten mobile row/marker adapter; no desktop parser or runtime dependency.
// @ref llp/1109.006-review-and-files.decision.md#mobile-presentation
import type { DiffFileModel } from './shared/diff';
type Hunk = DiffFileModel['hunks'][number];
export type ReviewPatchFile = DiffFileModel & { id: string; pureRename: boolean };
const gitNames = /^diff --git ("a\/(?:[^"\\]|\\.)*"|a\/.+?) ("b\/(?:[^"\\]|\\.)*"|b\/.+?)$/;
const hunkRange = /^@@ -(\d+)(?:,(\d+))? \+(\d+)(?:,(\d+))? @@/;
const filename = /^(---|\+\+\+)\s+([^\t\r\n]+)/;
const escapes: Record<string, string> = { '"': '"', '\\': '\\', a: '\x07', b: '\b', t: '\t', n: '\n', v: '\v', f: '\f', r: '\r' };
function decodeName(input: string, git = false): string {
  const raw = input.trim(); let value = raw;
  if (raw.startsWith('"')) {
    let result = '', at = 1, ended = false, invalid = false;
    while (at < raw.length) {
      if (raw[at] === '"') { ended = true; break; }
      if (raw[at] !== '\\') { result += raw[at++]; continue; }
      if (escapes[raw[at + 1]] !== undefined) { result += escapes[raw[at + 1]]; at += 2; continue; }
      const bytes: number[] = [];
      do {
        const octal = raw.slice(at + 1, at + 4);
        if (!/^[0-3][0-7]{2}$/.test(octal)) { invalid = true; break; }
        bytes.push(parseInt(octal, 8)); at += 4;
      } while (raw[at] === '\\' && /^[0-7]$/.test(raw[at + 1] ?? ''));
      if (invalid) break;
      result += new TextDecoder('utf-8', { ignoreBOM: true }).decode(new Uint8Array(bytes));
    }
    if (ended && !invalid) value = result;
  }
  return git && (value.startsWith('a/') || value.startsWith('b/')) ? value.slice(2) : value;
}
function splitPrefix(text: string, prefix: string): string[] {
  const starts: number[] = text.startsWith(prefix) ? [0] : [];
  let at = text.indexOf('\n' + prefix);
  while (at !== -1) { starts.push(at + 1); at = text.indexOf('\n' + prefix, at + 1); }
  if (!starts.length) return [text];
  if (starts[0] !== 0) starts.unshift(0);
  return starts.map((start, i) => text.slice(start, starts[i + 1] ?? text.length));
}
const linesOf = (text: string) => text.match(/[^\n]*\n|[^\n]+$/g) ?? [''];
function unifiedHeader(lines: string[], at: number): boolean {
  return /^---[ \t]+[^ \t\r\n]/.test(lines[at] ?? '') && /^\+\+\+[ \t]+[^ \t\r\n]/.test(lines[at + 1] ?? '');
}
function unifiedParts(text: string): string[] {
  const lines = linesOf(text), parts: string[] = []; let part = '', old = 0, next = 0, opened = false;
  for (let at = 0; at < lines.length; at++) {
    const line = lines[at];
    if (old <= 0 && next <= 0) {
      if (unifiedHeader(lines, at)) {
        if (part) parts.push(part);
        part = line + lines[++at]; opened = true; continue;
      }
      const hunk = opened ? line.match(hunkRange) : null;
      if (hunk) { old = Number(hunk[2] ?? 1); next = Number(hunk[4] ?? 1); }
    } else if (line[0] === ' ') { old = Math.max(0, old - 1); next = Math.max(0, next - 1); }
    else if (line[0] === '-') old = Math.max(0, old - 1);
    else if (line[0] === '+') next = Math.max(0, next - 1);
    part += line;
  }
  parts.push(part); return parts;
}
function fileOf(blob: string, git: boolean, id: string): ReviewPatchFile {
  const file: ReviewPatchFile = { id, path: '', previous: '', status: 'modified', additions: 0, deletions: 0, binary: false, hunks: [], pureRename: false };
  const parts = splitPrefix(blob, '@@ '), headers = linesOf(parts[0]);
  for (const raw of headers) {
    const line = raw.trimEnd(), names = line.startsWith('diff --git') ? line.match(gitNames) : null;
    if (names) { file.path = decodeName(names[2], true); const previous = decodeName(names[1], true); file.previous = previous !== file.path ? previous : ''; continue; }
    const name = line.match(filename);
    if (name) {
      const value = decodeName(name[2], git);
      if (value !== '/dev/null') { if (name[1] === '---') { file.previous = value; file.path = value; } else file.path = value; }
      continue;
    }
    if (git) {
      if (line.startsWith('new file mode')) { file.status = 'added'; file.pureRename = false; }
      else if (line.startsWith('deleted file mode')) { file.status = 'deleted'; file.pureRename = false; }
      else if (line.startsWith('similarity index')) { file.status = 'renamed'; file.pureRename = line.startsWith('similarity index 100%'); }
      else if (line.startsWith('rename from ') || line.startsWith('copy from ')) file.previous = decodeName(line.slice(line.startsWith('rename') ? 12 : 10));
      else if (line.startsWith('rename to ') || line.startsWith('copy to ')) file.path = decodeName(line.slice(line.startsWith('rename') ? 10 : 8));
      else if (line.startsWith('Binary files ') || line === 'GIT binary patch') file.binary = true;
    }
  }
  // The primary accepts the header blob even when its names/hunks are malformed.
  for (const part of parts.slice(1)) {
    const lines = linesOf(part), range = lines[0].match(hunkRange); if (!range) continue;
    let old = Number(range[1]), next = Number(range[3]), oldRead = 0, newRead = 0;
    const oldCount = Number(range[2] ?? 1), newCount = Number(range[4] ?? 1);
    const hunk: Hunk = { oldStart: old, newStart: next, lines: [] };
    while (lines.length > 1 && /^[\r\n]*$/.test(lines.at(-1)!)) lines.pop();
    for (const raw of lines.slice(1)) {
      if (oldRead >= oldCount && newRead >= newCount && raw[0] !== '\\'
        && (!/^[+\- ]/.test(raw) || /^--[ \t\r\n]*$/.test(raw))) break;
      const text = raw.slice(1).replace(/\n$/, '');
      if (raw[0] === '+') { hunk.lines.push({ kind: 'addition', text, old: 0, next: next++ }); file.additions++; newRead++; }
      else if (raw[0] === '-') { hunk.lines.push({ kind: 'deletion', text, old: old++, next: 0 }); file.deletions++; oldRead++; }
      else if (raw[0] === ' ') { hunk.lines.push({ kind: 'context', text, old: old++, next: next++ }); oldRead++; newRead++; }
    }
    if (oldRead !== oldCount || newRead !== newCount) {
      // Pierre repairs both starts from consumed-side boundaries, including a
      // zero declared count. Display coordinates follow the repaired starts;
      // side presence is the line kind, so an actual coordinate 0 stays 0.
      hunk.oldStart = hunk.oldStart - (oldCount === 0 ? 0 : 1) + (oldRead === 0 ? 0 : 1);
      hunk.newStart = hunk.newStart - (newCount === 0 ? 0 : 1) + (newRead === 0 ? 0 : 1);
      let oldLine = hunk.oldStart, newLine = hunk.newStart;
      for (const line of hunk.lines) {
        if (line.kind !== 'addition') line.old = oldLine++;
        if (line.kind !== 'deletion') line.next = newLine++;
      }
    }
    file.hunks.push(hunk);
  }
  if (!git && file.previous && file.previous !== file.path) { file.status = 'renamed'; file.pureRename = file.hunks.length === 0; }
  if (!file.path) file.path = file.previous;
  return file;
}
function hash(text: string, seed: number, multiplier: number): string {
  let value = seed >>> 0;
  for (let at = 0; at < text.length; at++) value = Math.imul(value ^ text.charCodeAt(at), multiplier) >>> 0;
  return value.toString(36);
}
/** Ordinals count accepted patches, including metadata-only patches, and accepted
 * files, including binary or malformed Git headers. Unsupported blobs do not count.
 * Hashes retain interior CRLF and UTF-16, independently of line presentation. */
export function mobileReviewPatchFiles(text: string, sectionId: string): ReviewPatchFile[] {
  if (!text) return [];
  const key = `${sectionId}:${text.length}:${hash(text, 0x811c9dc5, 0x01000193)}:${hash(text, 0x9e3779b9, 0x85ebca6b)}`;
  const patches = text.startsWith('From ') || text.includes('\nFrom ') ? text.split(/(?=^From [a-f0-9]+ .+$)/m) : [text];
  const files: ReviewPatchFile[] = [];
  patches.forEach((patch, patchIndex) => {
    const git = patch.startsWith('diff --git') || patch.includes('\ndiff --git');
    let fileIndex = 0;
    for (const blob of git ? splitPrefix(patch, 'diff --git') : unifiedParts(patch)) {
      if (git ? !blob.startsWith('diff --git') : !unifiedHeader(linesOf(blob), 0)) continue;
      files.push(fileOf(blob, git, 'ck1:' + JSON.stringify(['patch-file', key, String(patchIndex), String(fileIndex++)])));
    }
  });
  return files;
}

/*
                                 Apache License
                           Version 2.0, January 2004
                        http://www.apache.org/licenses/

TERMS AND CONDITIONS FOR USE, REPRODUCTION, AND DISTRIBUTION

1.  Definitions.

    "License" shall mean the terms and conditions for use, reproduction, and
    distribution as defined by Sections 1 through 9 of this document.

    "Licensor" shall mean the copyright owner or entity authorized by the
    copyright owner that is granting the License.

    "Legal Entity" shall mean the union of the acting entity and all other
    entities that control, are controlled by, or are under common control with
    that entity. For the purposes of this definition, "control" means (i) the
    power, direct or indirect, to cause the direction or management of such
    entity, whether by contract or otherwise, or (ii) ownership of fifty percent
    (50%) or more of the outstanding shares, or (iii) beneficial ownership of
    such entity.

    "You" (or "Your") shall mean an individual or Legal Entity exercising
    permissions granted by this License.

    "Source" form shall mean the preferred form for making modifications,
    including but not limited to software source code, documentation source, and
    configuration files.

    "Object" form shall mean any form resulting from mechanical transformation
    or translation of a Source form, including but not limited to compiled
    object code, generated documentation, and conversions to other media types.

    "Work" shall mean the work of authorship, whether in Source or Object form,
    made available under the License, as indicated by a copyright notice that is
    included in or attached to the work (an example is provided in the Appendix
    below).

    "Derivative Works" shall mean any work, whether in Source or Object form,
    that is based on (or derived from) the Work and for which the editorial
    revisions, annotations, elaborations, or other modifications represent, as a
    whole, an original work of authorship. For the purposes of this License,
    Derivative Works shall not include works that remain separable from, or
    merely link (or bind by name) to the interfaces of, the Work and Derivative
    Works thereof.

    "Contribution" shall mean any work of authorship, including the original
    version of the Work and any modifications or additions to that Work or
    Derivative Works thereof, that is intentionally submitted to Licensor for
    inclusion in the Work by the copyright owner or by an individual or Legal
    Entity authorized to submit on behalf of the copyright owner. For the
    purposes of this definition, "submitted" means any form of electronic,
    verbal, or written communication sent to the Licensor or its
    representatives, including but not limited to communication on electronic
    mailing lists, source code control systems, and issue tracking systems that
    are managed by, or on behalf of, the Licensor for the purpose of discussing
    and improving the Work, but excluding communication that is conspicuously
    marked or otherwise designated in writing by the copyright owner as "Not a
    Contribution."

    "Contributor" shall mean Licensor and any individual or Legal Entity on
    behalf of whom a Contribution has been received by Licensor and subsequently
    incorporated within the Work.

2.  Grant of Copyright License. Subject to the terms and conditions of this
    License, each Contributor hereby grants to You a perpetual, worldwide,
    non-exclusive, no-charge, royalty-free, irrevocable copyright license to
    reproduce, prepare Derivative Works of, publicly display, publicly perform,
    sublicense, and distribute the Work and such Derivative Works in Source or
    Object form.

3.  Grant of Patent License. Subject to the terms and conditions of this
    License, each Contributor hereby grants to You a perpetual, worldwide,
    non-exclusive, no-charge, royalty-free, irrevocable (except as stated in
    this section) patent license to make, have made, use, offer to sell, sell,
    import, and otherwise transfer the Work, where such license applies only to
    those patent claims licensable by such Contributor that are necessarily
    infringed by their Contribution(s) alone or by combination of their
    Contribution(s) with the Work to which such Contribution(s) was submitted.
    If You institute patent litigation against any entity (including a
    cross-claim or counterclaim in a lawsuit) alleging that the Work or a
    Contribution incorporated within the Work constitutes direct or contributory
    patent infringement, then any patent licenses granted to You under this
    License for that Work shall terminate as of the date such litigation is
    filed.

4.  Redistribution. You may reproduce and distribute copies of the Work or
    Derivative Works thereof in any medium, with or without modifications, and
    in Source or Object form, provided that You meet the following conditions:

    (a) You must give any other recipients of the Work or Derivative Works a
    copy of this License; and

    (b) You must cause any modified files to carry prominent notices stating
    that You changed the files; and

    (c) You must retain, in the Source form of any Derivative Works that You
    distribute, all copyright, patent, trademark, and attribution notices from
    the Source form of the Work, excluding those notices that do not pertain to
    any part of the Derivative Works; and

    (d) If the Work includes a "NOTICE" text file as part of its distribution,
    then any Derivative Works that You distribute must include a readable copy
    of the attribution notices contained within such NOTICE file, excluding
    those notices that do not pertain to any part of the Derivative Works, in at
    least one of the following places: within a NOTICE text file distributed as
    part of the Derivative Works; within the Source form or documentation, if
    provided along with the Derivative Works; or, within a display generated by
    the Derivative Works, if and wherever such third-party notices normally
    appear. The contents of the NOTICE file are for informational purposes only
    and do not modify the License. You may add Your own attribution notices
    within Derivative Works that You distribute, alongside or as an addendum to
    the NOTICE text from the Work, provided that such additional attribution
    notices cannot be construed as modifying the License.

    You may add Your own copyright statement to Your modifications and may
    provide additional or different license terms and conditions for use,
    reproduction, or distribution of Your modifications, or for any such
    Derivative Works as a whole, provided Your use, reproduction, and
    distribution of the Work otherwise complies with the conditions stated in
    this License.

5.  Submission of Contributions. Unless You explicitly state otherwise, any
    Contribution intentionally submitted for inclusion in the Work by You to the
    Licensor shall be under the terms and conditions of this License, without
    any additional terms or conditions. Notwithstanding the above, nothing
    herein shall supersede or modify the terms of any separate license agreement
    you may have executed with Licensor regarding such Contributions.

6.  Trademarks. This License does not grant permission to use the trade names,
    trademarks, service marks, or product names of the Licensor, except as
    required for reasonable and customary use in describing the origin of the
    Work and reproducing the content of the NOTICE file.

7.  Disclaimer of Warranty. Unless required by applicable law or agreed to in
    writing, Licensor provides the Work (and each Contributor provides its
    Contributions) on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
    KIND, either express or implied, including, without limitation, any
    warranties or conditions of TITLE, NON-INFRINGEMENT, MERCHANTABILITY, or
    FITNESS FOR A PARTICULAR PURPOSE. You are solely responsible for determining
    the appropriateness of using or redistributing the Work and assume any risks
    associated with Your exercise of permissions under this License.

8.  Limitation of Liability. In no event and under no legal theory, whether in
    tort (including negligence), contract, or otherwise, unless required by
    applicable law (such as deliberate and grossly negligent acts) or agreed to
    in writing, shall any Contributor be liable to You for damages, including
    any direct, indirect, special, incidental, or consequential damages of any
    character arising as a result of this License or out of the use or inability
    to use the Work (including but not limited to damages for loss of goodwill,
    work stoppage, computer failure or malfunction, or any and all other
    commercial damages or losses), even if such Contributor has been advised of
    the possibility of such damages.

9.  Accepting Warranty or Additional Liability. While redistributing the Work or
    Derivative Works thereof, You may choose to offer, and charge a fee for,
    acceptance of support, warranty, indemnity, or other liability obligations
    and/or rights consistent with this License. However, in accepting such
    obligations, You may act only on Your own behalf and on Your sole
    responsibility, not on behalf of any other Contributor, and only if You
    agree to indemnify, defend, and hold each Contributor harmless for any
    liability incurred by, or claims asserted against, such Contributor by
    reason of your accepting any such warranty or additional liability.

END OF TERMS AND CONDITIONS

APPENDIX: How to apply the Apache License to your work.

      To apply the Apache License to your work, attach the following
      boilerplate notice, with the fields enclosed by brackets "[]"
      replaced with your own identifying information. (Don't include
      the brackets!)  The text should be enclosed in the appropriate
      comment syntax for the file format. We also recommend that a
      file or class name and description of purpose be included on the
      same "printed page" as the copyright notice for easier
      identification within third-party archives.

Copyright 2025 Pierre Computer Company

Licensed under the Apache License, Version 2.0 (the "License"); you may not use
this file except in compliance with the License. You may obtain a copy of the
License at

       http://www.apache.org/licenses/LICENSE-2.0

Unless required by applicable law or agreed to in writing, software distributed
under the License is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR
CONDITIONS OF ANY KIND, either express or implied. See the License for the
specific language governing permissions and limitations under the License.

*/
