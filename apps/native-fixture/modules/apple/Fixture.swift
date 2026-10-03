// The native-module fixture (LLP 1024 D8) on Apple hosts: a coloured box the
// smoke can drive through every part of the seam. `exact-fixture` paints its
// colour with a layer background, which the ordinary `cacheDisplay` capture
// does not see, so its colour in a screenshot is the tokened snapshot's; it
// echoes each props object it accepts as a `message`, fires all nine events
// when `emit` changes, refuses `reject=true`, and after `destroy` calls back
// from a background thread (the host must drop it). `exact-plain` draws its
// colour in `draw(_:)`, which the ordinary capture does see. Neither takes
// hits: an agent `tap` lands on the node's own view.
//
// On iOS the module's hooks (LLP 1075.003 §3.2) take the long tail: what
// the authored header does not say — the bar's tint, and a leading button
// that clicks the authored control `data-menu` names, so a finger and the
// agent run one handler. The title, large or inline, Back and the
// Compose item are Exact's, from the header, in the first frame.
// `data-violate` makes the route hook write the content scroll view's inset,
// which Exact owns (the development check journals it); `data-transition`
// gives the stack a cross-fade through the forwarded delegate.
//
// On every Apple host, nodes the Contract marks `hook` (LLP 1075.003.000)
// reach `element`: the `badge` gets a context menu, an interaction Exact
// leaves to the app; each list row's `dot` hook does nothing, so what a
// hooked node costs is the cost of being hooked; with `data-violate` the
// detail's scroller (`detail-list`) gets an inset Exact owns.
import Foundation
#if os(macOS)
import AppKit
#else
import UIKit
#endif

final class FixtureModule: ExactModule {
    override class var views: [String: ExactNativeFactory] {
        ["exact-fixture": ExactNativeFactory(snapshot: true) { props, events in try FixtureBox(props: props, events: events) },
         "exact-plain": ExactNativeFactory { props, events in PlainBox(props: props, events: events) },
         "exact-screen": ExactNativeFactory { _, events in Screen(events: events) }]
    }

    override func element(_ element: ExactElement) {
        #if os(iOS)
        // Owned by Exact (LLP 1075.003.000 §3.6): the development check says so.
        if element.hook == .detailList, element.data[.violate] == "true" { element.scrollView?.contentInset.bottom = 1 }
        // A row's dot takes a recognizer of its own, which `elementEnded`
        // takes back; with `data-reuse` the hook says so, and its row may be
        // reused (LLP 1075.003.000.000 §8).
        if element.hook == .dot {
            if element.isNew { element.view?.addGestureRecognizer(DotPress()) }
            element.reusable = element.data[.reuse] == "true"
            return
        }
        #endif
        guard element.hook == .badge, element.isNew, let view = element.view else { return }
        #if os(iOS)
        view.addInteraction(UIContextMenuInteraction(delegate: badgeMenu))
        #else
        view.menu = NSMenu(title: "Badge \(element.data[.tone] ?? "")")
        #endif
    }

    #if os(macOS)
    private let toolbarPress = ToolbarPress()

    /// The window toolbar Exact installs for the authored `toolbar`: its
    /// display mode is the app's, and an item of its own goes after Exact's.
    /// (AppKit shows icons only under the compact style a window toolbar
    /// gets, so labels need the window's style too.)
    override func toolbar(_ toolbar: ExactToolbar) {
        toolbar.window?.toolbarStyle = .unified
        toolbar.toolbar.displayMode = .iconAndLabel
        let item = NSToolbarItem(itemIdentifier: .init("fixture.hooked"))
        item.label = "Hooked"
        item.image = NSImage(systemSymbolName: "star", accessibilityDescription: "Hooked")
        item.target = toolbarPress
        item.action = #selector(ToolbarPress.press)
        toolbar.add(item)
    }
    #endif

    #if os(iOS)
    private let badgeMenu = BadgeMenu()
    private let fade = Fade()
    private var targets: [String: Click] = [:]

    override func navigation(_ navigation: ExactNavigation) {
        navigation.controller.navigationBar.tintColor = .systemIndigo
    }

    override func route(_ route: ExactRoute) {
        let item = route.controller.navigationItem
        let more = route.data[.menu].map { id in
            let target = Click(route, id)
            targets[route.key] = target
            let button = UIBarButtonItem(image: UIImage(systemName: "ellipsis.circle"), style: .plain, target: target, action: #selector(Click.click))
            button.accessibilityIdentifier = "hook-more"
            return button
        }
        item.leftBarButtonItems = (item.leftBarButtonItems ?? []).filter { $0.accessibilityIdentifier != "hook-more" } + (more.map { [$0] } ?? [])
        if route.data[.violate] == "true" { route.contentScrollView?.contentInset.top = 1 }
        if let navigation = route.navigation {
            if route.data[.transition] == "true" { navigation.delegate = fade }
            else if navigation.delegate === fade { navigation.delegate = nil }
        }
    }

    override func elementEnded(_ element: ExactElement) {
        guard element.hook == .dot, let view = element.view else { return }
        for case let press as DotPress in view.gestureRecognizers ?? [] { view.removeGestureRecognizer(press) }
    }

    override func routeEnded(_ route: ExactRoute) {
        targets.removeValue(forKey: route.key)
    }

    override func tabs(_ tabs: ExactTabs) {
        tabs.controller.tabBar.tintColor = .systemIndigo
    }

    /// The app's own container, when the environment asks for it (the UIKit
    /// tests': `EXACT_FIXTURE_CONTAINER=app`): a segmented control over the
    /// selected tab's stack, through the seam alone.
    override func tabContainer(_ contents: ExactTabContents) -> UIViewController? {
        ProcessInfo.processInfo.environment["EXACT_FIXTURE_CONTAINER"] == "app" ? FixtureTabs(contents) : nil
    }
    #endif
}

#if os(iOS)
/// A container the app owns: every tab's stack a child, the selected one
/// shown; its control asks Exact to select, and Exact says what it chose.
final class FixtureTabs: UIViewController {
    let contents: ExactTabContents
    let control: UISegmentedControl
    init(_ contents: ExactTabContents) {
        self.contents = contents
        control = UISegmentedControl(items: contents.tabs.map { $0.item.title ?? $0.name })
        super.init(nibName: nil, bundle: nil)
        contents.onSelect = { [weak self] _ in self?.show() }
    }
    required init?(coder: NSCoder) { nil }
    override func viewDidLoad() {
        super.viewDidLoad()
        for tab in contents.tabs {
            addChild(tab.controller)
            view.addSubview(tab.controller.view)
            tab.controller.didMove(toParent: self)
        }
        control.addTarget(self, action: #selector(chose), for: .valueChanged)
        view.addSubview(control)
        show()
    }
    override func viewDidLayoutSubviews() {
        super.viewDidLayoutSubviews()
        let bar: CGFloat = 44, bottom = view.safeAreaInsets.bottom
        control.frame = CGRect(x: 16, y: view.bounds.height - bottom - bar, width: view.bounds.width - 32, height: bar - 8)
        for tab in contents.tabs { tab.controller.view.frame = view.bounds }
        additionalSafeAreaInsets.bottom = bar
    }
    private func show() {
        for (index, tab) in contents.tabs.enumerated() { tab.controller.view.isHidden = index != contents.selected }
        control.selectedSegmentIndex = contents.selected
        view.bringSubviewToFront(control)
    }
    @objc private func chose() {
        contents.select(control.selectedSegmentIndex)
        control.selectedSegmentIndex = contents.selected
    }
}

/// A native screen: a whole controller in the node's box, a child of the
/// route's controller (LLP 1075.003 §3.6).
final class Screen: ExactNativeScreen {
    init(events: ExactNativeEvents) {
        let controller = UIViewController()
        let label = UILabel()
        label.text = "A native screen"
        label.textAlignment = .center
        label.frame = controller.view.bounds
        label.autoresizingMask = [.flexibleWidth, .flexibleHeight]
        controller.view.addSubview(label)
        controller.view.backgroundColor = .secondarySystemBackground
        super.init(screen: controller, events: events)
    }
}
#else
/// macOS has no native screens (it projects no routes): a label in a box.
final class Screen: ExactNativeInstance {
    private let label = NSTextField(labelWithString: "A native screen")
    override init(events: ExactNativeEvents) { super.init(events: events) }
    override var view: ExactNativeView { label }
}
#endif

#if os(iOS)
/// A hook-made control that stands for an authored one: it clicks it, so the
/// agent, tapping the authored control, runs the same handler.
/// The dot's own recognizer: what an app's hook adds to a row's view.
private final class DotPress: UILongPressGestureRecognizer {}

/// The badge's context menu: one item, with the badge's word.
private final class BadgeMenu: NSObject, UIContextMenuInteractionDelegate {
    func contextMenuInteraction(_ interaction: UIContextMenuInteraction, configurationForMenuAtLocation location: CGPoint) -> UIContextMenuConfiguration? {
        UIContextMenuConfiguration(actionProvider: { _ in UIMenu(children: [UIAction(title: "Badge") { _ in }]) })
    }
}

/// The route is held weakly: the bar item holds this target, and the route's
/// controller holds the bar item.
private final class Click: NSObject {
    weak var route: ExactRoute?
    let id: String
    init(_ route: ExactRoute, _ id: String) { self.route = route; self.id = id }
    @objc func click() { route?.element(id)?.click() }
}

/// A cross-fade for every push and pop: an app's custom transition, through
/// the delegate Exact forwards (LLP 1075.003 §3.5).
private final class Fade: NSObject, UINavigationControllerDelegate, UIViewControllerAnimatedTransitioning {
    func navigationController(_ nav: UINavigationController, animationControllerFor operation: UINavigationController.Operation,
                              from: UIViewController, to: UIViewController) -> UIViewControllerAnimatedTransitioning? { self }
    func transitionDuration(using context: UIViewControllerContextTransitioning?) -> TimeInterval { 0.25 }
    func animateTransition(using context: UIViewControllerContextTransitioning) {
        guard let to = context.viewController(forKey: .to), let view = context.view(forKey: .to) else {
            return context.completeTransition(false)
        }
        view.frame = context.finalFrame(for: to)
        view.alpha = 0
        context.containerView.addSubview(view)
        UIView.animate(withDuration: transitionDuration(using: context), animations: { view.alpha = 1 }) { _ in
            context.completeTransition(!context.transitionWasCancelled)
        }
    }
}
#endif
let exactModule: ExactModule.Type = FixtureModule.self

private func rgb(_ hex: String?) -> (CGFloat, CGFloat, CGFloat) {
    guard let hex, hex.hasPrefix("#"), hex.count == 7, let v = UInt32(hex.dropFirst(), radix: 16) else { return (0.5, 0.5, 0.5) }
    return (CGFloat((v >> 16) & 0xff) / 255, CGFloat((v >> 8) & 0xff) / 255, CGFloat(v & 0xff) / 255)
}

private func echo(_ props: [String: String]) -> String {
    let data = (try? JSONSerialization.data(withJSONObject: props, options: [.sortedKeys, .withoutEscapingSlashes])) ?? Data()
    return "props:" + String(decoding: data, as: UTF8.self)
}

#if os(macOS)
private final class Passive: NSView {
    override func hitTest(_ point: NSPoint) -> NSView? { nil }
}
private final class Drawn: NSView {
    var tint = (CGFloat(0.5), CGFloat(0.5), CGFloat(0.5))
    override func hitTest(_ point: NSPoint) -> NSView? { subviews.isEmpty ? nil : super.hitTest(point) }
    override func draw(_ dirtyRect: NSRect) {
        NSColor(srgbRed: tint.0, green: tint.1, blue: tint.2, alpha: 1).setFill()
        bounds.fill()
    }
}
#else
private final class Passive: UIView {}
private final class Drawn: UIView {
    var tint = (CGFloat(0.5), CGFloat(0.5), CGFloat(0.5))
    override func draw(_ rect: CGRect) {
        UIColor(red: tint.0, green: tint.1, blue: tint.2, alpha: 1).setFill()
        UIRectFill(bounds)
    }
}
#endif

final class FixtureBox: ExactNativeInstance {
    private let box = Passive(frame: .zero)
    private var tint = (CGFloat(0.5), CGFloat(0.5), CGFloat(0.5))
    private var emit = "0"

    init(props: [String: String], events: ExactNativeEvents) throws {
        if props["reject"] == "true" { throw ExactNativeRefusal("reject=true") }
        super.init(events: events)
        #if os(macOS)
        box.wantsLayer = true
        #else
        box.isUserInteractionEnabled = false
        #endif
        apply(props)
        emit = props["emit"] ?? "0"
        events.message(echo(props))
        events.load()
    }

    override var view: ExactNativeView { box }

    private func apply(_ props: [String: String]) {
        events.intrinsicSize(CGSize(width: 200, height: 96))
        tint = rgb(props["tint"])
        #if os(macOS)
        box.layer?.backgroundColor = CGColor(srgbRed: tint.0, green: tint.1, blue: tint.2, alpha: 1)
        #else
        box.backgroundColor = UIColor(red: tint.0, green: tint.1, blue: tint.2, alpha: 1)
        #endif
    }

    override func setProps(_ props: [String: String]) throws {
        if props["reject"] == "true" { throw ExactNativeRefusal("reject=true") }
        apply(props)
        events.message(echo(props))
        let next = props["emit"] ?? "0"
        guard next != emit else { return }
        emit = next
        guard (Int(next) ?? 0) > 0 else { return }
        // Every event, from a background thread, in order: the host copies,
        // hops to its presenter and enters the runner after the batch.
        let events = self.events
        DispatchQueue.global().async {
            events.press(); events.change("changed"); events.hover(true); events.focus(); events.blur()
            events.key("Enter"); events.submit(); events.load(); events.message("hello")
        }
    }

    override func snapshot() throws -> Data {
        let size = box.bounds.size
        guard size.width > 0, size.height > 0 else { throw ExactNativeRefusal("no bounds yet") }
        let w = Int(size.width * 2), h = Int(size.height * 2)
        guard let ctx = CGContext(data: nil, width: w, height: h, bitsPerComponent: 8, bytesPerRow: w * 4,
                                  space: CGColorSpace(name: CGColorSpace.sRGB)!, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)
        else { throw ExactNativeRefusal("no context") }
        ctx.setFillColor(CGColor(srgbRed: tint.0, green: tint.1, blue: tint.2, alpha: 1))
        ctx.fill(CGRect(x: 0, y: 0, width: w, height: h))
        guard let image = ctx.makeImage() else { throw ExactNativeRefusal("no image") }
        #if os(macOS)
        let rep = NSBitmapImageRep(cgImage: image)
        guard let png = rep.representation(using: .png, properties: [:]) else { throw ExactNativeRefusal("no PNG") }
        return png
        #else
        guard let png = UIImage(cgImage: image).pngData() else { throw ExactNativeRefusal("no PNG") }
        return png
        #endif
    }

    override func destroy() {
        // A late callback, as a PTY's reader thread would make: the host has
        // already invalidated this instance's nonce and must drop it.
        let events = self.events
        DispatchQueue.global().asyncAfter(deadline: .now() + 0.1) { events.message("late") }
    }
}

#if os(macOS)
private final class FixtureEditor: NSTextView, NSTextViewDelegate {
    var events: ExactNativeEvents?
    override func becomeFirstResponder() -> Bool {
        let accepted = super.becomeFirstResponder()
        if accepted { events?.focus() }
        return accepted
    }
    override func resignFirstResponder() -> Bool {
        let accepted = super.resignFirstResponder()
        if accepted { events?.blur() }
        return accepted
    }
    func textDidChange(_ notification: Notification) { events?.change(string) }
    override func keyDown(with event: NSEvent) {
        events?.key(event.keyCode == 51 ? "Backspace" : event.characters ?? "")
        super.keyDown(with: event)
    }
}
#endif

final class PlainBox: ExactNativeInstance {
    private let box = Drawn(frame: .zero)
    #if os(macOS)
    private var editor: FixtureEditor?
    override var focusTarget: ExactNativeView? { editor }
    override func agentInput(_ input: ExactNativeInput) throws {
        guard let editor, editor.window?.firstResponder === editor else { throw ExactNativeRefusal("no focused fixture editor") }
        switch input {
        case .text(let text):
            editor.selectAll(nil)
            editor.insertText(text, replacementRange: editor.selectedRange())
        case .key(let key, let phase):
            // Exercise real AppKit commands; unsupported chords refuse before delivery.
            let chars: String, code: UInt16
            switch key {
            case "Backspace": chars = "\u{7f}"; code = 51
            case "ArrowLeft": chars = "\u{f702}"; code = 123
            case "ArrowRight": chars = "\u{f703}"; code = 124
            case "Enter": chars = "\r"; code = 36
            default: throw ExactNativeRefusal("fixture does not support key \(key)")
            }
            let phases: [NSEvent.EventType] = phase == "up" ? [.keyUp] : phase == "down" ? [.keyDown] : [.keyDown, .keyUp]
            for type in phases {
                guard let event = NSEvent.keyEvent(with: type,
                    location: .zero, modifierFlags: [], timestamp: ProcessInfo.processInfo.systemUptime,
                    windowNumber: editor.window!.windowNumber, context: nil, characters: chars,
                    charactersIgnoringModifiers: chars, isARepeat: false, keyCode: code) else { throw ExactNativeRefusal("no key event") }
                if type == .keyUp { editor.keyUp(with: event) } else { editor.keyDown(with: event) }
            }
        }
    }
    #endif

    init(props: [String: String], events: ExactNativeEvents) {
        super.init(events: events)
        #if os(iOS)
        box.isUserInteractionEnabled = false
        box.contentMode = .redraw
        #endif
        #if os(macOS)
        if props["customInput"] == "true" {
            let editor = FixtureEditor(frame: NSRect(x: 0, y: 0, width: 200, height: 48))
            editor.events = events
            editor.delegate = editor
            editor.isRichText = false
            editor.autoresizingMask = [.width, .height]
            box.addSubview(editor)
            self.editor = editor
        }
        #endif
        apply(props)
    }

    override var view: ExactNativeView { box }

    private func apply(_ props: [String: String]) {
        events.intrinsicSize(props["natural"] == "false" ? nil : CGSize(width: 120, height: props["expanded"] == "true" ? 64 : 32))
        box.tint = rgb(props["tint"])
    }

    override func setProps(_ props: [String: String]) throws {
        apply(props)
        #if os(macOS)
        box.needsDisplay = true
        #else
        box.setNeedsDisplay()
        #endif
    }
}

#if os(macOS)
/// The hooked toolbar item's target: an app's own action, which AppKit
/// validates through it.
private final class ToolbarPress: NSObject {
    @objc func press() {}
}
#endif
