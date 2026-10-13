// @ref LLP 1069.001 D5 — `input type="date|time|datetime-local"` is a
// `UIDatePicker` in its compact style: the value in place, the calendar or
// wheel on a tap, formatted in the reported locale. HTML's values carry no
// zone, so the picker reads and writes them at UTC and never converts; a
// choice is HTML's `input` then `change`. The bound value is written into it
// when it changes, as the web build writes an input's `value`: an action
// that has not written it yet (a `send` whose reply is in flight) leaves the
// person's choice showing (x2apps kanban2 #5).
#if os(iOS)
import UIKit

extension ControlHost {
    func makeDate(_ kind: String) -> UIControl {
        let picker = BoundDatePicker()
        picker.preferredDatePickerStyle = .compact
        picker.datePickerMode = kind == "date" ? .date : kind == "time" ? .time : .dateAndTime
        picker.timeZone = DateValue.utc
        picker.addTarget(self, action: #selector(dateChanged(_:)), for: .valueChanged)
        return picker
    }

    func configureDate(_ picker: UIDatePicker, _ owner: NodeView, accent: UIColor?) {
        let kind = kinds[owner.id] ?? "date"
        assign(picker, \.tintColor, accent)
        assign(picker, \.minimumDate, owner.props["min"].flatMap { DateValue.parse(kind, $0) })
        assign(picker, \.maximumDate, owner.props["max"].flatMap { DateValue.parse(kind, $0) })
        let bound = owner.props["value"] ?? ""
        (picker as? BoundDatePicker)?.placeholder = DateValue.placeholder(owner.props)
        if let picker = picker as? BoundDatePicker {
            guard picker.applied != bound else { return }
            picker.applied = bound
        }
        let value = DateValue.parse(kind, bound)
        // No date is a state a picker cannot show: it is recorded, and read
        // back as "" until a value is applied or chosen (b6 review B7; the
        // Mac's `DateField.empty`).
        (picker as? BoundDatePicker)?.empty = value == nil
        if let value, picker.date != value {
            picker.setDate(value, animated: false)
        }
    }

    @objc func dateChanged(_ picker: UIDatePicker) {
        let id = UInt32(picker.tag)
        guard presenter.views[id] != nil, let kind = kinds[id] else { return }
        (picker as? BoundDatePicker)?.empty = false
        presenter.controlValue(id, DateValue.format(kind, picker.date), input: true, change: true)
        if let owner = presenter.views[id] { configureDate(picker, owner, accent: picker.tintColor) }
    }

    /// The agent's `type <id> <value>` (D9): the value a pick reports, in
    /// HTML's format; the runner holds it to the format and `min`/`max`.
    func typeDate(_ picker: UIDatePicker, _ node: NodeView, _ text: String) -> [String: Any] {
        let kind = kinds[node.id] ?? "date"
        // An empty value clears it, as deleting every segment does on the web.
        if text.isEmpty {
            (picker as? BoundDatePicker)?.empty = true
            presenter.controlValue(node.id, "", input: true, change: true)
            return ["typed": Int(node.id), "value": presenter.views[node.id]?.props["value"] ?? "", "delivery": "host-activation", "native": "control"]
        }
        guard let date = DateValue.parse(kind, text) else {
            return ["error": "\"\(text)\" is not a \(kind) value (HTML's format, as 2026-09-27, 14:30 or 2026-09-27T14:30)"]
        }
        picker.setDate(date, animated: false)
        (picker as? BoundDatePicker)?.empty = false
        presenter.controlValue(node.id, text, input: true, change: true)
        if let owner = presenter.views[node.id] { configureDate(picker, owner, accent: picker.tintColor) }
        // What it shows, as the web's reply says: the choice, or what the
        // action wrote over it.
        return ["typed": Int(node.id), "value": DateValue.shown(kind, picker), "delivery": "host-activation", "native": "control"]
    }
}

/// A picker that knows the bound value last written into it (`configureDate`).
/// HTML's empty date, time or datetime is a state a `UIDatePicker`, which
/// always holds a date, cannot show: it showed today (bench t7-wizard,
/// 2026-10-08). Its own content is masked out, still taking the tap that
/// opens the calendar, under the platform's pill, in the same system
/// fill, which holds the author's `placeholder` in the placeholder colour
/// and, with none, no words at all (LLP 1115.000 D13.1): a date format there
/// (Chrome's `mm/dd/yyyy`, or the locale's own) is a browser's empty field
/// drawn on a control that has none. The pill is layers, not a view, so it
/// takes no touch and no part in Auto Layout or the picker's measured size.
/// Opening it starts at today, and only a choice commits it.
final class BoundDatePicker: UIDatePicker {
    var applied: String?
    /// The author's `placeholder`, "" when there is none.
    var placeholder = "" { didSet { if placeholder != oldValue, empty { setNeedsLayout() } } }
    /// No date: an empty bound value or a cleared one, which the picker's
    /// own `date` cannot say.
    var empty = false { didSet { if empty != oldValue { setNeedsLayout() } } }
    private let pill = CALayer(), words = CATextLayer()
    private static let nothing = "exact.empty-date"
    override func layoutSubviews() {
        super.layoutSubviews()
        // UIKit rebuilds its content as the mode or date changes, so the
        // mask is renewed here. A mask that shows nothing hides today's date
        // whole and takes no part in hit testing, so a tap still opens the
        // calendar. (It had been faded to 2%, which iOS 26's glass label
        // still showed: a ghost of today's date under the placeholder.)
        for view in subviews {
            if empty, view.layer.mask?.name != Self.nothing {
                let mask = CALayer()
                mask.name = Self.nothing
                view.layer.mask = mask
            } else if !empty, view.layer.mask?.name == Self.nothing {
                view.layer.mask = nil
            }
        }
        CATransaction.begin(); CATransaction.setDisableActions(true)
        defer { CATransaction.commit() }
        guard empty else { pill.removeFromSuperlayer(); return }
        if pill.superlayer !== layer {
            pill.zPosition = 1
            pill.addSublayer(words)
            words.alignmentMode = .center
            words.truncationMode = .end
            layer.addSublayer(pill)
        }
        let frame = subviews.map(\.frame).reduce(CGRect.null) { $0.union($1) }
        pill.frame = frame.isNull ? bounds : frame
        pill.cornerRadius = min(pill.frame.height / 2, 8)
        pill.backgroundColor = UIColor.tertiarySystemFill.resolvedColor(with: traitCollection).cgColor
        let font = UIFont.preferredFont(forTextStyle: .body, compatibleWith: traitCollection)
        words.string = placeholder
        words.font = font
        words.fontSize = font.pointSize
        words.foregroundColor = UIColor.placeholderText.resolvedColor(with: traitCollection).cgColor
        words.contentsScale = window?.screen.scale ?? traitCollection.displayScale
        words.frame = CGRect(x: 0, y: (pill.bounds.height - font.lineHeight) / 2, width: pill.bounds.width, height: font.lineHeight)
    }
    override func traitCollectionDidChange(_ previous: UITraitCollection?) {
        super.traitCollectionDidChange(previous)
        if empty { setNeedsLayout() }
    }
    /// The pill as shown, for a test: its words and its frame.
    var placeholderShown: (text: String, frame: CGRect)? {
        pill.superlayer === layer ? ((words.string as? String) ?? "", pill.frame) : nil
    }
    override var accessibilityValue: String? {
        get { empty ? "" : super.accessibilityValue }
        set { super.accessibilityValue = newValue }
    }
}

extension DateValue {
    /// What a picker shows as HTML's value: "" while it holds no date.
    static func shown(_ kind: String, _ picker: UIDatePicker) -> String {
        (picker as? BoundDatePicker)?.empty == true ? "" : format(kind, picker.date)
    }
}
#endif
