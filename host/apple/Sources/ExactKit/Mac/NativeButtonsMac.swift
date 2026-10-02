// Native buttons on AppKit (LLP 1069.011): a `button appearance="auto"` is a
// `Control` of type `button`, and its control is AppKit's own `NSButton`,
// its look the `buttonStyles` row's macOS column (`borderless`, `push`,
// `push-accent`, `glass`, `glass-accent`). Its title and symbol are the
// node's face, read from the kernel (`exact_button_face`). The button takes
// the click and runs the node's activation (D4); it is never a key view, so
// the node keeps keys and focus; it is the one accessibility element. A
// glass look's button is the glass body the glass-group pass isolates (D9).
#if os(macOS)
import AppKit
import MachO

/// AppKit's button, as a native button's control.
final class NativeButtonMac: NSButton {
    weak var owner: NodeView?
    struct Written: Equatable {
        var face: ButtonFace
        var accent: NSColor?
        var enabled: Bool
        var label: String?
        var testId: String?
        var selected: Bool
        var expanded: String?
    }
    var written: Written?
    /// Whether it draws glass: the glass bezel, in the macOS 26 design.
    var isGlass = false
    /// The look drawn, its name in the table's macOS column.
    var drawn = "push"
    override var acceptsFirstResponder: Bool { false }

    /// Whether AppKit draws this app in the macOS 26 design, where the glass
    /// bezel draws glass. AppKit keys that design on the SDK the app was
    /// linked with; under an older one its glass bezel draws no bezel at
    /// all, so a glass look draws as a push button, as before macOS 26.
    static let glassDrawn: Bool = {
        guard #available(macOS 26.0, *), let header = _dyld_get_image_header(0) else { return false }
        var at = UnsafeRawPointer(header).advanced(by: MemoryLayout<mach_header_64>.size)
        for _ in 0..<header.pointee.ncmds {
            let command = at.loadUnaligned(as: load_command.self)
            if command.cmd == UInt32(LC_BUILD_VERSION) {
                return at.loadUnaligned(as: build_version_command.self).sdk >= 26 << 16
            }
            at = at.advanced(by: Int(command.cmdsize))
        }
        return false
    }()
}

extension NodeView {
    /// A `button appearance="auto"` (LLP 1069.011 D3).
    var isNativeButton: Bool { kind == "control" && props["type"] == "button" }

    /// Where a native control sits: the isolation container's content when
    /// the glass-group pass made one (D9), else the node.
    var controlMount: NSView {
        if #available(macOS 26.0, *), let isolation = glassIsolation as? NSGlassEffectContainerView, let content = isolation.contentView {
            return content
        }
        return self
    }

    /// Its face's title, as its control shows it.
    var nativeTitle: String? { (presenter?.controls.controls[id] as? NativeButtonMac)?.written?.face.title }

    /// The native glass button this node shows, the glass body the
    /// glass-group pass isolates (D9); nil for anything else.
    var nativeGlassBody: NSView? {
        guard isNativeButton, let b = presenter?.controls.controls[id] as? NativeButtonMac, b.isGlass else { return nil }
        return b
    }

    /// D4: the button's action is what a custom button's click does, once.
    /// The target is this node or, without a handler, the nearest ancestor
    /// with one whose box holds the click (refused at a disabled or inert
    /// one), as iOS resolves it. Each node from this one up to the target
    /// takes the focus when it can, unless a `retainFocus` ancestor keeps it,
    /// as a click's `mouseDown` walks them; a target that cannot take it
    /// clears it. Then `press` and the canvas's pointer return.
    func activateNative() {
        guard let presenter, !disabled, !inert else { return }
        let click = convert(NSPoint(x: bounds.midX, y: bounds.midY), to: nil)
        var target: NodeView?
        var at: NSView? = self
        while let view = at {
            if let node = view as? NodeView {
                if node.disabled || node.inert { break }
                if node.handlers.contains("press") {
                    if node.bounds.contains(node.convert(click, from: nil)) { target = node }
                    break
                }
            }
            at = view.superview
        }
        let retains = { (node: NodeView) -> Bool in
            var up: NSView? = node
            while let view = up {
                if (view as? NodeView)?.props["retainFocus"] == "true" { return true }
                up = view.superview
            }
            return false
        }
        at = self
        while let view = at, view !== presenter.viewport {
            if let node = view as? NodeView {
                if node.acceptsFirstResponder, !retains(node) { window?.makeFirstResponder(node) }
                if node === target {
                    if !node.acceptsFirstResponder, !retains(node) { window?.makeFirstResponder(nil) }
                    break
                }
            }
            at = view.superview
        }
        guard let target, presenter.views[target.id] === target else { return }
        presenter.press(target.id)
        target.finishPointerPress()
    }
}

extension ControlHost {
    func makeNativeButton(_ node: NodeView) -> NSControl {
        let button = NativeButtonMac(title: "", target: nil, action: nil)
        button.owner = node
        button.refusesFirstResponder = true
        return button
    }

    @objc func nativePressed(_ sender: NSControl) {
        (sender as? NativeButtonMac)?.owner?.activateNative()
    }

    /// Its look, face, accent, enabled state and accessibility, written only
    /// when one of them changes.
    func configureNative(_ button: NativeButtonMac, _ owner: NodeView, accent: NSColor?) {
        let face = presenter.buttonFace?(owner.id) ?? ButtonFace()
        let written = NativeButtonMac.Written(
            face: face, accent: accent, enabled: !owner.disabled,
            label: owner.props["accessibilityLabel"] ?? face.title, testId: owner.props["testId"],
            selected: owner.props["accessibilitySelected"] == "true", expanded: owner.props["accessibilityExpanded"])
        guard button.written != written else { return }
        if !face.known, button.written?.face.style != face.style {
            presenter.session?.log("buttonStyle `\(face.style)` is not a button style; drawing bordered")
        }
        button.written = written
        var look = ButtonFace.drawn(face.macos).name
        var glass = false
        if look == "glass" || look == "glass-accent" {
            if #available(macOS 26.0, *), NativeButtonMac.glassDrawn {
                button.bezelStyle = .glass
                glass = true
            } else {
                look = look == "glass" ? "push" : "push-accent"
            }
        }
        if !glass { button.bezelStyle = .push }
        button.isBordered = look != "borderless"
        button.bezelColor = look.hasSuffix("-accent") ? (accent ?? .controlAccentColor) : nil
        button.contentTintColor = look == "borderless" ? (accent ?? .controlAccentColor) : nil
        button.title = face.title ?? ""
        button.image = face.symbol.flatMap { NSImage(systemSymbolName: $0, accessibilityDescription: nil) }
        button.imagePosition = button.image == nil ? .noImage : face.title == nil ? .imageOnly : face.leading ? .imageLeading : .imageTrailing
        button.lineBreakMode = .byTruncatingTail
        button.isEnabled = written.enabled
        button.setAccessibilityLabel(written.label)
        button.setAccessibilityIdentifier(written.testId)
        button.setAccessibilitySelected(written.selected)
        if let expanded = written.expanded { button.setAccessibilityExpanded(expanded == "true") }
        button.drawn = look
        if button.isGlass != glass {
            button.isGlass = glass
            owner.syncGlassSlot()
        }
    }

    /// What the agent's `layout` says of a native button (D10).
    func nativeObservation(_ button: NativeButtonMac) -> [String: Any] {
        let face = button.written?.face ?? ButtonFace()
        return ["view": "NSButton", "style": face.style, "drawn": button.drawn, "title": face.title as Any,
                "symbol": face.symbol as Any, "enabled": button.isEnabled,
                "size": [Agent.r2(button.frame.width), Agent.r2(button.frame.height)]]
    }
}
#endif
