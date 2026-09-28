// @ref LLP 1069.001 D2, D5 — which platform control a `Control` node is, and
// a select's menu as the runtime reads it from the kernel: both hosts'
// presenters build the platform's own control from these.
import Foundation

enum ControlKinds {
    /// The chrome index's keys for the controls the presenter projects.
    static let indexed = ["type:checkbox", "type:select", "type:range"]
    /// `switch`, `checkbox` or the `type` prop's value.
    static func kind(_ props: [String: String]) -> String {
        switch props["type"] {
        case "select": return "select"
        case "range": return "range"
        default: return props["accessibilityRole"] == "switch" ? "switch" : "checkbox"
        }
    }
}

/// A range's `min`, `max` and `step` by HTML's rules, and its value
/// clamped and snapped as HTML sanitizes it (`exact_kernel::Range`).
struct RangeSpec {
    var min = 0.0, max = 100.0
    var step: Double? = 1

    init(_ props: [String: String]) {
        let number = { (s: String?) in s.flatMap { Double($0.trimmingCharacters(in: .whitespaces)) }.flatMap { $0.isFinite ? $0 : nil } }
        min = number(props["min"]) ?? 0
        max = Swift.max(number(props["max"]) ?? 100, min)
        step = props["step"]?.trimmingCharacters(in: .whitespaces).lowercased() == "any" ? nil : (number(props["step"]).flatMap { $0 > 0 ? $0 : nil } ?? 1)
    }

    func sanitize(_ value: Double) -> Double {
        let clamped = Swift.min(Swift.max(value, min), max)
        guard let step else { return clamped }
        var snapped = min + ((clamped - min) / step + 0.5).rounded(.down) * step
        if snapped > max { snapped -= step }
        return Swift.min(Swift.max((snapped * 1e9).rounded() / 1e9, min), max)
    }

    /// The value it shows: its `value`, or the midpoint as HTML's default.
    func shown(_ props: [String: String]) -> Double {
        sanitize(props["value"].flatMap { Double($0) } ?? (min + (max - min) / 2))
    }

    /// A number as HTML writes one: no trailing `.0`.
    static func format(_ v: Double) -> String {
        v == v.rounded() && abs(v) < 1e15 ? String(Int64(v)) : String(v)
    }
}

/// A select's options and the one it shows (`exact_select_options`).
struct SelectMenu: Equatable {
    struct Option: Equatable { let value: String; let label: String; let disabled: Bool }
    var options: [Option] = []
    var chosen: Int?
    var chosenValue: String? { chosen.map { options[$0].value } }

    init(options: [Option] = [], chosen: Int? = nil) { self.options = options; self.chosen = chosen }
    init(json: Data) {
        guard let obj = try? JSONSerialization.jsonObject(with: json) as? [String: Any] else { return }
        options = (obj["options"] as? [[String: Any]] ?? []).map {
            Option(value: $0["value"] as? String ?? "", label: $0["label"] as? String ?? "", disabled: $0["disabled"] as? Bool ?? false)
        }
        chosen = (obj["chosen"] as? Int).flatMap { $0 < options.count ? $0 : nil }
    }

    /// The agent's `type <select> <value>` (LLP 1069.001 D9): the refusal
    /// when no enabled option has `value`.
    func refusal(_ value: String, id: UInt32) -> String? {
        if options.contains(where: { $0.value == value && !$0.disabled }) { return nil }
        return "select \(id) has no enabled option \"\(value)\" (options: \(options.map { "\"\($0.value)\"" }.joined(separator: ", ")))"
    }
}
