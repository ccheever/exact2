// @ref LLP 1104 D3–D6: native field chrome around the kernel's content box.
#if os(iOS) || os(tvOS)
import UIKit

extension NodeView {
    var isNativeTextControl: Bool {
        (field != nil || textArea != nil) && props["markup"] != "markdown" && style["appearance"]?.string != "none"
    }
    func nativeEditorRect(in proposed: CGRect) -> CGRect? {
        guard isNativeTextControl, let rect = nativeFieldContent else { return nil }
        // UIKit also asks with a synthetic 100×100 box to derive its insets.
        // Return the published insets applied to that argument, rather than
        // an absolute rect: the latter hid values or moved their baseline.
        // UIKit centres its single line within the resulting content box.
        return proposed.inset(by: UIEdgeInsets(top: rect.minY, left: rect.minX,
            bottom: max(0, bounds.height - rect.maxY), right: max(0, bounds.width - rect.maxX)))
    }
    func layoutField() {
        guard let field else { return }
        field.frame = isNativeTextControl ? bounds : contentBox()
        field.setNeedsLayout()
    }
    func styleNativeField() {
        guard let field else { return }
        #if !os(tvOS)
        field.borderStyle = isNativeTextControl ? .roundedRect : .none
        field.adjustsFontForContentSizeCategory = isNativeTextControl
        #endif
        if isNativeTextControl {
            field.defaultTextAttributes[.kern] = number("letter_spacing")
            field.tintColor = caretColor ?? channels("accent_color").map { TextEngine.color($0) }
            showFocusRing(false)
        }
        layoutField()
    }
    func applyFieldContent(_ payload: [String: Any]) {
        if let r = payload["rect"] as? [Double], r.count == 4 {
            nativeFieldContent = CGRect(x: r[0], y: r[1], width: r[2], height: r[3])
        } else { nativeFieldContent = nil }
        layoutField(); layoutTextArea()
    }
}

extension TextArea {
    func configureNativeChrome(_ native: Bool) {
        #if !os(tvOS)
        adjustsFontForContentSizeCategory = native
        layer.borderWidth = native ? FieldChromeCache.textareaStroke : 0
        layer.borderColor = native ? UIColor.separator.resolvedColor(with: traitCollection).cgColor : nil
        layer.cornerRadius = native ? FieldChromeCache.textareaRadius : 0
        // Use the text view's own default inset for its extended native look;
        // lineFragmentPadding stays zero so the published content rect is exact.
        let standard = UITextView(frame: .zero)
        var inset = standard.textContainerInset
        inset.left += standard.textContainer.lineFragmentPadding
        inset.right += standard.textContainer.lineFragmentPadding
        textContainerInset = native ? inset : .zero
        #endif
        textContainer.lineFragmentPadding = 0
    }
}
#endif
