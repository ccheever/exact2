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
        #if !os(tvOS)
        if let chrome = searchChrome {
            chrome.frame = field.frame
            if chrome.font != field.font { chrome.font = field.font }
        }
        #endif
        field.setNeedsLayout()
    }
    func styleNativeField() {
        guard let field else { return }
        #if !os(tvOS)
        let search = isNativeTextControl && props["type"] == "search"
        field.borderStyle = isNativeTextControl && !search ? .roundedRect : .none
        field.adjustsFontForContentSizeCategory = isNativeTextControl
        field.clearButtonMode = search ? .whileEditing : .never
        if search, searchChrome == nil {
            insertSubview(SearchChrome(), belowSubview: field)
        } else if !search, let chrome = searchChrome {
            chrome.removeFromSuperview()
        }
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

#if !os(tvOS)
/// An in-content `input type="search"`'s chrome (LLP 1115 wave 2): UIKit's
/// own `UISearchTextField`, empty and behind the editing field, so the fill,
/// the shape and the magnifier are the platform's on every iOS version. The
/// field above it edits, draws the text and placeholder where the kernel put
/// them (measured from this same class, `FieldChromeCache`, kind 2), and
/// shows the clear button while editing where this one would.
final class SearchChrome: UISearchTextField {
    override init(frame: CGRect) {
        super.init(frame: frame)
        isUserInteractionEnabled = false
        isAccessibilityElement = false
        accessibilityElementsHidden = true
        clearButtonMode = .always
    }
    required init?(coder: NSCoder) { nil }
}

extension NodeView {
    /// The search chrome behind this node's field, when it is a native search field.
    var searchChrome: SearchChrome? { subviews.lazy.compactMap { $0 as? SearchChrome }.first }
}
#endif

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
