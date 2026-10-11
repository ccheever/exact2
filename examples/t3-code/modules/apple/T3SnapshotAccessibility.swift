import ApplicationServices
import Foundation

// Immutable identity plus a thread-safe cancellation signal shared with AX work.
final class T3SnapshotLifetime {
    let owner: String
    private let lock = NSLock()
    private var stopped = false
    init(owner: String) { self.owner = owner }
    var cancelled: Bool { lock.lock(); defer { lock.unlock() }; return stopped }
    @discardableResult func cancel(owner: String) -> Bool {
        guard owner == self.owner else { return false }
        lock.lock(); stopped = true; lock.unlock(); return true
    }
    func canPublish(owner: String, enabled: Bool, closing: Bool) -> Bool {
        !cancelled && self.owner == owner && enabled && !closing
    }
}

struct T3SnapshotBudget {
    let deadline: TimeInterval
    let now: () -> TimeInterval
    let cancelled: () -> Bool
    init(seconds: TimeInterval = 3, now: @escaping () -> TimeInterval = { ProcessInfo.processInfo.systemUptime }, cancelled: @escaping () -> Bool = { false }) {
        self.now = now; self.cancelled = cancelled; deadline = now() + seconds
    }
    var remaining: TimeInterval { cancelled() ? 0 : max(0, deadline - now()) }
}

// Flat-text is a reference-supported metadata format. Resolve only the selected
// window; a failed identity match never falls back to another focused window.
enum T3SnapshotAccessibility {
    static func uniqueWindow(_ candidates: [(UInt32?, CGRect?)], windowId: UInt32, bounds: CGRect) -> Int? {
        let exact = candidates.indices.filter { candidates[$0].0 == windowId }
        if !exact.isEmpty { return exact.count == 1 ? exact[0] : nil }
        let matches = candidates.indices.filter { index in
            guard candidates[index].0 == nil, let other = candidates[index].1 else { return false }
            return abs(other.minX - bounds.minX) < 1 && abs(other.minY - bounds.minY) < 1 && abs(other.width - bounds.width) < 1 && abs(other.height - bounds.height) < 1
        }
        return matches.count == 1 ? matches[0] : nil
    }
    static func read(owner: pid_t, windowId: UInt32, bounds: CGRect, cancelled: @escaping () -> Bool = { false }) -> [String: Any] {
        precondition(!Thread.isMainThread, "Accessibility capture must run off the UI thread")
        let budget = T3SnapshotBudget(cancelled: cancelled)
        guard bounds.width > 0, bounds.height > 0 else { return [:] }
        let app = AXUIElementCreateApplication(owner)
        func attribute(_ element: AXUIElement, _ name: String) -> CFTypeRef? {
            let remaining = budget.remaining
            guard remaining > 0 else { return nil }
            AXUIElementSetMessagingTimeout(element, Float(min(0.25, remaining)))
            var value: CFTypeRef?
            let result = AXUIElementCopyAttributeValue(element, name as CFString, &value)
            return result == .success && budget.remaining > 0 ? value : nil
        }
        guard let windows = attribute(app, kAXWindowsAttribute) as? [AXUIElement] else { return [:] }
        let identities = windows.map { window -> (UInt32?, CGRect?) in
            if let number = attribute(window, "AXWindowNumber") as? NSNumber { return (number.uint32Value, nil) }
            guard let position = attribute(window, kAXPositionAttribute), CFGetTypeID(position) == AXValueGetTypeID(),
                  let size = attribute(window, kAXSizeAttribute), CFGetTypeID(size) == AXValueGetTypeID() else { return (nil, nil) }
            var point = CGPoint.zero, extent = CGSize.zero
            guard AXValueGetValue(unsafeBitCast(position, to: AXValue.self), .cgPoint, &point),
                  AXValueGetValue(unsafeBitCast(size, to: AXValue.self), .cgSize, &extent) else { return (nil, nil) }
            return (nil, CGRect(origin: point, size: extent))
        }
        let selected = uniqueWindow(identities, windowId: windowId, bounds: bounds).map { windows[$0] }
        guard let selected else { return [:] }
        var pieces: [String] = [], count = 0, length = 0, truncated = false
        func visit(_ element: AXUIElement, depth: Int) {
            guard depth < 24, count < 2000, length < 32000, budget.remaining > 0 else { truncated = true; return }
            count += 1
            guard let role = attribute(element, kAXRoleAttribute) as? String else { return }
            let subrole = attribute(element, kAXSubroleAttribute) as? String ?? ""
            guard role != "AXSecureTextField", subrole != "AXSecureTextField" else { return }
            let textAttributes = role == "AXTextField" && subrole.isEmpty ? [kAXTitleAttribute, kAXDescriptionAttribute] : [kAXTitleAttribute, kAXDescriptionAttribute, kAXValueAttribute]
            for key in textAttributes {
                if let text = attribute(element, key) as? String, !text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty, pieces.last != text {
                    let remaining = max(0, 32000 - length - (pieces.isEmpty ? 0 : 1))
                    let value = String(text.prefix(remaining)); truncated = truncated || value.count < text.count
                    pieces.append(value); length += value.count + (pieces.count > 1 ? 1 : 0)
                }
            }
            if let children = attribute(element, kAXChildrenAttribute) as? [AXUIElement] { for child in children { visit(child, depth: depth + 1) } }
        }
        visit(selected, depth: 0)
        let text = pieces.joined(separator: "\n").trimmingCharacters(in: .whitespacesAndNewlines)
        return text.isEmpty || cancelled() ? [:] : ["accessibleText": text, "accessibility": ["format": "flat-text", "text": text, "truncated": truncated]]
    }
}
