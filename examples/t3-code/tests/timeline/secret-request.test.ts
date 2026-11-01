import { describe, expect, it } from "bun:test";

import {
  secretRequestAnswerInput,
  secretRequestDisplay,
  secretRequestFailureMessage,
  type SecretRequestItem,
} from "../../src/timeline/secret-request";
import { contractComponent } from "../support/contract-action-fixture";
import { appText } from "../support/app-files";

const item = {
  id: "turn-item:secret-request:1",
  threadId: "thread-1",
};

describe("secretRequestDisplay", () => {
  it("shows the form only while pending and maps every answer to its outcome copy", () => {
    const display = (secretStatus: SecretRequestItem["secretStatus"]) =>
      secretRequestDisplay({ secretStatus }, "local");
    expect(display("pending")).toEqual({ kind: "pending" });
    expect(display("saved")).toEqual({
      kind: "answered",
      outcome: "saved",
      label: "Saved securely and kept private",
    });
    expect(display("declined")).toEqual({
      kind: "answered",
      outcome: "declined",
      label: "Declined",
    });
    expect(display("cancelled")).toEqual({
      kind: "answered",
      outcome: "ended",
      label: "Request ended",
    });
  });

  it("never offers the form for a request inherited from another thread", () => {
    expect(secretRequestDisplay({ secretStatus: "pending" }, "inherited")).toEqual({
      kind: "pending-elsewhere",
      label: "Waiting for an answer in the original thread",
    });
    expect(secretRequestDisplay({ secretStatus: "saved" }, "inherited")).toMatchObject({
      outcome: "saved",
    });
  });
});

describe("secretRequestAnswerInput", () => {
  it("refuses a blank save and trims the value the server will store", () => {
    expect(secretRequestAnswerInput(item, { type: "save", secret: "   " })).toBeNull();
    expect(secretRequestAnswerInput(item, { type: "save", secret: " whsec_1 " })).toEqual({
      threadId: item.threadId,
      turnItemId: item.id,
      answer: { type: "save", secret: "whsec_1" },
    });
    expect(secretRequestAnswerInput(item, { type: "decline" })).toEqual({
      threadId: item.threadId,
      turnItemId: item.id,
      answer: { type: "decline" },
    });
  });
});

describe("secretRequestFailureMessage", () => {
  it("passes through server errors but hides anything that could echo the payload", () => {
    expect(
      secretRequestFailureMessage({ _tag: "SecretRequestError", message: "This secret request was already answered." }),
    ).toBe("This secret request was already answered.");
    expect(secretRequestFailureMessage(new Error('Expected string, got "whsec_1"'))).toBe(
      "Could not answer the request. Try again.",
    );
    expect(secretRequestFailureMessage(undefined)).toBe("Could not answer the request. Try again.");
  });
});

describe("SecretRequestCard", () => {
  const card = contractComponent("secret-card.contract", "SecretRequestCard");
  const press = (selection: string) => {
    const focused: string[] = [];
    const state = card.state();
    card.call("selected", state, {}, [{ text: selection }]);
    card.call("pressCard", state, { card: { id: "s1" }, focus: (id: string) => focused.push(id) }, []);
    return focused;
  };
  it("a click on the card outside its controls focuses the field, unless it ends a text selection (#26151)", () => {
    expect(press("")).toEqual(["secret-value-s1"]);
    expect(press("https://example.com/very/long")).toEqual([]);
  });
  it("a long link in the reason wraps anywhere (#26145)", () => {
    expect(appText("secret-card.contract")).toContain('text card.reason selectionchange=selected font-size="0.875rem" line-height="1.25rem" overflow-wrap="anywhere"');
  });
});
