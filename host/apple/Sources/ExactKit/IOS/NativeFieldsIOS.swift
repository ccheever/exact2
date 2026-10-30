// @ref LLP 1104 D3–D6: native field chrome around the kernel's content box.
#if os(iOS) || os(tvOS)
import UIKit

extension NodeView {
    /// UIKit's field for an `input`.
    func makeField() {
        let f = TextField(frame: .zero)
        f.owner = self
        f.borderStyle = .none
        f.backgroundColor = .clear
        f.delegate = self
        f.addTarget(self, action: #selector(fieldChanged), for: .editingChanged)
        addSubview(f)
        field = f
    }
    /// An `input` made inside a closed `dialog` or popover, which the
    /// kernel has not laid out (`MenuHost.layOut`), has no UIKit field
    /// until its box first shows or a prompt takes its keyboard from it
    /// (`PromptField`): made then, with the props and style it has.
    func makeWaitingField() {
        guard fieldWaits else { return }
        fieldWaits = false
        makeField()
        applyFieldStyle()
        if let field { applyField(field, set: props, clear: []) }
        layoutField()
    }
    /// HTML's `enterkeyhint` as UIKit's return key, for an `input` and a
    /// `textarea` alike; `unsaid` is the key with no hint.
    static func returnKey(_ hint: String?, unsaid: UIReturnKeyType) -> UIReturnKeyType {
        switch hint {
        case "done": .done
        case "go": .go
        case "next": .next
        case "search": .search
        case "send": .send
        case "enter", "previous": .default
        default: unsaid
        }
    }
    /// The props an `input`'s field shows: `set` and `clear` are what changed.
    func applyField(_ f: UITextField, set: [String: String], clear: [String]) {
        #if os(iOS)
        f.pasteDelegate = FieldSelections.tracks(self) ? self : nil
        #endif
        f.tintColor = props["emojiPicker"] == "true" ? .clear : nil
        if (set["emojiPicker"] != nil || clear.contains("emojiPicker")), f.isFirstResponder { f.reloadInputViews() }
        if let v = props["value"] { writeValue(v, into: f) }
        applyPlaceholder(f); applyFieldName(f)
        // The web's `type` and `inputmode`, as UIKit spells them.
        let type = props["type"] ?? "text"
        f.isSecureTextEntry = type == "password"
        // `autocomplete` names the field to AutoFill over what `type` implies (LLP 1102 §3.6).
        let content = Autofill.contentType(props["autocomplete"], fallback: type == "password" ? .password : type == "email" ? .emailAddress : nil)
        let traitsChanged = f.autocapitalizationType != inputCapitalization || f.autocorrectionType != inputCorrection || f.spellCheckingType != inputSpellChecking || f.smartQuotesType != inputSmartQuotes || f.smartDashesType != inputSmartDashes || f.textContentType != content
        f.textContentType = content
        f.autocapitalizationType = inputCapitalization
        f.autocorrectionType = inputCorrection
        f.spellCheckingType = inputSpellChecking
        f.smartQuotesType = inputSmartQuotes
        f.smartDashesType = inputSmartDashes
        let keyboard: UIKeyboardType = switch props["inputMode"] ?? type {
        case "email": .emailAddress
        case "numeric": .numberPad
        case "number": wholeNumbers ? .numberPad : .decimalPad
        case "decimal": .decimalPad
        case "tel": .phonePad
        case "url": .URL
        case "search": .webSearch
        default: .default
        }
        let keyboardChanged = f.keyboardType != keyboard
        f.keyboardType = keyboard
        let barChanged = applyPadBar() // a pad's missing keys its field asks for: ±, Done (LLP 1116.003 D7)
        // A keyboard that is up takes the field's traits, type and bar at once
        if traitsChanged || keyboardChanged || barChanged, f.isFirstResponder { f.reloadInputViews() }
        // HTML's `enterkeyhint` labels the return key (feed F25); UIKit
        // has no `previous`, which keeps the default. Unsaid, it is Done
        // (Search on a search field, as `UISearchBar`'s), a `submit`
        // handler or not: Return puts the keyboard away from a single-line
        // field unless the action focuses it again (D9.2), so Done says
        // what it does (LLP 1116.019 D5). The document keeps no hint, so
        // the web's keyboard stays its own.
        f.returnKeyType = NodeView.returnKey(props["enterKeyHint"], unsaid: type == "search" ? .search : .done)
        f.isEnabled = !disabled
        if headerField != nil { presenter?.menus.promptInputChanged(self) } // a prompt's field stands in for it (PromptFieldIOS.swift)
        if isNativeTextControl { styleNativeField() }
        updateNumberStepper() // the platform's stepping control for `type="number"` (LLP 1116.007 D4)
    }
    /// Whether a `type="number"` field takes only whole numbers, so the
    /// number pad, with no decimal point, serves it (LLP 1116.019 D5): its
    /// `step` is unsaid (HTML's default is 1) or whole, and its `min`, the
    /// step's base, is unsaid or whole; a value HTML cannot parse counts as
    /// unsaid, as an invalid step is HTML's 1. `step="any"`, a fractional
    /// step or a fractional min keeps the decimal pad. A negative min gets
    /// the bar's ± either way (`PadBarIOS`).
    var wholeNumbers: Bool {
        func whole(_ key: String) -> Bool {
            guard let n = props[key].flatMap({ Double($0.trimmingCharacters(in: .whitespaces)) }), n.isFinite else { return true }
            return n == n.rounded()
        }
        return props["step"]?.trimmingCharacters(in: .whitespaces).lowercased() != "any" && whole("step") && whole("min")
    }
    /// The style an `input`'s field shows.
    func applyFieldStyle() {
    field?.textAlignment = NSTextAlignment(rawValue: textAlignmentCode) ?? .left
    if let f = field, let t = text {
        f.font = t.font(size: number("font_size", PageFacts.defaultRootFontSize), weight: Int(number("font_weight", 400)), family: Int(number("font_family")), italic: (style["font_style"]?.string) == "italic", numeric: Int(number("font_variant_numeric")),
            stretch: CGFloat(number("font_stretch", 100)), variations: FontAxis.of(style))
        f.textColor = fieldInk
        applyPlaceholder(f)
        styleNativeField()
        updateNumberStepper() // `appearance` may have changed (LLP 1116.007 D4)
        applyFieldDirection()
    }
    }
    /// The node's `direction` as the text's base, at its side (LLP 1001 §1),
    /// in its field and in the one standing in for it (a search bar's, an
    /// alert prompt's: `headerField`). UIKit sets default attributes over the
    /// whole text, a composition's marked text too, so not while one is
    /// marked: its end (`unmarkText`, `fieldChanged`) applies it.
    func applyFieldDirection() {
        let alignment = NSTextAlignment(rawValue: textAlignmentCode) ?? .left
        for f in [field, headerField].compactMap({ $0 }) where f.markedTextRange == nil {
            if f.textAlignment != alignment { f.textAlignment = alignment } // a stand-in's too: CSS's side, not UIKit's
            let paragraph = (f.defaultTextAttributes[.paragraphStyle] as? NSParagraphStyle)?.mutableCopy() as? NSMutableParagraphStyle ?? NSMutableParagraphStyle()
            guard paragraph.baseWritingDirection != fieldBaseDirection || paragraph.alignment != alignment else { continue }
            paragraph.baseWritingDirection = fieldBaseDirection
            paragraph.alignment = alignment
            f.defaultTextAttributes[.paragraphStyle] = paragraph
        }
    }
    /// A plain field's paragraph base direction: CSS `direction`, said either
    /// way as Apple text's is (`TextEngine.setBaseDirection`), so no first
    /// strong character picks it (LLP 1001 §1).
    var fieldBaseDirection: NSWritingDirection {
        style["direction"]?.string == "rtl" ? .rightToLeft : .leftToRight
    }
    /// A field's ground where it writes none: the design's `Field` where it
    /// overrides that role (LLP 1121 D9), else none, as before (the page
    /// shows through the box Exact paints; a search field's chrome keeps
    /// UIKit's fill).
    var fieldGroundColor: UIColor? {
        guard isNativeTextControl, !wrote("background_color") else { return nil }
        return ThemeRoles.color("Field", in: self)
    }
    /// The ground the node's layer paints under its field: none where a
    /// search field's chrome fills it (`styleNativeField`).
    var fieldGround: CGColor? {
        #if !os(tvOS)
        if searchChrome != nil { return nil }
        #endif
        return fieldGroundColor?.resolvedColor(with: traitCollection).cgColor
    }
    var isNativeTextControl: Bool {
        (field != nil || textArea != nil) && props["markup"] != "markdown" && style["appearance"]?.string != "none"
    }
    /// A native field in a grouped list's row (LLP 1116.005 D2), as the
    /// kernel publishes it with the editor box: borderless, its cell being
    /// the field, as a hand-built settings form's is.
    /// The design's Field under a field's own bezel sits on the bezel's
    /// corners (BoxLayerIOS.swift); the background's own clip still holds
    /// inside it. Nil in a grouped cell, which has no corners.
    func fieldOutline() -> CGPath? {
        guard !isCellField else { return nil }
        let r = min(FieldChromeCache.textareaRadius, bounds.width / 2, bounds.height / 2)
        return CGPath(roundedRect: bounds, cornerWidth: r, cornerHeight: r, transform: nil)
    }
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
        // Layout's inset less what the drawn box lost to the pixel grid, so
        // the text keeps layout's width (`PixelGridIOS.swift`).
        var insets = UIEdgeInsets(top: rect.minY, left: rect.minX,
            bottom: max(0, layoutBounds.height - rect.maxY) + bounds.height - layoutBounds.height,
            right: max(0, layoutBounds.width - rect.maxX) + bounds.width - layoutBounds.width)
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
        // The chrome's fill is the design's Field where it overrides it.
        searchChrome?.backgroundColor = fieldGroundColor
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
        applyKernelContentBox(box, padding: NodeView.sentPadding(payload))
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

#if os(iOS) || os(tvOS)
extension Presenter {
    /// Applies a `fieldContent` op. A content box touches its node here,
    /// where it takes effect: the paragraph laid out in it, and a native
    /// button's face, whose padding the kernel resolved with it, asked for
    /// again at its next read.
    func applyContentGeometry(_ v: NodeView, _ op: BatchOp) {
        v.applyFieldContent(op.payload)
        // A paragraph's content box is where its pixels are, not what they
        // say (`textRasterGeometryChanged`). One with no pixels for it (a new
        // paragraph, a box of another size) is painted as the batch ends, with
        // the batch's other new words (`paintChangedText`). A paragraph drawn
        // in its view instead (lines flowed around shapes placed in the view,
        // columns) and a field's words are laid out in the box again.
        if v.isParagraph {
            v.textRasterGeometryChanged()
            if applying, v.textRasterKey == nil { changedText.insert(v.id) }
        }
        touched(v.id, textChanged: !v.isParagraph || !v.canRasterText)
        controls.faceChanged(v.id)
    }
}
#endif
