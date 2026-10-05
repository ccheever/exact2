// @ref LLP 1069.001 D5 — `input type="date|time|datetime-local"` is an
// `NSDatePicker`, a text field and stepper, formatted in the reported
// locale. HTML's values carry no zone, so the picker reads and writes them
// at UTC and never converts; an edit is HTML's `input` then `change`, and
// the committed value is what it shows after the action (D4).
#if os(macOS)
import AppKit

extension ControlHost {
    func makeDate(_ kind: String) -> NSControl {
        let picker = DateField()
        picker.datePickerStyle = .textFieldAndStepper
        picker.datePickerElements = kind == "date" ? .yearMonthDay : kind == "time" ? .hourMinute : [.yearMonthDay, .hourMinute]
        picker.timeZone = DateValue.utc
        picker.isBezeled = true
        return picker
    }

    func configureDate(_ picker: NSDatePicker, _ owner: NodeView, accent: NSColor?) {
        let kind = kinds[owner.id] ?? "date"
        picker.minDate = owner.props["min"].flatMap { DateValue.parse(kind, $0) }
        picker.maxDate = owner.props["max"].flatMap { DateValue.parse(kind, $0) }
        let value = owner.props["value"].flatMap { DateValue.parse(kind, $0) }
        if let field = picker as? DateField {
            field.placeholder = DateField.placeholder(kind, picker.locale ?? .current)
            field.kind = kind
            field.empty = value == nil
        }
        if let value, picker.dateValue != value {
            picker.dateValue = value
        }
    }

    func dateChanged(_ picker: NSDatePicker) {
        let id = UInt32(picker.tag)
        guard presenter.views[id] != nil, let kind = kinds[id] else { return }
        presenter.controlValue(id, DateValue.format(kind, picker.dateValue), input: true, change: true)
        if let owner = presenter.views[id] { configureDate(picker, owner, accent: nil) }
    }

    /// The agent's `type <id> <value>` (D9): the value an edit reports, in
    /// HTML's format; the runner holds it to the format and `min`/`max`.
    func typeDate(_ picker: NSDatePicker, _ node: NodeView, _ text: String) -> [String: Any] {
        let kind = kinds[node.id] ?? "date"
        // An empty value clears it, as deleting every segment does on the web.
        if text.isEmpty {
            presenter.controlValue(node.id, "", input: true, change: true)
            if let owner = presenter.views[node.id] { configureDate(picker, owner, accent: nil) }
            return ["typed": Int(node.id), "value": presenter.views[node.id]?.props["value"] ?? "", "delivery": "host-activation", "native": "control"]
        }
        guard let date = DateValue.parse(kind, text) else {
            return ["error": "\"\(text)\" is not a \(kind) value (HTML's format, as 2026-09-27, 14:30 or 2026-09-27T14:30)"]
        }
        picker.dateValue = date
        presenter.controlValue(node.id, text, input: true, change: true)
        let shown = presenter.views[node.id]?.props["value"] ?? ""
        if let owner = presenter.views[node.id] { configureDate(picker, owner, accent: nil) }
        if shown != text { return ["error": "\(kind) #\(node.id) refused \"\(text)\" (see logs); it shows \"\(shown)\""] }
        return ["typed": Int(node.id), "value": shown, "delivery": "host-activation", "native": "control"]
    }
}

/// HTML's empty date, time or datetime: the format as a placeholder
/// (Chrome's `mm/dd/yyyy`, `--:-- --`), where an `NSDatePicker`, which
/// always holds a date, showed its reference date, 1/1/2001 (x2apps kanban
/// F23). Editing an empty one starts at now, and only a change commits it.
final class DateField: NSDatePicker {
    var kind = "date"
    var placeholder = ""
    var empty = false { didSet { if empty != oldValue { refresh() } } }
    private var editing: Bool {
        guard let responder = window?.firstResponder as? NSView else { return false }
        return responder === self || responder.isDescendant(of: self)
    }
    private func refresh() {
        textColor = empty && !editing ? .clear : .controlTextColor
        needsDisplay = true
    }
    override func becomeFirstResponder() -> Bool {
        guard super.becomeFirstResponder() else { return false }
        if empty {
            // Now, as the picker reads its values: the wall clock's fields at UTC.
            let local = DateFormatter()
            local.dateFormat = "yyyy-MM-dd'T'HH:mm"
            dateValue = DateValue.parse("datetime-local", local.string(from: Date())) ?? dateValue
        }
        refresh()
        return true
    }
    override func resignFirstResponder() -> Bool {
        guard super.resignFirstResponder() else { return false }
        DispatchQueue.main.async { [weak self] in self?.refresh() }
        return true
    }
    override func draw(_ dirtyRect: NSRect) {
        super.draw(dirtyRect)
        guard empty, !editing else { return }
        let attributes: [NSAttributedString.Key: Any] = [.font: font ?? .systemFont(ofSize: NSFont.systemFontSize),
                                                         .foregroundColor: NSColor.placeholderTextColor]
        let height = placeholder.size(withAttributes: attributes).height
        placeholder.draw(at: NSPoint(x: 4, y: (bounds.height - height) / 2), withAttributes: attributes)
    }
    override func accessibilityValue() -> Any? { empty ? "" : super.accessibilityValue() }

    /// The locale's order of fields, written as Chrome writes an empty one.
    static func placeholder(_ kind: String, _ locale: Locale) -> String {
        func written(_ template: String) -> String {
            let pattern = DateFormatter.dateFormat(fromTemplate: template, options: 0, locale: locale) ?? ""
            var out = "", last: Character?
            for c in pattern where c != "'" {
                if c == last, c.isLetter { continue }
                last = c
                switch c {
                case "y": out += "yyyy"
                case "M", "L": out += "mm"
                case "d": out += "dd"
                case "h", "H", "k", "K", "m", "a": out += "--"
                // A narrow no-break space before `a` too, as Chrome spaces it.
                default: if c.isWhitespace { out += " " } else if !c.isLetter { out.append(c) }
                }
            }
            return out
        }
        switch kind {
        case "date": return written("yMMdd")
        case "time": return written("jmm")
        default: return written("yMMdd") + ", " + written("jmm")
        }
    }
}
#endif
