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

    #if os(iOS)
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
    override func hitTest(_ point: NSPoint) -> NSView? { nil }
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

final class PlainBox: ExactNativeInstance {
    private let box = Drawn(frame: .zero)

    init(props: [String: String], events: ExactNativeEvents) {
        super.init(events: events)
        #if os(iOS)
        box.isUserInteractionEnabled = false
        box.contentMode = .redraw
        #endif
        box.tint = rgb(props["tint"])
    }

    override var view: ExactNativeView { box }

    override func setProps(_ props: [String: String]) throws {
        box.tint = rgb(props["tint"])
        #if os(macOS)
        box.needsDisplay = true
        #else
        box.setNeedsDisplay()
        #endif
    }
}
