#if os(macOS)
import AppKit

/// Draws the composer's rich text the way T3's Tiptap surface does, while the
/// text view keeps the plain serialized prompt the Contract sees: inline
/// chips (ContextChip), the four inline marks (composer-rich-text.ts) and
/// task checkboxes. Source syntax that the reference never shows is kept at
/// zero width and clear; the space a chip or checkbox needs is a kern on its
/// first character, and an underlay below the text view paints the pills,
/// code backgrounds and boxes. The caret steps over that hidden source.
final class T3ComposerStyler {
    weak var editor: T3ComposerEditor?
    private(set) weak var view: NSTextView?
    private var underlay: T3ComposerUnderlay?
    private var observers: [NSObjectProtocol] = []
    private var click: NSClickGestureRecognizer?
    private var clickTarget: ClickTarget?
    /// Exact's `textColor` / `font` writes. On an empty prompt they only reach
    /// the typing attributes, which no storage edit reports.
    private var authored: [NSKeyValueObservation] = []

    var richText = true { didSet { if oldValue != richText { restyle() } } }
    /// Context ids the draft holds a record for, with each record's chip kind
    /// ("thread", "pr-open"…); a reference missing here draws unresolved.
    var contexts: [String: String] = [:] { didSet { if oldValue != contexts { restyle() } } }
    /// The selected provider's skill chip labels by skill name (composer-editor-menu.ts
    /// `skillChipLabels`, the reference's `skillLabelFor`): a skill's own display name.
    var skills: [String: String] = [:] { didSet { if oldValue != skills { restyle() } } }
    /// Where an image chip's draft image lives (T3ComposerImageChip.swift).
    var imageDirectory: URL?
    /// The chips' hover details (T3ComposerChipTips.swift, lane r4-timeline).
    private(set) lazy var tips = T3ComposerChipTips(styler: self)

    /// Chips carried through the edits since the last restyle, with their
    /// source: a chip stays one while edits leave it whole (Tiptap keeps a
    /// node even once the space after it is gone).
    private var carried: [(chip: T3ComposerChip, source: String)]?
    /// The text `chips` describe.
    private var chipText: String?

    struct Mark { let range: NSRange; let kind: String }
    struct Task { let line: Int; let marker: NSRange; let box: NSRange; let checked: Bool }
    private(set) var chips: [T3ComposerChip] = []
    /// Zero-width source runs: rich text markers and task markers.
    private(set) var hidden: [NSRange] = []
    private(set) var marks: [Mark] = []
    private(set) var tasks: [Task] = []
    private var styled: String?
    private var styling = false
    /// The prompt font and ink Exact authored. NSTextView reports the first
    /// character's, which styling changes, so they are kept from Exact's own
    /// whole-text writes (its `font` and `textColor`) instead.
    private(set) var baseFont: NSFont?
    private(set) var baseInk: NSColor?
    private var pending = false
    private var selection = NSRange(location: 0, length: 0)

    // MARK: Lifetime

    func attach(_ view: NSTextView) {
        detach()
        self.view = view
        if let scroller = view.enclosingScrollView, let host = scroller.superview {
            let layer = T3ComposerUnderlay(frame: scroller.frame)
            layer.styler = self
            host.addSubview(layer, positioned: .below, relativeTo: scroller)
            underlay = layer
            scroller.postsFrameChangedNotifications = true
            scroller.contentView.postsBoundsChangedNotifications = true
            let center = NotificationCenter.default
            for (name, object) in [(NSView.frameDidChangeNotification, scroller as NSView), (NSView.boundsDidChangeNotification, scroller.contentView),
                                   (NSView.frameDidChangeNotification, view as NSView)] {
                observers.append(center.addObserver(forName: name, object: object, queue: .main) { [weak self] _ in self?.layoutUnderlay() })
            }
        }
        let recognizer = NSClickGestureRecognizer(target: ClickTarget(self), action: #selector(ClickTarget.clicked(_:)))
        clickTarget = recognizer.target as? ClickTarget
        recognizer.delegate = clickTarget
        recognizer.delaysPrimaryMouseButtonEvents = false
        view.addGestureRecognizer(recognizer)
        click = recognizer
        baseFont = view.font; baseInk = view.textColor
        authored = [
            view.observe(\.textColor, options: [.new]) { [weak self] _, change in
                guard let self, !self.styling, let ink = change.newValue ?? nil, ink != .clear else { return }
                self.baseInk = ink
                self.scheduleRestyle()
            },
            view.observe(\.font, options: [.new]) { [weak self] _, change in
                guard let self, !self.styling, let font = change.newValue ?? nil, font.pointSize >= 1 else { return }
                self.baseFont = font
                self.scheduleRestyle()
            },
        ]
        styled = nil
        restyle()
    }

    func detach() {
        for observer in observers { NotificationCenter.default.removeObserver(observer) }
        observers.removeAll()
        underlay?.removeFromSuperview(); underlay = nil
        if let click { view?.removeGestureRecognizer(click) }
        click = nil; clickTarget = nil
        authored.forEach { $0.invalidate() }; authored.removeAll()
        view = nil
    }

    private func layoutUnderlay() {
        guard let underlay, let scroller = view?.enclosingScrollView else { return }
        if underlay.frame != scroller.frame { underlay.frame = scroller.frame }
        underlay.needsDisplay = true
        tips.refresh()
    }

    // MARK: Styling

    /// A storage edit Exact made (a value write, a font from the style): the
    /// rich attributes are reapplied after it settles.
    func storageEdited() {
        guard !styling, let storage = view?.textStorage else { return }
        // An attributes-only edit over the whole text is Exact restyling the field.
        if !storage.editedMask.contains(.editedCharacters), storage.editedMask.contains(.editedAttributes),
           storage.editedRange.location == 0, storage.editedRange.length == storage.length, storage.length > 0 {
            if let font = storage.attribute(.font, at: 0, effectiveRange: nil) as? NSFont, font.pointSize >= 1 { baseFont = font }
            if let ink = storage.attribute(.foregroundColor, at: 0, effectiveRange: nil) as? NSColor, ink != .clear { baseInk = ink }
        }
        scheduleRestyle()
    }

    /// Reapply the rich attributes once the current edit settles.
    private func scheduleRestyle() {
        guard !pending else { return }
        pending = true
        DispatchQueue.main.async { [weak self] in
            guard let self else { return }
            self.pending = false
            self.styled = nil
            self.restyle()
        }
    }

    func restyleIfTextMoved() {
        if view?.string != styled { restyle() }
        // NSTextView takes typing attributes from the character before the
        // caret, which may be hidden syntax; typing always starts plain.
        else if let view, !view.hasMarkedText(), let font = baseFont {
            var base: [NSAttributedString.Key: Any] = [.font: font, .foregroundColor: baseInk ?? .textColor]
            if let paragraph = view.defaultParagraphStyle { base[.paragraphStyle] = paragraph }
            view.typingAttributes = base
        }
    }

    func restyle() {
        guard let view, !view.hasMarkedText(), let storage = view.textStorage, !styling else { return }
        styling = true
        defer { styling = false }
        let string = view.string
        let length = storage.length
        if baseFont == nil || baseFont!.pointSize < 1 { baseFont = view.font.flatMap { $0.pointSize >= 1 ? $0 : nil } }
        let font = baseFont ?? NSFont.systemFont(ofSize: 14)
        let ink = baseInk ?? view.textColor ?? .textColor
        chips = chipsAfterEdits(string)
        // Past ~60k characters the marks are left unstyled so typing stays responsive.
        let rich = richText && length <= 60_000
        (marks, hidden) = rich ? T3ComposerStyler.inlineMarks(string, excluding: chips) : ([], [])
        tasks = rich ? T3ComposerStyler.taskLines(string, excluding: chips) : []
        storage.beginEditing()
        var base: [NSAttributedString.Key: Any] = [.font: font, .foregroundColor: ink]
        if let paragraph = view.defaultParagraphStyle { base[.paragraphStyle] = paragraph }
        storage.setAttributes(base, range: NSRange(location: 0, length: length))
        let tiny = NSFont.systemFont(ofSize: 0.01)
        let invisible: [NSAttributedString.Key: Any] = [.font: tiny, .foregroundColor: NSColor.clear, .kern: 0]
        for mark in marks {
            switch mark.kind {
            case "bold": storage.addAttribute(.font, value: T3ComposerStyler.bold(storage.attribute(.font, at: mark.range.location, effectiveRange: nil) as? NSFont ?? font), range: mark.range)
            case "italic": storage.addAttribute(.font, value: T3ComposerStyler.italic(storage.attribute(.font, at: mark.range.location, effectiveRange: nil) as? NSFont ?? font), range: mark.range)
            case "strike": storage.addAttribute(.strikethroughStyle, value: NSUnderlineStyle.single.rawValue, range: mark.range)
            case "code":
                storage.addAttribute(.font, value: NSFont.monospacedSystemFont(ofSize: (font.pointSize * 0.92 * 100).rounded() / 100, weight: .regular), range: mark.range)
            default: break
            }
        }
        for range in hidden where NSMaxRange(range) <= length { storage.addAttributes(invisible, range: range) }
        // Code keeps its pill padding (0.23em) as the hidden backticks' advance.
        for mark in marks where mark.kind == "code" {
            let pad = font.pointSize * 0.23
            if mark.range.location > 0 { storage.addAttribute(.kern, value: pad, range: NSRange(location: mark.range.location - 1, length: 1)) }
            if mark.range.length > 0 { storage.addAttribute(.kern, value: pad, range: NSRange(location: NSMaxRange(mark.range) - 1, length: 1)) }
        }
        for task in tasks where NSMaxRange(task.marker) <= length {
            storage.addAttributes(invisible, range: task.marker)
            storage.addAttribute(.kern, value: T3ComposerStyler.boxSize(font) + 8, range: NSRange(location: task.marker.location, length: 1))
        }
        for chip in chips where chip.end <= length {
            storage.addAttributes(invisible, range: chip.range)
            storage.addAttribute(.kern, value: chipWidth(chip, font: font), range: NSRange(location: chip.start, length: 1))
        }
        storage.endEditing()
        view.typingAttributes = base
        styled = string
        underlay?.needsDisplay = true
        tips.refresh()
    }

    /// An edit the text view is about to make (the delegate's change hook).
    func noteEdit(_ range: NSRange, replacement length: Int) {
        let base: [(chip: T3ComposerChip, source: String)]
        if let carried { base = carried } else if let text = chipText, text == view?.string {
            base = chips.map { ($0, (text as NSString).substring(with: $0.range)) }
        } else { base = [] }
        let delta = length - range.length
        carried = base.compactMap { entry in
            let chip = entry.chip
            // Before the chip (an insertion at its start included): it moves.
            if NSMaxRange(range) <= chip.start {
                return (T3ComposerChip(kind: chip.kind, start: chip.start + delta, end: chip.end + delta, label: chip.label, detail: chip.detail), entry.source)
            }
            // After it (typing right at its end included) it stays; inside it, it is gone.
            return range.location >= chip.end ? entry : nil
        }
    }

    /// The text's chips: what the source parses to, plus every chip an edit
    /// left whole (still its own source, where it was).
    private func chipsAfterEdits(_ string: String) -> [T3ComposerChip] {
        defer { carried = nil; chipText = string }
        if string == chipText { return chips }
        let parsed = T3ComposerText.chips(string)
        guard let carried, !carried.isEmpty else { return parsed }
        let text = string as NSString
        let kept = carried.filter { NSMaxRange($0.chip.range) <= text.length && text.substring(with: $0.chip.range) == $0.source }.map(\.chip)
        let fresh = parsed.filter { chip in !kept.contains { NSIntersectionRange($0.range, chip.range).length > 0 } }
        return (kept + fresh).sorted { $0.start < $1.start }
    }

    static func bold(_ font: NSFont) -> NSFont { NSFontManager.shared.convert(font, toHaveTrait: .boldFontMask) }
    static func italic(_ font: NSFont) -> NSFont { NSFontManager.shared.convert(font, toHaveTrait: .italicFontMask) }
    static func boxSize(_ font: NSFont) -> CGFloat { (font.pointSize * 15.2 / 14 * 10).rounded() / 10 }

    // MARK: Chips (ContextChip metrics, in em of the prompt font)

    /// ContextChip's `font-medium text-[0.86em]`: the prompt's own family (a chip inherits it) at
    /// weight 500, matched as CSS does: the family's medium face, else its regular one, never bold.
    func chipFont(_ font: NSFont) -> NSFont {
        let size = font.pointSize * 0.86
        let regular = NSFont(descriptor: font.fontDescriptor, size: size) ?? NSFont.systemFont(ofSize: size)
        guard let family = font.familyName else { return NSFont.systemFont(ofSize: size, weight: .medium) }
        let wanted = NSFontDescriptor(fontAttributes: [.family: family, .traits: [NSFontDescriptor.TraitKey.weight: NSFont.Weight.medium]])
        guard let medium = NSFont(descriptor: wanted, size: size), medium.familyName == family else { return regular }
        let weight = (medium.fontDescriptor.object(forKey: .traits) as? [NSFontDescriptor.TraitKey: Any])?[.weight] as? CGFloat ?? 0
        return weight > NSFont.Weight.medium.rawValue + 0.05 ? regular : medium
    }
    func chipWidth(_ chip: T3ComposerChip, font: NSFont) -> CGFloat {
        let size = font.pointSize * 0.86
        let label = (displayLabel(chip) as NSString).size(withAttributes: [.font: chipFont(font)]).width
        let suffix = chipSuffix(chip)
        let extra = suffix.isEmpty ? 0 : size * 0.33 + ceil((suffix as NSString).size(withAttributes: [.font: suffixFont(font)]).width * 10) / 10
        return 2 + size * 0.5 * 2 + size * 1.17 + size * 0.33 + ceil(label * 10) / 10 + extra
    }
    /// A file chip's size ("40 KB", text-[10px] of a 14px prompt).
    func suffixFont(_ font: NSFont) -> NSFont { NSFont.systemFont(ofSize: font.pointSize * 10 / 14, weight: .medium) }
    func chipSuffix(_ chip: T3ComposerChip) -> String {
        guard chip.kind == "context", let value = contextPath(chip).flatMap({ contexts[$0] }) else { return "" }
        let fields = value.split(separator: "\t", omittingEmptySubsequences: false)
        return fields.count > 1 ? String(fields[1]) : ""
    }
    /// An image chip's draft image ("image\t<size>\t<draft id>"), when it can be drawn.
    func chipImage(_ chip: T3ComposerChip) -> T3ComposerImageChip.Entry? {
        guard chipKind(chip) == "image", let value = contextPath(chip).flatMap({ contexts[$0] }) else { return nil }
        let fields = value.split(separator: "\t", omittingEmptySubsequences: false)
        return fields.count > 2 ? T3ComposerImageChip.entry(directory: imageDirectory, id: String(fields[2])) : nil
    }
    func displayLabel(_ chip: T3ComposerChip) -> String {
        if chip.kind == "citation" {
            let quote = chip.label.split(whereSeparator: \.isNewline).joined(separator: " ")
            return quote.count > 40 ? String(quote.prefix(39)) + "…" : quote
        }
        // A skill reads as ComposerPromptEditorTiptap's skillLabelFor names it: the provider's skill
        // of that name by formatProviderSkillDisplayName (its display name), else the name as
        // formatProviderSkillDisplayName title-cases one without: `$frontend-design` is "Frontend Design".
        if chip.kind == "skill" {
            if let label = skills[chip.label] { return label }
            return chip.label.split(whereSeparator: { $0.isWhitespace || $0 == ":" || $0 == "_" || $0 == "-" })
                .map { $0.prefix(1).uppercased() + $0.dropFirst() }.joined(separator: " ")
        }
        return chip.label
    }
    /// A context chip's `kind/contextId`, read from its source link.
    func contextPath(_ chip: T3ComposerChip) -> String? {
        guard chip.kind == "context", let view, chip.end <= (view.string as NSString).length else { return nil }
        let source = (view.string as NSString).substring(with: chip.range)
        guard let marker = source.range(of: "t3-context://v1/") else { return nil }
        return String(source[marker.upperBound...].dropLast())
    }
    /// The chip's colour family (ContextChip `kind`).
    func chipKind(_ chip: T3ComposerChip) -> String {
        switch chip.kind {
        case "mention", "skill", "citation": return chip.kind
        default: return contextPath(chip).flatMap { contexts[$0] }.map { $0.split(separator: "\t", maxSplits: 1).first.map(String.init) ?? $0 } ?? chip.detail
        }
    }
    func terminalText(_ chip: T3ComposerChip) -> String? {
        guard chipKind(chip) == "terminal", let value = contextPath(chip).flatMap({ contexts[$0] }) else { return nil }
        return value.components(separatedBy: "\t").dropFirst(2).joined(separator: "\t")
    }
    func chipResolved(_ chip: T3ComposerChip) -> Bool {
        guard chip.kind == "context" else { return true }
        if let text = terminalText(chip) { return !text.isEmpty }
        return contextPath(chip).map { contexts[$0] != nil } ?? false
    }

    // MARK: Geometry

    /// Segment frames for a source range, in the text view's coordinates.
    func segments(_ range: NSRange) -> [NSRect] {
        guard let view, range.location >= 0, NSMaxRange(range) <= (view.string as NSString).length else { return [] }
        let origin = view.textContainerOrigin
        // A TextKit 2 view must never be asked for its layoutManager: that
        // switches it to TextKit 1 for good, mid-draw.
        if let manager = view.textLayoutManager {
            guard let content = manager.textContentManager,
                  let start = content.location(content.documentRange.location, offsetBy: range.location),
                  let end = content.location(start, offsetBy: range.length), let textRange = NSTextRange(location: start, end: end) else { return [] }
            var rects: [NSRect] = []
            manager.enumerateTextSegments(in: textRange, type: .standard, options: []) { _, frame, _, _ in
                rects.append(frame.offsetBy(dx: origin.x, dy: origin.y)); return true
            }
            return rects
        }
        guard let layout = view.layoutManager, let container = view.textContainer else { return [] }
        let glyphs = layout.glyphRange(forCharacterRange: range, actualCharacterRange: nil)
        var rects: [NSRect] = []
        layout.enumerateEnclosingRects(forGlyphRange: glyphs, withinSelectedGlyphRange: NSRange(location: NSNotFound, length: 0), in: container) { rect, _ in
            rects.append(rect.offsetBy(dx: origin.x, dy: origin.y))
        }
        return rects
    }

    /// The baseline under a character, in the text view's coordinates.
    func baseline(at location: Int) -> CGFloat? {
        guard let view, location >= 0, location <= (view.string as NSString).length else { return nil }
        let origin = view.textContainerOrigin
        if let manager = view.textLayoutManager {
            guard let content = manager.textContentManager,
                  let at = content.location(content.documentRange.location, offsetBy: location),
                  let fragment = manager.textLayoutFragment(for: at), let element = fragment.textElement?.elementRange else { return nil }
            let index = location - content.offset(from: content.documentRange.location, to: element.location)
            let lines = fragment.textLineFragments
            guard let line = lines.first(where: { NSLocationInRange(index, $0.characterRange) }) ?? lines.last else { return nil }
            // The prompt font's baseline in this line box (a fixed line height
            // sets the descent at its foot), so a line of only hidden syntax
            // (an empty task item) keeps its neighbours' baseline.
            let descent = abs(baseFont?.descender ?? view.font?.descender ?? 0)
            return origin.y + fragment.layoutFragmentFrame.minY + line.typographicBounds.maxY - descent
        }
        guard let layout = view.layoutManager, location < (view.string as NSString).length else { return nil }
        let glyph = layout.glyphIndexForCharacter(at: location)
        return origin.y + layout.lineFragmentRect(forGlyphAt: glyph, effectiveRange: nil).minY + layout.location(forGlyphAt: glyph).y
    }

    /// An inline-flex box with vertical-align: middle (its middle half an
    /// x-height above the baseline), as T3's chips and task boxes sit.
    func middleAligned(height: CGFloat, at location: Int, fallback: NSRect) -> CGFloat {
        guard let baseline = baseline(at: location), let font = baseFont else { return fallback.minY + (fallback.height - height) / 2 }
        return baseline - font.xHeight / 2 - height / 2
    }

    func caretRect(_ location: Int) -> NSRect? {
        guard let view else { return nil }
        let length = (view.string as NSString).length
        if length == 0 { return NSRect(x: view.textContainerOrigin.x, y: view.textContainerOrigin.y, width: 0, height: view.defaultParagraphStyle?.minimumLineHeight ?? 17) }
        if location < length, let first = segments(NSRange(location: location, length: 1)).first { return NSRect(x: first.minX, y: first.minY, width: 0, height: first.height) }
        guard let last = segments(NSRange(location: max(0, location - 1), length: 1)).last else { return nil }
        let previous = (view.string as NSString).character(at: max(0, location - 1))
        if previous == 10 { return NSRect(x: view.textContainerOrigin.x, y: last.maxY, width: 0, height: last.height) }
        return NSRect(x: last.maxX, y: last.minY, width: 0, height: last.height)
    }

    /// isCaretOnVisualEdge: the collapsed caret sits on the first or last visual line.
    func caretOnVisualEdge(first: Bool, in view: NSTextView) -> Bool {
        let selection = view.selectedRange()
        guard selection.length == 0 else { return false }
        let length = (view.string as NSString).length
        guard length > 0, let caret = caretRect(selection.location), let edge = caretRect(first ? 0 : length) else { return true }
        return abs(caret.midY - edge.midY) < 1
    }

    func checkbox(for task: Task) -> NSRect? {
        guard view != nil, let first = segments(NSRange(location: task.marker.location, length: 1)).first,
              let font = baseFont else { return nil }
        let size = T3ComposerStyler.boxSize(font)
        return NSRect(x: first.minX, y: middleAligned(height: size, at: task.marker.location, fallback: first), width: size, height: size)
    }

    // MARK: The caret over hidden source

    private func atom(containing offset: Int) -> NSRange? {
        if let chip = T3ComposerText.chip(containing: offset, in: chips) { return chip.range }
        if let task = tasks.first(where: { offset >= $0.marker.location && offset < NSMaxRange($0.marker) }) { return task.marker }
        return hidden.first { offset > $0.location && offset < NSMaxRange($0) }
    }

    /// A click or programmatic move into hidden source lands on its edge, on
    /// the side the caret came from. True when the selection was moved.
    func snapSelection() -> Bool {
        guard let view, !styling, !view.hasMarkedText() else { return false }
        let current = view.selectedRange()
        defer { selection = view.selectedRange() }
        // Mid-edit the atoms still describe the old text (NSTextView moves the
        // selection before it announces the change): never snap on them.
        guard current.length == 0, view.string == styled, let atom = atom(containing: current.location) else { return false }
        let task = tasks.contains { $0.marker == atom }
        let target = task || current.location >= selection.location ? NSMaxRange(atom) : atom.location
        guard target != current.location, target <= (view.string as NSString).length else { return false }
        view.setSelectedRange(NSRange(location: target, length: 0))
        return true
    }

    /// ←/→ step over a chip or hidden marker as one character; ⌫/⌦ remove a
    /// whole chip, and ⌫ at a task's text start removes the checkbox.
    func chipCommand(_ selector: Selector, in view: NSTextView) -> Bool {
        let selection = view.selectedRange()
        guard selection.length == 0, view.string == styled else { return false }
        let caret = selection.location
        let text = view.string as NSString
        switch selector {
        case #selector(NSResponder.moveLeft(_:)):
            if let chip = chips.first(where: { $0.end == caret }) { view.setSelectedRange(NSRange(location: chip.start, length: 0)); return true }
            if let run = hidden.first(where: { NSMaxRange($0) == caret }), run.location > 0 {
                var target = run.location
                while let more = hidden.first(where: { NSMaxRange($0) == target }) { target = more.location }
                guard target > 0 else { return false }
                let previous = text.rangeOfComposedCharacterSequence(at: target - 1)
                view.setSelectedRange(NSRange(location: previous.location, length: 0)); return true
            }
            return false
        case #selector(NSResponder.moveRight(_:)):
            if let chip = chips.first(where: { $0.start == caret }) { view.setSelectedRange(NSRange(location: chip.end, length: 0)); return true }
            if let run = hidden.first(where: { $0.location == caret }), NSMaxRange(run) < text.length {
                var target = NSMaxRange(run)
                while let more = hidden.first(where: { $0.location == target }) { target = NSMaxRange(more) }
                guard target < text.length else { return false }
                view.setSelectedRange(NSRange(location: NSMaxRange(text.rangeOfComposedCharacterSequence(at: target)), length: 0)); return true
            }
            return false
        case #selector(NSResponder.deleteBackward(_:)):
            if let chip = chips.first(where: { $0.end == caret }) { return editor?.apply(chip.range, "", in: view) ?? false }
            if let task = tasks.first(where: { NSMaxRange($0.marker) == caret }) { return editor?.apply(task.marker, "", in: view) ?? false }
            if let run = hidden.first(where: { NSMaxRange($0) == caret }), run.location > 0 {
                return editor?.apply(text.rangeOfComposedCharacterSequence(at: run.location - 1), "", in: view, caret: run.location - 1) ?? false
            }
            return false
        case #selector(NSResponder.deleteForward(_:)):
            if let chip = chips.first(where: { $0.start == caret }) { return editor?.apply(chip.range, "", in: view) ?? false }
            return false
        default: return false
        }
    }

    // MARK: Task checkboxes

    fileprivate func box(at point: NSPoint) -> Task? {
        tasks.first { checkbox(for: $0)?.insetBy(dx: -2, dy: -2).contains(point) == true }
    }
    fileprivate func toggle(_ task: Task) {
        guard let view, let editor else { return }
        let caret = view.selectedRange()
        editor.apply(task.box, task.checked ? "[ ]" : "[x]", in: view, caret: caret.location)
    }

    private final class ClickTarget: NSObject, NSGestureRecognizerDelegate {
        weak var styler: T3ComposerStyler?
        init(_ styler: T3ComposerStyler) { self.styler = styler }
        func gestureRecognizerShouldBegin(_ recognizer: NSGestureRecognizer) -> Bool {
            guard let styler, let view = styler.view, view.isEditable else { return false }
            return styler.box(at: recognizer.location(in: view)) != nil
        }
        @objc func clicked(_ recognizer: NSClickGestureRecognizer) {
            guard let styler, let view = styler.view, let task = styler.box(at: recognizer.location(in: view)) else { return }
            styler.toggle(task)
        }
    }

    // MARK: Parsing (composer-rich-text.ts parseInlineMarkdown, with offsets)

    static func inlineMarks(_ string: String, excluding chips: [T3ComposerChip]) -> ([Mark], [NSRange]) {
        let text = string as NSString
        var marks: [Mark] = [], hidden: [NSRange] = []
        var lineStart = 0
        while lineStart <= text.length {
            let newline = text.range(of: "\n", options: [], range: NSRange(location: lineStart, length: text.length - lineStart))
            let lineEnd = newline.location == NSNotFound ? text.length : newline.location
            parseLine(text, lineStart, lineEnd, &marks, &hidden)
            if newline.location == NSNotFound { break }
            lineStart = lineEnd + 1
        }
        let overlaps = { (range: NSRange) in chips.contains { NSIntersectionRange($0.range, range).length > 0 || ($0.start < range.location && $0.end > range.location) } }
        return (marks.filter { !overlaps($0.range) }, hidden.filter { !overlaps($0) })
    }

    private struct Frame { let delimiter: String; let mark: String; let open: NSRange; var spans: [Mark] }

    private static func isSpace(_ unit: unichar?) -> Bool {
        guard let unit, let scalar = Unicode.Scalar(unit) else { return false }
        return CharacterSet.whitespacesAndNewlines.contains(scalar)
    }
    private static func isWord(_ unit: unichar?) -> Bool {
        guard let unit else { return false }
        return (unit >= 48 && unit <= 57) || (unit >= 65 && unit <= 90) || (unit >= 97 && unit <= 122) || unit == 95
    }

    private static func parseLine(_ text: NSString, _ start: Int, _ end: Int, _ marks: inout [Mark], _ hidden: inout [NSRange]) {
        var stack: [Frame] = []
        var spans: [Mark] = []
        func push(_ mark: Mark) { if stack.isEmpty { spans.append(mark) } else { stack[stack.count - 1].spans.append(mark) } }
        func unit(_ index: Int) -> unichar? { index >= start && index < end ? text.character(at: index) : nil }
        var index = start
        while index < end {
            let char = text.character(at: index)
            if char == 92 { index += 2; continue } // backslash escapes stay literal
            if char == 96 { // `
                var run = index
                while run < end, text.character(at: run) == 96 { run += 1 }
                let length = run - index
                if length == 1 {
                    let close = text.range(of: "`", options: [], range: NSRange(location: index + 1, length: end - index - 1)).location
                    if close != NSNotFound, close > index + 1, unit(close + 1) != 96 {
                        push(Mark(range: NSRange(location: index + 1, length: close - index - 1), kind: "code"))
                        hidden.append(NSRange(location: index, length: 1)); hidden.append(NSRange(location: close, length: 1))
                        index = close + 1
                        continue
                    }
                }
                index = run
                continue
            }
            guard char == 42 || char == 95 || char == 126 else { index += 1; continue } // * _ ~
            var runEnd = index
            while runEnd < end, text.character(at: runEnd) == char { runEnd += 1 }
            let before = unit(index - 1), after = unit(runEnd)
            let canClose = before != nil && !isSpace(before) && (char != 95 || !isWord(after))
            let canOpen = after != nil && !isSpace(after) && (char != 95 || !isWord(before))
            let delimiterChar = String(utf16CodeUnits: [char], count: 1)
            while index < runEnd {
                let top = stack.last
                let opensNestedBold = top?.mark == "italic" && runEnd - index == 2 && canOpen && (char == 42 || char == 95) && !stack.contains { $0.mark == "bold" }
                if !opensNestedBold, canClose, let top, text.substring(with: NSRange(location: index, length: min(top.delimiter.utf16.count, runEnd - index))) == top.delimiter,
                   index + top.delimiter.utf16.count <= runEnd {
                    stack.removeLast()
                    let close = NSRange(location: index, length: top.delimiter.utf16.count)
                    hidden.append(top.open); hidden.append(close)
                    let inner = NSRange(location: NSMaxRange(top.open), length: close.location - NSMaxRange(top.open))
                    push(Mark(range: inner, kind: top.mark))
                    for span in top.spans { push(span) }
                    index += top.delimiter.utf16.count
                } else if canOpen && (char != 126 || runEnd - index >= 2) {
                    let length = char == 126 || runEnd - index >= 2 ? 2 : 1
                    let mark = char == 126 ? "strike" : length == 2 ? "bold" : "italic"
                    if stack.contains(where: { $0.mark == mark }) { index = runEnd; continue }
                    stack.append(Frame(delimiter: String(repeating: delimiterChar, count: length), mark: mark, open: NSRange(location: index, length: length), spans: []))
                    index += length
                } else {
                    index = runEnd
                }
            }
        }
        // Unclosed delimiters stay literal; their inner closed spans survive.
        while let frame = stack.popLast() { for span in frame.spans { push(span) } }
        marks.append(contentsOf: spans)
    }

    /// `- [ ] ` and `- [x] ` lines (composer-rich-text-doc.ts TaskItem).
    static func taskLines(_ string: String, excluding chips: [T3ComposerChip]) -> [Task] {
        T3ComposerText.taskBoxes(string).compactMap { entry in
            let text = string as NSString
            let markerStart = entry.box.location - 2
            var markerEnd = NSMaxRange(entry.box)
            while markerEnd < text.length, text.character(at: markerEnd) == 32 || text.character(at: markerEnd) == 9 { markerEnd += 1 }
            let marker = NSRange(location: markerStart, length: markerEnd - markerStart)
            guard markerEnd > NSMaxRange(entry.box), !chips.contains(where: { NSIntersectionRange($0.range, marker).length > 0 }) else { return nil }
            return Task(line: markerStart, marker: marker, box: entry.box, checked: entry.checked)
        }
    }
}

/// Paints below the text view (whose own background is clear): chip pills,
/// code backgrounds and task checkboxes, at the source's laid-out positions.
final class T3ComposerUnderlay: NSView {
    weak var styler: T3ComposerStyler?
    override var isFlipped: Bool { true }
    override func hitTest(_ point: NSPoint) -> NSView? { nil }

    override func draw(_ dirtyRect: NSRect) {
        guard let styler, let view = styler.view else { return }
        let dark = effectiveAppearance.bestMatch(from: [.aqua, .darkAqua]) == .darkAqua
        let font = styler.baseFont ?? NSFont.systemFont(ofSize: 14)
        let toSelf = { (rect: NSRect) in view.convert(rect, to: self) }
        for mark in styler.marks where mark.kind == "code" {
            // The pill is the code font's content box plus 0.046em above and
            // below, hung from the line's baseline as an inline box is.
            let mono = NSFont.monospacedSystemFont(ofSize: (font.pointSize * 0.92 * 100).rounded() / 100, weight: .regular)
            let padY = font.pointSize * 0.046, height = mono.ascender - mono.descender + padY * 2
            let segments = styler.segments(mark.range)
            // A wrapped span's later lines sit as far below their own line tops.
            var drop: CGFloat?
            if let base = styler.baseline(at: mark.range.location), let first = segments.first { drop = base - first.minY }
            for segment in segments {
                let pad = font.pointSize * 0.23
                let top = drop.map { segment.minY + $0 - mono.ascender - padY } ?? segment.midY - height / 2
                let rect = NSRect(x: segment.minX - pad, y: top, width: segment.width + pad * 2, height: height)
                (dark ? NSColor(srgbRed: 0.506, green: 0.506, blue: 0.506, alpha: 0.12) : NSColor(srgbRed: 0.443, green: 0.443, blue: 0.478, alpha: 0.12)).setFill()
                NSBezierPath(roundedRect: toSelf(rect), xRadius: font.pointSize * 0.276, yRadius: font.pointSize * 0.276).fill()
            }
        }
        for task in styler.tasks {
            guard let box = styler.checkbox(for: task) else { continue }
            T3ComposerUnderlay.drawCheckbox(toSelf(box), checked: task.checked, dark: dark)
        }
        for chip in styler.chips {
            guard let first = styler.segments(NSRange(location: chip.start, length: 1)).first else { continue }
            let size = font.pointSize * 0.86
            let height = (size * 1.41 * 100).rounded() / 100
            let width = styler.chipWidth(chip, font: font)
            let rect = toSelf(NSRect(x: first.minX, y: styler.middleAligned(height: height, at: chip.start, fallback: first), width: width, height: height))
            drawChip(chip, in: rect, size: size, dark: dark, styler: styler, font: font)
        }
    }

    private func drawChip(_ chip: T3ComposerChip, in rect: NSRect, size: CGFloat, dark: Bool, styler: T3ComposerStyler, font: NSFont) {
        let resolved = styler.chipResolved(chip)
        let image = resolved ? styler.chipImage(chip) : nil
        let palette = image?.palette ?? T3ComposerUnderlay.palette[styler.chipKind(chip)] ?? T3ComposerUnderlay.palette["thread"]!
        let ink = resolved ? NSColor(t3Hex: dark ? palette[1] : palette[0]) : (styler.baseInk ?? .textColor)
        let path = NSBezierPath(roundedRect: rect.insetBy(dx: 0.5, dy: 0.5), xRadius: size * 0.5, yRadius: size * 0.5)
        NSColor(t3Hex: palette[2]).setFill(); path.fill()
        NSColor(t3Hex: dark ? palette[4] : palette[3]).setStroke()
        path.lineWidth = 1
        if !resolved { path.setLineDash([3, 2], count: 2, phase: 0) }
        path.stroke()
        let icon = size * 1.17
        let iconRect = NSRect(x: rect.minX + 1 + size * 0.5, y: rect.midY - icon / 2, width: icon, height: icon)
        if let image {
            // The thumbnail, object-cover in a rounded-sm (6px of 16) square.
            NSGraphicsContext.saveGraphicsState()
            NSBezierPath(roundedRect: iconRect, xRadius: icon * 6 / 16, yRadius: icon * 6 / 16).addClip()
            NSImage(cgImage: image.thumbnail, size: iconRect.size).draw(in: iconRect, from: .zero, operation: .sourceOver, fraction: 1, respectFlipped: true, hints: [.interpolation: NSImageInterpolation.high])
            NSGraphicsContext.restoreGraphicsState()
        } else {
            T3ComposerIcons.draw(T3ComposerIcons.name(for: chip, kind: styler.chipKind(chip)), in: iconRect, color: ink, dark: dark, path: chip.detail)
        }
        let label = styler.displayLabel(chip) as NSString
        let labelFont = styler.chipFont(font)
        let labelSize = label.size(withAttributes: [.font: labelFont])
        let x = iconRect.maxX + size * 0.33
        let suffix = styler.chipSuffix(chip) as NSString
        let suffixFont = styler.suffixFont(font)
        let suffixWidth = suffix.length > 0 ? suffix.size(withAttributes: [.font: suffixFont]).width : 0
        let labelEnd = rect.maxX - 1 - size * 0.5 - (suffix.length > 0 ? suffixWidth + size * 0.33 : 0)
        label.draw(in: NSRect(x: x, y: rect.midY - labelSize.height / 2, width: labelEnd - x + 1, height: labelSize.height),
                   withAttributes: [.font: labelFont, .foregroundColor: ink])
        if suffix.length > 0 {
            let suffixHeight = suffix.size(withAttributes: [.font: suffixFont]).height
            suffix.draw(at: NSPoint(x: rect.maxX - 1 - size * 0.5 - suffixWidth, y: rect.midY - suffixHeight / 2), withAttributes: [.font: suffixFont, .foregroundColor: ink])
        }
    }

    static func drawCheckbox(_ rect: NSRect, checked: Bool, dark: Bool) {
        let path = NSBezierPath(roundedRect: rect.insetBy(dx: 0.5, dy: 0.5), xRadius: 3, yRadius: 3)
        if checked {
            NSColor(t3Hex: dark ? "#99c8ff" : "#0075ff").setFill(); path.fill()
            let check = NSBezierPath()
            check.move(to: NSPoint(x: rect.minX + rect.width * 0.24, y: rect.minY + rect.height * 0.52))
            check.line(to: NSPoint(x: rect.minX + rect.width * 0.42, y: rect.minY + rect.height * 0.7))
            check.line(to: NSPoint(x: rect.minX + rect.width * 0.76, y: rect.minY + rect.height * 0.32))
            check.lineWidth = 1.8; check.lineCapStyle = .round; check.lineJoinStyle = .round
            (dark ? NSColor(t3Hex: "#1b1b1b") : NSColor.white).setStroke(); check.stroke()
        } else {
            (dark ? NSColor(t3Hex: "#3b3b3b") : NSColor.white).setFill(); path.fill()
            NSColor(t3Hex: dark ? "#858585" : "#767676").setStroke(); path.lineWidth = 1; path.stroke()
        }
    }

    /// ContextChip kinds: [light ink, dark ink, background, light border,
    /// dark border] from oklch(0.62 C H) mixed as the reference's tokens do.
    static let palette: [String: [String]] = [
        "thread": ["#2a3f40", "#c9e2df", "#009c961c", "#a6cccb", "#21a7a161"],
        "mention": ["#2c3e45", "#cbe0e6", "#0096af1c", "#a9cad5", "#39a1b861"],
        "file": ["#2b3d4b", "#cadfed", "#0090cd1c", "#a8c8e0", "#389cd361"],
        "skill": ["#433448", "#e7d4ea", "#b261be1c", "#d4b8db", "#ba72c661"],
        "review-comment": ["#3a374e", "#dcd8f2", "#8a70dd1c", "#c3bde6", "#9580e261"],
        "pull-request": ["#35394f", "#d5daf4", "#7079e41c", "#bac1e9", "#7d88e861"],
        "pr-open": ["#2c3f39", "#cbe2d6", "#009f6e1c", "#a9cdbd", "#3ba97d61"],
        "pr-draft": ["#393a3f", "#dadcdf", "#7f87931c", "#c1c3ca", "#8c939d61"],
        "pr-merged": ["#3a374e", "#dcd8f2", "#8a70dd1c", "#c3bde6", "#9580e261"],
        "pr-closed": ["#4b3337", "#f2d3d4", "#d556651c", "#e4b6b9", "#db697461"],
        "citation": ["#2f3b4f", "#ceddf4", "#4684e51c", "#aec5e9", "#5a91e961"],
        "terminal": ["#2c3f39", "#cbe2d6", "#009f6e1c", "#a9cdbd", "#3ba97d61"],
        "image": ["#4b3337", "#f2d3d4", "#d556651c", "#e4b6b9", "#db697461"],
        // r4-composer: an attached video's file chip (ContextChip kind "video", oklch(0.62 0.16 48)).
        "video": ["#4a352e", "#f0d6c8", "#d062171c", "#e1baa7", "#d7733b61"],
    ]
}

extension NSColor {
    /// `#rrggbb` or `#rrggbbaa` in sRGB.
    convenience init(t3Hex hex: String) {
        let digits = hex.hasPrefix("#") ? String(hex.dropFirst()) : hex
        let value = UInt64(digits, radix: 16) ?? 0
        let hasAlpha = digits.count == 8
        let rgb = hasAlpha ? value >> 8 : value
        self.init(srgbRed: CGFloat((rgb >> 16) & 0xff) / 255, green: CGFloat((rgb >> 8) & 0xff) / 255,
                  blue: CGFloat(rgb & 0xff) / 255, alpha: hasAlpha ? CGFloat(value & 0xff) / 255 : 1)
    }
}
#endif
