// The shared output buffer vectors (macos/tests/terminal/vectors.json) against the TypeScript port of the
// reference buffer (terminal-output.ts, T3 Code 1e2ecbd975 packages/client-runtime/src/state/terminalOutput.ts).
// The terminal AppKit tests replay the same file against T3TerminalOutput.swift, so both agree.
import { describe, expect, it } from "bun:test";

import vectors from "./macos/tests/terminal/vectors.json";
import {
  appendOutput,
  EMPTY_TERMINAL_OUTPUT_STATE,
  INITIAL_TERMINAL_OUTPUT_CURSOR,
  readTerminalOutputUpdate,
  resetOutput,
  terminalOutputText,
  type TerminalOutputCursor,
  type TerminalOutputState,
} from "./terminal-output";

type Text = string | { length: number; head: string; tail: string };
type Step = {
  op: string; data: string; parts?: [string, number][]; repeat?: number; max?: number; readEach?: boolean; stale?: boolean;
  expect: { text: Text; retainedBytes: number; chunks: number; nextOffset: number; resetVersion: number; read: { type: string; data: Text }; staleRead?: { type: string; data: Text } };
};
const matches = (actual: string, expected: Text) =>
  typeof expected === "string" ? expect(actual).toBe(expected) : expect({ length: actual.length, head: actual.slice(0, 64), tail: actual.slice(-64) }).toEqual(expected);

describe("terminal output vectors", () => {
  for (const vector of vectors.cases as { name: string; steps: Step[] }[]) {
    it(vector.name, () => {
      let state: TerminalOutputState = EMPTY_TERMINAL_OUTPUT_STATE;
      let cursor: TerminalOutputCursor = INITIAL_TERMINAL_OUTPUT_CURSOR;
      for (const step of vector.steps) {
        const data = step.parts ? step.parts.map(([text, count]) => text.repeat(count)).join("") : step.data;
        const max = step.max ?? 512 * 1024;
        const types = new Set<string>();
        let received = "";
        for (let index = 0; index < (step.repeat ?? 1); index += 1) {
          state = step.op === "reset" ? resetOutput(state, data, max) : appendOutput(state, data, max);
          if (step.readEach) {
            const update = readTerminalOutputUpdate(state, cursor);
            types.add(update.type);
            if (update.type !== "none") received += update.data;
            cursor = update.cursor;
          }
        }
        matches(terminalOutputText(state), step.expect.text);
        expect(state.retainedBytes).toBe(step.expect.retainedBytes);
        expect(state.chunks.length).toBe(step.expect.chunks);
        expect(state.nextOffset).toBe(step.expect.nextOffset);
        expect(state.resetVersion).toBe(step.expect.resetVersion);
        if (step.readEach) {
          expect([...types].join(",")).toBe(step.expect.read.type);
          matches(received, step.expect.read.data);
        } else if (step.readEach !== false) {
          const update = readTerminalOutputUpdate(state, cursor);
          expect(update.type).toBe(step.expect.read.type as typeof update.type);
          matches(update.type === "none" ? "" : update.data, step.expect.read.data);
          cursor = update.cursor;
        }
        if (step.expect.staleRead) {
          const update = readTerminalOutputUpdate(state, INITIAL_TERMINAL_OUTPUT_CURSOR);
          expect(update.type).toBe(step.expect.staleRead.type as typeof update.type);
          matches(update.type === "none" ? "" : update.data, step.expect.staleRead.data);
        }
        expect(state.retainedBytes).toBeLessThanOrEqual(Math.max(0, max));
      }
    });
  }
});
