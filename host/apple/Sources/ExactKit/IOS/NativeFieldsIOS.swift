// @ref LLP 1104 D3–D6: native field chrome around the kernel's content box.
#if os(iOS) || os(tvOS)
import UIKit

extension NodeView {
    var isNativeTextControl: Bool {
        (field != nil || textArea != nil) && props["markup"] != "markdown" && style["appearance"]?.string != "none"
    }
    /// A native field in a grouped list's row (LLP 1116.005 D2), as the
    /// kernel publishes it with the editor box: borderless, its cell being
    /// the field, as a hand-built settings form's is.
    var isCellField: Bool { isNativeTextControl && fieldInCell }
    /// Whether a touch a grouped row leaves to no control falls to the row's
    /// field (`cellField`). It widens the field's target to the cell, as a
    /// hand-built settings cell's field has it, and draws nothing: the
    /// field's frame is its kernel box, and a text or message beside or
    /// under it is not covered by it. `check` judges what lies over a view
    /// by where a finger at its middle lands, so it asks with this off
    /// (LLP 1116.009 D5): a title under its field read as covered by it.
    package static var touchFallsToCellField = true
    func nativeEditorRect(in proposed: CGRect) -> CGRect? {
        guard isNativeTextControl, let rect = kernelContentBox else { return nil }
        // UIKit also asks with a synthetic 100×100 box to derive its insets.
        // Return the published insets applied to that argument, rather than
        // an absolute rect: the latter hid values or moved their baseline.
        // UIKit centres its single line within the resulting content box.
        var insets = UIEdgeInsets(top: rect.minY, left: rect.minX,
            bottom: max(0, bounds.height - rect.maxY), right: max(0, bounds.width - rect.maxX))
        // A number input's stepper takes the text's trailing end (LLP 1116.007 D4).
        let room = stepperRoom
        if room > 0 { if style["direction"]?.string == "rtl" { insets.left += room } else { insets.right += room } }
        var content = proposed.inset(by: insets)
        content.size.width = max(0, content.width)
        return content
    }
    func layoutField() {
        guard let field else { return }
        field.frame = isNativeTextControl ? bounds : contentBox()
        layoutStepper()
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
        field.borderStyle = isNativeTextControl && !search && !isCellField ? .roundedRect : .none
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
        let r = payload["rect"] as? [Double]
        let box = r.flatMap { $0.count == 4 ? CGRect(x: $0[0], y: $0[1], width: $0[2], height: $0[3]) : nil }
        applyKernelContentBox(box)
        let cell = box != nil && payload["cell"] as? Bool == true
        if cell != fieldInCell {
            fieldInCell = cell
            if field != nil { styleNativeField() }
            if textArea != nil { styleTextArea() }
        }
    }
}

#if os(iOS)
extension NodeView {
    /// The cell is the field (LLP 1116.005 D2): a touch in a grouped list's
    /// cell that its row leaves to no control of its own (the cell's margins,
    /// a text beside or under the field) goes to the row's one field, as a
    /// hand-built settings cell's text field takes the whole cell. `hit` is
    /// what the cell's own hit-testing found, kept unless it is the cell,
    /// its content view or a box or text with no handler.
    package static func cellField(in cell: UICollectionViewCell, hit: UIView?) -> UIView? {
        guard let hit else { return nil }
        guard touchFallsToCellField else { return hit }
        if let node = hit as? NodeView {
            guard node.handlers.isEmpty, node.kind == "view" || node.kind == "text" else { return hit }
        } else if hit !== cell && hit !== cell.contentView {
            return hit
        }
        var found: [NodeView] = []
        func walk(_ view: UIView) {
            for sub in view.subviews where found.count < 2 && !sub.isHidden {
                if let node = sub as? NodeView, node.isCellField { found.append(node) } else { walk(sub) }
            }
        }
        walk(cell.contentView)
        guard found.count == 1, let only = found.first, !only.inert, !only.disabled,
              let editor: UIView = only.field ?? only.textArea else { return hit }
        return editor
    }
}
#endif

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
    /// `cell`: in a grouped list's row (LLP 1116.005 D2) the cell is its
    /// chrome: no line or corners, its text at the cell's margin where the
    /// row's other text starts, and the text view's own space above and below.
    func configureNativeChrome(_ native: Bool, cell: Bool = false) {
        #if !os(tvOS)
        adjustsFontForContentSizeCategory = native
        let bezel = native && !cell
        layer.borderWidth = bezel ? FieldChromeCache.textareaStroke : 0
        layer.borderColor = bezel ? UIColor.separator.resolvedColor(with: traitCollection).cgColor : nil
        layer.cornerRadius = bezel ? FieldChromeCache.textareaRadius : 0
        // Use the text view's own default inset for its extended native look;
        // lineFragmentPadding stays zero so the published content rect is exact.
        let standard = UITextView(frame: .zero)
        var inset = standard.textContainerInset
        inset.left = cell ? 0 : inset.left + standard.textContainer.lineFragmentPadding
        inset.right = cell ? 0 : inset.right + standard.textContainer.lineFragmentPadding
        textContainerInset = native ? inset : .zero
        #endif
        textContainer.lineFragmentPadding = 0
    }
}
#endif
