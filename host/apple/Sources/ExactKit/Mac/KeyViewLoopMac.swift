// Tab on the Mac (LLP 1115.000 D7.2): the key-view loop the presenter builds
// from the tree, and the person's Keyboard Navigation setting, which says
// whether it reaches every control or only text fields and what takes keys
// of its own. A `tabindex` the author wrote decides either way.
#if os(macOS)
import AppKit

/// macOS "Keyboard navigation" (System Settings ▸ Keyboard): off, the
/// default, Tab moves between text fields and lists; on, it reaches every
/// control. AppKit's own controls read it (`NSButton.canBecomeKeyView`), and
/// so do Exact's.
enum KeyboardNavigation {
    /// What stands in for the person's setting: an agent's `prefer
    /// keyboard-navigation` or a test's. Under the agent it is off from
    /// launch, the system's default and never the machine's (LLP 1069.007
    /// D2), unless `EXACT_DEVICE=real` asks for the machine's. Nil reads the
    /// machine.
    nonisolated(unsafe) static var agent: Bool? = ExactEnv.agentMode && ExactEnv.environment["EXACT_DEVICE"] != "real" ? false : nil
    /// The setting now. Read at each use, so a change in System Settings is
    /// the next Tab's (`Presenter.flushKeyViewLoop` rebuilds the loop when
    /// the setting it was built for is not this one).
    static var on: Bool { agent ?? NSApp.isFullKeyboardAccessEnabled }
}

extension NodeView {
    /// A box the author made a taker of keys: no button, with `focus`,
    /// `blur` or `key` handlers, or a canvas that takes input. AppKit keeps a
    /// custom view that accepts first responder in the Tab loop whatever
    /// Keyboard Navigation says (measured on macOS 27.0), and focuses it on a
    /// click (`takesClickFocus`).
    var takesKeysOfItsOwn: Bool {
        !isButton && !isRadio && (canvases?.wantsInput(id) == true || !handlers.isDisjoint(with: Self.focusEvents))
    }
    /// Whether Tab stops here. The author's `tabindex` wins: zero or more is
    /// a stop, a negative one never. Without one, a stop by kind follows the
    /// Keyboard Navigation setting as AppKit's controls do: off, only what
    /// takes keys of its own; on, every control, a button, a pressable, a
    /// radio group's one stop and a reorder grip included. (A text field's
    /// stop is its own view's, and AppKit's.)
    var tabStop: Bool {
        if let index = explicitTabIndex { return index >= 0 }
        return tabbable && (KeyboardNavigation.on || takesKeysOfItsOwn)
    }
}

extension Presenter {
    /// The same editing descendant takes explicit, sequential and modal focus.
    func keyView(of v: NodeView) -> NSView {
        // A list that is a table: the table is what takes the keys (LLP 1115.002 D5).
        if let table = tables.feeds[v.id]?.table { return table }
        if v.kind == "native", let target = session?.natives.focusTarget(v) { return target }
        if v.isNativeButton, let button = controls.controls[v.id] as? NativeButtonMac { return button }
        return v.textArea ?? v.field ?? v.nativeValueControl ?? v
    }

    /// Rebuild the key-view loop if a batch left it stale, or the Keyboard
    /// Navigation setting changed since it was built: called for each key
    /// event before AppKit reads `nextKeyView` (the view's monitor, and the
    /// agent's `type`, which sends to the window directly).
    func flushKeyViewLoop() {
        guard keyViewLoopStale || keyViewLoopMode != KeyboardNavigation.on else { return }
        keyViewLoopStale = false
        syncKeyViewLoop()
    }

    /// Sequential focus after a batch: tree order, then `tabIndex` > 0, as
    /// HTML. `autorecalculatesKeyViewLoop` stays false: the loop is this
    /// one, never AppKit's geometric one. Tab from the viewport reaches the
    /// first stop. Hidden popover rows stay out (their container is hidden).
    /// A dialog's sheet and a popover AppKit shows are windows of their own
    /// (SheetsMac.swift, PopoversMac.swift): the open dialog's loop starts
    /// at its sheet's content, and each such popover is a loop of its own,
    /// as an `NSPopover`'s is.
    func syncKeyViewLoop() {
        keyViewLoopMode = KeyboardNavigation.on
        keyViewBefore.removeAllObjects()
        if let dialog = dialogs.active { keyViewLoop([dialog], from: dialogs.entries.last?.sheet?.content ?? viewport) }
        else { keyViewLoop(root.subviews.compactMap { $0 as? NodeView }, from: viewport) }
        for popover in menus.windowed { if let node = popover.node { keyViewLoop([node], from: popover.content) } }
    }
    /// One loop: the stops under `tops`, entered from `start`.
    private func keyViewLoop(_ tops: [NodeView], from start: NSView) {
        var listed: [(view: NSView, order: Int)] = []
        // A node that can hold the focus and is no stop is where Tab starts
        // from while it has it: it points on to the next stop after it in
        // tree order, and no stop to it. A paragraph a click selected, a
        // `tabindex=-1` box an action focused, and with Keyboard Navigation
        // off a button `focus()` or `autofocus` reached: AppKit goes on from
        // a focused view that is no key view the same way. So does a
        // control that lost the focus by becoming disabled (`tabStart`).
        var starts: [(NSView, Int)] = []
        func walk(_ v: NodeView) {
            if v.inert || v.isHidden { return }
            if Self.tabbable(v) { listed.append((keyView(of: v), v.tabOrder)) }
            else if keyView(of: v).acceptsFirstResponder || keyView(of: v) === tabStart { starts.append((keyView(of: v), listed.count)) }
            for run in v.syncInlineKeyViews() {
                if run.canBecomeKeyView { listed.append((run, run.tabOrder)) }
                else if run.acceptsFirstResponder { starts.append((run, listed.count)) }
            }
            // A table's rows are in its cells, not among the list's own subviews; a route in the detail's
            // pane of a split is in that pane's view, and stands where it was written (LLP 1115.002 D4).
            let own = v.container.subviews.compactMap { $0 as? NodeView }
            for child in tables.feeds[v.id]?.mounted ?? navigation.split.authored(own, in: v.container) { walk(child) }
            for popover in menus.following(v) { walk(popover) }
        }
        for top in tops { walk(top) }
        let tabbable = listed.enumerated().sorted { a, b in
            let ia = a.element.order, ib = b.element.order
            let pa = ia > 0 ? ia : Int.max, pb = ib > 0 ? ib : Int.max
            if pa != pb { return pa < pb }
            return a.offset < b.offset
        }.map(\.element)
        // First, so the loop's own links below decide each stop's previous.
        for (start, next) in starts {
            let from = start
            from.nextKeyView = tabbable.isEmpty ? nil : (next < listed.count ? listed[next] : tabbable[0]).view
            // Shift-Tab from it: the stop before it, which AppKit's loop cannot say (no stop points to a start);
            // with none, the loop's last, as Tab from past the last stop wraps to the loop's first.
            if !listed.isEmpty { keyViewBefore.setObject((next > 0 ? listed[next - 1] : tabbable[tabbable.count - 1]).view, forKey: from) }
        }
        if tabbable.isEmpty {
            start.nextKeyView = nil
            return
        }
        for (i, v) in tabbable.enumerated() {
            v.view.nextKeyView = tabbable[(i + 1) % tabbable.count].view
        }
        start.nextKeyView = tabbable[0].view
    }

    /// Tab, or Shift-Tab, from a focused node's view: AppKit's loop from a
    /// stop, and forward from a view that is none (its `nextKeyView`); back
    /// from one that is none, the stop before it in the tree.
    func tab(from view: NSView, back: Bool, in window: NSWindow) {
        flushKeyViewLoop()
        if back, view.previousValidKeyView == nil, let before = keyViewBefore.object(forKey: view) {
            // Selected as AppKit selects a key view (a field's text with it), when it is one now.
            if let prior = before.previousKeyView, prior.nextValidKeyView === before { window.selectKeyView(following: prior) }
            else if before.canBecomeKeyView { window.makeFirstResponder(before) }
        } else if back { window.selectPreviousKeyView(view) } else { window.selectNextKeyView(view) }
    }

    /// Tab, or Shift-Tab, while the window holds the focus a disabled
    /// control gave up: on from that control's place, as Chrome goes on
    /// from its starting point, not from the top. True when taken.
    func tabFromStart(_ event: NSEvent, in other: NSWindow?) -> Bool {
        guard event.type == .keyDown, NodeView.keyName(event) == "Tab", event.modifierFlags.intersection([.command, .control, .option]).isEmpty,
              let start = tabStart, let window = start.window, window === (other ?? event.window),
              !start.isHiddenOrHasHiddenAncestor, keyTarget(start)?.inert != true else { return false }
        let current = window.firstResponder
        guard current == nil || current === window || current === window.contentView || current === viewport || current === session?.view else { return false }
        // AppKit's `selectNextKeyView` goes on from the first responder, which here is the window.
        flushKeyViewLoop()
        if !event.modifierFlags.contains(.shift) { window.selectKeyView(following: start) }
        else if start.previousValidKeyView != nil { window.selectKeyView(preceding: start) }
        else { tab(from: start, back: true, in: window) }
        return true
    }

    /// The view a new window's caret goes to (LLP 1115.000 D7.5): the first
    /// stop of the loop, which with Keyboard Navigation off is the first
    /// text field (or box that takes keys of its own) and with it on the
    /// first control. Nil when there is none, and when it is a canvas that
    /// takes input or a node painted through a canvas: a caret there would
    /// be in the canvas's first frame (the readback fixture, LLP 1014), so
    /// such a window opens with nothing focused, as every window used to.
    var initialKeyView: NSView? {
        flushKeyViewLoop()
        guard let first = viewport.nextValidKeyView, let node = keyTarget(first) else { return nil }
        if node.kind == "canvas" || node.inputCanvas != nil || node.canvasAbove != nil { return nil }
        return first
    }

    /// A new window puts the caret in its first key view, as AppKit does a
    /// window's `initialFirstResponder`, where the window's owner asked for
    /// it (`ExactView.focusesFirstKeyView`; an embedder's window is its own
    /// to decide). Once, when the launch `autofocus` runs, the turn after
    /// the first frame, so no field editor stands before the first pixel.
    /// What the author wrote wins: an `autofocus` took the focus just before
    /// this, and whatever holds it by now keeps it.
    func focusInitialKeyView() {
        guard let view = session?.view, view.focusesFirstKeyView, let window = view.window else { return }
        view.focusesFirstKeyView = false
        let current = window.firstResponder
        guard current == nil || current === window || current === window.contentView || current === viewport || current === view,
              let first = initialKeyView else { return }
        window.makeFirstResponder(first)
    }

    /// A Tab stop (LLP 1088 D7.3; LLP 1115.000 D7.2): an explicit `tabindex`
    /// ≥ 0, a text field, or what is one by kind under the Keyboard
    /// Navigation setting (`NodeView.tabStop`); an explicit negative never,
    /// though it still takes a click.
    static func tabbable(_ v: NodeView) -> Bool {
        if v.formDisabled || v.cssVisibilityHidden { return false }
        // A list that is a table is one stop, as AppKit's table is whatever
        // the Keyboard Navigation setting says; the node a row's click
        // presses is none, since the table selects its rows (LLP 1115.002 D5).
        if let tables = v.presenter?.tables, !tables.feeds.isEmpty {
            if tables.feeds[v.id] != nil { return true }
            if v.explicitTabIndex == nil, tables.pressesRow(v) { return false }
        }
        if let index = v.explicitTabIndex, index < 0 { return false }
        if v.field != nil || v.textArea != nil || v.isDateOrSelect { return true }
        if v.kind == "native", v.presenter?.session?.natives.focusTarget(v) != nil { return true }
        if v.isButton || v.kind == "toggle" || v.pressable { return v.explicitTabIndex != nil || KeyboardNavigation.on || v.takesKeysOfItsOwn }
        return v.canBecomeKeyView
    }
}
/// A responder for a run, with no text, input surface or accessibility
/// element of its own. The paragraph still paints, hits and selects it.
final class InlineKeyView: NSView {
    weak var owner: NodeView?
    let id: UInt32
    var run: InlineText? { owner?.presenter?.inlineText(id) }
    var tabOrder: Int { run?.props["tabIndex"].flatMap(Int.init) ?? 0 }
    init(owner: NodeView, id: UInt32) {
        self.owner = owner; self.id = id
        super.init(frame: .zero)
        focusRingType = .exterior
        autoresizingMask = [.width, .height]
        setAccessibilityElement(false); setAccessibilityHidden(true)
    }
    required init?(coder: NSCoder) { nil }
    override var isFlipped: Bool { true }
    override func hitTest(_ point: NSPoint) -> NSView? { nil }
    override var acceptsFirstResponder: Bool {
        guard let owner, let run, !owner.isHiddenOrHasHiddenAncestor, !owner.cssVisibilityHidden,
              !run.run(dark: owner.drawsDark).hidden else { return false }
        return !owner.formDisabled && owner.presenter?.inlineInert(id) == false
    }
    override var canBecomeKeyView: Bool {
        guard acceptsFirstResponder, let run else { return false }
        if let index = run.props["tabIndex"].flatMap(Int.init) { return index >= 0 }
        return KeyboardNavigation.on
    }
    override func becomeFirstResponder() -> Bool {
        guard acceptsFirstResponder, super.becomeFirstResponder() else { return false }
        owner?.presenter?.collections.pinsChanged()
        owner?.presenter?.focus(id)
        noteFocusRingMaskChanged()
        return true
    }
    override func resignFirstResponder() -> Bool {
        guard super.resignFirstResponder() else { return false }
        owner?.presenter?.collections.pinsChanged()
        owner?.presenter?.blur(id)
        noteFocusRingMaskChanged()
        return true
    }
    override var focusRingMaskBounds: NSRect {
        guard window?.firstResponder === self, let owner, let run else { return .zero }
        let rect = owner.inlineRects(run).reduce(CGRect.null) { $0.union($1) }
        return rect.isNull ? .zero : convert(rect, from: owner)
    }
    override func drawFocusRingMask() {
        guard let owner, let run else { return }
        for rect in owner.inlineRects(run) { NSBezierPath(roundedRect: convert(rect, from: owner), xRadius: 2, yRadius: 2).fill() }
    }
    override func keyDown(with event: NSEvent) {
        guard event.modifierFlags.intersection([.command, .control, .option]).isEmpty else { super.keyDown(with: event); return }
        let key = NodeView.keyName(event)
        if key == "Tab", let window { owner?.presenter?.tab(from: self, back: event.modifierFlags.contains(.shift), in: window) }
        else if key == "Enter" || key == " " && run?.props["href"] == nil { _ = owner?.activateInline(id) }
        else { super.keyDown(with: event) }
    }
}

extension NodeView {
    func syncInlineKeyViews() -> [InlineKeyView] {
        let runs = inlineText.filter {
            $0.props["tabIndex"] != nil || $0.isLink || $0.handlers.contains("press")
        }
        let live = Set(runs.map(\.id))
        for (id, view) in inlineKeyViews where !live.contains(id) {
            presenter?.focusEvents.removed(id)
            if view.window?.firstResponder === view { view.window?.makeFirstResponder(nil) }
            view.removeFromSuperview(); inlineKeyViews.removeValue(forKey: id)
        }
        return runs.map { run in
            let view = inlineKeyViews[run.id] ?? InlineKeyView(owner: self, id: run.id)
            inlineKeyViews[run.id] = view
            if view.superview !== self { addSubview(view) }
            view.frame = bounds
            view.noteFocusRingMaskChanged()
            return view
        }
    }
}
#endif
