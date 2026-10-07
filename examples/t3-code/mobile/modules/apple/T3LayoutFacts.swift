#if os(iOS)
// Existing X22/#127: app-owned UIKit facts for mobile safe-area-dependent chrome.
// @ref llp/1107.004-home-projection.decision.md#rendering
import UIKit

private final class T3LayoutProbe: UIView {
    var changed: (() -> Void)?
    override func layoutSubviews() { super.layoutSubviews(); changed?() }
    override func safeAreaInsetsDidChange() { super.safeAreaInsetsDidChange(); changed?() }
    override func didMoveToWindow() { super.didMoveToWindow(); changed?() }
    override func didMoveToSuperview() { super.didMoveToSuperview(); changed?() }
    override func hitTest(_ point: CGPoint, with event: UIEvent?) -> UIView? { nil }
}

final class T3LayoutFacts: ExactNativeInstance {
    private let root = T3LayoutProbe()
    private var previous = ""
    private var alive = true
    private weak var observedWindow: UIWindow?
    private var keyboardObservers: [NSObjectProtocol] = []
    private weak var guideOwner: UIView?
    private var guideProbe: UIView?
    private var guideConstraints: [NSLayoutConstraint] = []
    private var ownerRevision = 0
    private var recheckQueued = false
    private var refreshing = false
    override var view: UIView { root }
    static let factory = ExactNativeFactory { _, events in T3LayoutFacts(events: events) }
    override init(events: ExactNativeEvents) {
        super.init(events: events)
        root.backgroundColor = .clear
        root.isAccessibilityElement = false
        root.changed = { [weak self] in self?.windowChanged() }
    }
    private func windowChanged() {
        guard alive, !refreshing else { return }
        refreshing = true
        defer { refreshing = false }
        let window = root.window
        if observedWindow !== window {
            removeKeyboardObservers()
            removeGuideProbe()
            observedWindow = window
            previous = ""
            if let window {
                for name in [UIResponder.keyboardWillChangeFrameNotification,
                             UIResponder.keyboardDidChangeFrameNotification,
                             UIResponder.keyboardDidHideNotification] {
                    keyboardObservers.append(NotificationCenter.default.addObserver(
                        forName: name, object: nil, queue: .main) { [weak self, weak window] _ in
                        guard let self, let window, self.root.window === window,
                              self.observedWindow === window else { return }
                        self.windowChanged()
                    })
                }
            }
        }
        let owner = contentOwner(in: window)
        if guideOwner !== owner {
            removeGuideProbe()
            if let owner {
                guideOwner = owner
                let probe = UIView()
                probe.isHidden = true
                probe.isUserInteractionEnabled = false
                probe.isAccessibilityElement = false
                probe.translatesAutoresizingMaskIntoConstraints = false
                owner.addSubview(probe)
                guideProbe = probe
                guideConstraints = [
                    probe.topAnchor.constraint(equalTo: owner.keyboardLayoutGuide.topAnchor),
                    probe.leadingAnchor.constraint(equalTo: owner.leadingAnchor),
                    probe.widthAnchor.constraint(equalToConstant: 0),
                    probe.heightAnchor.constraint(equalToConstant: 0),
                ]
                NSLayoutConstraint.activate(guideConstraints)
                scheduleOwnerRecheck()
            }
        }
        publish()
    }
    // The viewport moves into the presented content controller. Its public
    // responder chain identifies that owner without inspecting host classes.
    private func contentOwner(in window: UIWindow?) -> UIView? {
        guard let window else { return nil }
        var responder: UIResponder? = root
        while let current = responder {
            if let controller = current as? UIViewController,
               let owner = controller.viewIfLoaded, owner.window === window,
               root.isDescendant(of: owner) { return owner }
            responder = current.next
        }
        return nil
    }
    private func scheduleOwnerRecheck() {
        guard !recheckQueued else { return }
        recheckQueued = true
        let revision = ownerRevision
        // Containment and the newly dependent guide settle after this mount.
        // This is one recheck per owner change, not keyboard polling.
        DispatchQueue.main.async { [weak self] in
            guard let self, self.alive, self.ownerRevision == revision else { return }
            self.recheckQueued = false
            if let owner = self.guideOwner, self.contentOwner(in: self.root.window) === owner {
                owner.layoutIfNeeded()
            }
            self.windowChanged()
        }
    }
    private func removeGuideProbe() {
        ownerRevision += 1
        recheckQueued = false
        NSLayoutConstraint.deactivate(guideConstraints)
        guideConstraints.removeAll()
        guideProbe?.removeFromSuperview()
        guideProbe = nil
        guideOwner = nil
    }
    private func removeKeyboardObservers() {
        keyboardObservers.forEach(NotificationCenter.default.removeObserver)
        keyboardObservers.removeAll()
    }
    private func publish() {
        guard alive, let window = root.window else { return }
        let glass: Bool
        if #available(iOS 26.0, *) { glass = true } else { glass = false }
        let docked = guideOwner.map {
            t3KeyboardDocked(bounds: $0.bounds, safeBottom: $0.safeAreaInsets.bottom,
                             guide: $0.keyboardLayoutGuide.layoutFrame)
        } ?? false
        let value: [String: Any] = ["safeTop": Double(window.safeAreaInsets.top), "safeBottom": Double(window.safeAreaInsets.bottom), "liquidGlass": glass,
            "keyboardDocked": docked]
        guard let data = try? JSONSerialization.data(withJSONObject: value, options: .sortedKeys) else { return }
        let text = String(decoding: data, as: UTF8.self)
        guard text != previous else { return }
        previous = text
        events.change(text)
    }
    override func destroy() {
        alive = false
        root.changed = nil
        removeGuideProbe()
        removeKeyboardObservers()
        observedWindow = nil
    }
}
// The public guide rests over the safe area when closed or undocked. A docked
// hardware accessory bar is a bottom obstruction too; editor focus is not one.
private func t3KeyboardDocked(bounds: CGRect, safeBottom: CGFloat, guide: CGRect) -> Bool {
    guard bounds.width > 0, bounds.height > 0,
          [bounds.minX, bounds.maxX, bounds.minY, bounds.maxY,
           guide.minX, guide.maxX, guide.minY, guide.maxY, safeBottom].allSatisfy({ $0.isFinite }),
          guide.minX <= bounds.minX, guide.maxX >= bounds.maxX,
          guide.maxY >= bounds.maxY else { return false }
    let overlap = max(0, bounds.maxY - max(bounds.minY, guide.minY))
    return overlap > max(0, safeBottom)
}
#endif
