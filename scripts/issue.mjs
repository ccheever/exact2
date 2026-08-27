#!/usr/bin/env bun
/**
 * cdcstack filesystem-issue tool: scaffold, close, validate, mechanically repair.
 *
 * @system @ref docs/issues.md — the format this tool enforces
 *
 * Why this exists: `observability-issues-format` went red on main from four
 * separate lanes on 2026-08-26 alone, and three of those four authors HAD
 * written a resolution — they put it where a human reads it (on the `Status:`
 * line, or in a `## Resolution` body section) instead of in the metadata line
 * the checker parses. That is a tooling defect, not four author mistakes:
 * nothing told them the rule at the moment they needed it, and the diagnostic
 * named a different fact than the one that would have fixed the file.
 *
 * So this tool has three jobs, cheapest-first:
 *   1. `check`  — diagnostics that TEACH: found vs. required, at a line number,
 *                 with the exact block to paste.
 *   2. `check --fix` — repair the purely mechanical cases.
 *   3. `new` / `close` — write the metadata so it is never hand-typed.
 *
 * `scripts/check-issues-format.mjs` (registered as `observability-issues-format`)
 * is a thin entry point over `runFormatCheck()` here, so there is exactly one
 * implementation of the rules.
 *
 * The rules themselves are unchanged from cdcstack. Nothing here relaxes what a
 * conformant issue is; it only stops rejecting equivalent surface syntax (a
 * leading `- ` on a metadata line) and makes conformance cheap to reach.
 *
 * THE SAFETY CONTRACT for every write path (`--fix` and `close`): author text is
 * MOVED, never invented, reworded, truncated, or dropped. Where a value cannot
 * be relocated safely the tool refuses and says so — a silently lost sentence is
 * worse than a red.
 */

import { spawnSync } from 'node:child_process';
import { existsSync, mkdirSync, readFileSync, renameSync, writeFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';

const REQUIRED_FIELDS = ['Status', 'Systems', 'Author', 'Date'];
const SCORE_FIELDS = ['Impact', 'Urgency', 'Ease', 'Confidence', 'Score reviewed', 'Score rationale'];
const SCORE_DIMENSIONS = ['Impact', 'Urgency', 'Ease', 'Confidence'];

/** Closure words authors reach for instead of the single legal token `Closed`. */
const CLOSURE_SYNONYMS = /^(closed|resolved|fixed|done|complete|completed)\b/i;
/**
 * Separators between a status token and the qualifier appended to it.
 * Deliberately excludes parentheses: a parenthesised qualifier is unwrapped by
 * an exact whole-value match instead, so `Closed (fixed) and (verified)` is not
 * mangled into `fixed) and (verified`.
 */
const QUALIFIER_LEAD = /^[\s—–:;,.\-]+/;

/**
 * The pre-LLP-0361 header shape: a plain bullet list with no bold markers
 * (`- Status: open`). Nine issues in this shape landed on main on 2026-08-26
 * from two lanes whose authors never saw the check run, so the failure
 * population has two halves — a misplaced field, and an older format entirely.
 * `--fix` normalises the second mechanically: the label changes, the author's
 * value never does.
 */
const LEGACY_BULLET_LINE = /^([-*+][ \t]+)([A-Z][A-Za-z ]*):[ \t]*(.*)$/;
/** Legacy labels that mean a cdcstack field under a different name. */
const LEGACY_FIELD_ALIASES = new Map([['Area', 'Systems']]);
const KNOWN_FIELD_NAMES = new Set([
  ...REQUIRED_FIELDS,
  ...SCORE_FIELDS,
  ...LEGACY_FIELD_ALIASES.keys(),
  'Resolution',
  'Severity',
  'Related',
  'Marker',
  'Opened',
  'Filed',
]);

/**
 * Locate a contiguous run of legacy bullet metadata in the header region.
 * Returns null unless the run carries at least two known field names, so a
 * body list such as `- Note: something` is never mistaken for a header.
 */
export function findLegacyHeader(parsed) {
  const { lines, bodyStart } = parsed;
  for (let index = 0; index < bodyStart; index += 1) {
    const match = LEGACY_BULLET_LINE.exec(lines[index]);
    if (!match || !KNOWN_FIELD_NAMES.has(match[2])) continue;
    const rows = [];
    let cursor = index;
    while (cursor < bodyStart) {
      const row = LEGACY_BULLET_LINE.exec(lines[cursor]);
      if (row && KNOWN_FIELD_NAMES.has(row[2])) {
        rows.push({ index: cursor, marker: row[1], name: row[2], value: row[3] });
        cursor += 1;
        continue;
      }
      if (rows.length > 0 && isContinuation(lines[cursor]) && /^\s/.test(lines[cursor])) {
        cursor += 1;
        continue;
      }
      break;
    }
    return rows.length >= 2 ? { rows } : null;
  }
  return null;
}

// ---------------------------------------------------------------------------
// Parsing
// ---------------------------------------------------------------------------

/**
 * Metadata lines are `**Name:** value`, optionally preceded by a markdown list
 * marker. cdcstack specifies the field names and their values, not whether the
 * header block is rendered as a list — so `- **Status:** Open` carries exactly
 * the same meaning as `**Status:** Open` and is parsed identically. (Before
 * this, a bulleted header block read as "missing Status / missing Systems /
 * missing Author / missing Date": four diagnostics, none of which named the
 * bullet.)
 *
 * The captured name is NOT trimmed, so `** Impact:** 3` stays unrecognised
 * exactly as it was under the previous whole-file regex — widening that would
 * put new reds on issues that pass today.
 */
const METADATA_LINE = /^(?:[-*+][ \t]+)?\*\*([^*:]+):\*\*[ \t]*(.*?)[ \t]*$/;

function isHeading(line) {
  return /^#{1,6}\s/.test(line);
}

/**
 * A metadata value may wrap onto following lines — indented (`  more text`) or,
 * under markdown lazy continuation, not indented at all. Inside the metadata
 * block, any non-blank line that is neither a heading nor another metadata line
 * continues the value above it.
 */
function isContinuation(line) {
  return line.trim() !== '' && !isHeading(line) && !METADATA_LINE.test(line);
}

export function parseIssue(text) {
  const lines = text.split(/\r?\n/);
  const eol = text.includes('\r\n') ? '\r\n' : '\n';

  let blockStart = -1;
  for (let index = 0; index < lines.length; index += 1) {
    if (METADATA_LINE.test(lines[index])) {
      blockStart = index;
      break;
    }
  }
  let blockEnd = blockStart < 0 ? 0 : blockStart;
  while (blockEnd < lines.length && (METADATA_LINE.test(lines[blockEnd]) || isContinuation(lines[blockEnd]))) {
    blockEnd += 1;
  }
  // Two different regions, for two different jobs. `blockStart`/`blockEnd` is
  // the CONTIGUOUS run that decides which lines continue a value. `bodyStart`
  // is where prose begins, and decides whether a metadata line is the issue's
  // own header (safe to rewrite) or a quotation inside the body (never touch).
  let bodyStart = lines.length;
  for (let index = 0; index < lines.length; index += 1) {
    if (/^#{2,6}\s/.test(lines[index])) {
      bodyStart = index;
      break;
    }
  }

  const fields = new Map();
  for (let index = 0; index < lines.length; index += 1) {
    const match = METADATA_LINE.exec(lines[index]);
    if (!match) continue;
    const name = match[1];
    if (fields.has(name)) continue; // first occurrence wins, as before

    const inHeader = index >= blockStart && index < blockEnd;
    let endIndex = index;
    const limit = inHeader ? blockEnd : lines.length;
    while (endIndex + 1 < limit && isContinuation(lines[endIndex + 1])) endIndex += 1;
    const continuations = lines.slice(index + 1, endIndex + 1).map((line) => line.trim());
    // The previous whole-file regex let `\s*` cross a newline, so an empty
    // `**Systems:**` took its value from the following line. Preserved, or
    // issues that are green today would go red.
    const value = match[2] !== '' ? match[2] : (continuations[0] ?? '');
    const fullValue = [match[2], ...continuations].filter(Boolean).join(' ');

    fields.set(name, {
      name,
      value,
      fullValue,
      line: index + 1,
      endLine: endIndex + 1,
      marker: /^([-*+][ \t]+)/.exec(lines[index])?.[1] ?? '',
      inHeader,
      beforeBody: index < bodyStart,
    });
  }
  // Where a newly inserted metadata line belongs: after the last header field.
  let headerEnd = blockEnd;
  for (const entry of fields.values()) {
    if (entry.beforeBody && entry.endLine > headerEnd) headerEnd = entry.endLine;
  }
  return { lines, eol, blockStart, blockEnd, bodyStart, headerEnd, fields };
}

export function fieldValue(parsed, name) {
  return parsed.fields.get(name)?.value ?? '';
}

/** Locate a `## Resolution` body section, if the author wrote one. */
export function findResolutionSection(parsed) {
  const { lines } = parsed;
  for (let index = 0; index < lines.length; index += 1) {
    if (!/^#{2,6}\s+resolution\b/i.test(lines[index])) continue;
    const body = [];
    for (let cursor = index + 1; cursor < lines.length; cursor += 1) {
      if (isHeading(lines[cursor])) break;
      body.push(lines[cursor]);
    }
    const paragraph = body.join('\n').trim().split(/\n\s*\n/)[0]?.trim() ?? '';
    return { line: index + 1, paragraph, firstSentence: firstSentence(paragraph) };
  }
  return null;
}

function firstSentence(paragraph) {
  if (!paragraph) return '';
  const flattened = paragraph.replace(/\s*\n\s*/g, ' ').trim();
  const match = /^(.+?[.!?])(\s|$)/.exec(flattened);
  return (match ? match[1] : flattened).trim();
}

function isIsoDate(value) {
  if (!/^\d{4}-\d{2}-\d{2}$/.test(value)) return false;
  const parsed = new Date(`${value}T00:00:00Z`);
  return Number.isFinite(parsed.getTime()) && parsed.toISOString().slice(0, 10) === value;
}

// ---------------------------------------------------------------------------
// Validation
// ---------------------------------------------------------------------------

/**
 * @returns {Array<{path:string,line?:number,code:string,message:string,found?:string,required?:string,hint?:string,fixable:boolean}>}
 */
export function validateIssue(path, text) {
  const problems = [];
  const closed = path.startsWith('issues/closed/');
  const directory = closed ? 'issues/closed/' : 'issues/';
  const expectedStatus = closed ? 'Closed' : 'Open';
  const push = (problem) => problems.push({ path, fixable: false, ...problem });

  if (!/^issues\/(?:closed\/)?\d{8}-[a-z0-9-]+\.md$/.test(path)) {
    push({
      code: 'filename',
      message: 'Filename must be `YYYYMMDD-slug.md` (lowercase, hyphenated, no sequential number).',
      found: path,
      hint: 'cdcstack "Naming": the date prefix is the filing date; the slug is under ~6 words.',
    });
  }

  const parsed = parseIssue(text);
  const status = parsed.fields.get('Status');
  // A whole header in the pre-LLP-0361 plain-bullet shape reads as "every field
  // is missing". Say what is actually wrong, once, instead of four times.
  const legacy = REQUIRED_FIELDS.some((field) => parsed.fields.has(field)) ? null : findLegacyHeader(parsed);
  if (legacy) {
    // One accurate diagnostic instead of four inaccurate ones. Reporting
    // "missing Status" about a file whose line 3 reads `- Status: open` is how
    // this check taught nine authors nothing.
    const names = legacy.rows.map((row) => row.name);
    push({
      code: 'legacy:header',
      line: legacy.rows[0].index + 1,
      message: `The header is in the pre-LLP-0361 plain-bullet shape, so no field is visible to the checker — the required fields are not missing, they are unmarked.`,
      found: parsed.lines[legacy.rows[0].index].trim(),
      required: `**${legacy.rows[0].name === 'Area' ? 'Systems' : legacy.rows[0].name}:** ${legacy.rows[0].value}`,
      fixable: true,
      hint:
        `All ${legacy.rows.length} bullet fields (${names.join(', ')}) need \`**Name:**\` markers. ` +
        '`node scripts/issue.mjs check --fix` rewrites the labels — including `Area` -> `Systems` — and never touches your values. ' +
        'Anything still missing afterwards is reported field by field.',
    });
    return problems;
  }

  for (const field of REQUIRED_FIELDS) {
    if (fieldValue(parsed, field)) continue;
    if (field === 'Status') continue; // reported once, with its value, below
    const entry = parsed.fields.get(field);
    const derivedDate = field === 'Date' ? dateFromFilename(path) : null;
    push({
      code: entry ? `empty:${field}` : `missing:${field}`,
      line: entry?.line,
      message: entry
        ? `**${field}:** is present but its value is empty.`
        : `Missing required **${field}:** metadata line.`,
      required: `**${field}:** ${placeholderFor(field, path)}`,
      fixable: field === 'Date' && Boolean(derivedDate),
      hint:
        field === 'Date'
          ? derivedDate
            ? 'The filing date. `--fix` fills it from the filename prefix.'
            : 'The filing date. This filename carries no usable date, so a human has to write it.'
          : field === 'Systems'
            ? 'Comma-separated subsystems, e.g. `Verification, Tooling`. A machine cannot infer this.'
            : 'Who filed it, e.g. `Claude Opus 5, directed by Charlie Cheever`.',
    });
  }

  if (!status || !status.value) {
    push({
      code: status ? 'empty:Status' : 'missing:Status',
      line: status?.line,
      message: status
        ? '**Status:** is present but its value is empty.'
        : 'Missing required **Status:** metadata line.',
      required: `**Status:** ${expectedStatus}`,
      fixable: true,
      hint: `The path decides the value: a file under \`${directory}\` is ${expectedStatus}.`,
    });
  } else if (status.value !== expectedStatus) {
    const { remainder, recognized } = splitStatus(status.fullValue, expectedStatus);
    push({
      code: 'status:value',
      line: status.line,
      message: `**Status:** must be exactly \`${expectedStatus}\` on the ${closed ? 'closed' : 'open'} path — the value is the whole rest of the line, so a qualifier makes it a different status.`,
      found: `**Status:** ${status.fullValue}`,
      required: `**Status:** ${expectedStatus}`,
      fixable: recognized,
      hint: recognized
        ? closed
          ? `\`--fix\` rewrites the token to \`Closed\` and moves ${remainder ? `\`${truncate(remainder)}\`` : 'any qualifier'} verbatim into **Resolution:** (or into a labelled body note if **Resolution:** is already set). It never rewords it.`
          : `\`--fix\` rewrites the token to \`Open\` and moves ${remainder ? `\`${truncate(remainder)}\`` : 'any qualifier'} verbatim into a labelled body note directly under the header.`
        : closed
          ? 'cdcstack issue statuses are exactly `Open` and `Closed`. Set it to `Closed` and put the detail in **Resolution:**.'
          : 'This issue is under `issues/` but its Status reads as closed. Either move it properly — `node scripts/issue.mjs close <path> --resolution "…"` — or set **Status:** Open.',
    });
  }

  if (closed && !fieldValue(parsed, 'Resolution')) {
    const section = findResolutionSection(parsed);
    push({
      code: 'missing:Resolution',
      message: 'An issue under `issues/closed/` must carry a one-line **Resolution:** metadata line.',
      required: '**Resolution:** <fixed-by commit/PR | "obsolete" | "graduated to <tracker-id>">',
      fixable: false,
      hint: section
        ? `This file HAS a \`## Resolution\` section at line ${section.line}, but the checker reads metadata lines only — never body sections. Your own first sentence, ready to paste into the header block:\n\n    **Resolution:** ${section.firstSentence || '<one line>'}\n\n\`--fix\` will not write this for you; \`--fix --lift-resolution\` copies that sentence verbatim if you have read it and agree.`
        : 'Put it in the header block, directly under **Status:**. `--fix` will not write a resolution — a machine-invented one is worse than a red.',
    });
  }

  const presentScoreFields = SCORE_FIELDS.filter((field) => parsed.fields.has(field));
  if (presentScoreFields.length > 0) {
    for (const field of SCORE_FIELDS) {
      if (parsed.fields.has(field)) continue;
      push({
        code: `score:missing:${field}`,
        message: `The optional priority score is all-or-none: **${field}:** is missing while ${presentScoreFields.map((name) => `**${name}:**`).join(', ')} ${presentScoreFields.length === 1 ? 'is' : 'are'} present.`,
        required: `**${field}:** ${field === 'Score reviewed' ? new Date().toISOString().slice(0, 10) : field === 'Score rationale' ? '<one sentence>' : '<1-5>'}`,
        hint: 'Either complete the set (docs/issue-priority-scoring.md) or delete the partial fields — both are conformant.',
      });
    }
    for (const field of SCORE_DIMENSIONS) {
      const entry = parsed.fields.get(field);
      if (!entry || /^[1-5]$/.test(entry.value)) continue;
      push({
        code: `score:range:${field}`,
        line: entry.line,
        message: `**${field}:** must be a bare integer from 1 to 5.`,
        found: `**${field}:** ${entry.value}`,
        required: `**${field}:** 3`,
      });
    }
    const reviewed = parsed.fields.get('Score reviewed');
    if (reviewed && !isIsoDate(reviewed.value)) {
      push({
        code: 'score:reviewed',
        line: reviewed.line,
        message: '**Score reviewed:** must be a valid `YYYY-MM-DD` date.',
        found: `**Score reviewed:** ${reviewed.value}`,
        required: `**Score reviewed:** ${new Date().toISOString().slice(0, 10)}`,
      });
    }
    const rationale = parsed.fields.get('Score rationale');
    if (rationale && !rationale.value) {
      push({
        code: 'score:rationale',
        line: rationale.line,
        message: '**Score rationale:** must be non-empty (one sentence).',
        required: '**Score rationale:** <one sentence saying why this ranks where it does>',
      });
    }
  }

  return problems;
}

function placeholderFor(field, path) {
  if (field === 'Date') return dateFromFilename(path) ?? 'YYYY-MM-DD';
  if (field === 'Systems') return '<comma-separated subsystems>';
  if (field === 'Author') return '<who filed it>';
  return '<value>';
}

function dateFromFilename(path) {
  const match = /(\d{4})(\d{2})(\d{2})-/.exec(path.split('/').pop() ?? '');
  if (!match) return null;
  const iso = `${match[1]}-${match[2]}-${match[3]}`;
  return isIsoDate(iso) ? iso : null;
}

/**
 * Split a Status value into the legal token and the qualifier appended to it.
 *
 * Only the EXPECTED token is ever removed. A closure synonym (`Resolved`,
 * `Fixed`, …) carries the KIND of resolution, which is exactly what
 * `Resolution:` is for, so the whole value survives as the remainder rather
 * than losing its first word. `recognized: false` means the value is not a
 * status word at all and moving text around would be guessing.
 */
export function splitStatus(value, expected) {
  const exact = new RegExp(`^${expected}\\b`, 'i');
  if (exact.test(value)) {
    // `Closed (fixed on main)` -> `fixed on main`, but only when the remainder
    // is exactly one balanced parenthesised group.
    const wrapped = new RegExp(`^${expected}\\s*\\(([^()]*)\\)$`, 'i').exec(value);
    if (wrapped) return { remainder: wrapped[1].trim(), recognized: true };
    return { remainder: value.replace(exact, '').replace(QUALIFIER_LEAD, '').trim(), recognized: true };
  }
  if (expected === 'Closed' && CLOSURE_SYNONYMS.test(value)) {
    return { remainder: value.trim(), recognized: true };
  }
  return { remainder: value, recognized: false };
}

function truncate(text, limit = 72) {
  return text.length > limit ? `${text.slice(0, limit - 1)}…` : text;
}

// ---------------------------------------------------------------------------
// Line-level editing primitives
// ---------------------------------------------------------------------------

/** Replace a field AND its continuation lines with one metadata line. */
function replaceField(lines, entry, value) {
  const next = [...lines];
  next.splice(entry.line - 1, entry.endLine - entry.line + 1, `${entry.marker}**${entry.name}:** ${value}`);
  return next;
}

function detectMarker(parsed) {
  for (const entry of parsed.fields.values()) {
    if (entry.inHeader) return entry.marker;
  }
  return '';
}

/**
 * Insert a metadata line, or fill an existing one in place. Filling in place
 * matters: a present-but-empty `**Date:**` must not gain a second `**Date:**`.
 */
function upsertHeaderField(lines, parsed, name, value, { after, first } = {}) {
  // Only ever rewrite a line inside the metadata block: an issue that QUOTES
  // `**Status:** …` in its prose must not have its prose edited.
  const existing = parsed.fields.get(name);
  if (existing?.beforeBody) return replaceField(lines, existing, value);

  const line = `${detectMarker(parsed)}**${name}:** ${value}`;
  const anchor = after ? parsed.fields.get(after) : null;
  const next = [...lines];
  if (anchor?.beforeBody) {
    next.splice(anchor.endLine, 0, line);
    return next;
  }
  if (parsed.blockStart >= 0) {
    next.splice(first ? parsed.blockStart : parsed.headerEnd, 0, line);
    return next;
  }
  // No metadata block at all: start one under the title.
  const titleIndex = next.findIndex((text) => /^#\s/.test(text));
  if (titleIndex >= 0) next.splice(titleIndex + 1, 0, '', line);
  else next.splice(0, 0, line, '');
  return next;
}

/** Park relocated author text in the body, just past the metadata block. */
function insertBodyNote(lines, parsed, note) {
  const insertAt = parsed.blockStart >= 0 ? parsed.headerEnd : 0;
  const next = [...lines];
  next.splice(
    insertAt,
    0,
    '',
    `Status note (moved verbatim off the **Status:** line by \`node scripts/issue.mjs\`; cdcstack issue statuses are exactly \`Open\` or \`Closed\`): ${note}`,
  );
  return next;
}

function render(parsed, lines) {
  return lines.join(parsed.eol);
}

// ---------------------------------------------------------------------------
// Mechanical repair
// ---------------------------------------------------------------------------

/**
 * Repairs the cases where the correct content is already in the file and only
 * its placement is wrong. Never invents a Resolution body.
 *
 * @returns {{text:string, changes:string[]}}
 */
export function fixIssue(path, text, { liftResolution = false } = {}) {
  const closed = path.startsWith('issues/closed/');
  const expectedStatus = closed ? 'Closed' : 'Open';
  const changes = [];
  let parsed = parseIssue(text);
  let lines = [...parsed.lines];
  let pendingNote = '';

  // Stale format first: a plain-bullet header has no fields at all as far as
  // every other rule is concerned, so normalise the labels before judging them.
  if (!REQUIRED_FIELDS.some((field) => parsed.fields.has(field))) {
    const legacy = findLegacyHeader(parsed);
    if (legacy) {
      const seen = new Set();
      for (const row of legacy.rows) {
        const alias = LEGACY_FIELD_ALIASES.get(row.name);
        const name = alias && !seen.has(alias) && !legacy.rows.some((other) => other.name === alias) ? alias : row.name;
        seen.add(name);
        lines[row.index] = `${row.marker}**${name}:** ${row.value}`.trimEnd();
      }
      const renamed = legacy.rows.filter((row) => LEGACY_FIELD_ALIASES.has(row.name)).map((row) => `${row.name} -> ${LEGACY_FIELD_ALIASES.get(row.name)}`);
      changes.push(
        `normalised ${legacy.rows.length} pre-LLP-0361 bullet field${legacy.rows.length === 1 ? '' : 's'} to \`**Name:** value\`` +
          (renamed.length > 0 ? ` (renamed ${renamed.join(', ')}; values untouched)` : ''),
      );
      parsed = parseIssue(render(parsed, lines));
      lines = [...parsed.lines];
    }
  }

  const status = parsed.fields.get('Status');
  if (!status || !status.value) {
    lines = upsertHeaderField(lines, parsed, 'Status', expectedStatus, { first: true });
    changes.push(`set **Status:** ${expectedStatus} (the path decides it)`);
  } else if (status.value !== expectedStatus) {
    const { remainder, recognized } = splitStatus(status.fullValue, expectedStatus);
    if (recognized) {
      lines = replaceField(lines, status, expectedStatus);
      changes.push(`**Status:** \`${truncate(status.fullValue)}\` -> \`${expectedStatus}\``);
      if (remainder) pendingNote = remainder;
    }
  }

  parsed = parseIssue(render(parsed, lines));
  lines = [...parsed.lines];

  if (pendingNote) {
    if (closed && !fieldValue(parsed, 'Resolution')) {
      lines = upsertHeaderField(lines, parsed, 'Resolution', pendingNote, { after: 'Status' });
      changes.push(`moved the Status-line text into **Resolution:** verbatim: \`${truncate(pendingNote)}\``);
    } else {
      lines = insertBodyNote(lines, parsed, pendingNote);
      changes.push(`moved the Status-line text into a labelled body note verbatim: \`${truncate(pendingNote)}\``);
    }
    parsed = parseIssue(render(parsed, lines));
    lines = [...parsed.lines];
  }

  if (!fieldValue(parsed, 'Date')) {
    const derived = dateFromFilename(path);
    if (derived) {
      lines = upsertHeaderField(lines, parsed, 'Date', derived, { after: 'Author' });
      changes.push(`set **Date:** ${derived} from the filename prefix`);
      parsed = parseIssue(render(parsed, lines));
      lines = [...parsed.lines];
    }
  }

  if (closed && liftResolution && !fieldValue(parsed, 'Resolution')) {
    const section = findResolutionSection(parsed);
    if (section?.firstSentence) {
      lines = upsertHeaderField(lines, parsed, 'Resolution', section.firstSentence, { after: 'Status' });
      changes.push(`lifted the first sentence of the \`## Resolution\` section verbatim (--lift-resolution): \`${truncate(section.firstSentence)}\``);
    }
  }

  return { text: render(parsed, lines), changes };
}

// ---------------------------------------------------------------------------
// Reporting
// ---------------------------------------------------------------------------

const HEADER_TEMPLATE = `
── the cdcstack issue header ──────────────────────────────────────────────
# <one-line title>

**Status:** Open                  exactly \`Open\`, or exactly \`Closed\` under issues/closed/
**Systems:** <comma-separated>
**Author:** <who filed it>
**Date:** <YYYY-MM-DD>
**Resolution:** <one line>        required under issues/closed/

optional             **Severity:** · **Related:** · **Marker:**
optional all-or-none **Impact:** **Urgency:** **Ease:** **Confidence:**
                     **Score reviewed:** **Score rationale:**

A leading \`- \` is fine. The value is the WHOLE rest of the line, so a
qualifier on **Status:** ("Closed (fixed upstream)", "Open — unverified")
makes it a different status. Body sections such as \`## Resolution\` are
never read as metadata.
────────────────────────────────────────────────────────────────────────────`;

const COMMANDS_FOOTER = `
  repair the mechanical parts   node scripts/issue.mjs check --fix
  file an issue correctly       node scripts/issue.mjs new "<title>" --systems "<a, b>"
  close one correctly           node scripts/issue.mjs close <path> --resolution "<one line>"`;

export function formatReport(problems) {
  const byPath = new Map();
  for (const problem of problems) {
    if (!byPath.has(problem.path)) byPath.set(problem.path, []);
    byPath.get(problem.path).push(problem);
  }
  const blocks = [];
  for (const [path, entries] of byPath) {
    const lines = [path];
    for (const problem of entries) {
      const where = problem.line ? `line ${problem.line}` : 'header';
      lines.push(`  ${where.padEnd(10)}${problem.message}`);
      if (problem.found) lines.push(`              found     ${problem.found}`);
      if (problem.required) lines.push(`              required  ${problem.required}`);
      if (problem.hint) {
        for (const hintLine of problem.hint.split('\n')) {
          lines.push(hintLine ? `              ${hintLine}` : '');
        }
      }
      lines.push('');
    }
    blocks.push(lines.join('\n').replace(/\n+$/, ''));
  }
  const fixable = problems.filter((problem) => problem.fixable).length;
  const summary =
    `${problems.length} problem${problems.length === 1 ? '' : 's'} in ${byPath.size} file${byPath.size === 1 ? '' : 's'}` +
    (fixable > 0 ? ` — ${fixable} repairable by \`--fix\`` : '');
  return `${blocks.join('\n\n')}\n${HEADER_TEMPLATE}\n${summary}\n${COMMANDS_FOOTER}\n`;
}

// ---------------------------------------------------------------------------
// Repository-level check
// ---------------------------------------------------------------------------

export function trackedIssuePaths(root) {
  const tracked = spawnSync('git', ['ls-files', '-z', '--', 'issues/*.md', 'issues/closed/*.md'], {
    cwd: root,
    encoding: 'utf8',
  });
  if (tracked.status !== 0) throw new Error('git ls-files failed');
  return tracked.stdout.split('\0').filter((path) => path && path !== 'issues/README.md');
}

/**
 * @returns {{problems:Array, fixed:Array<{path:string,changes:string[]}>, scanned:number}}
 */
export function runFormatCheck(root, { fix = false, liftResolution = false, paths = null } = {}) {
  const problems = [];
  const fixed = [];
  const all = trackedIssuePaths(root);
  const scoped = Array.isArray(paths) && paths.length > 0;
  const selected = scoped ? all.filter((path) => paths.includes(path)) : all;
  for (const path of selected) {
    const absolute = resolve(root, path);
    let text = readFileSync(absolute, 'utf8');
    if (fix) {
      const result = fixIssue(path, text, { liftResolution });
      if (result.changes.length > 0 && result.text !== text) {
        writeFileSync(absolute, result.text);
        text = result.text;
        fixed.push({ path, changes: result.changes });
      }
    }
    problems.push(...validateIssue(path, text));
  }

  return { problems, fixed, scanned: selected.length };
}

// ---------------------------------------------------------------------------
// Authoring commands
// ---------------------------------------------------------------------------

export function slugify(title) {
  return title
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '')
    .split('-')
    .filter(Boolean)
    .slice(0, 6)
    .join('-');
}

export function renderNewIssue({ title, status = 'Open', systems, author, date, severity, related, body }) {
  const header = [`# ${title}`, '', `**Status:** ${status}`, `**Systems:** ${systems}`];
  if (severity) header.push(`**Severity:** ${severity}`);
  header.push(`**Author:** ${author}`, `**Date:** ${date}`);
  if (related) header.push(`**Related:** ${related}`);
  header.push('', body?.trim() ? body.trim() : '<what is wrong, where, and how to reproduce it>', '');
  return header.join('\n');
}

function gitUserName(root) {
  const result = spawnSync('git', ['config', 'user.name'], { cwd: root, encoding: 'utf8' });
  return result.status === 0 ? result.stdout.trim() : '';
}

class UsageError extends Error {}

/** A value-taking flag must actually carry a value, never the boolean `true`. */
function optionalText(args, name) {
  const value = args[name];
  if (value === undefined) return null;
  if (typeof value !== 'string' || value.trim() === '') throw new UsageError(`--${name} needs a value.`);
  return value;
}

function commandNew(root, args) {
  const title = args._[0];
  if (!title) {
    return fail('usage: node scripts/issue.mjs new "<one-line title>" --systems "<a, b>" [--author …] [--severity P2] [--related …] [--slug …] [--body …]');
  }
  const systems = optionalText(args, 'systems');
  if (!systems) {
    return fail(
      'An issue needs **Systems:** and a machine cannot infer it.\n' +
        '  node scripts/issue.mjs new "Router drops the back gesture" --systems "exact-router, Apple host"',
    );
  }
  const date = optionalText(args, 'date') ?? new Date().toISOString().slice(0, 10);
  if (!isIsoDate(date)) return fail(`--date ${date} must be a valid YYYY-MM-DD date.`);
  const slug = optionalText(args, 'slug') ?? slugify(title);
  if (!/^[a-z0-9-]+$/.test(slug)) return fail(`slug \`${slug}\` must be lowercase, digits, and hyphens only`);
  const filename = `${date.replace(/-/g, '')}-${slug}.md`;
  const path = `issues/${filename}`;
  // Both halves of the lifecycle share one filename space: an open issue that
  // collides with a CLOSED one turns `close` into a file-destroying move.
  for (const candidate of [path, `issues/closed/${filename}`]) {
    if (existsSync(resolve(root, candidate))) {
      return fail(`${candidate} already exists — pick a discriminating slug with --slug (cdcstack same-day collision rule).`);
    }
  }
  const author = optionalText(args, 'author') ?? gitUserName(root);
  if (!author) return fail('Could not determine **Author:** — pass --author "<who filed it>".');
  const text = renderNewIssue({
    title,
    systems,
    author,
    date,
    severity: optionalText(args, 'severity') ?? undefined,
    related: optionalText(args, 'related') ?? undefined,
    body: optionalText(args, 'body') ?? undefined,
  });
  const problems = validateIssue(path, text);
  if (problems.length > 0) {
    console.error(`scaffold produced a non-conformant issue — this is a bug in issue.mjs:\n${formatReport(problems)}`);
    return 1;
  }
  mkdirSync(dirname(resolve(root, path)), { recursive: true });
  writeFileSync(resolve(root, path), text);
  if (!args.quiet) console.log(`\n${text}`);
  console.log(`${path}: conformant. Close it later with:\n  node scripts/issue.mjs close ${path} --resolution "<one line>"`);
  return 0;
}

function resolveIssuePath(root, given) {
  const candidates = given.includes('/')
    ? [given]
    : [`issues/${given}`, `issues/${given}.md`, `issues/closed/${given}`, `issues/closed/${given}.md`];
  for (const candidate of candidates) {
    if (existsSync(resolve(root, candidate))) return candidate;
  }
  return null;
}

function commandClose(root, args) {
  const given = args._[0];
  if (!given) return fail('usage: node scripts/issue.mjs close <path|slug> --resolution "<one line>"');
  const resolution = optionalText(args, 'resolution');
  if (!resolution) {
    return fail(
      'A closed issue needs a one-line **Resolution:** and this tool will not write one for you.\n' +
        '  --resolution "fixed by <commit/PR>" | "obsolete" | "graduated to <tracker-id>"',
    );
  }
  if (/[\r\n]/.test(resolution)) return fail('--resolution must be ONE line; put the detail in the body.');
  const path = resolveIssuePath(root, given);
  if (!path) return fail(`no such issue: ${given}`);
  if (path.startsWith('issues/closed/')) return fail(`${path} is already closed.`);
  const target = path.replace(/^issues\//, 'issues/closed/');
  const targetAbsolute = resolve(root, target);
  if (existsSync(targetAbsolute)) {
    return fail(`${target} already exists — closing would destroy it. Rename one of the two (cdcstack same-day collision rule).`);
  }

  const absolute = resolve(root, path);
  let parsed = parseIssue(readFileSync(absolute, 'utf8'));
  let lines = [...parsed.lines];
  const carried = [];

  const status = parsed.fields.get('Status');
  if (status) {
    const { remainder, recognized } = splitStatus(status.fullValue, 'Open');
    // Anything that is not the bare token `Open` is author text and is kept,
    // whether or not it looked like a status word. `Blocked on <tracker-id>` is
    // exactly the shape cdcstack forbids as a Status and must not vanish.
    const leftover = recognized ? remainder : status.fullValue;
    lines = replaceField(lines, status, 'Closed');
    if (leftover) carried.push(leftover);
  }
  parsed = parseIssue(render(parsed, lines));
  lines = [...parsed.lines];
  if (!parsed.fields.get('Status')) {
    lines = upsertHeaderField(lines, parsed, 'Status', 'Closed', { first: true });
    parsed = parseIssue(render(parsed, lines));
    lines = [...parsed.lines];
  }

  const existingResolution = parsed.fields.get('Resolution');
  if (existingResolution?.fullValue) carried.push(`previous **Resolution:** ${existingResolution.fullValue}`);
  lines = upsertHeaderField(lines, parsed, 'Resolution', resolution.trim(), { after: 'Status' });

  for (const note of carried) {
    parsed = parseIssue(render(parsed, lines));
    lines = insertBodyNote([...parsed.lines], parsed, note);
  }

  parsed = parseIssue(render(parsed, lines));
  const text = render(parsed, parsed.lines);
  const problems = validateIssue(target, text);
  if (problems.length > 0) {
    console.error(`Cannot close ${path} — the reshaped issue is still non-conformant:\n\n${formatReport(problems)}`);
    return 1;
  }

  writeFileSync(absolute, text);
  mkdirSync(dirname(targetAbsolute), { recursive: true });
  const moved = spawnSync('git', ['mv', path, target], { cwd: root, encoding: 'utf8' });
  if (moved.status !== 0) {
    renameSync(absolute, targetAbsolute);
    console.log(`git mv declined (${(moved.stderr || '').trim() || 'git unavailable'}); moved with a plain rename — stage it yourself.`);
  }
  console.log(`${path} -> ${target}\n  **Status:** Closed\n  **Resolution:** ${resolution.trim()}`);
  for (const note of carried) console.log(`  kept verbatim in a body note: ${truncate(note)}`);
  console.log('Commit the move together with the fix (cdcstack: close-with-fix is atomic).');
  return 0;
}

function commandCheck(root, args) {
  const { problems, fixed, scanned } = runFormatCheck(root, {
    fix: Boolean(args.fix),
    liftResolution: Boolean(args['lift-resolution']),
    paths: args._,
  });
  if (args.fix) console.log(`--fix: ${scanned} issue${scanned === 1 ? '' : 's'} scanned, ${fixed.length} rewritten.`);
  for (const entry of fixed) {
    console.log(`fixed ${entry.path}`);
    for (const change of entry.changes) console.log(`    ${change}`);
  }
  if (fixed.length > 0) console.log('');
  if (problems.length > 0) {
    console.error(formatReport(problems));
    return 1;
  }
  console.log('Issue format passed.');
  return 0;
}

function fail(message) {
  console.error(message);
  return 2;
}

/** Flags that never take a value; everything else consumes the next token. */
const BOOLEAN_FLAGS = new Set(['fix', 'lift-resolution', 'quiet', 'help']);

function parseArgs(argv) {
  const args = { _: [] };
  for (let index = 0; index < argv.length; index += 1) {
    const token = argv[index];
    if (token.startsWith('--')) {
      const [name, inline] = token.slice(2).split(/=(.*)/s);
      if (inline !== undefined) args[name] = inline;
      else if (BOOLEAN_FLAGS.has(name)) args[name] = true;
      else if (argv[index + 1] !== undefined) args[name] = argv[++index];
      else args[name] = true; // rejected by optionalText with a usage message
    } else {
      args._.push(token);
    }
  }
  return args;
}

const USAGE = `cdcstack filesystem issues — see docs/issues.md.

  node scripts/issue.mjs new "<title>" --systems "<a, b>" [--author …] [--severity P2]
                                      [--related …] [--slug …] [--date YYYY-MM-DD] [--body …]
  node scripts/issue.mjs close <path|slug> --resolution "<one line>"
  node scripts/issue.mjs check [<path> …] [--fix] [--lift-resolution]
${HEADER_TEMPLATE}
`;

export function main(argv, root = process.cwd()) {
  const [subcommand, ...rest] = argv;
  try {
    const args = parseArgs(rest);
    switch (subcommand) {
      case 'new':
        return commandNew(root, args);
      case 'close':
        return commandClose(root, args);
      case 'check':
      case undefined:
        return commandCheck(root, args);
      case 'help':
      case '--help':
      case '-h':
        console.log(USAGE);
        return 0;
      default:
        return fail(`unknown subcommand \`${subcommand}\`\n\n${USAGE}`);
    }
  } catch (error) {
    if (error instanceof UsageError) return fail(`${error.message}\n\n${USAGE}`);
    throw error;
  }
}

if (import.meta.main) {
  process.exit(main(process.argv.slice(2)));
}
