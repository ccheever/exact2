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
        var listed: [NodeView] = []
        // A node that can hold the focus and is no stop is where Tab starts
        // from while it has it: it points on to the next stop after it in
        // tree order, and no stop to it. A paragraph a click selected, a
        // `tabindex=-1` box an action focused, and with Keyboard Navigation
        // off a button `focus()` or `autofocus` reached: AppKit goes on from
        // a focused view that is no key view the same way.
        var starts: [(NodeView, Int)] = []
        func walk(_ v: NodeView) {
            if v.inert || v.isHidden { return }
            if Self.tabbable(v) { listed.append(v) } else if keyView(of: v).acceptsFirstResponder { starts.append((v, listed.count)) }
            for child in v.container.subviews.compactMap({ $0 as? NodeView }) { walk(child) }
            for popover in menus.following(v) { walk(popover) }
        }
        for top in tops { walk(top) }
        let tabbable = listed.enumerated().sorted { a, b in
            let ia = a.element.tabOrder, ib = b.element.tabOrder
            let pa = ia > 0 ? ia : Int.max, pb = ib > 0 ? ib : Int.max
            if pa != pb { return pa < pb }
            return a.offset < b.offset
        }.map(\.element)
        // First, so the loop's own links below decide each stop's previous.
        for (start, next) in starts {
            let from = keyView(of: start)
            from.nextKeyView = tabbable.isEmpty ? nil : keyView(of: next < listed.count ? listed[next] : tabbable[0])
            // Shift-Tab from it: the stop before it, which AppKit's loop cannot say (no stop points to a start);
            // with none, the loop's last, as Tab from past the last stop wraps to the loop's first.
            if !listed.isEmpty { keyViewBefore.setObject(keyView(of: next > 0 ? listed[next - 1] : tabbable[tabbable.count - 1]), forKey: from) }
        }
        if tabbable.isEmpty {
            start.nextKeyView = nil
            return
        }
        for (i, v) in tabbable.enumerated() {
            keyView(of: v).nextKeyView = keyView(of: tabbable[(i + 1) % tabbable.count])
        }
        start.nextKeyView = keyView(of: tabbable[0])
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
        if let index = v.explicitTabIndex, index < 0 { return false }
        if v.field != nil || v.textArea != nil || v.isDateOrSelect { return true }
        if v.kind == "native", v.presenter?.session?.natives.focusTarget(v) != nil { return true }
        if v.isButton || v.kind == "toggle" || v.pressable { return v.explicitTabIndex != nil || KeyboardNavigation.on || v.takesKeysOfItsOwn }
        return v.canBecomeKeyView
    }
}
#endif
