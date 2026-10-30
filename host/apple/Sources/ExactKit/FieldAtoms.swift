// A Markdown editor's inline atoms in the text view (LLP 1045.000 §4, §10
// Q9): each pill is its attachment's picture, which takes no events, so the
// text view finds the atom under a press itself (D7), shows the arrow and
// the `title` tooltip over it on the Mac (D2), and gives assistive
// technology an element for it (D8), after the field: on the Mac its host
// lists it (`NodeView.atomHost`), as VoiceOver's next-item command passes
// over a text view's own children; on iOS it follows the text view in its
// node, as UIKit visits no element's own elements.
import CoreText
#if canImport(UIKit)
import UIKit
#else
import AppKit
#endif

extension TextArea {
    #if os(macOS)
    private var atomStorage: NSAttributedString? { textStorage }
    private var atomOrigin: CGPoint { textContainerOrigin }
    #else
    private var atomStorage: NSAttributedString? { textStorage }
    private var atomOrigin: CGPoint { CGPoint(x: textContainerInset.left, y: textContainerInset.top) }
    #endif

    /// Each atom, with its pill's frame in this view's coordinates.
    package func atomFrames() -> [(range: NSRange, atom: MarkupAtom, frame: CGRect)] {
        guard markup != nil, let storage = atomStorage, let manager = textLayoutManager, let content = manager.textContentManager else { return [] }
        let runs = storage.atomRuns
        guard !runs.isEmpty else { return [] }
        var out: [(NSRange, MarkupAtom, CGRect)] = []
        let start = content.documentRange.location, origin = atomOrigin
        for (range, atom) in runs {
            guard let from = content.location(start, offsetBy: range.location), let to = content.location(from, offsetBy: range.length),
                  let text = NSTextRange(location: from, end: to) else { continue }
            manager.ensureLayout(for: text)
            var frame = CGRect.null
            manager.enumerateTextSegments(in: text, type: .standard, options: [.rangeNotRequired]) { _, rect, _, _ in
                if rect.width > 0 { frame = frame.union(rect) }
                return true
            }
            if !frame.isNull { out.append((range, atom, frame.offsetBy(dx: origin.x, dy: origin.y))) }
        }
        return out
    }

    /// The atom whose pill holds `point`, in this view's coordinates.
    package func atom(at point: CGPoint) -> (range: NSRange, atom: MarkupAtom, frame: CGRect)? {
        atomFrames().first { $0.frame.contains(point) }
    }

    /// A press on an atom: it is selected, then the field's `atompress`
    /// carries it (D5, D7). The modifiers held: Shift, Control, Option, Command.
    package func pressAtom(_ range: NSRange, held: (shift: Bool, ctrl: Bool, alt: Bool, meta: Bool) = (false, false, false, false)) {
        guard let owner, let storage = atomStorage, let atom = storage.attribute(.markupAtom, at: range.location, effectiveRange: nil) as? MarkupAtom else { return }
        #if os(macOS)
        if window?.firstResponder !== self { window?.makeFirstResponder(self) }
        setSelectedRange(range)
        #else
        if !isFirstResponder { _ = becomeFirstResponder() }
        selectedRange = range
        #endif
        let source = (storage.string as NSString).substring(with: range)
        owner.pressAtom(kind: atom.kind, source: source, href: atom.href, range: range, held: held)
    }

    /// The atoms changed (a restyle, a new width, a new face, markup turned
    /// off): what is read and pointed at follows. An element whose atom
    /// keeps its range and role is kept, so VoiceOver's focus on it stays
    /// while its face's name or title changes, as an upload's percentage
    /// does; it is read anew, and on macOS a title change is posted for it.
    package func atomsChanged() {
        let frames = atomFrames()
        let old = atomElements
        #if os(macOS)
        var renamed: [AtomElement] = []
        #endif
        atomElements = frames.map { range, atom, _ in
            guard let kept = old.first(where: { $0.range == range && $0.atom.role == atom.role }) else {
                return AtomElement(field: self, range: range, atom: atom)
            }
            #if os(macOS)
            if kept.atom.spokenName != atom.spokenName || kept.atom.title != atom.title { renamed.append(kept) }
            #endif
            kept.atom = atom
            return kept
        }
        let changed = old.count != atomElements.count || zip(old, atomElements).contains { $0 !== $1 }
        #if os(macOS)
        // The tooltip's text is asked of this view when it shows (below).
        removeAllToolTips()
        for (_, atom, frame) in frames where !atom.title.isEmpty { addToolTip(frame, owner: self, userData: nil) }
        window?.invalidateCursorRects(for: self)
        observeAtomLook(!frames.isEmpty)
        if frames.isEmpty { Self.atomFields.remove(self) } else { Self.atomFields.add(self) }
        if changed { NSAccessibility.post(element: owner?.atomParent ?? self, notification: .layoutChanged) }
        for element in renamed { NSAccessibility.post(element: element, notification: .titleChanged) }
        #else
        installAtomTap(!frames.isEmpty)
        watchAtomLook(!frames.isEmpty)
        guard changed, let owner else { return }
        owner.accessibilityElements = atomElements.isEmpty ? nil : [self] + atomElements
        UIAccessibility.post(notification: .layoutChanged, argument: nil)
        #endif
    }
}

#if os(macOS)
extension TextArea: NSViewToolTipOwner {
    /// A pill's `title` (D2), for the tooltip rectangle under `point`.
    package func view(_ view: NSView, stringForToolTip tag: NSView.ToolTipTag, point: NSPoint, userData: UnsafeMutableRawPointer?) -> String {
        atom(at: point)?.atom.title ?? ""
    }

    /// A selected pill draws in the selection's colour, emphasized only in
    /// the key window, and a token pill in the accent colour: each change
    /// restyles it.
    func observeAtomLook(_ on: Bool) {
        let target = on ? window : nil
        guard target.map(ObjectIdentifier.init) != atomLook.window else { return }
        atomLook.tokens.forEach(NotificationCenter.default.removeObserver)
        atomLook = (target.map(ObjectIdentifier.init), [])
        guard let target else { return }
        let restyle = { [weak self] (_: Notification) in
            if let self, self.markup != nil { self.owner?.restyleMarkup() }
        }
        let center = NotificationCenter.default
        atomLook.tokens = [center.addObserver(forName: NSWindow.didBecomeKeyNotification, object: target, queue: .main, using: restyle),
                           center.addObserver(forName: NSWindow.didResignKeyNotification, object: target, queue: .main, using: restyle),
                           center.addObserver(forName: NSColor.systemColorsDidChangeNotification, object: nil, queue: .main, using: restyle)]
    }

    /// The atom's element at a point in screen coordinates, where assistive
    /// technology points: VoiceOver's cursor following the mouse, the
    /// trackpad commander, Accessibility Inspector.
    func atomElement(atScreen point: NSPoint) -> AtomElement? {
        guard !atomElements.isEmpty, let window, let hit = atom(at: convert(window.convertPoint(fromScreen: point), from: nil)) else { return nil }
        return atomElements.first { $0.range == hit.range }
    }
    /// The arrow over a pill, as over a token; the I-beam elsewhere.
    func addAtomCursorRects() {
        for (_, _, frame) in atomFrames() { addCursorRect(frame, cursor: .arrow) }
    }
    /// The view a click on a pill lands on: TextKit 2's text view takes a
    /// click through its own gesture recognizers, before any `mouseDown`, so
    /// a pill's click goes to a view of its own (`AtomCatcher`) instead.
    func atomHit(_ point: NSPoint) -> NSView? {
        guard markup != nil, isSelectable, atom(at: convert(point, from: superview)) != nil else { return nil }
        if let catcher = subviews.first(where: { $0 is AtomCatcher }) { return catcher }
        let catcher = AtomCatcher(field: self)
        addSubview(catcher)
        return catcher
    }
    /// A press on a pill: the atom's, not the text's.
    func pressAtom(with event: NSEvent) -> Bool {
        guard markup != nil, isSelectable, let hit = atom(at: convert(event.locationInWindow, from: nil)) else { return false }
        let flags = event.modifierFlags
        pressAtom(hit.range, held: (flags.contains(.shift), flags.contains(.control), flags.contains(.option), flags.contains(.command)))
        return true
    }
}

extension TextArea {
    /// The Markdown fields with atoms, whose nodes' hosts list them.
    nonisolated(unsafe) static let atomFields = NSHashTable<TextArea>.weakObjects()
}

extension NodeView {
    /// The node whose elements list this field's atoms, after the field (LLP
    /// 1045.000 D8, §10 Q10): its nearest ancestor that holds more than the
    /// field, or that is an element. VoiceOver's next-item command passes over
    /// a text view's own children, and reads a scroll area that holds more
    /// than one item only when the person interacts with it; one that holds a
    /// single item it passes through, so the atoms go past every wrapper,
    /// scrolling or not, that holds the field alone. An image or a range is a
    /// leaf, which lists nothing, and is passed too. Nil for a field with no
    /// such ancestor: its own node lists them.
    var atomHost: NodeView? {
        var view = superview
        while let v = view {
            if let node = v as? NodeView, node.props["accessibilityRole"] != "img", node.rangeRole == nil,
               node.isAccessibilityElement() || Self.holdsMoreThanOneNode(node) { return node }
            view = v.superview
        }
        return nil
    }
    /// Whether `view` holds a second node below it, scroll and clip views
    /// passed through; it stops at the second.
    private static func holdsMoreThanOneNode(_ view: NSView) -> Bool {
        var found = 0
        func walk(_ v: NSView) -> Bool {
            for sub in v.subviews {
                if sub is NodeView { found += 1; if found > 1 { return true } } else if walk(sub) { return true }
            }
            return false
        }
        return walk(view)
    }
    /// The element that lists this field's atoms: a scrolling host's scroll
    /// view (the atoms are its children, beside the field, not after the
    /// scroll area), the host, or the element that lists the host's children.
    var atomParent: Any? {
        let host = atomHost ?? self
        if let scroll = host.scroll { return scroll }
        return host.isAccessibilityElement() ? host : NSAccessibility.unignoredAncestor(of: host)
    }
    /// The atoms of the fields this node hosts, each run after the element
    /// that holds its field, in text order. The children are taken unignored,
    /// as VoiceOver reads them, so a view that holds the field among siblings
    /// (a scroll view's document) is passed into. A scrolling node's scroll
    /// view lists them (`inScroll`), as its children hold the field.
    func withAtoms(_ children: [Any]?, inScroll: Bool = false) -> [Any]? {
        guard let children, scroll == nil || inScroll, let window, TextArea.atomFields.count > 0 else { return children }
        let hosted = TextArea.atomFields.allObjects.filter { field in
            guard field.window === window, !field.atomElements.isEmpty, let node = field.owner else { return false }
            return (node.atomHost ?? node) === self
        }
        guard !hosted.isEmpty else { return children }
        var listed = NSAccessibility.unignoredChildren(from: children)
        for field in hosted {
            guard let node = field.owner, let at = listed.lastIndex(where: { child in
                guard let view = child as? NSView else { return false }
                return view === node || node.isDescendant(of: view) || view.isDescendant(of: node)
            }) else { continue }
            listed.insert(contentsOf: field.atomElements as [Any], at: at + 1)
        }
        return listed
    }
}

/// What a click on a pill lands on: it draws nothing and is hit only where
/// `TextArea.atomHit` says.
final class AtomCatcher: NSView {
    weak var field: TextArea?
    init(field: TextArea) { self.field = field; super.init(frame: .zero) }
    required init?(coder: NSCoder) { fatalError("not coded") }
    override func hitTest(_ point: NSPoint) -> NSView? { nil }
    override func acceptsFirstMouse(for event: NSEvent?) -> Bool { true }
    // A press on a pill is the text area's node's `pointerdown` and `pointerup` too.
    override func mouseDown(with event: NSEvent) { field?.owner?.pointerPressed(event); _ = field?.pressAtom(with: event) }
    override func mouseUp(with event: NSEvent) { field?.owner?.pointerReleased(event) }
    override func isAccessibilityElement() -> Bool { false }
    // The window's hit test lands here, on the pill (`TextArea.atomHit`): the atom's element, not the text view above.
    override func accessibilityHitTest(_ point: NSPoint) -> Any? { field?.atomElement(atScreen: point) ?? super.accessibilityHitTest(point) }
}

/// An atom as assistive technology sees it: a button (or link, as
/// declared) named by its face's `ariaLabel` or else its label, described
/// by its title, at its pill. The field's host lists it after the field
/// (`NodeView.withAtoms`), and its parent is the element that lists it
/// (`NodeView.atomParent`). The agent joins it to the field's node, as iOS's
/// container does.
package final class AtomElement: NSAccessibilityElement, AgentOwned {
    weak var field: TextArea?
    let range: NSRange
    var atom: MarkupAtom
    var agentViewId: UInt32? { field?.owner?.id }
    init(field: TextArea, range: NSRange, atom: MarkupAtom) {
        (self.field, self.range, self.atom) = (field, range, atom)
        super.init()
    }
    package override func accessibilityParent() -> Any? { field?.owner?.atomParent ?? field }
    package override func accessibilityRole() -> NSAccessibility.Role? { atom.role == "link" ? .link : .button }
    package override func accessibilityLabel() -> String? { atom.spokenName }
    package override func accessibilityHelp() -> String? { atom.title.isEmpty ? nil : atom.title }
    /// Not under a host whose elements are hidden, which lists none.
    package override func isAccessibilityElement() -> Bool {
        guard let host = field?.owner.map({ $0.atomHost ?? $0 }) else { return true }
        return host.props["accessibilityElementsHidden"] != "true" && !host.accessibilityHiddenWritten
    }
    package override func accessibilityFrame() -> NSRect {
        guard let field, let window = field.window, let frame = field.atomFrames().first(where: { $0.range == range })?.frame else { return .zero }
        return window.convertToScreen(field.convert(frame, to: nil))
    }
    package override func accessibilityPerformPress() -> Bool {
        guard let field else { return false }
        field.pressAtom(range)
        return true
    }
}
#else
extension TextArea {
    private final class AtomTap: NSObject, UIGestureRecognizerDelegate {
        weak var field: TextArea?
        init(_ field: TextArea) { self.field = field }
        func gestureRecognizerShouldBegin(_ g: UIGestureRecognizer) -> Bool {
            guard let field else { return false }
            return field.atom(at: g.location(in: field)) != nil
        }
        func gestureRecognizer(_ g: UIGestureRecognizer, shouldRecognizeSimultaneouslyWith other: UIGestureRecognizer) -> Bool { false }
        @objc func tapped(_ g: UITapGestureRecognizer) {
            guard let field, g.state == .ended, let hit = field.atom(at: g.location(in: field)) else { return }
            field.pressAtom(hit.range)
            // UIKit's own tap may place a caret after this: the atom stays
            // selected, if it is still there.
            DispatchQueue.main.async { [weak field] in
                guard let field, field.selectedRange.length == 0, field.textStorage.atomRange(at: hit.range.location) == hit.range else { return }
                field.selectedRange = hit.range
            }
        }
    }
    func installAtomTap(_ on: Bool) {
        if on, atomTap == nil {
            let target = AtomTap(self), tap = UITapGestureRecognizer(target: target, action: #selector(AtomTap.tapped(_:)))
            tap.delegate = target
            addGestureRecognizer(tap)
            atomTap = (target, tap)
        } else if !on, let (_, tap) = atomTap {
            removeGestureRecognizer(tap)
            atomTap = nil
        }
    }
    /// A pill is a picture drawn for one appearance: drawn again for
    /// another (light or dark, Increase Contrast).
    func watchAtomLook(_ on: Bool) {
        guard on, atomLook == nil else { return }
        atomLook = registerForTraitChanges([UITraitUserInterfaceStyle.self, UITraitAccessibilityContrast.self]) { (field: TextArea, _: UITraitCollection) in
            if field.markup != nil { field.owner?.restyleMarkup() }
        }
    }
}

/// An atom as assistive technology sees it, beside the text view in its
/// node: a button (or link, as declared) named by its face's `ariaLabel` or
/// else its label, described by its title, at its pill.
package final class AtomElement: UIAccessibilityElement {
    weak var field: TextArea?
    let range: NSRange
    var atom: MarkupAtom { didSet { accessibilityLabel = atom.spokenName; accessibilityHint = atom.title.isEmpty ? nil : atom.title } }
    init(field: TextArea, range: NSRange, atom: MarkupAtom) {
        (self.field, self.range, self.atom) = (field, range, atom)
        super.init(accessibilityContainer: field.owner ?? field)
        accessibilityLabel = atom.spokenName
        accessibilityHint = atom.title.isEmpty ? nil : atom.title
        accessibilityTraits = atom.role == "link" ? .link : .button
    }
    package override var accessibilityFrame: CGRect {
        get {
            guard let field, let frame = field.atomFrames().first(where: { $0.range == range })?.frame else { return .zero }
            return UIAccessibility.convertToScreenCoordinates(frame, in: field)
        }
        set {}
    }
    package override func accessibilityActivate() -> Bool {
        guard let field else { return false }
        field.pressAtom(range)
        return true
    }
}
#endif

extension NodeView {
    /// An atom's press, as the field's or the text's `atompress` (host kind 59).
    package func pressAtom(kind: String, source: String, href: String, range: NSRange, held: (shift: Bool, ctrl: Bool, alt: Bool, meta: Bool)) {
        guard handlers.contains("atompress"), !disabled, !inert, let session = presenter?.session else { return }
        let mods = [(held.shift, "Shift+"), (held.ctrl, "Control+"), (held.alt, "Alt+"), (held.meta, "Meta+")].filter(\.0).map(\.1).joined()
        session.atomPress(node: id, payload: "\(range.location),\(NSMaxRange(range)),\(mods)\n\(kind)\n\(href)\n\(source)")
    }

    /// The scheme of the `href` a reader atom's run carries (`MarkupRuns.atomScheme`)
    static let readerAtomScheme = "exact-atom:"

    /// A reader atom's run detail, `[kind, source, href, start, end, …]`,
    /// from the `href` its run carries (`readerAtomScheme`); nil for any
    /// other run.
    static func readerAtom(_ url: String) -> (detail: [Any], range: NSRange)? {
        guard url.hasPrefix(readerAtomScheme), let json = String(url.dropFirst(readerAtomScheme.count)).removingPercentEncoding,
              let detail = try? JSONSerialization.jsonObject(with: Data(json.utf8)) as? [Any], detail.count >= 5,
              let start = detail[3] as? Int, let end = detail[4] as? Int, end >= start else { return nil }
        return (detail, NSRange(location: start, length: end - start))
    }

    /// A Markdown reader's `rangeFrame` over source offsets (D7): the union
    /// of the pills whose source ranges meet `start..<end`; collapsed at a
    /// pill's edge, that edge, on the side the paragraph's direction puts
    /// it. Nil where no pill is, and on the Mac for a paragraph its region
    /// reader draws (LLP 1072: 64 KB of text, or 16 KB in 256 runs or more,
    /// a Markdown text being a run per piece), which lays out off the main
    /// thread.
    func readerAtomFrame(_ start: Int, _ end: Int) -> CGRect? {
        guard props["markup"] == "markdown", let text else { return nil }
        let spec = paragraphSpec(), fit = props["fitScale"].flatMap(Float.init) ?? 1
        var at = 0, frame = CGRect.null
        for run in spec.runs {
            let length = (run.text as NSString).length
            defer { at += length }
            guard let atom = Self.readerAtom(run.href)?.range else { continue }
            let (s, e) = (atom.location, NSMaxRange(atom))
            guard start == end ? start == s || start == e : s < end && start < e else { continue }
            let rects = shapedRects(at, at + length, empty: false, font: text.font(run.scaled(fit)) as CTFont)
            if start == end {
                guard let piece = start == s ? rects.first : rects.last else { continue }
                let x = (start == s) != (spec.direction == 1) ? piece.minX : piece.maxX
                return CGRect(x: x, y: piece.minY, width: 0, height: piece.height)
            }
            frame = rects.reduce(frame) { $0.union($1) }
        }
        return frame.isNull ? nil : frame
    }

    /// A reader atom's press: the `href` its run carries (`readerAtomScheme`).
    func pressReaderAtom(_ url: String) -> Bool {
        guard url.hasPrefix(Self.readerAtomScheme) else { return false }
        guard let (detail, range) = Self.readerAtom(url) else { return true }
        #if os(macOS)
        let flags = NSApp.currentEvent?.modifierFlags ?? []
        let held = (flags.contains(.shift), flags.contains(.control), flags.contains(.option), flags.contains(.command))
        #else
        let held = (false, false, false, false)
        #endif
        pressAtom(kind: detail[0] as? String ?? "", source: detail[1] as? String ?? "", href: detail[2] as? String ?? "",
                  range: range, held: held)
        return true
    }
}

extension ExactSession {
    /// An atom's press, to a node that hears `atompress` (host kind 59, LLP 1045.000 D7).
    package func atomPress(node: UInt32, payload: String) {
        guard booted, state != .destroyed, presenter.views[node]?.handlers.contains("atompress") == true else { return }
        apply(runtime.fieldEvent(node, 59, payload, now: now()))
    }
}
