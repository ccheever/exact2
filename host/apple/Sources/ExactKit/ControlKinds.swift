// @ref LLP 1069.001 D2, D5 — which platform control a `Control` node is, and
// a select's menu as the runtime reads it from the kernel: both hosts'
// presenters build the platform's own control from these.
import Foundation

enum ControlKinds {
    /// The chrome index's keys for the controls the presenter projects.
    static let indexed = ["type:checkbox", "type:select"]
    /// `switch`, `checkbox` or the `type` prop's value.
    static func kind(_ props: [String: String]) -> String {
        switch props["type"] {
        case "select": return "select"
        default: return props["accessibilityRole"] == "switch" ? "switch" : "checkbox"
        }
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
