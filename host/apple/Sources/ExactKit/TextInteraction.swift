// Paragraph input/AX projection. The run table owns identity; native elements
// are created only when accessibility requests them, never when mounting text.
import CoreText
#if os(macOS)
import AppKit
#else
import UIKit
#endif

extension Presenter {
    func hoverInline(_ id: UInt32?) {
        inlineHoverTransition &+= 1
        let generation = inlineHoverTransition
        let next = id.flatMap { inlineEnabled($0) ? $0 : nil }
        guard next != hoveredInline else { return }
        let old = hoveredInline
        hoveredInline = nil
        if let old, inlineText(old) != nil { onHover?(old, false) }
        // Only an admitted enter is hovered. A boundary callback that installs
        // another hit path supersedes this transition, including an empty one.
        guard inlineHoverTransition == generation else { return }
        if let next, inlineEnabled(next) {
            #if !os(macOS)
            if let old = hovered { hover(old, false) }
            #endif
            guard inlineHoverTransition == generation else { return }
            hoveredInline = next
            onHover?(next, true)
        }
    }
}

extension NodeView {
    func updateInlineInteraction() {
        #if os(macOS)
        syncHoverTracking()
        needsDisplay = true
        #else
        #if !os(tvOS)
        if inlineText.contains(where: { $0.handlers.contains("hover") }), hoverRecognizer == nil {
            let gesture = UIHoverGestureRecognizer(target: self, action: #selector(hovering(_:)))
            gesture.delaysTouchesBegan = false; gesture.delaysTouchesEnded = false; gesture.cancelsTouchesInView = false
            addGestureRecognizer(gesture); hoverRecognizer = gesture
        }
        #endif
        setNeedsDisplay()
        #endif
    }
    // Accessibility needs the source, not a colour-resolved layout specification.
    var paragraphText: String {
        props["text"] ?? inlineText.lazy.filter(\.paints).map(\.text).joined()
    }
    /// What is spoken: hidden inline runs are omitted. `paragraphText` stays
    /// the source, which selection still reads.
    var visibleParagraphText: String {
        guard !inlineText.isEmpty else { return paragraphText }
        return inlineText.filter { $0.paints && !$0.run(dark: drawsDark).hidden }.map(\.text).joined()
    }
    /// A heading's level, 1–6, which is what assistive technology is told:
    /// its `aria-level`, else ARIA's default 2 (LLP 1115 D3 styles it as
    /// level 2 too). Only a text with `role="heading"` is a heading: an
    /// `aria-level` alone is not one, on any host (LLP 1115.000 D12.5,
    /// the kernel's `Props::heading`); nil for any other text.
    var headingLevel: Int? {
        guard props["accessibilityRole"] == "heading" else { return nil }
        if let level = Int(props["accessibilityHeadingLevel"] ?? ""), (1...6).contains(level) { return level }
        return 2
    }
    /// A heading by its role, which is what the iOS bar takes for a route's
    /// title: an `aria-level` with no `role="heading"` is not one there (LLP
    /// 1115.000 D12.5), as it has no heading style on any host.
    var isHeading: Bool { isParagraph && props["accessibilityRole"] == "heading" }
    func updateTextAccessibility() {
        guard isParagraph else { return }
        let label = props["accessibilityLabel"] ?? visibleParagraphText
        #if os(macOS)
        setAccessibilityElement(true)
        // A live-region or dialog role is a group holding these words (`updateRoleAccessibility`).
        guard groupSubrole == nil else { return }
        if accessibilitySubrole() != nil { setAccessibilitySubrole(nil) }
        if let level = headingLevel {
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
        // A heading says so, as on the web and macOS (found by `tree --ax` parity, LLP 1080.002).
        if headingLevel != nil { accessibilityTraits.insert(.header) } else { accessibilityTraits.remove(.header) }
        accessibilityLabel = label
        #endif
    }
    func textAccessibilityChildren() -> [Any]? {
        #if os(macOS)
        // Made at the first ask and kept until the runs change (`inlineText`):
        // an element made per ask is invalid to another process, VoiceOver
        // included, as it is in a hand-built view (X78). A group-role
        // paragraph's words are kept the same way, and made again when it
        // becomes or stops being a group or, with no runs, its text changes.
        let key = inlineAccessibilityKey
        let children: [NSAccessibilityElement]
        if let kept = inlineAccessibility, kept.key == key { children = kept.elements } else {
            children = key == nil ? inlineElements() : groupElements()
            inlineAccessibility = (key, children)
        }
        return children.isEmpty && key == nil ? nil : children
        #else
        let children: [Any] = inlineElements()
        guard !children.isEmpty else { return nil }
        let paragraph = UIAccessibilityElement(accessibilityContainer: self)
        paragraph.accessibilityLabel = props["accessibilityLabel"] ?? visibleParagraphText
        paragraph.accessibilityLanguage = presenter?.documentLanguage
        paragraph.accessibilityTraits = .staticText
        paragraph.accessibilityFrameInContainerSpace = contentBox()
        return [paragraph] + children
        #endif
    }
    private var interactiveRuns: [InlineText] {
        inlineText.filter { !$0.run(dark: drawsDark).hidden && (!($0.props["href"] ?? "").isEmpty || !$0.handlers.isEmpty || $0.props["accessibilityLabel"] != nil || ownsMenu($0)) }
    }
    /// A run with its own `contextPopover` is an element on macOS, whose
    /// show menu opens it (X77), as one with its own `contextmenu` is.
    private func ownsMenu(_ run: InlineText) -> Bool {
        #if os(macOS)
        return run.props["contextPopover"]?.isEmpty == false
        #else
        return false
        #endif
    }
    private func inlineElements() -> [InlineAccessibility] {
        let interactive = interactiveRuns
        guard !interactive.isEmpty else { return [] }
        let text = paragraphText as NSString
        return interactive.map { InlineAccessibility(owner: self, run: $0, text: text) }
    }
    #if os(macOS)
    /// What a paragraph's kept elements were made from besides its runs: nil
    /// unless its role is a group, then its text when it has no runs.
    private var inlineAccessibilityKey: String? {
        guard isParagraph, groupSubrole != nil else { return nil }
        return inlineText.isEmpty ? visibleParagraphText : ""
    }
    /// A paragraph whose role is a group (`updateRoleAccessibility`) holds
    /// its words as WebKit's does: a static text for each stretch between
    /// its links and the links themselves, in reading order, so no word is
    /// read twice.
    private func groupElements() -> [NSAccessibilityElement] {
        guard !inlineText.isEmpty else { return [ParagraphWords(owner: self, text: visibleParagraphText, runs: [])] }
        let text = paragraphText as NSString
        let links = Set(interactiveRuns.map(\.id))
        var children: [NSAccessibilityElement] = [], words: [InlineText] = []
        func stretch() {
            let said = words.map(\.text).joined()
            if !said.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty { children.append(ParagraphWords(owner: self, text: said, runs: words)) }
            words = []
        }
        for run in inlineText {
            if links.contains(run.id) { stretch(); children.append(InlineAccessibility(owner: self, run: run, text: text)) }
            else if run.paints, !run.run(dark: drawsDark).hidden { words.append(run) }
        }
        stretch()
        return children
    }
    #endif
}

#if os(macOS)
/// Words a group-role paragraph holds: a static text of its runs `runs`
/// between two links, or its whole text when it has no runs.
private final class ParagraphWords: NSAccessibilityElement, AgentOwned {
    weak var owner: NodeView?
    let runs: [UInt32]
    var agentViewId: UInt32? { owner?.id }
    init(owner: NodeView, text: String, runs: [InlineText]) {
        self.owner = owner; self.runs = runs.map(\.id)
        super.init()
        setAccessibilityParent(owner)
        setAccessibilityRole(.staticText)
        setAccessibilityValue(text)
        setAccessibilityLabel(text)
    }
    override func accessibilityFrame() -> NSRect {
        guard let owner, let window = owner.window else { return .zero }
        guard !runs.isEmpty else { return window.convertToScreen(owner.convert(owner.contentBox(), to: nil)) }
        let rect = runs.compactMap { owner.presenter?.inlineText($0) }.flatMap(owner.inlineRects).reduce(CGRect.null) { $0.union($1) }
        return rect.isNull ? .zero : window.convertToScreen(owner.convert(rect, to: nil))
    }
}
private final class InlineAccessibility: NSAccessibilityElement, AgentOwned {
    weak var owner: NodeView?
    package let id: UInt32
    var agentViewId: UInt32? { id } // the run's own view (LLP 1080.002 D5)
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
        super.accessibilityAttributeNames() + [NSAccessibility.Attribute(rawValue: "AXLanguage"), .enabled]
    }
    override func accessibilityAttributeValue(_ attribute: NSAccessibility.Attribute) -> Any? {
        if attribute.rawValue == "AXLanguage" { return owner?.presenter?.documentLanguage }
        if attribute == .enabled, owner?.presenter?.inlineAriaUnavailable(id) == true { return false }
        return super.accessibilityAttributeValue(attribute)
    }
    override func accessibilityFrame() -> NSRect {
        guard let owner, let run = owner.presenter?.inlineText(id), let window = owner.window else { return .zero }
        let rect = owner.inlineRects(run).reduce(CGRect.null) { $0.union($1) }
        return rect.isNull ? .zero : window.convertToScreen(owner.convert(rect, to: nil))
    }
    override func accessibilityPerformPress() -> Bool { owner?.activateInline(id) ?? false }
    // VoiceOver's show menu: the run's own `contextmenu`, offered only when it has one (X84).
    override func accessibilityPerformShowMenu() -> Bool { owner?.inlineContextMenu(id) ?? false }
    override func isAccessibilitySelectorAllowed(_ selector: Selector) -> Bool {
        selector == #selector(accessibilityPerformShowMenu) ? owner?.inlineAnswersContextMenu(id) == true : super.isAccessibilitySelectorAllowed(selector)
    }
}
#else
private final class InlineAccessibility: UIAccessibilityElement, AgentOwned {
    weak var owner: NodeView?
    package let id: UInt32
    var agentViewId: UInt32? { id } // the run's own view (LLP 1080.002 D5)
    init(owner: NodeView, run: InlineText, text: NSString) {
        self.owner = owner; id = run.id
        super.init(accessibilityContainer: owner)
        accessibilityIdentifier = run.props["testId"]
        accessibilityLanguage = owner.presenter?.documentLanguage
        accessibilityLabel = run.props["accessibilityLabel"] ?? text.substring(with: run.range)
        accessibilityTraits = (run.props["href"] ?? "").isEmpty ? .staticText : .link
        if owner.presenter?.inlineAriaUnavailable(id) == true { accessibilityTraits.insert(.notEnabled) }
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
