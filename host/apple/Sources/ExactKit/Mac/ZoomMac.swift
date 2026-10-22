// View ▸ Actual Size ⌘0, Zoom In ⌘+ and Zoom Out ⌘- (LLP 1021 §5.3), as
// Safari, Preview and Electron's default menu have them. The zoom is the main
// window's, one per session, and multiplies the root font size last, over
// the app's own `setRootFontSize` as a browser's page zoom is over an
// author's `html { font-size }` (`Runner::set_root_font_zoom`). Declared
// deviation: a browser's zoom scales `px` lengths too; here only what
// follows the root size (`rem`, `em` and inherited text) grows.
#if os(macOS)
import AppKit

/// The zoom levels Safari offers (Settings ▸ Websites ▸ Page Zoom), which
/// Zoom In and Zoom Out step through.
enum PageZoom {
    static let levels: [Double] = [0.5, 0.75, 0.85, 1, 1.15, 1.25, 1.5, 1.75, 2, 2.5, 3]
    static func next(after zoom: Double) -> Double? { levels.first { $0 > zoom + 1e-9 } }
    static func previous(before zoom: Double) -> Double? { levels.last { $0 < zoom - 1e-9 } }
}

/// Each session's zoom, which outlives a reload (a new runner is told it
/// again with the page's facts, `tellPage`) but not the session.
private let zooms = NSMapTable<ExactSession, NSNumber>.weakToStrongObjects()

extension ExactSession {
    /// The zoom over this session's root font size; 1 is Actual Size.
    var rootZoom: Double {
        get { zooms.object(forKey: self)?.doubleValue ?? 1 }
        set {
            zooms.setObject(NSNumber(value: newValue), forKey: self)
            if booted { apply(runtime.setRootFontZoom(newValue)) }
        }
    }
}

final class ZoomMenuTarget: NSObject, NSMenuItemValidation {
    /// The main window's session, as AppKit sends a nil-targeted action to
    /// the key window and then the main one: the session whose window became
    /// key last (`DevMenu.select`), while that window is key or main.
    private var session: ExactSession? {
        guard let session = DevMenu.session, let window = session.view?.window, window.isKeyWindow || window.isMainWindow else { return nil }
        return session
    }
    @objc func actualSize(_ sender: Any?) { session?.rootZoom = 1 }
    @objc func zoomIn(_ sender: Any?) {
        guard let session, let zoom = PageZoom.next(after: session.rootZoom) else { return }
        session.rootZoom = zoom
    }
    @objc func zoomOut(_ sender: Any?) {
        guard let session, let zoom = PageZoom.previous(before: session.rootZoom) else { return }
        session.rootZoom = zoom
    }
    /// Safari's: Actual Size while zoomed, and each step while there is one.
    func validateMenuItem(_ item: NSMenuItem) -> Bool {
        guard let zoom = session?.rootZoom else { return false }
        switch item.action {
        case #selector(actualSize(_:)): return zoom != 1
        case #selector(zoomIn(_:)): return PageZoom.next(after: zoom) != nil
        case #selector(zoomOut(_:)): return PageZoom.previous(before: zoom) != nil
        default: return false
        }
    }
}
#endif
