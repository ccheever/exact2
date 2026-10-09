#if os(macOS)
import AppKit
import WebKit

/// The agent's native input to a page in a window (browser-surface part 5; X1 path B): `NSEvent` mouse and key
/// events, trusted in the page, in place of CDP's `Input.dispatchMouseEvent` and the guest's `sendInputEvent`
/// (MIT reference, see LICENSE-T3, T3 Code 1e2ecbd975: apps/desktop/src/preview/Manager.ts `performAutomationClick`,
/// `performAutomationPress`; apps/web/src/components/preview/previewClickFocus.ts). The events go to the page's
/// view while it holds the window's focus, and the focus goes back to what had it, so the person's next keys
/// never land in the page (`runPreviewClickKeepingHostFocus`).
enum T3BrowserAutomationInput {
    private static var eventNumber = 0

    /// A left click at a viewport point (CSS pixels).
    static func click(_ web: WKWebView, window: NSWindow, x: Double, y: Double) {
        let scale = web.pageZoom * web.magnification
        let local = NSPoint(x: x * scale, y: web.isFlipped ? y * scale : web.bounds.height - y * scale)
        let location = web.convert(local, to: nil)
        withFocus(web, window) {
            for type in [NSEvent.EventType.leftMouseDown, .leftMouseUp] {
                eventNumber += 1
                guard let event = NSEvent.mouseEvent(with: type, location: location, modifierFlags: [], timestamp: ProcessInfo.processInfo.systemUptime,
                                                     windowNumber: window.windowNumber, context: nil, eventNumber: eventNumber, clickCount: 1, pressure: type == .leftMouseDown ? 1 : 0) else { continue }
                if type == .leftMouseDown { web.mouseDown(with: event) } else { web.mouseUp(with: event) }
            }
        }
    }

    /// One key press (`makePreviewAutomationNativeKeySequence`): `chord` names the key as T3TerminalKeys does
    /// ("Meta+a", "Enter"); `text` is a printable key's character, typed when no other modifier than Shift is held.
    static func press(_ web: WKWebView, window: NSWindow, key: [String: Any]) throws {
        let chord = key["chord"] as? String ?? ""
        var event = T3TerminalKeys.chord(chord)
        if event == nil, let text = key["text"] as? String, let character = text.first, text.count == 1 { event = T3TerminalKeys.event(for: character) }
        guard let event else { throw T3BrowserHostError.operation("no native key for \(chord)") }
        let characters = (key["text"] as? String).flatMap { $0.isEmpty ? nil : $0 } ?? event.characters
        withFocus(web, window) {
            for type in [NSEvent.EventType.keyDown, .keyUp] {
                guard let ns = NSEvent.keyEvent(with: type, location: .zero, modifierFlags: event.flags, timestamp: ProcessInfo.processInfo.systemUptime,
                                                windowNumber: window.windowNumber, context: nil, characters: event.flags.contains(.command) ? event.characters : characters,
                                                charactersIgnoringModifiers: event.plain, isARepeat: false, keyCode: event.code) else { continue }
                if type == .keyDown { web.keyDown(with: ns) } else { web.keyUp(with: ns) }
            }
        }
    }

    /// The page's view holds the window's focus for the input, then the focus goes back.
    private static func withFocus(_ web: WKWebView, _ window: NSWindow, _ body: () -> Void) {
        let previous = window.firstResponder
        if previous !== web { window.makeFirstResponder(web) }
        body()
        if previous !== web, window.firstResponder === web { restore(previous, in: window) }
    }

    /// The window's focus back to what had it (or to the window, when that is gone).
    static func restore(_ previous: NSResponder?, in window: NSWindow) {
        if let previous, previous.acceptsFirstResponder, (previous as? NSView)?.window === window || previous === window { window.makeFirstResponder(previous) }
        else { window.makeFirstResponder(nil) }
    }

    /// The system clipboard as the paste command's formats (`clipboard.read()` in the reference): text and HTML.
    static func clipboard() -> [[String: String]] {
        var formats: [[String: String]] = []
        let board = NSPasteboard.general
        if let text = board.string(forType: .string) { formats.append(["type": "text/plain", "data": text]) }
        if let html = board.string(forType: .html) { formats.append(["type": "text/html", "data": html]) }
        return formats
    }
}
#endif
