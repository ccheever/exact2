// A Markdown editor's inline atoms in the text view (LLP 1045.000 §4, §10
// Q9): each pill is its attachment's picture, which takes no events, so the
// text view finds the atom under a press itself (D7), shows the arrow and
// the `title` tooltip over it on the Mac (D2), and gives assistive
// technology an element for it (D8): a child of the text view on the Mac, an
// element beside it on iOS, where UIKit visits no element's own elements.
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

    /// The atoms changed (a restyle): what is read and pointed at follows.
    package func atomsChanged() {
        let frames = atomFrames()
        #if os(macOS)
        removeAllToolTips()
        for (_, atom, frame) in frames where !atom.title.isEmpty { addToolTip(frame, owner: atom.title as NSString, userData: nil) }
        window?.invalidateCursorRects(for: self)
        let old = atomElements
        atomElements = frames.map { range, atom, _ in
            old.first { $0.range == range && $0.atom.label == atom.label && $0.atom.role == atom.role } ?? AtomElement(field: self, range: range, atom: atom)
        }
        for element in atomElements { element.atom = frames.first { $0.range == element.range }?.atom ?? element.atom }
        if old.count != atomElements.count || zip(old, atomElements).contains(where: { $0 !== $1 }) {
            NSAccessibility.post(element: self, notification: .layoutChanged)
        }
        #else
        installAtomTap(!frames.isEmpty)
        guard let owner else { return }
        if frames.isEmpty {
            if atomElements.isEmpty == false { atomElements = []; owner.accessibilityElements = nil }
            return
        }
        atomElements = frames.map { AtomElement(field: self, range: $0.range, atom: $0.atom) }
        owner.accessibilityElements = [self] + atomElements
        UIAccessibility.post(notification: .layoutChanged, argument: nil)
        #endif
    }
}

#if os(macOS)
extension TextArea {
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

/// What a click on a pill lands on: it draws nothing and is hit only where
/// `TextArea.atomHit` says.
final class AtomCatcher: NSView {
    weak var field: TextArea?
    init(field: TextArea) { self.field = field; super.init(frame: .zero) }
    required init?(coder: NSCoder) { fatalError("not coded") }
    override func hitTest(_ point: NSPoint) -> NSView? { nil }
    override func acceptsFirstMouse(for event: NSEvent?) -> Bool { true }
    override func mouseDown(with event: NSEvent) { _ = field?.pressAtom(with: event) }
    override func isAccessibilityElement() -> Bool { false }
}

/// An atom as assistive technology sees it: a button (or link, as
/// declared) named by its label, described by its title, at its pill.
package final class AtomElement: NSAccessibilityElement {
    weak var field: TextArea?
    let range: NSRange
    var atom: MarkupAtom
    init(field: TextArea, range: NSRange, atom: MarkupAtom) {
        (self.field, self.range, self.atom) = (field, range, atom)
        super.init()
    }
    package override func accessibilityParent() -> Any? { field }
    package override func accessibilityRole() -> NSAccessibility.Role? { atom.role == "link" ? .link : .button }
    package override func accessibilityLabel() -> String? { atom.label }
    package override func accessibilityHelp() -> String? { atom.title.isEmpty ? nil : atom.title }
    package override func isAccessibilityElement() -> Bool { true }
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
            // UIKit's own tap may place a caret after this: the atom stays selected.
            DispatchQueue.main.async { [weak field] in if field?.selectedRange.length == 0 { field?.selectedRange = hit.range } }
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
}

/// An atom as assistive technology sees it, beside the text view in its
/// node: a button (or link, as declared) named by its label, described by
/// its title, at its pill.
package final class AtomElement: UIAccessibilityElement {
    weak var field: TextArea?
    let range: NSRange
    let atom: MarkupAtom
    init(field: TextArea, range: NSRange, atom: MarkupAtom) {
        (self.field, self.range, self.atom) = (field, range, atom)
        super.init(accessibilityContainer: field.owner ?? field)
        accessibilityLabel = atom.label
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
    /// An atom's press, as the field's or the text's `atompress` (host kind 58).
    package func pressAtom(kind: String, source: String, href: String, range: NSRange, held: (shift: Bool, ctrl: Bool, alt: Bool, meta: Bool)) {
        guard handlers.contains("atompress"), !disabled, !inert, let session = presenter?.session else { return }
        let mods = [(held.shift, "Shift+"), (held.ctrl, "Control+"), (held.alt, "Alt+"), (held.meta, "Meta+")].filter(\.0).map(\.1).joined()
        session.atomPress(node: id, payload: "\(range.location),\(NSMaxRange(range)),\(mods)\n\(kind)\n\(href)\n\(source)")
    }

    /// A reader atom's press: the `href` its run carries (`MarkupRuns.atomScheme`).
    func pressReaderAtom(_ url: String) -> Bool {
        guard url.hasPrefix("exact-atom:") else { return false }
        guard let json = String(url.dropFirst("exact-atom:".count)).removingPercentEncoding,
              let detail = try? JSONSerialization.jsonObject(with: Data(json.utf8)) as? [Any], detail.count >= 5,
              let start = detail[3] as? Int, let end = detail[4] as? Int else { return true }
        #if os(macOS)
        let flags = NSApp.currentEvent?.modifierFlags ?? []
        let held = (flags.contains(.shift), flags.contains(.control), flags.contains(.option), flags.contains(.command))
        #else
        let held = (false, false, false, false)
        #endif
        pressAtom(kind: detail[0] as? String ?? "", source: detail[1] as? String ?? "", href: detail[2] as? String ?? "",
                  range: NSRange(location: start, length: end - start), held: held)
        return true
    }
}

extension ExactSession {
    /// An atom's press, to a node that hears `atompress` (host kind 58, LLP 1045.000 D7).
    package func atomPress(node: UInt32, payload: String) {
        guard booted, state != .destroyed, presenter.views[node]?.handlers.contains("atompress") == true else { return }
        apply(runtime.fieldEvent(node, 58, payload, now: now()))
    }
}
