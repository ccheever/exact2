// @ref LLP 1047.001 D4 — GPU surfaces and Canvas 2D (the Surfaces
// capability) are a linked capability: their implementation is the
// `ExactSurfaces` module, which an app's composition links only when its
// plan has a surface or a 2D canvas, and installs here. The core names no
// type of theirs: it speaks to them through these protocols, and without the
// module it holds the do-nothing defaults below — a plan that uses Surfaces
// is refused at boot (`Unlinked("surfaces")`), so they are never asked for
// a canvas.
#if canImport(UIKit)
import UIKit
#else
import AppKit
#endif
import CExact

/// What the core asks of the capability.
package protocol SurfacesCapability: AnyObject {
    /// A session's GPU surfaces.
    func canvases() -> Canvases
    /// A presenter's 2D canvases.
    func canvas2D() -> Canvas2DCanvases
    /// A `canvas` node's GPU view.
    func makeMetalView() -> MetalView
    /// Canvas 2D's text measure, which the runner calls back.
    var canvasTextMeasure: ExactCanvasTextFn { get }
    /// Whether a surface wants the pointer locked.
    var pointerLockPreferred: Bool { get }
    /// Whether a GPU module is loaded, so a new generation's shaders matter.
    var shadersLoaded: Bool { get }
    /// Whether the loaded module accepts a generation's shaders.
    func acceptsShaders(_ sources: [String: Data]) -> Bool
    /// Hand a committed generation's shaders to the loaded module.
    func replaceShaders(_ sources: [String: Data])
}

/// The agent's clock over every world (LLP 1046.001).
package struct WorldClock {
    package var pending = false
    package var settleAt: Double?
    package var reply: [String: Any] = [:]
    package init(pending: Bool = false, settleAt: Double? = nil, reply: [String: Any] = [:]) {
        self.pending = pending
        self.settleAt = settleAt
        self.reply = reply
    }
}

/// A session's GPU surfaces, as the core sees them.
package protocol Canvases: AnyObject {
    var session: ExactSession? { get set }
    var ready: Bool { get }
    var failed: String? { get }
    var isEmpty: Bool { get }
    var status: String { get }
    var wantsFrames: Bool { get }
    var frameNow: Double? { get set }
    func reset()
    func surface(view: NodeView, name: String, values: Any)
    func destroy(view: UInt32)
    func loadIfNeeded()
    func surfaceWork(_ op: [String: Any], generation: Int)
    func post(_ name: String, _ text: String)
    func period(_ ms: Double)
    func tick(now: Double) -> Bool
    func settle(now: Double)
    func cancelMovedControls()
    func captureIfNeeded()
    func scheduleCapture()
    func wantsInput(_ id: UInt32) -> Bool
    func input(_ view: NodeView, _ event: [String: Any], timestamp: Double?) -> Bool
    func pressedControlKey(_ code: String, down: Bool, canvas: UInt32?, timestamp: Double?) -> Bool
    func ownsControl(_ node: UInt32) -> Bool
    func cancelControls(of node: UInt32)
    func control(_ node: NodeView, _ phase: String, id contact: Int, point: CGPoint, timestamp: Double?) -> Bool
    func refreshLifecycle()
    func lifecycleFrame()
    var lifecycleNeedsRetry: Bool { get }
    // The agent's seams (LLP 1046.001).
    func waitUntilReady() -> Bool
    func decorate(_ request: [String: Any], _ reply: [String: Any]) -> [String: Any]
    func worlds(_ request: [String: Any]) -> [[String: Any]]
    func restoreReply(_ reply: [String: Any]) -> [String: Any]
    func releaseContact(_ request: [String: Any]) -> [String: Any]?
    func clock(settle: Bool) -> WorldClock
    func world(_ agent: Agent, _ request: [String: Any]) -> [String: Any]
    func type(_ agent: Agent, _ view: NodeView, _ request: [String: Any]) -> [String: Any]
    #if os(macOS)
    func occlusionChanged()
    func readback(view: NodeView) -> NSBitmapImageRep?
    #else
    func readback(view: NodeView) -> UIImage?
    func picture(of view: NodeView) -> CGImage?
    #endif
}

extension Canvases {
    package func input(_ view: NodeView, _ event: [String: Any]) -> Bool { input(view, event, timestamp: nil) }
    func pressedControlKey(_ code: String, down: Bool, timestamp: Double?) -> Bool {
        pressedControlKey(code, down: down, canvas: nil, timestamp: timestamp)
    }
    func pressedControlKey(_ code: String, down: Bool) -> Bool {
        pressedControlKey(code, down: down, canvas: nil, timestamp: nil)
    }
}

/// A presenter's 2D canvases, as the core sees them.
package protocol Canvas2DCanvases: AnyObject {
    var textEngine: (() -> TextEngine?)? { get set }
    var assetBytes: ((String) -> Data?)? { get set }
    var onImage: ((String, CGImage?) -> Void)? { get set }
    var onHeld: ((UInt32, Bool) -> Void)? { get set }
    var onScale: ((CGFloat) -> Void)? { get set }
    var loadingCount: Int { get }
    func apply(_ id: UInt32, _ payload: [String: Any], layer parent: CALayer?)
    func forget(_ id: UInt32)
    func load(_ srcs: [String])
    func waitForReplays(timeout: TimeInterval)
}

extension Canvas2DCanvases {
    func waitForReplays() { waitForReplays(timeout: 1) }
}

/// A GPU surface's view (a node's `metal`).
#if os(macOS)
package protocol MetalView: NSView {}
#else
package protocol MetalView: UIView {
    var metalLayer: CAMetalLayer { get }
}
#endif

/// A GPU surface's input: the pointer, keys and touches it takes.
package protocol CanvasInput: AnyObject {
    func blur()
    #if os(macOS)
    func pointer(_ event: NSEvent, phase: String) -> Bool
    func key(_ event: NSEvent, down: Bool, source: NodeView) -> Bool
    func flags(_ event: NSEvent) -> Bool
    func wheel(_ event: NSEvent) -> Bool
    #else
    func touches(_ values: Set<UITouch>, phase: String, source: NodeView, event: UIEvent?) -> Bool
    func presses(_ presses: Set<UIPress>, down: Bool, source: NodeView) -> Bool
    #endif
}

/// The Surfaces module, installed by the composition.
package enum SurfacesLink {
    package static var installed: SurfacesCapability?
}

/// Whether a pointer lock is wanted (`ExactIOS` asks): a GPU surface's.
public enum ExactPointerLock {
    public static var preferred: Bool { SurfacesLink.installed?.pointerLockPreferred ?? false }
}

/// An app without the capability: no GPU surface.
final class NoCanvases: Canvases {
    weak var session: ExactSession?
    var ready: Bool { true }
    var failed: String? { nil }
    var isEmpty: Bool { true }
    var status: String { "not linked" }
    var wantsFrames: Bool { false }
    var frameNow: Double?
    func reset() {}
    func surface(view: NodeView, name: String, values: Any) {}
    func destroy(view: UInt32) {}
    func loadIfNeeded() {}
    func surfaceWork(_ op: [String: Any], generation: Int) {}
    func post(_ name: String, _ text: String) {}
    func period(_ ms: Double) {}
    func tick(now: Double) -> Bool { false }
    func settle(now: Double) {}
    func cancelMovedControls() {}
    func captureIfNeeded() {}
    func scheduleCapture() {}
    func wantsInput(_ id: UInt32) -> Bool { false }
    func input(_ view: NodeView, _ event: [String: Any], timestamp: Double?) -> Bool { false }
    func pressedControlKey(_ code: String, down: Bool, canvas: UInt32?, timestamp: Double?) -> Bool { false }
    func ownsControl(_ node: UInt32) -> Bool { false }
    func cancelControls(of node: UInt32) {}
    func control(_ node: NodeView, _ phase: String, id contact: Int, point: CGPoint, timestamp: Double?) -> Bool { false }
    func refreshLifecycle() {}
    func lifecycleFrame() {}
    var lifecycleNeedsRetry: Bool { false }
    func waitUntilReady() -> Bool { true }
    func decorate(_ request: [String: Any], _ reply: [String: Any]) -> [String: Any] { reply }
    func worlds(_ request: [String: Any]) -> [[String: Any]] { [] }
    func restoreReply(_ reply: [String: Any]) -> [String: Any] { reply }
    func releaseContact(_ request: [String: Any]) -> [String: Any]? { nil }
    func clock(settle: Bool) -> WorldClock { WorldClock() }
    func world(_ agent: Agent, _ request: [String: Any]) -> [String: Any] { ["error": "view \(request["id"] ?? "undefined") has no world"] }
    func type(_ agent: Agent, _ view: NodeView, _ request: [String: Any]) -> [String: Any] { ["error": "view \(view.id) has no world"] }
    #if os(macOS)
    func occlusionChanged() {}
    func readback(view: NodeView) -> NSBitmapImageRep? { nil }
    #else
    func readback(view: NodeView) -> UIImage? { nil }
    func picture(of view: NodeView) -> CGImage? { nil }
    #endif
}

/// An app without the capability: no 2D canvas.
final class NoCanvas2D: Canvas2DCanvases {
    var textEngine: (() -> TextEngine?)?
    var assetBytes: ((String) -> Data?)?
    var onImage: ((String, CGImage?) -> Void)?
    var onHeld: ((UInt32, Bool) -> Void)?
    var onScale: ((CGFloat) -> Void)?
    var loadingCount: Int { 0 }
    func apply(_ id: UInt32, _ payload: [String: Any], layer parent: CALayer?) {}
    func forget(_ id: UInt32) {}
    func load(_ srcs: [String]) {}
    func waitForReplays(timeout: TimeInterval) {}
}
