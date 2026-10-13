#if os(macOS)
import AppKit

/// composer-chips-accessibility (G1-8, G5-D4, DCP-Gv): the prompt as assistive technology reads it.
///
/// In T3 Code (1e2ecbd975, MIT, see LICENSE-T3) the composer is a contenteditable whose chips are elements
/// (ComposerPromptEditorTiptap.tsx's node views): Chromium serves it as an `AXTextArea` whose value holds each chip's label
/// ("Terminal 2 lines 1-738 ", `realinput-1011c` G1-8), and whose children are the chips, each named as its node view names
/// it (a PopoverTrigger is an `AXPopUpButton`). Here the chip is its hidden Markdown source in Exact's text view
/// (T3ComposerStyler), so AppKit served the source: VoiceOver read `$frontend-design` and `[Assistant quote](t3-citation://…)`,
/// and the chips' buttons were not elements a navigation-order reader keeps.
///
/// Two parts. The chips' elements (`T3ComposerChipElement`), the text view's accessibility children in text order, each
/// with the reference's role and name, pressable where the clone builds the press (a skill's and a terminal excerpt's
/// details, a citation's comment). And the text (`T3ComposerAccessibleText`): the text view's accessibility text, value
/// and ranges answered over a projection of the source where each chip reads as its label and the hidden Markdown
/// markers are left out, every range mapped back to the source for the selection, frames and lines. The text view is
/// Exact's (`TextArea`); the hatch cannot subclass it, so the answers are added to its class at run time, each one passing
/// to the class's own unless the instance carries a projection (an associated object) that differs from its source.
class T3ComposerChipElement: NSAccessibilityElement {
    /// NSAccessibilityElement does not adopt the NSAccessibilityElement protocol at run time, so a Swift reader of the
    /// text view's `accessibilityChildrenInNavigationOrder()` (typed by that protocol, as the agent's accessibility walk
    /// reads it) traps on a plain one in a checked build; this class adopts it.
    private static let adopted: Bool = objc_getProtocol("NSAccessibilityElement").map { class_addProtocol(T3ComposerChipElement.self, $0) } ?? false

    enum Part { case chip, citationLink, citationComment }
    private(set) weak var styler: T3ComposerStyler?
    let chip: T3ComposerChip
    let part: Part

    init(styler: T3ComposerStyler, view: NSTextView, chip: T3ComposerChip, part: Part, role: NSAccessibility.Role, label: String) {
        _ = T3ComposerChipElement.adopted
        self.styler = styler
        self.chip = chip
        self.part = part
        super.init()
        setAccessibilityRole(role)
        // realinput-1010c-fixes RC-4: an NSAccessibilityElement reports itself disabled unless told otherwise.
        setAccessibilityEnabled(true)
        // Not ignored: AppKit's navigation order and the agent's walk keep only elements (`isAccessibilityElement`).
        setAccessibilityElement(true)
        setAccessibilityParent(view)
        setAccessibilityLabel(label)
    }

    override func accessibilityFrame() -> NSRect {
        guard let styler, let view = styler.view, let window = view.window, let rect = rect(styler) else { return .zero }
        return window.convertToScreen(view.convert(rect, to: nil))
    }

    /// The part's frame in the text view: the comment button, the chip less its button, or the whole chip.
    func rect(_ styler: T3ComposerStyler) -> NSRect? {
        switch part {
        case .citationComment: return styler.citationActionRect(chip)
        case .citationLink:
            guard let chip = styler.chipRect(chip), let action = styler.citationActionRect(self.chip) else { return styler.chipRect(self.chip) }
            return NSRect(x: chip.minX, y: chip.minY, width: max(0, action.minX - chip.minX), height: chip.height)
        case .chip: return styler.chipRect(chip)
        }
    }
}

/// A chip whose press the clone builds: ContextChipPopover's trigger (a skill, a terminal excerpt) and a citation's
/// comment button (VO-Space, the agent's AX press).
final class T3ComposerChipPopupElement: T3ComposerChipElement {
    override func accessibilityPerformPress() -> Bool {
        guard let press = styler?.press else { return false }
        press.press(chip)
        return true
    }
}

enum T3ComposerChipNames {
    /// The role, name and whether the clone builds its press, for `chip`'s element (or a citation's two), as the
    /// reference's node view serves it: composerContextPresentation.tsx and contextChipParts.tsx for a context reference,
    /// ComposerMentionNodeView ("Preview <path>"), ComposerSkillNodeView (ContextChipPopover "Skill <label>"),
    /// AssistantCitationChip's composer source link and comment trigger.
    static func parts(_ chip: T3ComposerChip, styler: T3ComposerStyler) -> [(part: T3ComposerChipElement.Part, role: NSAccessibility.Role, label: String, press: Bool)] {
        let label = styler.displayLabel(chip)
        switch chip.kind {
        case "skill": return [(.chip, .popUpButton, "Skill \(label). Show details", true)]
        case "mention": return [(.chip, .button, "Preview \(chip.detail)", false)]
        case "citation":
            return [(.citationLink, .link, "View cited assistant text: \(label)", false),
                    (.citationComment, .popUpButton, chip.detail == "comment" ? "Edit citation comment" : "Add comment to citation", true)]
        default: break
        }
        // composerContextPresentation's fallback (UnresolvedContextChip): no record for the reference.
        if styler.chipMissing(chip) { return [(.chip, .group, "Unavailable context, \(label)", false)] }
        let kind = styler.chipKind(chip)
        if kind == "terminal" {
            // TerminalContextInlineChip: the popover's trigger while it holds text, else the expired shell.
            if let text = styler.terminalText(chip), !text.isEmpty { return [(.chip, .popUpButton, "Terminal excerpt, \(label). Show details", true)] }
            return [(.chip, .group, "Terminal excerpt, \(label), expired", false)]
        }
        let size = styler.chipSuffix(chip)
        let sized = size.isEmpty ? label : "\(label), \(size)"
        switch kind {
        case "thread": return [(.chip, .link, "Thread, \(label)", false)]
        case "file": return [(.chip, .button, "File attachment, \(sized)", false)]
        case "video": return [(.chip, .button, "Preview video attachment, \(sized)", false)]
        case "image": return [(.chip, .button, "Image attachment, \(sized)", false)]
        case "review-comment": return [(.chip, .popUpButton, "Review comment, \(label). Show details", false)]
        case "preview-annotation": return [(.chip, .popUpButton, "Preview annotation, \(label). Show details", false)]
        case "pull-request", "pr-open", "pr-draft", "pr-merged", "pr-closed": return [(.chip, .button, "Open pull request \(label)", false)]
        default: return [(.chip, .group, label, false)]
        }
    }

    static func elements(_ styler: T3ComposerStyler, view: NSTextView) -> [T3ComposerChipElement] {
        let length = (view.string as NSString).length
        return styler.chips.filter { $0.end <= length }.flatMap { chip in
            parts(chip, styler: styler).map { entry -> T3ComposerChipElement in
                entry.press ? T3ComposerChipPopupElement(styler: styler, view: view, chip: chip, part: entry.part, role: entry.role, label: entry.label)
                    : T3ComposerChipElement(styler: styler, view: view, chip: chip, part: entry.part, role: entry.role, label: entry.label)
            }
        }
    }
}

/// The prompt's text with each chip read as its label and the hidden Markdown markers left out, and the maps between its
/// offsets and the source's (UTF-16, as NSString and AppKit's accessibility ranges count).
struct T3ComposerAXProjection {
    struct Run { let source: NSRange; let display: NSRange; let chip: T3ComposerChip? }
    let text: String
    let runs: [Run]
    let sourceLength: Int
    var length: Int { (text as NSString).length }

    /// Nil when nothing in `source` reads differently (no chip and no hidden marker): the text view's own answers stand.
    init?(source: NSString, chips: [T3ComposerChip], hidden: [NSRange], label: (T3ComposerChip) -> String) {
        var swaps: [(range: NSRange, text: String, chip: T3ComposerChip?)] = chips.filter { $0.end <= source.length && $0.end > $0.start }.map { ($0.range, label($0), $0) }
        for range in hidden where range.length > 0 && NSMaxRange(range) <= source.length && !chips.contains(where: { NSIntersectionRange($0.range, range).length > 0 }) {
            swaps.append((range, "", nil))
        }
        guard !swaps.isEmpty else { return nil }
        swaps.sort { $0.range.location < $1.range.location }
        var runs: [Run] = [], out = "", at = 0, shown = 0
        func plain(to end: Int) {
            guard end > at else { return }
            let piece = source.substring(with: NSRange(location: at, length: end - at))
            runs.append(Run(source: NSRange(location: at, length: end - at), display: NSRange(location: shown, length: end - at), chip: nil))
            out += piece; shown += end - at; at = end
        }
        for swap in swaps where swap.range.location >= at {
            plain(to: swap.range.location)
            let width = (swap.text as NSString).length
            runs.append(Run(source: swap.range, display: NSRange(location: shown, length: width), chip: swap.chip))
            out += swap.text; shown += width; at = NSMaxRange(swap.range)
        }
        plain(to: source.length)
        text = out
        self.runs = runs
        sourceLength = source.length
    }

    /// A source offset in the display: inside a chip its start, inside hidden source the place it hides at.
    func display(_ offset: Int) -> Int {
        guard offset < sourceLength else { return length }
        guard let run = runs.first(where: { NSLocationInRange(offset, $0.source) }) else { return length }
        return run.chip == nil && run.display.length > 0 ? run.display.location + (offset - run.source.location) : run.display.location
    }
    /// A display offset as the start of a source range: a chip's from its start; past every run the source's end.
    func sourceStart(_ offset: Int) -> Int {
        guard let run = runs.first(where: { $0.display.length > 0 && NSLocationInRange(offset, $0.display) }) else { return sourceLength }
        return run.chip == nil ? run.source.location + (offset - run.display.location) : run.source.location
    }
    /// A display offset as the end of a source range (the character before it decides): a chip's to its end.
    func sourceEnd(_ offset: Int) -> Int {
        guard offset > 0 else { return 0 }
        guard let run = runs.first(where: { $0.display.length > 0 && NSLocationInRange(offset - 1, $0.display) }) else { return sourceLength }
        return run.chip == nil ? run.source.location + (offset - run.display.location) : NSMaxRange(run.source)
    }
    func source(_ range: NSRange) -> NSRange {
        let start = sourceStart(min(range.location, length))
        guard range.length > 0 else { return NSRange(location: start, length: 0) }
        return NSRange(location: start, length: max(0, sourceEnd(min(NSMaxRange(range), length)) - start))
    }
    func display(_ range: NSRange) -> NSRange {
        let start = display(range.location)
        guard range.length > 0 else { return NSRange(location: start, length: 0) }
        let last = NSMaxRange(range) - 1
        guard let run = runs.first(where: { NSLocationInRange(last, $0.source) }) else { return NSRange(location: start, length: length - start) }
        let end = run.chip == nil && run.display.length > 0 ? run.display.location + (last - run.source.location) + 1 : NSMaxRange(run.display)
        return NSRange(location: start, length: max(0, end - start))
    }
    /// `range` within the display text.
    func clamp(_ range: NSRange) -> NSRange {
        let start = min(max(0, range.location), length)
        return NSRange(location: start, length: min(max(0, range.length), length - start))
    }
    func string(_ range: NSRange) -> String { (text as NSString).substring(with: clamp(range)) }
    /// The chip run holding display offset `offset`.
    func chipRun(_ offset: Int) -> Run? { runs.first { $0.chip != nil && $0.display.length > 0 && NSLocationInRange(offset, $0.display) } }
}

/// The projected answers of one text view (the composer's, or the Settings prompt sample's).
final class T3ComposerAccessibleText: NSObject {
    private weak var styler: T3ComposerStyler?
    private var cache: (source: String, chips: [T3ComposerChip], projection: T3ComposerAXProjection?)?
    /// Set while the projection asks the text view's own answers.
    static var bypass = false
    private static var key = 0

    init(styler: T3ComposerStyler) { self.styler = styler }

    static func attach(_ styler: T3ComposerStyler, to view: NSTextView) {
        T3ComposerAccessibleTextHooks.install(on: view)
        objc_setAssociatedObject(view, &key, T3ComposerAccessibleText(styler: styler), .OBJC_ASSOCIATION_RETAIN_NONATOMIC)
    }
    static func detach(_ view: NSTextView) { objc_setAssociatedObject(view, &key, nil, .OBJC_ASSOCIATION_RETAIN_NONATOMIC) }
    static func of(_ object: AnyObject) -> T3ComposerAccessibleText? {
        guard !bypass else { return nil }
        return objc_getAssociatedObject(object, &key) as? T3ComposerAccessibleText
    }

    /// The projection of the text as the styler last laid it out; nil (the text view's own answers) while a composition is
    /// open, mid-edit, or when nothing reads differently.
    func projection(_ view: NSTextView) -> T3ComposerAXProjection? {
        guard let styler, styler.view === view, !view.hasMarkedText(), let styled = styler.styledText, styled == view.string else { return nil }
        if let cache, cache.source == styled, cache.chips == styler.chips { return cache.projection }
        let projection = T3ComposerAXProjection(source: styled as NSString, chips: styler.chips, hidden: styler.hidden + styler.tasks.map(\.marker),
                                                label: { styler.displayLabel($0) })
        cache = (styled, styler.chips, projection)
        return projection
    }

    // MARK: Answers (the legacy attribute names; the typed accessors share them)

    func value(_ name: String, view: NSTextView, _ p: T3ComposerAXProjection) -> Any?? {
        switch name {
        case NSAccessibility.Attribute.value.rawValue: return .some(p.text)
        case NSAccessibility.Attribute.numberOfCharacters.rawValue: return .some(NSNumber(value: p.length))
        case NSAccessibility.Attribute.selectedText.rawValue: return .some(p.string(p.display(view.selectedRange())))
        case NSAccessibility.Attribute.selectedTextRange.rawValue: return .some(NSValue(range: p.display(view.selectedRange())))
        case NSAccessibility.Attribute.selectedTextRanges.rawValue: return .some(view.selectedRanges.map { NSValue(range: p.display($0.rangeValue)) })
        case NSAccessibility.Attribute.visibleCharacterRange.rawValue, NSAccessibility.Attribute.sharedCharacterRange.rawValue:
            guard let own = Self.own(view) { view.accessibilityAttributeValue(NSAccessibility.Attribute(rawValue: name)) } as? NSValue else { return .some(nil) }
            return .some(NSValue(range: p.display(own.rangeValue)))
        default: return .none
        }
    }

    func parameterized(_ name: String, _ parameter: Any?, view: NSTextView, _ p: T3ComposerAXProjection) -> Any?? {
        let range = (parameter as? NSValue).map { p.clamp($0.rangeValue) }
        let index = (parameter as? NSNumber)?.intValue
        func own(_ value: Any?) -> Any? {
            Self.own(view) { view.accessibilityAttributeValue(NSAccessibility.ParameterizedAttribute(rawValue: name), forParameter: value) }
        }
        switch name {
        case NSAccessibility.ParameterizedAttribute.stringForRange.rawValue:
            guard let range else { return .some(nil) }
            return .some(p.string(range))
        case NSAccessibility.ParameterizedAttribute.attributedStringForRange.rawValue:
            guard let range else { return .some(nil) }
            return .some(attributed(range, view: view, p))
        case NSAccessibility.ParameterizedAttribute.rtfForRange.rawValue:
            guard let range else { return .some(nil) }
            let text = NSAttributedString(string: p.string(range))
            return .some(text.rtf(from: NSRange(location: 0, length: text.length), documentAttributes: [:]))
        case NSAccessibility.ParameterizedAttribute.boundsForRange.rawValue:
            guard let range else { return .some(nil) }
            return .some(NSValue(rect: frame(range, view: view, p)))
        case NSAccessibility.ParameterizedAttribute.rangeForPosition.rawValue:
            guard let point = (parameter as? NSValue)?.pointValue else { return .some(nil) }
            return .some(NSValue(range: position(point, view: view, p)))
        case NSAccessibility.ParameterizedAttribute.rangeForIndex.rawValue, NSAccessibility.ParameterizedAttribute.styleRangeForIndex.rawValue:
            guard let index else { return .some(nil) }
            if let chip = p.chipRun(index) { return .some(NSValue(range: chip.display)) }
            guard let own = own(NSNumber(value: p.sourceStart(index))) as? NSValue else { return .some(nil) }
            return .some(NSValue(range: p.display(own.rangeValue)))
        case NSAccessibility.ParameterizedAttribute.lineForIndex.rawValue:
            guard let index else { return .some(nil) }
            return .some(own(NSNumber(value: p.sourceStart(min(index, p.length)))))
        case NSAccessibility.ParameterizedAttribute.rangeForLine.rawValue:
            guard let own = own(parameter) as? NSValue else { return .some(nil) }
            return .some(NSValue(range: p.display(own.rangeValue)))
        default: return .none
        }
    }

    /// Selecting from VoiceOver (AXSelectedTextRange set): the source range the display range stands for.
    func select(_ range: NSRange, view: NSTextView, _ p: T3ComposerAXProjection) {
        view.setSelectedRange(p.source(p.clamp(range)))
    }

    /// The display range's attributed text: a plain run's own attributes, a chip's label (a link chip's run carries its
    /// element as AXLink, as Chromium marks a link in an editable's text).
    func attributed(_ range: NSRange, view: NSTextView, _ p: T3ComposerAXProjection) -> NSAttributedString {
        let out = NSMutableAttributedString()
        let links = (view.accessibilityChildren() ?? []).compactMap { $0 as? T3ComposerChipElement }.filter { $0.accessibilityRole() == .link }
        for run in p.runs where run.display.length > 0 {
            let part = NSIntersectionRange(run.display, range)
            guard part.length > 0 else { continue }
            if let chip = run.chip {
                var attributes: [NSAttributedString.Key: Any] = [:]
                if let link = links.first(where: { $0.chip == chip }) { attributes[NSAttributedString.Key("AXLink")] = link }
                out.append(NSAttributedString(string: p.string(part), attributes: attributes))
                continue
            }
            let source = NSRange(location: run.source.location + (part.location - run.display.location), length: part.length)
            let own = Self.own(view) { view.accessibilityAttributeValue(.attributedStringForRange, forParameter: NSValue(range: source)) } as? NSAttributedString
            out.append(own ?? NSAttributedString(string: p.string(part)))
        }
        return out
    }

    /// The display range's frame on screen: the source's, and each chip's pill it covers.
    func frame(_ range: NSRange, view: NSTextView, _ p: T3ComposerAXProjection) -> NSRect {
        var frame = (Self.own(view) { view.accessibilityAttributeValue(.boundsForRange, forParameter: NSValue(range: p.source(range))) } as? NSValue)?.rectValue ?? .zero
        guard let styler, let window = view.window else { return frame }
        for run in p.runs where run.chip != nil && NSIntersectionRange(run.display, range).length > 0 {
            guard let rect = run.chip.flatMap(styler.chipRect) else { continue }
            let screen = window.convertToScreen(view.convert(rect, to: nil))
            frame = frame.isEmpty ? screen : frame.union(screen)
        }
        return frame
    }

    /// The display range at a screen point: a chip's whole label on its pill.
    func position(_ point: NSPoint, view: NSTextView, _ p: T3ComposerAXProjection) -> NSRange {
        if let styler, let window = view.window {
            let local = view.convert(window.convertPoint(fromScreen: point), from: nil)
            if let run = p.runs.first(where: { $0.chip.flatMap(styler.chipRect)?.contains(local) == true }) { return run.display }
        }
        let own = (Self.own(view) { view.accessibilityAttributeValue(.rangeForPosition, forParameter: NSValue(point: point)) } as? NSValue)?.rangeValue
        return own.map { p.display($0) } ?? NSRange(location: p.length, length: 0)
    }

    static func own<T>(_ view: NSTextView, _ body: () -> T) -> T {
        let was = bypass
        bypass = true
        defer { bypass = was }
        return body()
    }
}

/// The run-time answers on the text view's class. Each one is added to (or wraps) the class's own once, and passes to it
/// for a text view with no projection, so another text area of the same class answers as before.
enum T3ComposerAccessibleTextHooks {
    private static var installed: Set<ObjectIdentifier> = []

    /// The class the instance was made as (KVO's notifying subclass is above it, and inherits what is added here).
    static func baseClass(_ object: AnyObject) -> AnyClass? {
        var cls: AnyClass? = object_getClass(object)
        while let current = cls, NSStringFromClass(current).hasPrefix("NSKVONotifying_") { cls = class_getSuperclass(current) }
        return cls
    }

    static func install(on view: NSTextView) {
        guard let cls = baseClass(view), !installed.contains(ObjectIdentifier(cls)) else { return }
        installed.insert(ObjectIdentifier(cls))
        typealias Projected = (NSTextView, T3ComposerAccessibleText, T3ComposerAXProjection)
        func projected(_ object: AnyObject) -> Projected? {
            guard let view = object as? NSTextView, let text = T3ComposerAccessibleText.of(view), let p = text.projection(view) else { return nil }
            return (view, text, p)
        }

        // By attribute name (NSTextView answers its parameterized attributes here itself, and its own callers may ask).
        hook(cls, NSSelectorFromString("accessibilityAttributeValue:"), as: (@convention(c) (AnyObject, Selector, NSString) -> AnyObject?).self) { own in
            { object, name in
                if let (view, text, p) = projected(object), case let .some(answer) = text.value(name as String, view: view, p) { return answer.map { $0 as AnyObject } }
                return own(object, NSSelectorFromString("accessibilityAttributeValue:"), name)
            } as @convention(block) (AnyObject, NSString) -> AnyObject?
        }
        hook(cls, NSSelectorFromString("accessibilityAttributeValue:forParameter:"), as: (@convention(c) (AnyObject, Selector, NSString, AnyObject?) -> AnyObject?).self) { own in
            { object, name, parameter in
                if let (view, text, p) = projected(object), case let .some(answer) = text.parameterized(name as String, parameter, view: view, p) { return answer.map { $0 as AnyObject } }
                return own(object, NSSelectorFromString("accessibilityAttributeValue:forParameter:"), name, parameter)
            } as @convention(block) (AnyObject, NSString, AnyObject?) -> AnyObject?
        }
        hook(cls, NSSelectorFromString("accessibilitySetValue:forAttribute:"), as: (@convention(c) (AnyObject, Selector, AnyObject?, NSString) -> Void).self) { own in
            { object, value, name in
                if let (view, text, p) = projected(object) {
                    if name as String == NSAccessibility.Attribute.selectedTextRange.rawValue, let range = (value as? NSValue)?.rangeValue { text.select(range, view: view, p); return }
                    if name as String == NSAccessibility.Attribute.selectedTextRanges.rawValue, let range = (value as? [NSValue])?.first?.rangeValue { text.select(range, view: view, p); return }
                }
                own(object, NSSelectorFromString("accessibilitySetValue:forAttribute:"), value, name)
            } as @convention(block) (AnyObject, AnyObject?, NSString) -> Void
        }

        // The typed accessors (NSAccessibility protocol), answered from the same names: what AppKit serves a client's
        // standard attributes from (realinput-1011c's out-of-process read listed the chip buttons set as children, which
        // the attribute-name path of NSTextView does not return).
        func answer(_ object: AnyObject, _ name: NSAccessibility.Attribute) -> Any?? {
            guard let (view, text, p) = projected(object) else { return .none }
            return text.value(name.rawValue, view: view, p)
        }
        func answer(_ object: AnyObject, _ name: NSAccessibility.ParameterizedAttribute, with parameter: Any) -> Any?? {
            guard let (view, text, p) = projected(object) else { return .none }
            return text.parameterized(name.rawValue, parameter, view: view, p)
        }
        hook(cls, #selector(NSTextView.accessibilityValue), as: (@convention(c) (AnyObject, Selector) -> Any?).self) { own in
            { object in
                if case let .some(got) = answer(object, .value) { return got }
                return own(object, #selector(NSTextView.accessibilityValue))
            } as @convention(block) (AnyObject) -> Any?
        }
        hook(cls, #selector(NSTextView.accessibilityNumberOfCharacters), as: (@convention(c) (AnyObject, Selector) -> Int).self) { own in
            { object in
                if case let .some(got) = answer(object, .numberOfCharacters), let n = got as? NSNumber { return n.intValue }
                return own(object, #selector(NSTextView.accessibilityNumberOfCharacters))
            } as @convention(block) (AnyObject) -> Int
        }
        hook(cls, #selector(NSTextView.accessibilitySelectedText), as: (@convention(c) (AnyObject, Selector) -> NSString?).self) { own in
            { object in
                if case let .some(got) = answer(object, .selectedText) { return got as? NSString }
                return own(object, #selector(NSTextView.accessibilitySelectedText))
            } as @convention(block) (AnyObject) -> NSString?
        }
        for (selector, name) in [(#selector(NSTextView.accessibilitySelectedTextRange), NSAccessibility.Attribute.selectedTextRange),
                                 (#selector(NSTextView.accessibilityVisibleCharacterRange), NSAccessibility.Attribute.visibleCharacterRange)] {
            hook(cls, selector, as: (@convention(c) (AnyObject, Selector) -> NSRange).self) { own in
                { object in
                    if case let .some(got) = answer(object, name), let value = got as? NSValue { return value.rangeValue }
                    return own(object, selector)
                } as @convention(block) (AnyObject) -> NSRange
            }
        }
        hook(cls, #selector(NSTextView.setAccessibilitySelectedTextRange(_:)), as: (@convention(c) (AnyObject, Selector, NSRange) -> Void).self) { own in
            { object, range in
                if let (view, text, p) = projected(object) { text.select(range, view: view, p); return }
                own(object, #selector(NSTextView.setAccessibilitySelectedTextRange(_:)), range)
            } as @convention(block) (AnyObject, NSRange) -> Void
        }
        hook(cls, #selector(NSTextView.accessibilityString(for:)), as: (@convention(c) (AnyObject, Selector, NSRange) -> NSString?).self) { own in
            { object, range in
                if case let .some(got) = answer(object, .stringForRange, with: NSValue(range: range)) { return got as? NSString }
                return own(object, #selector(NSTextView.accessibilityString(for:)), range)
            } as @convention(block) (AnyObject, NSRange) -> NSString?
        }
        hook(cls, #selector(NSTextView.accessibilityAttributedString(for:)), as: (@convention(c) (AnyObject, Selector, NSRange) -> NSAttributedString?).self) { own in
            { object, range in
                if case let .some(got) = answer(object, .attributedStringForRange, with: NSValue(range: range)) { return got as? NSAttributedString }
                return own(object, #selector(NSTextView.accessibilityAttributedString(for:)), range)
            } as @convention(block) (AnyObject, NSRange) -> NSAttributedString?
        }
        hook(cls, #selector(NSTextView.accessibilityFrame(for:)), as: (@convention(c) (AnyObject, Selector, NSRange) -> NSRect).self) { own in
            { object, range in
                if case let .some(got) = answer(object, .boundsForRange, with: NSValue(range: range)), let value = got as? NSValue { return value.rectValue }
                return own(object, #selector(NSTextView.accessibilityFrame(for:)), range)
            } as @convention(block) (AnyObject, NSRange) -> NSRect
        }
        hook(cls, NSSelectorFromString("accessibilityRangeForPosition:"), as: (@convention(c) (AnyObject, Selector, NSPoint) -> NSRange).self) { own in
            { object, point in
                if case let .some(got) = answer(object, .rangeForPosition, with: NSValue(point: point)), let value = got as? NSValue { return value.rangeValue }
                return own(object, NSSelectorFromString("accessibilityRangeForPosition:"), point)
            } as @convention(block) (AnyObject, NSPoint) -> NSRange
        }
        for (selector, name) in [(NSSelectorFromString("accessibilityRangeForIndex:"), NSAccessibility.ParameterizedAttribute.rangeForIndex),
                                 (NSSelectorFromString("accessibilityStyleRangeForIndex:"), NSAccessibility.ParameterizedAttribute.styleRangeForIndex),
                                 (NSSelectorFromString("accessibilityRangeForLine:"), NSAccessibility.ParameterizedAttribute.rangeForLine)] {
            hook(cls, selector, as: (@convention(c) (AnyObject, Selector, Int) -> NSRange).self) { own in
                { object, index in
                    if case let .some(got) = answer(object, name, with: NSNumber(value: index)), let value = got as? NSValue { return value.rangeValue }
                    return own(object, selector, index)
                } as @convention(block) (AnyObject, Int) -> NSRange
            }
        }
        hook(cls, NSSelectorFromString("accessibilityLineForIndex:"), as: (@convention(c) (AnyObject, Selector, Int) -> Int).self) { own in
            { object, index in
                if case let .some(got) = answer(object, .lineForIndex, with: NSNumber(value: index)), let n = got as? NSNumber { return n.intValue }
                return own(object, NSSelectorFromString("accessibilityLineForIndex:"), index)
            } as @convention(block) (AnyObject, Int) -> Int
        }
    }

    /// Adds `make`'s block as `selector` on `cls` (wrapping the class's own implementation when it has one), handing it the
    /// implementation it replaces, called as a C function of type `F`.
    private static func hook<F>(_ cls: AnyClass, _ selector: Selector, as _: F.Type, _ make: (F) -> Any) {
        guard let method = class_getInstanceMethod(cls, selector) else { return }
        let own = unsafeBitCast(method_getImplementation(method), to: F.self)
        let implementation = imp_implementationWithBlock(make(own))
        if owns(cls, selector) { method_setImplementation(method, implementation) }
        else { class_addMethod(cls, selector, implementation, method_getTypeEncoding(method)) }
    }

    private static func owns(_ cls: AnyClass, _ selector: Selector) -> Bool {
        var count: UInt32 = 0
        guard let methods = class_copyMethodList(cls, &count) else { return false }
        defer { free(methods) }
        return (0..<Int(count)).contains { method_getName(methods[$0]) == selector }
    }
}
#endif
