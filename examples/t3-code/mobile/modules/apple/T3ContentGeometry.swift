import Foundation

private final class T3ContentGeometryClock: @unchecked Sendable {
    static let shared = T3ContentGeometryClock()
    private let lock = NSLock()
    private var value = 0
    func next() -> Int {
        lock.lock()
        defer { lock.unlock() }
        precondition(value < 9_007_199_254_740_991)
        value += 1
        return value
    }
}

struct T3ContentGeometryLifetime {
    private(set) var owner = ""
    private(set) var generation = 0
    private(set) var serial = 0
    private(set) var attached = false
    private(set) var hinge = "unknown"

    mutating func bind(owner: String, attached: Bool) {
        self.owner = owner
        self.attached = attached
        generation = T3ContentGeometryClock.shared.next()
        serial = 0
    }

    mutating func observeHinge(_ present: Bool) {
        hinge = hinge == "present" || present ? "present" : "absent"
    }

    func owns(owner: String, generation: Int) -> Bool {
        attached && self.owner == owner && self.generation == generation
    }

    mutating func packet(metrics: [String: Any]?) -> String? {
        guard !owner.isEmpty else { return nil }
        precondition(serial < 9_007_199_254_740_991)
        serial += 1
        var value = attached ? metrics ?? [:] : [:]
        value["state"] = attached && metrics != nil ? "ready" : "unavailable"
        value["owner"] = owner
        value["generation"] = generation
        value["serial"] = serial
        value["hinge"] = hinge
        value["coordinateSpace"] = "content-observer"
        guard let data = try? JSONSerialization.data(withJSONObject: value, options: .sortedKeys) else { return nil }
        return "\(generation)\n\(serial)\n\(String(decoding: data, as: UTF8.self))"
    }
}

#if os(iOS)
import UIKit

private final class T3ContentGeometryProbe: UIView {
    var changed: (() -> Void)?
    override func layoutSubviews() { super.layoutSubviews(); changed?() }
    override func safeAreaInsetsDidChange() { super.safeAreaInsetsDidChange(); changed?() }
    override func didMoveToWindow() { super.didMoveToWindow(); changed?() }
    override func didMoveToSuperview() { super.didMoveToSuperview(); changed?() }
    override func hitTest(_ point: CGPoint, with event: UIEvent?) -> UIView? { nil }
}

final class T3ContentGeometry: ExactNativeInstance {
    private let root = T3ContentGeometryProbe()
    private var lifetime = T3ContentGeometryLifetime()
    private weak var observedWindow: UIWindow?
    private weak var observedParent: UIView?
    private var previous: NSDictionary?
    private var alive = true
    private var hingeInteraction: UIInteraction?
    override var view: UIView { root }
    static let factory = ExactNativeFactory { props, events in
        let instance = T3ContentGeometry(events: events)
        try instance.setProps(props)
        return instance
    }

    override init(events: ExactNativeEvents) {
        super.init(events: events)
        root.backgroundColor = .clear
        root.isUserInteractionEnabled = false
        root.isAccessibilityElement = false
        root.accessibilityElementsHidden = true
        root.changed = { [weak self] in self?.refresh() }
        MainActor.assumeIsolated {
            root.registerForTraitChanges([UITraitHorizontalSizeClass.self, UITraitVerticalSizeClass.self]) { [weak self] (_: T3ContentGeometryProbe, _: UITraitCollection) in
                self?.refresh()
            }
            #if canImport(UIKit, _version: 9127.0.85)
            if #available(iOS 27.1, *) {
                root.registerForTraitChanges(UITraitCollection.systemTraitsAffectingVerticalBarEdge) { [weak self] (_: T3ContentGeometryProbe, _: UITraitCollection) in
                    self?.refresh()
                }
            }
            #endif
        }
    }

    override func setProps(_ props: [String: String]) throws {
        guard let owner = props["owner"], !owner.isEmpty else {
            throw ExactNativeRefusal("Content geometry needs its mounted route owner.")
        }
        if owner != lifetime.owner {
            rebind(owner: owner)
        }
        refresh()
    }

    private func rebind(owner: String) {
        lifetime.bind(owner: owner, attached: root.window != nil && root.superview != nil)
        if let hingeInteraction { root.removeInteraction(hingeInteraction) }
        hingeInteraction = nil
        observedWindow = root.window
        observedParent = root.superview
        previous = nil
        #if canImport(UIKit, _version: 9127.0.85)
        if #available(iOS 27.1, *) {
            if lifetime.attached {
                let generation = lifetime.generation
                let interaction = UIHingeInteraction { [weak self] _, update in
                    guard let self, self.alive, self.root.window != nil, self.root.superview != nil,
                          self.lifetime.owns(owner: owner, generation: generation),
                          self.root.window === self.observedWindow, self.root.superview === self.observedParent else { return }
                    self.lifetime.observeHinge(update.hinge != nil)
                    self.publish()
                }
                hingeInteraction = interaction
                root.addInteraction(interaction)
            }
        } else {
            lifetime.observeHinge(false)
        }
        #else
        lifetime.observeHinge(false)
        #endif
    }

    private func refresh() {
        guard alive else { return }
        if root.window !== observedWindow || root.superview !== observedParent
            || lifetime.attached != (root.window != nil && root.superview != nil) {
            rebind(owner: lifetime.owner)
        }
        publish()
    }

    private func publish() {
        guard alive, !lifetime.owner.isEmpty else { return }
        let metrics = measure()
        var comparison = metrics ?? [:]
        comparison["state"] = metrics == nil ? "unavailable" : "ready"
        comparison["hinge"] = lifetime.hinge
        let next = comparison as NSDictionary
        guard previous?.isEqual(next) != true else { return }
        if let packet = lifetime.packet(metrics: metrics) {
            previous = next
            events.change(packet)
        }
    }

    private func measure() -> [String: Any]? {
        let bounds = root.bounds
        guard lifetime.attached, root.window != nil, root.superview != nil,
              bounds.width > 0, bounds.height > 0,
              [bounds.minX, bounds.maxX, bounds.minY, bounds.maxY].allSatisfy({ $0.isFinite }),
              [root.safeAreaInsets.top, root.safeAreaInsets.right, root.safeAreaInsets.bottom, root.safeAreaInsets.left]
                .allSatisfy({ $0.isFinite && $0 >= 0 }) else { return nil }
        var verticalBarEdge = "none"
        var regions: [[String: Any]] = []
        var bottomBarInset: Any = NSNull()
        #if canImport(UIKit, _version: 9127.0.85)
        if #available(iOS 27.1, *) {
            let rtl = root.effectiveUserInterfaceLayoutDirection == .rightToLeft
            switch root.traitCollection.verticalBarEdge {
            case .leading: verticalBarEdge = rtl ? "right" : "left"
            case .trailing: verticalBarEdge = rtl ? "left" : "right"
            default: break
            }
            let bottomInset = root.edgeInsets(for: .bar(onEdge: UIRectEdge.bottom, extent: 48)).bottom
            guard bottomInset.isFinite, bottomInset >= 0 else { return nil }
            bottomBarInset = Double(bottomInset)
            for kind in [UIView.ReservedRegion.Kind.division, .occlusion] {
                for region in root.reservedRegions(kind: kind) where region.isActive {
                    let frame = region.frame.intersection(bounds)
                    guard !frame.isNull, !frame.isEmpty else { continue }
                    guard [frame.minX, frame.maxX, frame.minY, frame.maxY].allSatisfy({ $0.isFinite }) else { return nil }
                    regions.append(["kind": kind == .division ? "division" : "occlusion",
                        "x": Double(frame.minX), "y": Double(frame.minY),
                        "width": Double(frame.width), "height": Double(frame.height)])
                }
            }
        }
        #endif
        return ["bounds": ["x": Double(bounds.minX), "y": Double(bounds.minY), "width": Double(bounds.width), "height": Double(bounds.height)],
            "safeArea": ["top": Double(root.safeAreaInsets.top), "right": Double(root.safeAreaInsets.right),
                "bottom": Double(root.safeAreaInsets.bottom), "left": Double(root.safeAreaInsets.left)],
            "horizontalSizeClass": sizeClass(root.traitCollection.horizontalSizeClass),
            "verticalSizeClass": sizeClass(root.traitCollection.verticalSizeClass),
            "verticalBarEdge": verticalBarEdge, "bottomBarInset": bottomBarInset, "reservedRegions": regions]
    }

    private func sizeClass(_ value: UIUserInterfaceSizeClass) -> String {
        switch value {
        case .compact: return "compact"
        case .regular: return "regular"
        default: return "unspecified"
        }
    }

    override func destroy() {
        guard alive else { return }
        lifetime.bind(owner: lifetime.owner, attached: false)
        previous = nil
        publish()
        alive = false
        root.changed = nil
        if let hingeInteraction { root.removeInteraction(hingeInteraction) }
        hingeInteraction = nil
        observedWindow = nil
        observedParent = nil
    }
}
#endif
