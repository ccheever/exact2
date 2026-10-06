// SSH password prompts (MIT reference, see LICENSE-T3: packages/ssh/src/auth.ts
// isSshAuthFailure, ASKPASS_POSIX_SCRIPT, buildSshChildEnvironment;
// apps/desktop/src/ssh/DesktopSshPasswordPrompts.ts and DesktopSshEnvironment.ts
// toSshPasswordPromptError; apps/web/src/components/desktop/SshPasswordPromptDialog.tsx,
// at 1e2ecbd975). An `ssh` run that fails authentication asks for a password; the
// answer reaches only the next `ssh` child, through SSH_ASKPASS and T3_SSH_AUTH_SECRET in
// its environment, never argv, a file, Keychain, defaults or a log. The queue blocks the
// SSH worker (T3Ssh's background queue), never the main thread, and expires a request
// after three minutes. The dialog is Contract (ssh-prompt.contract); its field is the
// native secure field below, which the module reads on Continue, so the password never
// enters TypeScript or Contract state (X35: since exact2 #134 a Contract password input's
// value is masked in agent output, but the app's state and data module stay outside that).
import Foundation
import AppKit

enum T3SshAuth {
    /// isSshAuthFailure: ssh's refusal of every offered method, or a generic auth failure.
    static func isAuthFailure(_ message: String) -> Bool {
        let normalized = message.lowercased()
        let patterns = [#"permission denied \((?:publickey|password|keyboard-interactive|hostbased|gssapi-with-mic)[^)]*\)"#,
                        #"authentication failed"#, #"too many authentication failures"#]
        return patterns.contains { normalized.range(of: $0, options: .regularExpression) != nil }
    }

    static let askpassScript = """
    #!/bin/sh
    # Invoked by ssh via SSH_ASKPASS when T3 Code re-runs ssh with a cached password
    # from the renderer's in-app prompt. We never expose a native dialog here - if
    # T3_SSH_AUTH_SECRET is missing, that's a caller bug and we fail loudly.
    if [ "${T3_SSH_AUTH_SECRET+x}" = "x" ]; then
      printf "%s\\n" "$T3_SSH_AUTH_SECRET"
      exit 0
    fi
    printf 'T3 Code ssh-askpass invoked without T3_SSH_AUTH_SECRET.\\n' >&2
    exit 1

    """

    /// ensureSshAskpassHelpers in a fresh `t3code-ssh-runtime-*` folder: `<it>/t3code-ssh-askpass/ssh-askpass.sh`, mode 0700.
    static func makeAskpassHelper(in parent: String = NSTemporaryDirectory()) throws -> String {
        let runtime = (parent as NSString).appendingPathComponent("t3code-ssh-runtime-\(UUID().uuidString.prefix(8))")
        let directory = (runtime as NSString).appendingPathComponent("t3code-ssh-askpass")
        try FileManager.default.createDirectory(atPath: directory, withIntermediateDirectories: true, attributes: [.posixPermissions: 0o700])
        let path = (directory as NSString).appendingPathComponent("ssh-askpass.sh")
        try askpassScript.write(toFile: path, atomically: true, encoding: .utf8)
        try FileManager.default.setAttributes([.posixPermissions: 0o700], ofItemAtPath: path)
        return path
    }

    /// buildSshChildEnvironment: interactive runs point ssh at the helper; the secret rides in the environment only.
    static func childEnvironment(base: [String: String], interactive: Bool, askpass: String, secret: String?) -> [String: String] {
        guard interactive else { return base }
        var environment = base
        environment["SSH_ASKPASS"] = askpass
        environment["SSH_ASKPASS_REQUIRE"] = "force"
        if let secret { environment["T3_SSH_AUTH_SECRET"] = secret } else { environment.removeValue(forKey: "T3_SSH_AUTH_SECRET") }
        if (environment["DISPLAY"] ?? "").isEmpty { environment["DISPLAY"] = "t3code" }
        return environment
    }
}

/// DesktopSshPasswordPrompts: requests in arrival order, one dialog at a time.
final class T3SshPrompts: @unchecked Sendable {
    struct Request { let id: String; let destination: String; let username: String?; let prompt: String; let attempt: Int; let expiresAt: Double }
    enum Outcome: Equatable { case password(String), cancelled, timedOut, windowClosed, stopped }

    static let defaultTimeout: TimeInterval = 3 * 60
    private let lock = NSLock()
    private var shown: [Request] = []               // the dialog's queue: removed only by an answer or a dismissal
    private var waiting: [String: DispatchSemaphore] = [:]
    private var outcomes: [String: Outcome] = [:]
    private var errors: [String: String] = [:]
    private var closed = false
    let timeout: TimeInterval
    /// A window can show the dialog; without one (the AppKit test binary) a refused key is final.
    let available: Bool
    private let changed: () -> Void
    private let clock: () -> Double
    /// Reads and clears the dialog's secure field for a request (main thread); tests answer directly.
    var readSecret: (String) -> String? = { id in Thread.isMainThread ? T3SshPasswordFields.take(id) : DispatchQueue.main.sync { T3SshPasswordFields.take(id) } }

    init(available: Bool, timeout: TimeInterval = T3SshPrompts.defaultTimeout, clock: @escaping () -> Double = { Date().timeIntervalSince1970 * 1000 }, changed: @escaping () -> Void = {}) {
        self.available = available; self.timeout = timeout; self.clock = clock; self.changed = changed
    }

    /// One prompt: blocks the calling (SSH worker) thread until an answer, the expiry or the window's end.
    func request(destination: String, username: String?, prompt: String, attempt: Int) -> Outcome {
        let semaphore = DispatchSemaphore(value: 0)
        let request = Request(id: UUID().uuidString.lowercased(), destination: destination, username: username, prompt: prompt, attempt: attempt, expiresAt: clock() + timeout * 1000)
        lock.lock()
        if closed { lock.unlock(); return .windowClosed }
        shown.append(request); waiting[request.id] = semaphore
        lock.unlock()
        changed()
        let finished = semaphore.wait(timeout: .now() + timeout) == .success
        lock.lock()
        waiting.removeValue(forKey: request.id)
        let outcome = finished ? (outcomes.removeValue(forKey: request.id) ?? .cancelled) : .timedOut
        if !finished { outcomes.removeValue(forKey: request.id) }
        lock.unlock()
        if !finished { changed() } // the dialog turns to "Expired" and waits for Dismiss
        return outcome
    }

    /// The dialog's answer. `submit` reads the field; a request no longer pending is the expiry error and stays shown.
    func resolve(id: String, answer: String) -> (ok: Bool, message: String) {
        let trimmed = id.trimmingCharacters(in: .whitespaces)
        guard !trimmed.isEmpty else { return (false, "Invalid SSH password prompt id.") }
        lock.lock()
        let semaphore = waiting[trimmed]
        if answer == "dismiss" || answer == "cancel" {
            shown.removeAll { $0.id == trimmed }; errors.removeValue(forKey: trimmed)
            if let semaphore, answer == "cancel" { outcomes[trimmed] = .cancelled; waiting.removeValue(forKey: trimmed); semaphore.signal() }
            lock.unlock(); changed()
            return (true, "")
        }
        guard let semaphore else {
            errors[trimmed] = "SSH password prompt expired. Try connecting again."
            lock.unlock(); changed()
            return (false, "SSH password prompt expired. Try connecting again.")
        }
        lock.unlock()
        let secret = readSecret(trimmed) ?? ""
        lock.lock()
        guard waiting[trimmed] === semaphore else { lock.unlock(); return (false, "SSH password prompt expired. Try connecting again.") }
        outcomes[trimmed] = .password(secret); waiting.removeValue(forKey: trimmed)
        shown.removeAll { $0.id == trimmed }; errors.removeValue(forKey: trimmed)
        semaphore.signal()
        lock.unlock(); changed()
        return (true, "")
    }

    /// The dialog's view of the queue: its first request, whether it still waits, and how many follow.
    func state() -> [String: Any] {
        lock.lock(); defer { lock.unlock() }
        guard let first = shown.first else { return ["request": NSNull(), "queued": 0] }
        var request: [String: Any] = ["requestId": first.id, "destination": first.destination, "prompt": first.prompt, "attempt": first.attempt,
                                      "expiresAt": first.expiresAt, "timeoutMs": timeout * 1000, "expired": waiting[first.id] == nil, "error": errors[first.id] ?? ""]
        request["username"] = first.username ?? NSNull()
        return ["request": request, "queued": shown.count - 1]
    }

    /// The window closed (or the service stopped): every waiting request fails, the queue empties.
    func closeAll(windowClosed: Bool) {
        lock.lock()
        closed = true
        for (id, semaphore) in waiting { outcomes[id] = windowClosed ? .windowClosed : .stopped; semaphore.signal() }
        waiting.removeAll(); shown.removeAll(); errors.removeAll()
        lock.unlock()
        DispatchQueue.main.async { T3SshPasswordFields.clearAll() }
    }

    /// toSshPasswordPromptError's messages.
    static func message(_ outcome: Outcome, destination: String) -> String {
        switch outcome {
        case .password: return ""
        case .cancelled: return "SSH authentication cancelled for \(destination)."
        case .timedOut: return "SSH authentication timed out for \(destination)."
        case .windowClosed: return "SSH authentication was cancelled because the app window closed."
        case .stopped: return "SSH password prompt service stopped."
        }
    }
}

/// The dialog's secure fields by request id (main thread): the module reads one on Continue.
enum T3SshPasswordFields {
    private static var fields: [String: WeakField] = [:]
    private final class WeakField { weak var field: T3SshPasswordView?; init(_ field: T3SshPasswordView) { self.field = field } }
    static func register(_ id: String, _ field: T3SshPasswordView) { fields = fields.filter { $0.value.field != nil }; fields[id] = WeakField(field) }
    static func unregister(_ id: String, _ field: T3SshPasswordView) { if fields[id]?.field === field { fields.removeValue(forKey: id) } }
    /// The typed password, and the field forgets it.
    static func take(_ id: String) -> String? {
        guard let field = fields[id]?.field else { return nil }
        let value = field.stringValue
        field.stringValue = ""
        return value
    }
    static func clearAll() { for entry in fields.values { entry.field?.stringValue = "" }; fields.removeAll() }
}

/// NSSecureTextField: masked, no copy or cut, the system's secure input while focused; the
/// accessible name is the request's prompt. Enter submits and Escape cancels, as the reference's
/// form and dialog do; nothing typed leaves the view except through `T3SshPasswordFields.take`.
final class T3SshPasswordView: NSSecureTextField, NSTextFieldDelegate {
    weak var owner: T3SshPasswordField?
    fileprivate weak var previousResponder: NSResponder?
    override func viewDidMoveToWindow() {
        super.viewDidMoveToWindow()
        guard let window else { return }
        // Focused and selected when the dialog opens (the reference's requestAnimationFrame focus + select).
        DispatchQueue.main.async { [weak self] in
            guard let self, self.window === window else { return }
            if self.previousResponder == nil, let current = window.firstResponder, current !== self.currentEditor() { self.previousResponder = current }
            window.makeFirstResponder(self)
            self.selectText(nil)
        }
    }
    override func performKeyEquivalent(with event: NSEvent) -> Bool {
        // ⌘C / ⌘X on a password copy nothing (the secure field editor refuses them; swallow them here too).
        if currentEditor() != nil, event.modifierFlags.contains(.command), ["c", "x"].contains(event.charactersIgnoringModifiers?.lowercased() ?? "") { return true }
        return super.performKeyEquivalent(with: event)
    }
    func control(_ control: NSControl, textView: NSTextView, doCommandBy commandSelector: Selector) -> Bool {
        if commandSelector == #selector(NSResponder.insertNewline(_:)) { owner?.events.submit(); return true }
        if commandSelector == #selector(NSResponder.cancelOperation(_:)) { owner?.events.key("Escape"); return true }
        return false
    }
    /// Gives the focus back to what held it before the dialog, when that is still in the window.
    func restoreFocus() {
        guard let window, let previous = previousResponder else { return }
        if let view = previous as? NSView, view.window !== window { return }
        window.makeFirstResponder(previous)
    }
}

final class T3SshPasswordField: ExactNativeInstance {
    private let field = T3SshPasswordView(frame: .zero)
    private var requestId = ""
    init(props: [String: String], events: ExactNativeEvents) {
        super.init(events: events)
        field.owner = self
        field.delegate = field
        field.isBordered = false; field.drawsBackground = false; field.focusRingType = .none
        field.font = NSFont.systemFont(ofSize: 14); field.lineBreakMode = .byClipping
        field.contentType = .password // autocomplete="current-password"
        apply(props)
    }
    override var view: ExactNativeView { field }
    override var focusTarget: ExactNativeView? { field }
    private func apply(_ props: [String: String]) {
        let id = props["request"] ?? ""
        if id != requestId {
            if !requestId.isEmpty { T3SshPasswordFields.unregister(requestId, field) }
            field.stringValue = ""
            requestId = id
            if !id.isEmpty { T3SshPasswordFields.register(id, field) }
        }
        field.setAccessibilityLabel(props["prompt"] ?? "SSH password")
        field.placeholderString = nil
        field.isEnabled = props["locked"] != "true"
    }
    override func setProps(_ props: [String: String]) throws { apply(props) }
    override func agentInput(_ input: ExactNativeInput) throws {
        switch input {
        case .text(let value): field.stringValue = value
        case .key(let chord, let phase):
            if phase == "up" { return }
            switch chord {
            case "Enter": events.submit()
            case "Escape": events.key("Escape")
            case "Tab": field.window?.selectNextKeyView(nil)
            default: throw ExactNativeRefusal("the SSH password field takes text, Enter, Escape and Tab")
            }
        }
    }
    override func destroy() {
        // The field forgets what was typed when the dialog goes away.
        field.stringValue = ""
        if !requestId.isEmpty { T3SshPasswordFields.unregister(requestId, field) }
        field.restoreFocus()
    }
}
