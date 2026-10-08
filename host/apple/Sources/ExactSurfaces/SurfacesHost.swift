// The Surfaces capability (LLP 1047.001 D4): GPU surfaces and Canvas 2D. An
// app's composition links this module only when its plan has a surface or a
// 2D canvas, and installs it before any session is made.
#if canImport(UIKit)
import UIKit
#else
import AppKit
#endif
import CExact
import ExactKit

public enum ExactSurfaces {
    public static func install() { SurfacesLink.installed = SurfacesHost() }
}

final class SurfacesHost: SurfacesCapability {
    func canvases() -> Canvases { CanvasesHost() }
    func canvas2D() -> Canvas2DCanvases { Canvas2DHost() }
    func makeMetalView() -> MetalView { MetalCanvasView(frame: .zero) }
    var canvasTextMeasure: ExactCanvasTextFn { CanvasText.measureRun }
    var pointerLockPreferred: Bool {
        #if os(iOS) || os(tvOS)
        CanvasInputHost.lockOwner != nil
        #else
        false
        #endif
    }
    var shadersLoaded: Bool { GpuModule.loaded != nil }
    func acceptsShaders(_ sources: [String: Data]) -> Bool { GpuModule.loaded?.accepts(sources) == true }
    func replaceShaders(_ sources: [String: Data]) { GpuModule.loaded?.replaceShaders(sources) }
}

extension CanvasesHost: Canvases {
    var isEmpty: Bool { entries.isEmpty }
    func refreshLifecycle() { lifecycle.refresh() }
    func lifecycleFrame() { lifecycle.frame() }
    var lifecycleNeedsRetry: Bool { lifecycle.needsRetry }
}

extension Canvas2DHost: Canvas2DCanvases {}
extension MetalCanvasView: MetalView {}
extension CanvasInputHost: CanvasInput {}

extension TextEngine {
    /// Canvas 2D's fonts and lines over this engine (LLP 1056 D8), made at
    /// first use and dropped with the engine's fonts.
    var canvasText: CanvasText {
        if let made = canvasTextCache as? CanvasText { return made }
        let made = CanvasText(engine: self)
        canvasTextCache = made
        return made
    }
}

extension NodeView {
    /// This module's view under a `canvas` node (the core holds it as a `MetalView`).
    var canvasMetal: MetalCanvasView? { metal as? MetalCanvasView }
}
