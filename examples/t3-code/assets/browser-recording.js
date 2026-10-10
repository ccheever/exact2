// browser-surface part 3 (capture): the Browser recording's in-page cursor and input overlay (MIT reference,
// see LICENSE-T3, T3 Code 1e2ecbd975: apps/desktop/src/preview/RecordingCursor.ts installRecordingCursor,
// RecordingInput.ts recordingKeyLabel / recordingKeysAreSensitive, PickPreload.ts's RECORDING_* channel
// handlers). A classic script: T3BrowserRecorder.swift evaluates it in the module's own content world
// (`.defaultClient`), so the page can neither see nor call it, and it shares only the DOM with the page.
//
// The recording's frames are WKWebView `takeSnapshot` images (no Screen Recording permission), which never
// include the native pointer: as in the reference (where Chromium's capture cursor misses the webview's
// placement), the human pointer is drawn into the page while recording and the native one hidden, so the
// recording and the person see the same single cursor. Key and pointer presses are not drawn here: they
// are forwarded as DesktopPreviewRecordingInput messages (`t3BrowserRecording`) to the native compositor
// (T3RecordingDecorations), which draws them over the captured frames.
//
// Loaded by Bun's `require` (browser-recording-input.test.ts), it exports the pure helpers and touches no
// window or document.
(function () {
  "use strict";

  /** RecordingInput.ts DEFAULT_RECORDING_INPUT_OPTIONS. */
  var DEFAULT_RECORDING_INPUT_OPTIONS = { showKeyPresses: false, showMousePresses: false };

  var KEY_LABELS = {
    Enter: "↵",
    Tab: "⇥",
    Backspace: "⌫",
    Delete: "⌦",
    Escape: "Esc",
    ArrowUp: "↑",
    ArrowDown: "↓",
    ArrowLeft: "←",
    ArrowRight: "→",
    " ": "Space",
    Space: "Space",
  };

  /** RecordingInput.ts recordingKeyLabel: formats a single chord without duplicating a modifier pressed on its own. */
  function recordingKeyLabel(input, isMac) {
    if (["Dead", "Process", "Unidentified", ""].indexOf(input.key) >= 0) return null;
    var modifiers = [
      input.ctrlKey || input.key === "Control" ? (isMac ? "⌃" : "Ctrl") : null,
      input.altKey || input.key === "Alt" ? (isMac ? "⌥" : "Alt") : null,
      input.shiftKey || input.key === "Shift" ? (isMac ? "⇧" : "Shift") : null,
      input.metaKey || input.key === "Meta" ? (isMac ? "⌘" : "Win") : null,
    ].filter(function (value) { return value !== null; });
    if (["Control", "Alt", "Shift", "Meta"].indexOf(input.key) < 0) {
      modifiers.push(Object.prototype.hasOwnProperty.call(KEY_LABELS, input.key) ? KEY_LABELS[input.key]
        : input.key.length === 1 ? input.key.toUpperCase() : input.key);
    }
    return modifiers.join(isMac ? "" : " + ");
  }

  /** RecordingInput.ts recordingKeysAreSensitive: unknown iframe or closed-shadow focus is excluded because its field type cannot be checked. */
  function recordingKeysAreSensitive(document) {
    var element = document.activeElement;
    while (element && element.shadowRoot && element.shadowRoot.activeElement) element = element.shadowRoot.activeElement;
    var type = element && typeof element.getAttribute === "function" ? element.getAttribute("type") : null;
    return (
      (element ? element.tagName : undefined) === "IFRAME" ||
      (element && typeof element.tagName === "string" ? element.tagName.indexOf("-") >= 0 : false) === true ||
      (typeof type === "string" ? type.toLowerCase() : undefined) === "password"
    );
  }

  if (typeof module === "object" && module && module.exports) {
    module.exports = { recordingKeyLabel: recordingKeyLabel, recordingKeysAreSensitive: recordingKeysAreSensitive, DEFAULT_RECORDING_INPUT_OPTIONS: DEFAULT_RECORDING_INPUT_OPTIONS };
    return;
  }
  if (typeof window === "undefined" || typeof document === "undefined") return;
  if (globalThis.__t3codeRecording) return; // one install per document; start() updates options

  function emit(input) {
    try { window.webkit.messageHandlers.t3BrowserRecording.postMessage(input); } catch (_) { /* the handler is gone: nothing listens */ }
  }

  var isMac = /Mac/.test(window.navigator.platform);

  /**
   * RecordingCursor.ts installRecordingCursor: the drawn human cursor (the native one transparent), the agent
   * cursor, and the key and pointer inputs the decorations draw. Returns its controls.
   */
  function installRecordingCursor(options) {
    var style = document.createElement("style");
    style.setAttribute("data-t3code-recording-style", "");
    style.textContent =
      "html, html * { cursor: none !important; } @media (prefers-reduced-motion: reduce) { [data-t3code-recording-agent-cursor] { transition: none !important; } }";
    var cursor = document.createElement("div");
    cursor.setAttribute("aria-hidden", "true");
    cursor.setAttribute("data-t3code-recording-cursor", "");
    cursor.style.cssText =
      "position:fixed;left:0;top:0;width:16px;height:24px;pointer-events:none;z-index:2147483647;display:none;";
    cursor.innerHTML =
      '<svg xmlns="http://www.w3.org/2000/svg" width="16" height="24" viewBox="0 0 16 24"><path d="M1 1v18l4-4 4 8 3-1.5-4-8H15Z" fill="black" stroke="white" stroke-width="1.5" stroke-linejoin="round"/></svg>';
    var agentCursor = document.createElement("div");
    agentCursor.setAttribute("aria-hidden", "true");
    agentCursor.setAttribute("data-t3code-recording-agent-cursor", "");
    agentCursor.style.cssText =
      "position:fixed;left:0;top:0;width:20px;height:20px;pointer-events:none;z-index:2147483647;display:none;filter:drop-shadow(0 1px 2px #0003);transition:transform 150ms ease-out,opacity 150ms ease-out;";
    // Match the MousePointer2 icon used by the live AgentBrowserCursor.
    agentCursor.innerHTML =
      '<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 24 24" fill="var(--recording-cursor-background,white)" stroke="var(--recording-cursor-primary,#2563eb)" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" style="transform:translate(-2px,-2px)"><path d="M4.037 4.688a.495.495 0 0 1 .651-.651l16 6.5a.5.5 0 0 1-.063.947l-6.124 1.58a2 2 0 0 0-1.438 1.435l-1.579 6.126a.5.5 0 0 1-.947.063z"/></svg>';
    document.documentElement.append(style, cursor, agentCursor);
    var controller = "none";
    var humanPoint = null;
    var drawHuman = function () {
      if (!humanPoint) return;
      cursor.style.transform = "translate(" + humanPoint.x + "px, " + humanPoint.y + "px)";
      cursor.style.display = "block";
    };
    var agentActive = false;
    var agentTimer;
    var setController = function (next, point) {
      if (point) humanPoint = point;
      controller = next;
      if (next === "agent") cursor.style.display = "none";
      if (next === "human") drawHuman();
      if (!agentActive) agentCursor.style.opacity = next === "human" ? "0.18" : "0.35";
    };
    var setTheme = function (theme) {
      agentCursor.style.setProperty("--recording-cursor-primary", (theme && theme.primary) || "#2563eb");
      agentCursor.style.setProperty("--recording-cursor-background", (theme && theme.background) || "white");
    };
    var lastKeyLabel = null;
    var pointerHeld = false;
    var pointerFrame;
    var pendingPointer;
    var cancelPendingPointer = function () {
      if (pointerFrame !== undefined) window.cancelAnimationFrame(pointerFrame);
      pointerFrame = undefined;
      pendingPointer = undefined;
    };
    var keyPress = function (input, held) {
      if (!options.showKeyPresses) return;
      var label = recordingKeysAreSensitive(document) ? null : recordingKeyLabel(input, isMac);
      lastKeyLabel = label;
      emit({ type: "key", label: label, held: held === true, width: window.innerWidth });
    };
    var pointer = function (point, phase) {
      if (!options.showMousePresses || (phase === "move" && !pointerHeld)) return;
      if (phase === "down") pointerHeld = true;
      if (phase === "up") pointerHeld = false;
      var input = { type: "pointer", phase: phase, x: point.x, y: point.y, width: window.innerWidth, height: window.innerHeight };
      if (phase === "move") {
        pendingPointer = input;
        if (pointerFrame === undefined) {
          pointerFrame = window.requestAnimationFrame(function () {
            pointerFrame = undefined;
            if (pendingPointer) emit(pendingPointer);
            pendingPointer = undefined;
          });
        }
      } else {
        cancelPendingPointer();
        emit(input);
      }
    };
    var move = function (point, phase) {
      agentCursor.style.transform = "translate(" + point.x + "px, " + point.y + "px)";
      agentCursor.style.display = "block";
      agentCursor.style.opacity = "1";
      agentActive = true;
      window.clearTimeout(agentTimer);
      agentTimer = window.setTimeout(function () {
        agentActive = false;
        agentCursor.style.opacity = controller === "human" ? "0.18" : "0.35";
      }, 700);
      pointer(point, phase === "click" ? "click" : "move");
    };
    var moveHuman = function (point) {
      if (controller === "agent") return;
      humanPoint = point;
      drawHuman();
    };
    var pointerMove = function (event) {
      if (event.pointerType === "touch") return;
      var point = { x: event.clientX, y: event.clientY };
      moveHuman(point);
      pointer(point, "move");
    };
    var pointerDown = function (event) {
      if (event.pointerType === "touch") return;
      moveHuman({ x: event.clientX, y: event.clientY });
      pointer({ x: event.clientX, y: event.clientY }, "down");
    };
    var pointerUp = function (event) {
      if (event.pointerType !== "touch") pointer({ x: event.clientX, y: event.clientY }, "up");
    };
    var keyDown = function (event) {
      if (event.isComposing || event.repeat) return;
      keyPress(event, true);
    };
    var keyUp = function () {
      if (!options.showKeyPresses) return;
      emit({ type: "key", label: recordingKeysAreSensitive(document) ? null : lastKeyLabel, held: false, width: window.innerWidth });
    };
    var hide = function () {
      cursor.style.display = "none";
      pointerHeld = false;
      cancelPendingPointer();
      if (options.showKeyPresses || options.showMousePresses) emit({ type: "clear" });
    };
    var leave = function (event) {
      if (event.relatedTarget === null) hide();
    };
    window.addEventListener("pointermove", pointerMove, true);
    window.addEventListener("pointerdown", pointerDown, true);
    window.addEventListener("pointerup", pointerUp, true);
    window.addEventListener("pointercancel", pointerUp, true);
    window.addEventListener("keydown", keyDown, true);
    window.addEventListener("keyup", keyUp, true);
    window.addEventListener("pointerout", leave, true);
    window.addEventListener("blur", hide);
    return {
      move: move,
      keyPress: keyPress,
      setController: setController,
      setTheme: setTheme,
      setOptions: function (next) { options = next; },
      dispose: function () {
        window.removeEventListener("pointermove", pointerMove, true);
        window.removeEventListener("pointerdown", pointerDown, true);
        window.removeEventListener("pointerup", pointerUp, true);
        window.removeEventListener("pointercancel", pointerUp, true);
        window.removeEventListener("keydown", keyDown, true);
        window.removeEventListener("keyup", keyUp, true);
        window.removeEventListener("pointerout", leave, true);
        window.removeEventListener("blur", hide);
        cancelPendingPointer();
        window.clearTimeout(agentTimer);
        cursor.remove();
        agentCursor.remove();
        style.remove();
      },
    };
  }

  var finite = function (value) { return typeof value === "number" && isFinite(value); };
  var readOptions = function (value) {
    return value !== null && typeof value === "object"
      ? { showKeyPresses: value.showKeyPresses === true, showMousePresses: value.showMousePresses === true }
      : { showKeyPresses: DEFAULT_RECORDING_INPUT_OPTIONS.showKeyPresses, showMousePresses: DEFAULT_RECORDING_INPUT_OPTIONS.showMousePresses };
  };
  var readPoint = function (point) {
    return point !== null && typeof point === "object" && finite(point.x) && finite(point.y) ? { x: point.x, y: point.y } : undefined;
  };
  var readController = function (value) { return value === "agent" || value === "human" || value === "none" ? value : null; };

  // PickPreload.ts's RECORDING_CURSOR / RECORDING_CONTROLLER / RECORDING_KEY / RECORDING_POINTER handlers.
  var recordingCursor = null;
  globalThis.__t3codeRecording = {
    /** RECORDING_CURSOR_CHANNEL active: install (or update) the cursor with the input options. */
    start: function (options, theme, controller) {
      var read = readOptions(options);
      if (!recordingCursor) recordingCursor = installRecordingCursor(read);
      else recordingCursor.setOptions(read);
      recordingCursor.setTheme(theme || null);
      var next = readController(controller);
      if (next) recordingCursor.setController(next);
      return true;
    },
    /** RECORDING_CURSOR_CHANNEL inactive. */
    stop: function () {
      if (recordingCursor) recordingCursor.dispose();
      recordingCursor = null;
      return true;
    },
    /** RECORDING_POINTER_CHANNEL: the agent's pointer. */
    move: function (point, phase) {
      var read = readPoint(point);
      if (read && recordingCursor) recordingCursor.move(read, phase === "click" ? "click" : "move");
      return true;
    },
    /** RECORDING_KEY_CHANNEL: a key the agent pressed. */
    keyPress: function (input) {
      if (input === null || typeof input !== "object" || typeof input.key !== "string" || !recordingCursor) return false;
      recordingCursor.keyPress({ key: input.key, metaKey: input.metaKey === true, ctrlKey: input.ctrlKey === true, altKey: input.altKey === true, shiftKey: input.shiftKey === true });
      return true;
    },
    /** RECORDING_CONTROLLER_CHANNEL. */
    setController: function (controller, point) {
      var next = readController(controller);
      if (next && recordingCursor) recordingCursor.setController(next, readPoint(point));
      return true;
    },
    /** ANNOTATION_THEME_CHANNEL's cursor half. */
    setTheme: function (theme) {
      if (recordingCursor) recordingCursor.setTheme(theme || null);
      return true;
    },
    /** Whether the cursor is installed in this document (the native side re-installs after a navigation). */
    active: function () { return recordingCursor !== null; },
  };
})();
