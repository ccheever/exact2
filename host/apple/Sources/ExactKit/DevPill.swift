// What the dev loop says before it has a plan to give (dtool2's pass 3 #8;
// llp/1023.001's dev status): a small pill in the window's bottom corner,
// "Dev loop: building the module producer …", while a native client's first
// join waits on the loop's producers, or the error their start ended with.
// Dev only (a connection to a dev URL; never in a release build), drawn
// by the host outside the page and the kernel's tree, never under the agent,
// and gone when the plan comes.
#if os(macOS)
import AppKit
#else
import UIKit
#endif

#if os(macOS)
/// A label clicks pass through: the page under it keeps them.
private final class DevPillLabel: NSTextField {
    override func hitTest(_ point: NSPoint) -> NSView? { nil }
}
#else
private final class DevPillLabel: UILabel {}
#endif

extension Presenter {
    /// Show `text` in the pill, or take the pill away (nil). Kept on the app
    /// (`ExactApp.devStatusText`), so a window that opens or a view that
    /// moves meanwhile shows it too (`ExactView`'s window callbacks).
    func showDevStatus(_ text: String?) {
        guard !ExactEnv.agentMode, !ExactEnv.releaseBuild, let text, let window = viewport.window else {
            devPill?.removeFromSuperview()
            devPill = nil
            return
        }
        #if os(macOS)
        let pill = (devPill as? DevPillLabel) ?? DevPillLabel(labelWithString: "")
        pill.stringValue = text
        pill.font = .systemFont(ofSize: NSFont.smallSystemFontSize)
        pill.textColor = .white
        pill.drawsBackground = true
        pill.backgroundColor = NSColor.black.withAlphaComponent(0.72)
        pill.wantsLayer = true
        pill.layer?.cornerRadius = 6
        pill.lineBreakMode = .byTruncatingTail
        pill.setAccessibilityElement(false)
        guard let host = window.contentView else { return }
        // One pill a window: a second session in it (a split's pane) shows none.
        if host.subviews.contains(where: { $0 is DevPillLabel && $0 !== devPill }) { devPill?.removeFromSuperview(); devPill = nil; return }
        if pill.superview !== host { host.addSubview(pill, positioned: .above, relativeTo: nil) }
        let size = pill.intrinsicContentSize
        let width = min(size.width + 16, max(host.bounds.width - 24, 40))
        pill.frame = NSRect(x: host.bounds.maxX - width - 12, y: 12, width: width, height: size.height + 8)
        pill.autoresizingMask = [.minXMargin, .maxYMargin]
        #else
        if window.subviews.contains(where: { $0 is DevPillLabel && $0 !== devPill }) { devPill?.removeFromSuperview(); devPill = nil; return }
        let pill = (devPill as? DevPillLabel) ?? DevPillLabel()
        pill.text = "  \(text)  "
        pill.font = .preferredFont(forTextStyle: .caption1)
        pill.textColor = .white
        pill.backgroundColor = UIColor.black.withAlphaComponent(0.72)
        pill.layer.cornerRadius = 8
        pill.clipsToBounds = true
        pill.numberOfLines = 2
        pill.isAccessibilityElement = false
        pill.isUserInteractionEnabled = false
        if pill.superview !== window { window.addSubview(pill) }
        window.bringSubviewToFront(pill)
        let inset = window.safeAreaInsets, room = window.bounds.width - inset.left - inset.right - 24
        let size = pill.sizeThatFits(CGSize(width: room, height: .greatestFiniteMagnitude))
        let width = min(size.width, room)
        pill.frame = CGRect(x: window.bounds.maxX - inset.right - 12 - width, y: window.bounds.maxY - inset.bottom - 12 - size.height - 8,
                            width: width, height: size.height + 8)
        pill.autoresizingMask = [.flexibleLeftMargin, .flexibleTopMargin]
        #endif
        devPill = pill
    }
}

extension PlanURL.DevStatus {
    /// As the dev menu's `url:` line says it.
    var said: String {
        switch self {
        case .preparing(let message): "dev loop: \(message)"
        case .failed(let message): "dev loop failed to start: \(message)"
        }
    }
}

extension ExactApp {
    /// The dev loop's status the windows show, or nil (one app, one status).
    nonisolated(unsafe) static var devStatusText: String?

    /// The dev loop's status in every window, or none.
    func showDevStatus(_ status: PlanURL.DevStatus?) {
        let text: String? = switch status {
        case .preparing(let message): "Dev loop: \(message)…"
        case .failed(let message): "Dev loop failed to start: \(message)"
        case nil: nil
        }
        Self.devStatusText = text
        refreshDevPills()
    }

    /// Each window's pill as the status is now: a view that left a window
    /// takes its pill away first, so a session still in that window can take it.
    func refreshDevPills() {
        let live = sessions.filter { $0.state != .destroyed }
        for session in live where session.presenter.viewport.window == nil { session.presenter.showDevStatus(nil) }
        for session in live { session.presenter.showDevStatus(Self.devStatusText) }
    }
}
