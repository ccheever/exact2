// @ref LLP 1069.001 D5 — `input type="date|time|datetime-local"` is a
// `UIDatePicker` in its compact style: the value in place, the calendar or
// wheel on a tap, formatted in the reported locale. HTML's values carry no
// zone, so the picker reads and writes them at UTC and never converts; a
// choice is HTML's `input` then `change`, and the committed value is what
// it shows after the action (D4).
#if os(iOS)
import UIKit

extension ControlHost {
    func makeDate(_ kind: String) -> UIControl {
        let picker = UIDatePicker()
        picker.preferredDatePickerStyle = .compact
        picker.datePickerMode = kind == "date" ? .date : kind == "time" ? .time : .dateAndTime
        picker.timeZone = DateValue.utc
        picker.addTarget(self, action: #selector(dateChanged(_:)), for: .valueChanged)
        return picker
    }

    func configureDate(_ picker: UIDatePicker, _ owner: NodeView, accent: UIColor?) {
        let kind = kinds[owner.id] ?? "date"
        picker.tintColor = accent
        picker.minimumDate = owner.props["min"].flatMap { DateValue.parse(kind, $0) }
        picker.maximumDate = owner.props["max"].flatMap { DateValue.parse(kind, $0) }
        if let value = owner.props["value"].flatMap({ DateValue.parse(kind, $0) }), picker.date != value {
            picker.setDate(value, animated: false)
        }
    }

    @objc func dateChanged(_ picker: UIDatePicker) {
        let id = UInt32(picker.tag)
        guard presenter.views[id] != nil, let kind = kinds[id] else { return }
        presenter.controlValue(id, DateValue.format(kind, picker.date), input: true, change: true)
        if let owner = presenter.views[id] { configureDate(picker, owner, accent: picker.tintColor) }
    }

    /// The agent's `type <id> <value>` (D9): the value a pick reports, in
    /// HTML's format; the runner holds it to the format and `min`/`max`.
    func typeDate(_ picker: UIDatePicker, _ node: NodeView, _ text: String) -> [String: Any] {
        let kind = kinds[node.id] ?? "date"
        guard let date = DateValue.parse(kind, text) else {
            return ["error": "\"\(text)\" is not a \(kind) value (HTML's format, as 2026-09-27, 14:30 or 2026-09-27T14:30)"]
        }
        picker.setDate(date, animated: false)
        presenter.controlValue(node.id, text, input: true, change: true)
        let shown = presenter.views[node.id]?.props["value"] ?? ""
        if let owner = presenter.views[node.id] { configureDate(picker, owner, accent: picker.tintColor) }
        if shown != text { return ["error": "\(kind) #\(node.id) refused \"\(text)\" (see logs); it shows \"\(shown)\""] }
        return ["typed": Int(node.id), "value": shown, "delivery": "host-activation", "native": "control"]
    }
}
#endif
