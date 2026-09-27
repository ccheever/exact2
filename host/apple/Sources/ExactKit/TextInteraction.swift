// Paragraph input/AX projection. The run table owns identity; native elements
// are created only when accessibility requests them, never when mounting text.
#if os(macOS)
import AppKit
#else
import UIKit
#endif

extension Presenter {
    func hoverInline(_ id: UInt32?) {
        let next = id.flatMap { inlineEnabled($0) ? $0 : nil }
        guard next != hoveredInline else { return }
        if let old = hoveredInline, inlineText(old) != nil { onHover?(old, false) }
        hoveredInline = next
        if let next {
            if let old = hovered { hover(old, false) }
            onHover?(next, true)
        }
    }
}

extension NodeView {
    func updateInlineInteraction() {
        #if os(macOS)
        updateTrackingAreas()
        needsDisplay = true
        #else
        if inlineText.contains(where: { $0.handlers.contains("hover") }), hoverRecognizer == nil {
            let gesture = UIHoverGestureRecognizer(target: self, action: #selector(hovering(_:)))
            gesture.delaysTouchesBegan = false; gesture.delaysTouchesEnded = false; gesture.cancelsTouchesInView = false
            addGestureRecognizer(gesture); hoverRecognizer = gesture
        }
        setNeedsDisplay()
        #endif
    }
    // Accessibility needs the source, not a colour-resolved layout specification.
    var paragraphText: String {
        props["text"] ?? inlineText.lazy.filter(\.paints).map(\.text).joined()
    }
    func updateTextAccessibility() {
        guard isParagraph else { return }
        let label = props["accessibilityLabel"] ?? paragraphText
        #if os(macOS)
        setAccessibilityElement(true)
        if let level = Int(props["accessibilityHeadingLevel"] ?? ""), (1...6).contains(level) {
            // The heading role WebKit exposes (AppKit's constant from macOS
            // 26), its level the value, as a `<h1>`–`<h6>` has on the web.
            setAccessibilityRole(NSAccessibility.Role(rawValue: "AXHeading"))
            setAccessibilityValue(level)
        } else {
            setAccessibilityRole(.staticText)
            setAccessibilityValue(label)
        }
        setAccessibilityLabel(label)
        #else
        // UIKit does not visit a container's children when it is itself an
        // accessibility element. Expose the paragraph plus its link targets.
        isAccessibilityElement = !inlineText.contains(where: { !($0.props["href"] ?? "").isEmpty || !$0.handlers.isEmpty || $0.props["accessibilityLabel"] != nil })
        accessibilityTraits.insert(.staticText)
        accessibilityLabel = label
        #endif
    }
    func inlineRects(_ run: InlineText) -> [CGRect] {
        guard let paragraph = paragraphLayout() else { return [] }
        // A run's range is in the source; the lines, in the shaped text (LLP 1053 G5).
        let spec = paragraphSpec()
        let lo = spec.source.collapsed(run.range.location), hi = spec.source.collapsed(NSMaxRange(run.range))
        return paragraph.selectionRects(NSRange(location: lo, length: hi - lo), align: spec.align, in: contentBox(), dirty: bounds)
    }
    func textAccessibilityChildren() -> [Any]? {
        let interactive = inlineText.filter { !($0.props["href"] ?? "").isEmpty || !$0.handlers.isEmpty || $0.props["accessibilityLabel"] != nil }
        guard !interactive.isEmpty else { return nil }
        let text = paragraphText as NSString
        let children: [Any] = interactive.map { InlineAccessibility(owner: self, run: $0, text: text) }
        #if os(macOS)
        return children
        #else
        let paragraph = UIAccessibilityElement(accessibilityContainer: self)
        paragraph.accessibilityLabel = props["accessibilityLabel"] ?? (text as String)
        paragraph.accessibilityLanguage = presenter?.documentLanguage
        paragraph.accessibilityTraits = .staticText
        paragraph.accessibilityFrameInContainerSpace = contentBox()
        return [paragraph] + children
        #endif
    }
}

#if os(macOS)
private final class InlineAccessibility: NSAccessibilityElement {
    weak var owner: NodeView?
    let id: UInt32
    init(owner: NodeView, run: InlineText, text: NSString) {
        self.owner = owner; id = run.id
        super.init()
        setAccessibilityParent(owner)
        setAccessibilityRole((run.props["href"] ?? "").isEmpty ? .staticText : .link)
        setAccessibilityIdentifier(run.props["testId"])
        setAccessibilityLabel(run.props["accessibilityLabel"] ?? text.substring(with: run.range))
        if let href = run.props["href"] { setAccessibilityURL(URL(string: href)) }
    }
    override func accessibilityAttributeNames() -> [NSAccessibility.Attribute] {
        super.accessibilityAttributeNames() + [NSAccessibility.Attribute(rawValue: "AXLanguage")]
    }
    override func accessibilityAttributeValue(_ attribute: NSAccessibility.Attribute) -> Any? {
        if attribute.rawValue == "AXLanguage" { return owner?.presenter?.documentLanguage }
        return super.accessibilityAttributeValue(attribute)
    }
    override func accessibilityFrame() -> NSRect {
        guard let owner, let run = owner.presenter?.inlineText(id), let window = owner.window else { return .zero }
        let rect = owner.inlineRects(run).reduce(CGRect.null) { $0.union($1) }
        return rect.isNull ? .zero : window.convertToScreen(owner.convert(rect, to: nil))
    }
    override func accessibilityPerformPress() -> Bool { owner?.activateInline(id) ?? false }
}
#else
private final class InlineAccessibility: UIAccessibilityElement {
    weak var owner: NodeView?
    let id: UInt32
    init(owner: NodeView, run: InlineText, text: NSString) {
        self.owner = owner; id = run.id
        super.init(accessibilityContainer: owner)
        accessibilityIdentifier = run.props["testId"]
        accessibilityLanguage = owner.presenter?.documentLanguage
        accessibilityLabel = run.props["accessibilityLabel"] ?? text.substring(with: run.range)
        accessibilityTraits = (run.props["href"] ?? "").isEmpty ? .staticText : .link
    }
    override var accessibilityFrame: CGRect {
        get {
            guard let owner, let run = owner.presenter?.inlineText(id) else { return .zero }
            let rect = owner.inlineRects(run).reduce(CGRect.null) { $0.union($1) }
            return rect.isNull ? .zero : UIAccessibility.convertToScreenCoordinates(rect, in: owner)
        }
        set { }
    }
    override func accessibilityActivate() -> Bool { owner?.activateInline(id) ?? false }
}
#endif
