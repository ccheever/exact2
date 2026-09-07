// Contract textarea: UITextView keeps Enter as a newline, never a submission.
#if os(iOS)
import UIKit

final class TextArea: UITextView {
    var placeholder = "" { didSet { setNeedsDisplay() } }
    override func draw(_ rect: CGRect) {
        super.draw(rect)
        if text.isEmpty, !placeholder.isEmpty {
            (placeholder as NSString).draw(in: bounds, withAttributes: [
                .font: font ?? UIFont.systemFont(ofSize: 16),
                .foregroundColor: (textColor ?? .black).withAlphaComponent(0.3),
            ])
        }
    }
}

extension NodeView {
    func makeTextArea() {
        let f = TextArea(frame: .zero)
        f.backgroundColor = .clear
        f.textContainerInset = .zero
        f.textContainer.lineFragmentPadding = 0
        f.delegate = self
        addSubview(f)
        textArea = f
    }
    func applyTextArea() {
        guard let f = textArea else { return }
        let value = props["value"] ?? ""
        if f.text != value { f.text = value }
        f.isEditable = !disabled && props["editable"] != "false"
        f.isSelectable = !disabled
        f.accessibilityLabel = props["accessibilityLabel"]
        f.accessibilityIdentifier = props["testId"]
        (f as? TextArea)?.placeholder = props["placeholder"] ?? ""
    }
    func styleTextArea() {
        guard let f = textArea, let t = text else { return }
        f.font = t.font(size: number("font_size", 16), weight: Int(number("font_weight", 400)), family: Int(number("font_family")), italic: (style["font_style"] as? String) == "italic")
        f.textColor = color("text_color", .black)
        layoutTextArea()
    }
    func layoutTextArea() { textArea?.frame = fieldBox() }
    func textViewDidChange(_ textView: UITextView) {
        textView.setNeedsDisplay()
        if !disabled, handlers.contains("change") { presenter?.change(id, textView.text ?? "") }
    }
    func textViewDidBeginEditing(_ textView: UITextView) {
        presenter?.editing = self
        if handlers.contains("focus") { presenter?.focus(id) }
        presenter?.reveal(self)
    }
    func textViewDidEndEditing(_ textView: UITextView) {
        if presenter?.editing === self { presenter?.editing = nil }
        if handlers.contains("blur") { presenter?.blur(id) }
    }
}
#endif
