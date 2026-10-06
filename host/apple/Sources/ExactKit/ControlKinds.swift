// @ref LLP 1069.001 D2, D5 — which platform control a `Control` node is, and
// a select's menu as the runtime reads it from the kernel: both hosts'
// presenters build the platform's own control from these.
import Foundation
import MachO

enum ControlKinds {
    /// The chrome index's keys for the controls the presenter projects.
    static let indexed = ["type:checkbox", "type:radio", "type:select", "type:range", "type:date", "type:time", "type:datetime-local", "type:button"]
    static let dates: Set<String> = ["date", "time", "datetime-local"]
    /// `switch`, `checkbox`, or the `type` prop's value (`radio`, a select, …).
    static func kind(_ props: [String: String]) -> String {
        switch props["type"] {
        case "button": return "button" // LLP 1069.011 D3: before the checkbox default
        case "select": return "select"
        case "radio": return "radio" // x2apps survey #2
        case "range": return "range"
        case let t? where dates.contains(t): return t
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

/// A date control's value in HTML's format, read and written as the
/// platform's `Date` at UTC: HTML's values carry no zone, so the picker
/// shows the wall time the string names and never converts it.
enum DateValue {
    static let utc = TimeZone(identifier: "UTC")!
    private static func formatter(_ pattern: String) -> DateFormatter {
        let f = DateFormatter()
        f.locale = Locale(identifier: "en_US_POSIX")
        f.calendar = Calendar(identifier: .iso8601)
        f.timeZone = utc
        f.dateFormat = pattern
        return f
    }
    private static let date = formatter("yyyy-MM-dd"), time = formatter("HH:mm"), seconds = formatter("HH:mm:ss"),
                       local = formatter("yyyy-MM-dd'T'HH:mm"), localSeconds = formatter("yyyy-MM-dd'T'HH:mm:ss")

    static func parse(_ kind: String, _ s: String) -> Date? {
        switch kind {
        case "date": return date.date(from: s)
        case "time": return time.date(from: s) ?? seconds.date(from: String(s.prefix(8)))
        default: return local.date(from: s) ?? localSeconds.date(from: String(s.prefix(19)))
        }
    }

    static func format(_ kind: String, _ d: Date) -> String {
        switch kind {
        case "date": return date.string(from: d)
        case "time": return time.string(from: d)
        default: return local.string(from: d)
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

    /// The agent's `type <select> <value>` (LLP 1069.001 D9): the enabled
    /// option with that value, else the one option with that label, as the
    /// web host chooses (Playwright's `selectOption`; kanban F17, shop F10);
    /// else the refusal, naming the options.
    func choose(_ text: String, id: UInt32) -> (value: String?, refusal: String?) {
        let enabled = options.filter { !$0.disabled }
        if enabled.contains(where: { $0.value == text }) { return (text, nil) }
        let trimmed = text.trimmingCharacters(in: .whitespaces)
        let labelled = enabled.filter { $0.label.trimmingCharacters(in: .whitespaces) == trimmed }
        if labelled.count == 1 { return (labelled[0].value, nil) }
        let twice = labelled.count > 1 ? " (that label is on more than one option: choose by value)" : ""
        return (nil, "select \(id) has no enabled option \"\(text)\"\(twice) (options: \(enabled.map { "\"\($0.value)\" \"\($0.label)\"" }.joined(separator: ", ")))")
    }
}

/// A button's face (`exact_press_face`, LLP 1069.011.000 D1), custom or
/// native, as the kernel reads it from its children: its title, its symbol as
/// the platform names it, whether its image is a raster instead, whether the
/// image leads, its `aria-label`, whether its children fit (one text, one
/// image, nothing else); and a native button's `buttonStyles` row (LLP
/// 1069.011 D2) — each platform's draw, a `~` marking a stand-in.
struct ButtonFace: Equatable {
    /// Whether the node is a button at all; the rest is empty when not.
    var button = true
    var title: String?
    var symbol: String?
    var raster = false
    var leading = true
    var label: String?
    var fits = true
    var style = "bordered"
    var ios = "bordered"
    var iosBefore26 = "bordered"
    var macos = "push"
    var known = true

    init() {}
    init(json: Data) {
        guard let o = try? JSONSerialization.jsonObject(with: json) as? [String: Any] else { return }
        button = o["button"] as? Bool ?? true
        title = o["title"] as? String
        symbol = o["symbol"] as? String
        raster = o["raster"] as? Bool ?? false
        leading = o["leading"] as? Bool ?? true
        label = o["label"] as? String
        fits = o["fits"] as? Bool ?? true
        style = o["style"] as? String ?? "bordered"
        ios = o["ios"] as? String ?? "bordered"
        iosBefore26 = o["iosBefore26"] as? String ?? "bordered"
        macos = o["macos"] as? String ?? "push"
        known = o["known"] as? Bool ?? true
    }

    /// The text a platform entry shows (a menu row, a confirmation's action):
    /// the title, else the label (a symbol-only button's).
    var shown: String? { title ?? label }

    /// A platform name without its stand-in mark, and whether it had one.
    static func drawn(_ name: String) -> (name: String, standIn: Bool) {
        name.hasPrefix("~") ? (String(name.dropFirst()), true) : (name, false)
    }
}

/// Whether UIKit and AppKit draw this app in their 26 design (Liquid Glass):
/// they key it on the SDK the main executable records in `LC_BUILD_VERSION`,
/// not on the OS, so an app linked to record an older SDK
/// (`designRequiresCompatibility`, `host/apple/build.mjs`) draws as before 26
/// on iOS 27 and macOS 27, where a glass button configuration or bezel draws
/// no glass. Read once; callers also check the OS (`#available(… 26.0, *)`).
enum LinkedDesign {
    static let liquidGlass: Bool = {
        guard let header = _dyld_get_image_header(0) else { return false }
        var at = UnsafeRawPointer(header).advanced(by: MemoryLayout<mach_header_64>.size)
        for _ in 0..<header.pointee.ncmds {
            let command = at.loadUnaligned(as: load_command.self)
            if command.cmd == UInt32(LC_BUILD_VERSION) {
                return at.loadUnaligned(as: build_version_command.self).sdk >= 26 << 16
            }
            at = at.advanced(by: Int(command.cmdsize))
        }
        return false
    }()
}

