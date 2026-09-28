// @ref LLP 1069.001 D5 — `input type="date|time|datetime-local"` is an
// `NSDatePicker`, a text field and stepper, formatted in the reported
// locale. HTML's values carry no zone, so the picker reads and writes them
// at UTC and never converts; an edit is HTML's `input` then `change`, and
// the committed value is what it shows after the action (D4).
#if os(macOS)
import AppKit

extension ControlHost {
    func makeDate(_ kind: String) -> NSControl {
        let picker = NSDatePicker()
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
        if let value = owner.props["value"].flatMap({ DateValue.parse(kind, $0) }), picker.dateValue != value {
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
#endif
