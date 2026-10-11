// Ported from T3 Code 1e2ecbd975 (MIT, see LICENSE-T3): apps/desktop/src/preview/PickedElementPayload.test.ts (11 tests,
// the it.each's 9 rows), under their own names. The validators run in the data module here (browser-annotation.ts):
// the module's overlay hands the payload through unchecked, as the preload hands it to Electron's main. Then the
// clone's own rows: composerContextRecords' previewAnnotationContext* and normalizeElementContextSelection, and the
// draft's annotations (addPreviewAnnotation's chip, the send's records, the native editor's chip kinds).
import { describe, expect, it } from 'bun:test';
import {
  adoptPreviewAnnotations, annotationChipContexts, annotationMessageRecords, isPickedElementPayload, isPreviewAnnotationPayload, normalizeElementContextSelection,
  previewAnnotationContextLabel, previewAnnotationContextRecord, previewAnnotationLink, referencedAnnotations, savePreviewAnnotation, type PreviewAnnotationPayload,
} from './browser-annotation';
import type { T3Client } from './client';

function validPayload(overrides?: Record<string, unknown>): Record<string, unknown> {
  return {
    pageUrl: 'https://example.com/', pageTitle: 'Example', tagName: 'button', selector: 'button.submit', htmlPreview: '<button>Save</button>', componentName: 'SubmitButton',
    source: { functionName: 'SubmitButton', fileName: '/repo/src/Button.tsx', lineNumber: 12, columnNumber: 5 },
    stack: [{ functionName: 'SubmitButton', fileName: '/repo/src/Button.tsx', lineNumber: 12, columnNumber: 5 }],
    styles: '.submit { color: white; }', pickedAt: '2026-05-03T18:00:00.000Z', ...overrides,
  };
}

describe('isPickedElementPayload', () => {
  it('accepts a complete, well-typed payload', () => {
    expect(isPickedElementPayload(validPayload())).toBe(true);
  });
  it('accepts nullable string fields when null', () => {
    expect(isPickedElementPayload(validPayload({ pageTitle: null, selector: null, componentName: null, source: null }))).toBe(true);
  });
  it('accepts an empty stack array', () => {
    expect(isPickedElementPayload(validPayload({ stack: [] }))).toBe(true);
  });
  it('accepts stack frames with null fields', () => {
    expect(isPickedElementPayload(validPayload({ stack: [{ functionName: null, fileName: null, lineNumber: null, columnNumber: null }] }))).toBe(true);
  });
  it('rejects null and primitive inputs', () => {
    expect(isPickedElementPayload(null)).toBe(false);
    expect(isPickedElementPayload(undefined)).toBe(false);
    expect(isPickedElementPayload('string')).toBe(false);
    expect(isPickedElementPayload(42)).toBe(false);
    expect(isPickedElementPayload([])).toBe(false);
  });
  it.each<[string, Record<string, unknown>]>([
    ['missing pageUrl', validPayload({ pageUrl: undefined })],
    ['wrong-type pageUrl', validPayload({ pageUrl: 123 })],
    ['missing tagName', validPayload({ tagName: undefined })],
    ['missing htmlPreview', validPayload({ htmlPreview: undefined })],
    ['missing styles', validPayload({ styles: undefined })],
    ['missing pickedAt', validPayload({ pickedAt: undefined })],
    ['wrong-type pageTitle', validPayload({ pageTitle: 99 })],
    ['wrong-type selector', validPayload({ selector: 99 })],
    ['wrong-type componentName', validPayload({ componentName: 99 })],
  ])('rejects payloads with %s', (_label, value) => {
    expect(isPickedElementPayload(value)).toBe(false);
  });
  it('rejects malformed source frames', () => {
    expect(isPickedElementPayload(validPayload({ source: { functionName: 0, fileName: null, lineNumber: null, columnNumber: null } }))).toBe(false);
  });
  it('rejects non-finite numeric line/column numbers', () => {
    expect(isPickedElementPayload(validPayload({ source: { functionName: null, fileName: null, lineNumber: Number.POSITIVE_INFINITY, columnNumber: null } }))).toBe(false);
    expect(isPickedElementPayload(validPayload({ source: { functionName: null, fileName: null, lineNumber: Number.NaN, columnNumber: null } }))).toBe(false);
  });
  it('rejects malformed stack arrays', () => {
    expect(isPickedElementPayload(validPayload({ stack: 'not-an-array' }))).toBe(false);
    expect(isPickedElementPayload(validPayload({ stack: [{ bogus: true }] }))).toBe(false);
  });
});

function validAnnotation(overrides?: Record<string, unknown>): Record<string, unknown> {
  return {
    id: 'annotation_1', pageUrl: 'https://example.com/', pageTitle: 'Example', comment: 'Make this clearer',
    elements: [{ id: 'element_1', element: validPayload(), rect: { x: 10, y: 20, width: 100, height: 40 } }],
    regions: [{ id: 'region_1', rect: { x: 5, y: 6, width: 20, height: 30 } }],
    strokes: [{ id: 'stroke_1', color: '#7c3aed', width: 4, points: [{ x: 10, y: 10 }, { x: 20, y: 20 }], bounds: { x: 6, y: 6, width: 18, height: 18 } }],
    styleChanges: [{ targetId: 'element_1', selector: 'button.submit', property: 'opacity', previousValue: '1', value: '0.5' }],
    screenshot: null, createdAt: '2026-06-11T00:00:00.000Z', ...overrides,
  };
}

describe('isPreviewAnnotationPayload', () => {
  it('accepts a structured annotation draft before screenshot capture', () => {
    expect(isPreviewAnnotationPayload(validAnnotation())).toBe(true);
  });
  it('rejects screenshots supplied by the guest preload', () => {
    expect(isPreviewAnnotationPayload(validAnnotation({ screenshot: { dataUrl: 'bad' } }))).toBe(false);
  });
  it('rejects malformed geometry and nested element payloads', () => {
    expect(isPreviewAnnotationPayload(validAnnotation({ regions: [{ id: 'region_1', rect: { x: 0, y: 0, width: 'wide' } }] }))).toBe(false);
    expect(isPreviewAnnotationPayload(validAnnotation({ elements: [{ id: 'element_1', element: {}, rect: {} }] }))).toBe(false);
  });
});

// ── Clone rows ──────────────────────────────────────────────────────────────────────────────────
const annotation = validAnnotation() as unknown as PreviewAnnotationPayload;
const fakeClient = () => ({ local: { drafts: {}, snapshotDrafts: {} }, draftKey: 'local:thread-1' } as unknown as T3Client);

describe('previewAnnotationContextRecord (composerContextRecords)', () => {
  it('labels an annotation by its comment, else its page title, else "Preview annotation"', () => {
    expect(previewAnnotationContextLabel({ comment: '  Make   this\nclearer ', pageTitle: 'Example' })).toBe('Make this clearer');
    expect(previewAnnotationContextLabel({ comment: 'x'.repeat(60), pageTitle: null })).toBe(`${'x'.repeat(47)}…`);
    expect(previewAnnotationContextLabel({ comment: ' ', pageTitle: ' Dashboard ' })).toBe('Dashboard');
    expect(previewAnnotationContextLabel({ comment: '', pageTitle: null })).toBe('Preview annotation');
  });

  it('carries the targets, the style changes and the summary an agent needs', () => {
    const record = previewAnnotationContextRecord(annotation, { screenshotContextId: 'image_abc' });
    expect(record).toMatchObject({
      version: 1, kind: 'preview-annotation', contextId: 'preview-annotation_annotation_1', label: 'Make this clearer', annotationId: 'annotation_1',
      pageUrl: 'https://example.com/', pageTitle: 'Example', comment: 'Make this clearer', targetSummary: '1 selected element, 1 marked region, 1 drawing',
      styleChanges: ['opacity: 1 → 0.5'], elementIds: ['element_1'], regionCount: 1, strokeCount: 1, screenshotContextId: 'image_abc',
    });
    expect((record.elements as unknown[])[0]).toEqual({ pageUrl: 'https://example.com/', pageTitle: 'Example', tagName: 'button', selector: 'button.submit',
      htmlPreview: '<button>Save</button>', componentName: 'SubmitButton', source: { functionName: 'SubmitButton', fileName: '/repo/src/Button.tsx', lineNumber: 12, columnNumber: 5 },
      styles: '.submit { color: white; }' });
  });

  it('normalizes a picked element as the composer stores it (the first stack frame for a missing source)', () => {
    const element = normalizeElementContextSelection({ ...(validPayload() as never), tagName: ' BUTTON ', source: null, htmlPreview: `\n${'a'.repeat(5000)}\n`, selector: ' ' });
    expect(element?.tagName).toBe('button');
    expect(element?.selector).toBeNull();
    expect(element?.source?.functionName).toBe('SubmitButton');
    expect(element?.htmlPreview.length).toBe(4000);
    expect(normalizeElementContextSelection({ ...(validPayload() as never), pageUrl: ' ' })).toBeNull();
  });
});

describe('the draft\'s annotations (addPreviewAnnotation)', () => {
  it('keeps the annotation beside the drafts and sends a record for each chip the prompt still has', () => {
    const client = fakeClient();
    savePreviewAnnotation(client, annotation, '0f8c1b52-9a4c-4d2e-8a0b-3c2d1e0f9a8b');
    const link = previewAnnotationLink(annotation);
    expect(link).toBe('[Make this clearer](t3-context://v1/preview-annotation/preview-annotation_annotation_1)');
    const prompt = `Please fix ${link} soon`;
    expect(referencedAnnotations(client, prompt).map(entry => entry.annotation.id)).toEqual(['annotation_1']);
    expect(annotationChipContexts(client, prompt)).toEqual({ 'preview-annotation/preview-annotation_annotation_1': 'preview-annotation' });
    // The crop links its image record only when that image goes with the message.
    expect(annotationMessageRecords(client, prompt)[0]?.screenshotContextId).toBeUndefined();
    expect(annotationMessageRecords(client, prompt, new Set(['0f8c1b52-9a4c-4d2e-8a0b-3c2d1e0f9a8b']))[0]?.screenshotContextId).toBe('image_0f8c1b52-9a4c-4d2e-8a0b-3c2d1e0f9a8b');
    expect(annotationMessageRecords(client, 'no chip here')).toEqual([]);
  });

  it('restores only well-formed annotations from saved preferences', () => {
    const client = fakeClient();
    savePreviewAnnotation(client, annotation, 'image-id');
    const next = {};
    adoptPreviewAnnotations(next, { previewAnnotations: { ...(client.local as never as { previewAnnotations: object }).previewAnnotations, 'preview-annotation_bad': { annotation: { id: 'bad' } } } });
    expect(Object.keys((next as { previewAnnotations: object }).previewAnnotations)).toEqual(['preview-annotation_annotation_1']);
  });
});
