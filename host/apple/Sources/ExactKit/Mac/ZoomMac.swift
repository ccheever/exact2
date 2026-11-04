// View ▸ Actual Size ⌘0, Zoom In ⌘+ and Zoom Out ⌘- (LLP 1021 §5.3.1), as
// Safari, Preview and Electron's default menu have them, and the app's
// `setPageZoom(factor)`, as Electron's `webContents.setZoomFactor`. A page
// zoom, as Chromium's: the page's scroll view magnifies its document
// (`NSScrollView.magnification`), so a CSS px is `factor` points, the
// viewport the page lays out in is the view's size over the factor, and
// AppKit backs every view of the page, text and native controls with it,
// at the window's backing scale times the factor (`pixelScale`).
#if os(macOS)
import AppKit

/// The zoom levels Safari offers (Settings ▸ Websites ▸ Page Zoom), which
/// Zoom In and Zoom Out step through.
enum PageZoom {
    static let levels: [Double] = [0.5, 0.75, 0.85, 1, 1.15, 1.25, 1.5, 1.75, 2, 2.5, 3]
    static func next(after zoom: Double) -> Double? { levels.first { $0 > zoom + 1e-9 } }
    static func previous(before zoom: Double) -> Double? { levels.last { $0 < zoom - 1e-9 } }
    /// Chrome's page zoom limits, which `setPageZoom` is clamped to (Chrome
    /// 156 clamps a profile's zoom of 8 to 5 and of 0.1 to 0.25).
    static let range: ClosedRange<Double> = 0.25...5
}

extension NSView {
    /// Device pixels per unit of this view's own coordinates: the window's
    /// backing scale times every magnification above the view (the page
    /// zoom), which is what AppKit backs the view's own layer at. Nil
    /// outside a window.
    package var pixelScale: CGFloat? {
        guard window != nil else { return nil }
        let unit = convertToBacking(NSSize(width: 1, height: 1))
        return max(abs(unit.width), abs(unit.height))
    }
}

extension ExactSession {
    /// The page zoom: points per CSS px of the session's view, 1 at Actual
    /// Size. It is the viewport's, so it outlives a reload, not the session.
    var pageZoom: Double {
        get { Double(presenter.viewport.magnification) }
        set {
            let viewport = presenter.viewport
            let factor = min(max(newValue, PageZoom.range.lowerBound), PageZoom.range.upperBound)
            guard CGFloat(factor) != viewport.magnification else { return }
            viewport.minMagnification = CGFloat(PageZoom.range.lowerBound)
            viewport.maxMagnification = CGFloat(PageZoom.range.upperBound)
            // The page's top left point in view stays where it is, as a browser keeps its scroll anchor.
            viewport.setMagnification(CGFloat(factor), centeredAt: viewport.contentView.bounds.origin)
            view?.zoomChanged()
        }
    }

    /// The app's `setPageZoom(factor)`: a factor that is not finite and
    /// positive is refused in the journal; any other is clamped to
    /// Chromium's limits.
    func setPageZoom(_ args: [Any]) {
        guard let factor = (args.first as? NSNumber)?.doubleValue, args.count == 1, factor.isFinite, factor > 0 else {
            log("setPageZoom refused: the page zoom is a number above 0 (1 is Actual Size)")
            return
        }
        pageZoom = factor
    }
}

extension Presenter {
    /// A view over the page in its scroll view, not in its document (a
    /// dialog's backdrop, a popover's layer): its own bounds carry the page
    /// zoom, so what it holds is laid out in CSS px, as the page is.
    func zoomOverPage(_ view: NSView) {
        guard view.superview === viewport else { return }
        let zoom = viewport.magnification
        let size = NSSize(width: view.frame.width / zoom, height: view.frame.height / zoom)
        if view.bounds.size != size { view.setBoundsSize(size) }
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
    @objc func actualSize(_ sender: Any?) { session?.pageZoom = 1 }
    @objc func zoomIn(_ sender: Any?) {
        guard let session, let zoom = PageZoom.next(after: session.pageZoom) else { return }
        session.pageZoom = zoom
    }
    @objc func zoomOut(_ sender: Any?) {
        guard let session, let zoom = PageZoom.previous(before: session.pageZoom) else { return }
        session.pageZoom = zoom
    }
    /// Safari's: Actual Size while zoomed, and each step while there is one.
    func validateMenuItem(_ item: NSMenuItem) -> Bool {
        guard let zoom = session?.pageZoom else { return false }
        switch item.action {
        case #selector(actualSize(_:)): return zoom != 1
        case #selector(zoomIn(_:)): return PageZoom.next(after: zoom) != nil
        case #selector(zoomOut(_:)): return PageZoom.previous(before: zoom) != nil
        default: return false
        }
    }
}
#endif
