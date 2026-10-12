#if os(macOS)
import AppKit
import WebKit

/// browser-surface part 2: a Browser tab's zoom and appearance (MIT reference, see LICENSE-T3, T3 Code 1e2ecbd975:
/// apps/desktop/src/preview/Manager.ts `applyZoom`, `normalizeZoomFactor`, `setColorScheme`, `applyColorScheme`;
/// packages/contracts/src/preview.ts `PREVIEW_ZOOM_LEVELS`).
///
/// - Zoom is WebKit's `pageZoom`: the page's CSS pixels grow, its viewport narrows and `devicePixelRatio` follows,
///   as Chromium's `setZoomFactor` does. The data module steps the ladder (browser-viewport.ts); a value off the
///   ladder snaps to its nearest step here too, as Manager.ts clamps one that arrives over IPC. The web view keeps
///   it across navigations.
/// - Appearance is the web view's `appearance`, which WebKit hands the page as `prefers-color-scheme`: Light and
///   Dark force it; System clears it, so the page follows the window, which follows the app's theme (the reference
///   clears the CDP override, and Electron's native theme follows the app's theme too).
extension T3BrowserSession {
    static let zoomLevels: [Double] = [0.25, 0.33, 0.5, 0.67, 0.75, 0.8, 0.9, 1.0, 1.1, 1.25, 1.5, 1.75, 2.0, 2.5, 3.0, 4.0, 5.0]

    static func normalizedZoom(_ value: Double) -> Double {
        guard value.isFinite else { return 1 }
        return zoomLevels.min { abs($0 - value) < abs($1 - value) } ?? 1
    }

    /// Manager.ts `findZoomStep`: the ladder step at or below a zoom.
    static func zoomStep(_ current: Double) -> Int {
        guard let index = zoomLevels.firstIndex(where: { abs($0 - current) < 0.001 || $0 > current }) else { return zoomLevels.count - 1 }
        return abs(zoomLevels[index] - current) < 0.001 ? index : index - 1
    }

    /// Manager.ts `nextZoomLevel`, from the page's own zoom (the reference steps the tab's authoritative zoom, so held keys
    /// never step from a stale value).
    static func nextZoom(_ current: Double, up: Bool) -> Double {
        let step = zoomStep(current)
        return up ? zoomLevels[min(step + 1, zoomLevels.count - 1)] : zoomLevels[max(step - 1, 0)]
    }

    var zoomFactor: Double { Double(web.pageZoom) }

    var colorScheme: String {
        switch web.appearance?.name {
        case NSAppearance.Name.darkAqua?: return "dark"
        case NSAppearance.Name.aqua?: return "light"
        default: return "system"
        }
    }

    func setZoom(_ value: Double) {
        let next = CGFloat(Self.normalizedZoom(value))
        guard abs(web.pageZoom - next) > 0.0001 else { return }
        web.pageZoom = next
        changed?()
    }

    func setColorScheme(_ scheme: String) {
        let next: NSAppearance? = scheme == "dark" ? NSAppearance(named: .darkAqua) : scheme == "light" ? NSAppearance(named: .aqua) : nil
        guard web.appearance?.name != next?.name else { return }
        web.appearance = next
        changed?()
    }

    /// Joined to the tab's report (`presentation.browserTabs[id]`).
    var navigationReport: [String: Any] { ["zoomFactor": zoomFactor, "colorScheme": colorScheme] }
}

extension T3BrowserSessions {
    /// Part 2's op (T3Module+Browser.swift): `browserSet` sets a tab's zoom (`zoom`, or a ladder step `zoomStep`: in, out)
    /// and/or its appearance; nil for any other op.
    func performNavigation(_ request: [String: Any]) -> [String: Any]? {
        guard request["op"] as? String == "browserSet" else { return nil }
        let generation = request["generation"] as? Int ?? 0
        guard let session = sessions[request["tab"] as? String ?? ""] else { return ["ok": true, "generation": generation, "value": ["done": false]] }
        if let zoom = (request["zoom"] as? NSNumber)?.doubleValue { session.setZoom(zoom) }
        if let step = request["zoomStep"] as? String, step == "in" || step == "out" { session.setZoom(T3BrowserSession.nextZoom(session.zoomFactor, up: step == "in")) }
        if let scheme = request["colorScheme"] as? String, ["system", "light", "dark"].contains(scheme) { session.setColorScheme(scheme) }
        return ["ok": true, "generation": generation, "value": ["done": true, "zoomFactor": session.zoomFactor, "colorScheme": session.colorScheme]]
    }
}
#endif
