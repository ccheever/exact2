// @ref LLP 1069.001 D5 — `input type="date|time|datetime-local"` is an
// `NSDatePicker`, a text field and stepper, formatted in the reported
// locale. HTML's values carry no zone, so the picker reads and writes them
// at UTC and never converts; an edit is HTML's `input` then `change`. The
// bound value is written into it when it changes, as the web build writes
// an input's `value` and this host a select's or a text field's: an action
// that has not written it yet (a `send` whose reply is in flight) leaves the
// person's choice showing, where it used to clear it (x2apps kanban2 #5).
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
        let bound = owner.props["value"] ?? ""
        let value = DateValue.parse(kind, bound)
        guard let field = picker as? DateField else { return }
        field.placeholder = DateField.placeholder(kind, picker.locale ?? .current)
        field.kind = kind
        guard field.applied != bound else { return }
        field.applied = bound
        field.empty = value == nil
        if let value, picker.dateValue != value {
            picker.dateValue = value
        }
    }

    /// What it shows, in HTML's format: `""` while empty.
    func shownDate(_ picker: NSDatePicker, _ kind: String) -> String {
        (picker as? DateField)?.empty == true ? "" : DateValue.format(kind, picker.dateValue)
    }

    func dateChanged(_ picker: NSDatePicker) {
        let id = UInt32(picker.tag)
        guard presenter.views[id] != nil, let kind = kinds[id] else { return }
        (picker as? DateField)?.empty = false
        presenter.controlValue(id, DateValue.format(kind, picker.dateValue), input: true, change: true)
        if let owner = presenter.views[id] { configureDate(picker, owner, accent: nil) }
    }

    /// The agent's `type <id> <value>` (D9): the value an edit reports, in
    /// HTML's format; the runner holds it to the format and `min`/`max`.
    func typeDate(_ picker: NSDatePicker, _ node: NodeView, _ text: String) -> [String: Any] {
        let kind = kinds[node.id] ?? "date"
        // An empty value clears it, as deleting every segment does on the web.
        guard text.isEmpty || DateValue.parse(kind, text) != nil else {
            return ["error": "\"\(text)\" is not a \(kind) value (HTML's format, as 2026-09-27, 14:30 or 2026-09-27T14:30)"]
        }
        if let date = DateValue.parse(kind, text) { picker.dateValue = date }
        (picker as? DateField)?.empty = text.isEmpty
        presenter.controlValue(node.id, text, input: true, change: true)
        if let owner = presenter.views[node.id] { configureDate(picker, owner, accent: nil) }
        // What it shows, as the web's reply says: the choice, or what the
        // action wrote over it.
        return ["typed": Int(node.id), "value": shownDate(picker, kind), "delivery": "host-activation", "native": "control"]
    }
}

/// HTML's empty date, time or datetime: the format as a placeholder
/// (Chrome's `mm/dd/yyyy`, `--:-- --`), where an `NSDatePicker`, which
/// always holds a date, showed its reference date, 1/1/2001 (x2apps kanban
/// F23). Editing an empty one starts at now, and only a change commits it.
final class DateField: NSDatePicker {
    var kind = "date"
    var placeholder = ""
    /// The bound value last written into it (`configureDate`).
    var applied: String?
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
    static func placeholder(_ kind: String, _ locale: Locale) -> String { DateValue.placeholder(kind, locale) }
}
#endif
