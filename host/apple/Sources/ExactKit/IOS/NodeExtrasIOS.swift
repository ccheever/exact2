// The fields a node's view rarely sets, apart from the view (LLP 1010 §6:
// a list's memory is its rows'). Every accessor reads and writes as the
// stored property it replaces did.
#if os(iOS) || os(tvOS)
import UIKit

/// What most nodes never set — gestures and their holds, editors and
/// embedded platform views, a canvas's overlay, symbols, focus rings, a
/// scroller's bookkeeping — held apart and made on the first write that is
/// not a default (`NodeView.more`). A list's rows set none of it, so each
/// row's view is about 500 bytes smaller.
/// A fixed gradient as last aimed: its layer, style, appearance and boxes,
/// and the gradient and stops made from them.
struct AimedGradient {
    weak var layer: CAGradientLayer?
    let source: BatchValue, dark: Bool, bounds: CGRect, box: CGRect
    let gradient: Gradient, stops: ([CGFloat], [CGColor])
}

final class NodeExtras {
    var inlinePressed: UInt32?
    /// A Markdown link pressed in this text: its target, followed on release over it.
    var linkPressed: String?
    var svgPressed: UInt32?
    var clipPath: CGPath?
    var clipRule: CGPathFillRule = .winding
    /// A `background-image` gradient Core Animation paints (LLP 1066).
    var boxGradient: CAGradientLayer?
    /// What a fixed gradient was last aimed with (`reaimFixedGradient`).
    var aimedGradient: AimedGradient?
    /// `box-shadow` and the clip it casts outside (`BoxShadow.swift`).
    var shadowCaster: ShadowCaster?
    /// Inset `box-shadow`s (LLP 1077 D4).
    var insetCaster: InsetShadowCaster?
    var clipBox: PlainView?
    /// The box layout moved it from (LLP 1063).
    var layoutOffset: CGPoint = .zero
    /// How far its scroller's scroll moves a sticky box (LLP 1083, `Sticky.swift`).
    var stickyOffset: CGPoint = .zero
    var keyboardLift: CGFloat = 0
    var layoutScale = CGPoint(x: 1, y: 1)
    /// Its surface at a layout transition's size (`Surface.swift`).
    var surface: SurfaceLayer?
    var layoutPanRecognizer: UIPanGestureRecognizer?
    var layoutPanOrigin: CGPoint = .zero
    var swipeRecognizer: UIPanGestureRecognizer?
    var swipeArmed: Bool = false
    /// Where a swipe's first finger landed, in the window (its shouldBegin's edge rule).
    var swipeDownX: CGFloat?
    var swipeHold: SwipeHold?
    var heightRecognizer: UIPanGestureRecognizer?
    var heightHold: HeightDragHold?
    var reorderPan: UIPanGestureRecognizer?
    var reorderPress: UILongPressGestureRecognizer?
    var reorderHold: ReorderHold?
    var reorderOrigin: CGPoint = .zero
    var transformRecognizer: UIPanGestureRecognizer?
    var transformHold: TransformDragHold?
    var transformContact: TransformContact?
    var swipeOrigin: Double = 0
    var contextRecognizer: UILongPressGestureRecognizer?
    var doubleRecognizer: UITapGestureRecognizer?
    #if !os(tvOS)
    var hoverRecognizer: UIHoverGestureRecognizer?
    #endif
    var textArea: UITextView?
    var field: UITextField?
    var pendingValue: String?
    var video: VideoView?
    var web: UIView?
    var metal: MetalView?
    var canvasInput: CanvasInput?
    var overlay: PlainView?
    var needsCapture: Bool = false
    var paintedThisTurn: Bool = false
    var placement: [Double]?
    var symbolView: UIImageView?
    var symbolFound = false
    var symbolKey: String?
    var symbolRefusal: String?
    var focusRing: CAShapeLayer?
    var scrollEventQueued: Bool = false
    var lastScrollEvent: CGPoint = .zero
    var dispatchingScrollEvent: Bool = false
    var beforeLayoutScroll: CGPoint?
    var hiddenScroll: CGPoint?
    var followedScroll: (top: CGFloat, end: Bool)?
    var followsEndAfterInteraction = false
    var followingEndAnimated = false
    var anchoredScrollTop: CGFloat?
    var retainedScrollTop: CGFloat?
    var materialKind: String?
    var materialInteractive: Bool = false
    var hiddenBeforePlacement = false
    var placementHidden = false
    var materialView: UIVisualEffectView?
    /// `glassGroup`'s view and a grouped glass's slot (`GlassGroup.swift`).
    var glassGroupView: GlassGroupView?
    var glassSlot: GlassSlot?
    var pendingScrollLeft: Double?
    var pendingScrollTop: Double?
    /// A collapsing title's scroller (LLP 1075.003 Stage 3): its expanded
    /// title's inset, where `scrollTop` 0 rests; 0 for every other scroller.
    var scrollOrigin: CGFloat = 0
    /// …and the smallest inset it has had: its title collapsed.
    var scrollCollapsed: CGFloat = 0
    /// What the style last said of the scroll view's indicators and deceleration.
    var scrollWritten: String?
    /// A hatched node whose hatch undoes its own additions: its row may be
    /// reused (LLP 1075.003.000.000 §8).
    var hatchReusable = false
    var readingAnchors: [(node: NodeView, y: CGFloat)] = []
    weak var activeReadingAnchor: NodeView?
    /// A plain scroller's anchor across a batch (`ScrollAnchoring.swift`).
    var scrollAnchor: (node: NodeView, y: CGFloat)?
    #if !os(tvOS)
    lazy var swipeFeedback = UISelectionFeedbackGenerator()
    #endif
}

extension NodeView {
    var more: NodeExtras {
        if let extras { return extras }
        let made = NodeExtras(); extras = made
        return made
    }
    var inlinePressed: UInt32? { get { extras?.inlinePressed } set { if newValue != nil || extras != nil { more.inlinePressed = newValue } } }
    var linkPressed: String? { get { extras?.linkPressed } set { if newValue != nil || extras != nil { more.linkPressed = newValue } } }
    var svgPressed: UInt32? { get { extras?.svgPressed } set { if newValue != nil || extras != nil { more.svgPressed = newValue } } }
    var clipPath: CGPath? { get { extras?.clipPath } set { if newValue != nil || extras != nil { more.clipPath = newValue } } }
    var clipRule: CGPathFillRule { get { extras?.clipRule ?? .winding } set { if newValue != .winding || extras != nil { more.clipRule = newValue } } }
    var boxGradient: CAGradientLayer? { get { extras?.boxGradient } set { if newValue != nil || extras != nil { more.boxGradient = newValue } } }
    var aimedGradient: AimedGradient? { get { extras?.aimedGradient } set { if newValue != nil || extras != nil { more.aimedGradient = newValue } } }
    var shadowCaster: ShadowCaster? { get { extras?.shadowCaster } set { if newValue != nil || extras != nil { more.shadowCaster = newValue } } }
    var insetCaster: InsetShadowCaster? { get { extras?.insetCaster } set { if newValue != nil || extras != nil { more.insetCaster = newValue } } }
    var clipBox: PlainView? { get { extras?.clipBox } set { if newValue != nil || extras != nil { more.clipBox = newValue } } }
    var layoutOffset: CGPoint { get { extras?.layoutOffset ?? .zero } set { if newValue != .zero || extras != nil { more.layoutOffset = newValue } } }
    var stickyOffset: CGPoint { get { extras?.stickyOffset ?? .zero } set { if newValue != .zero || extras != nil { more.stickyOffset = newValue } } }
    /// How far a keyboard toolbar rides up with the keyboard (KeyboardToolbarIOS).
    var keyboardLift: CGFloat { get { extras?.keyboardLift ?? 0 } set { if newValue != 0 || extras != nil { more.keyboardLift = newValue } } }
    var layoutScale: CGPoint { get { extras?.layoutScale ?? CGPoint(x: 1, y: 1) } set { if newValue != CGPoint(x: 1, y: 1) || extras != nil { more.layoutScale = newValue } } }
    var surface: SurfaceLayer? { get { extras?.surface } set { if newValue != nil || extras != nil { more.surface = newValue } } }
    var layoutPanRecognizer: UIPanGestureRecognizer? { get { extras?.layoutPanRecognizer } set { if newValue != nil || extras != nil { more.layoutPanRecognizer = newValue } } }
    var layoutPanOrigin: CGPoint { get { extras?.layoutPanOrigin ?? .zero } set { if newValue != .zero || extras != nil { more.layoutPanOrigin = newValue } } }
    var swipeRecognizer: UIPanGestureRecognizer? { get { extras?.swipeRecognizer } set { if newValue != nil || extras != nil { more.swipeRecognizer = newValue } } }
    var swipeDownX: CGFloat? { get { extras?.swipeDownX } set { if newValue != nil || extras != nil { more.swipeDownX = newValue } } }
    var swipeArmed: Bool { get { extras?.swipeArmed ?? false } set { if newValue || extras != nil { more.swipeArmed = newValue } } }
    var swipeHold: SwipeHold? { get { extras?.swipeHold } set { if newValue != nil || extras != nil { more.swipeHold = newValue } } }
    var heightRecognizer: UIPanGestureRecognizer? { get { extras?.heightRecognizer } set { if newValue != nil || extras != nil { more.heightRecognizer = newValue } } }
    var heightHold: HeightDragHold? { get { extras?.heightHold } set { if newValue != nil || extras != nil { more.heightHold = newValue } } }
    var reorderPan: UIPanGestureRecognizer? { get { extras?.reorderPan } set { if newValue != nil || extras != nil { more.reorderPan = newValue } } }
    var reorderPress: UILongPressGestureRecognizer? { get { extras?.reorderPress } set { if newValue != nil || extras != nil { more.reorderPress = newValue } } }
    var reorderHold: ReorderHold? { get { extras?.reorderHold } set { if newValue != nil || extras != nil { more.reorderHold = newValue } } }
    var reorderOrigin: CGPoint { get { extras?.reorderOrigin ?? .zero } set { if newValue != .zero || extras != nil { more.reorderOrigin = newValue } } }
    var transformRecognizer: UIPanGestureRecognizer? { get { extras?.transformRecognizer } set { if newValue != nil || extras != nil { more.transformRecognizer = newValue } } }
    var transformHold: TransformDragHold? { get { extras?.transformHold } set { if newValue != nil || extras != nil { more.transformHold = newValue } } }
    var transformContact: TransformContact? { get { extras?.transformContact } set { if newValue != nil || extras != nil { more.transformContact = newValue } } }
    var swipeOrigin: Double { get { extras?.swipeOrigin ?? 0 } set { if newValue != 0 || extras != nil { more.swipeOrigin = newValue } } }
    var contextRecognizer: UILongPressGestureRecognizer? { get { extras?.contextRecognizer } set { if newValue != nil || extras != nil { more.contextRecognizer = newValue } } }
    var doubleRecognizer: UITapGestureRecognizer? { get { extras?.doubleRecognizer } set { if newValue != nil || extras != nil { more.doubleRecognizer = newValue } } }
    #if !os(tvOS)
    var hoverRecognizer: UIHoverGestureRecognizer? { get { extras?.hoverRecognizer } set { if newValue != nil || extras != nil { more.hoverRecognizer = newValue } } }
    #endif
    package var textArea: UITextView? { get { extras?.textArea } set { if newValue != nil || extras != nil { more.textArea = newValue } } }
    var field: UITextField? { get { extras?.field } set { if newValue != nil || extras != nil { more.field = newValue } } }
    var pendingValue: String? { get { extras?.pendingValue } set { if newValue != nil || extras != nil { more.pendingValue = newValue } } }
    var video: VideoView? { get { extras?.video } set { if newValue != nil || extras != nil { more.video = newValue } } }
    var web: UIView? { get { extras?.web } set { if newValue != nil || extras != nil { more.web = newValue } } }
    package var metal: MetalView? { get { extras?.metal } set { if newValue != nil || extras != nil { more.metal = newValue } } }
    package var canvasInput: CanvasInput? { get { extras?.canvasInput } set { if newValue != nil || extras != nil { more.canvasInput = newValue } } }
    package var overlay: PlainView? { get { extras?.overlay } set { if newValue != nil || extras != nil { more.overlay = newValue } } }
    package var needsCapture: Bool { get { extras?.needsCapture ?? false } set { if newValue || extras != nil { more.needsCapture = newValue } } }
    package var paintedThisTurn: Bool { get { extras?.paintedThisTurn ?? false } set { if newValue || extras != nil { more.paintedThisTurn = newValue } } }
    package var placement: [Double]? { get { extras?.placement } set { if newValue != nil || extras != nil { more.placement = newValue } } }
    var symbolView: UIImageView? { get { extras?.symbolView } set { if newValue != nil || extras != nil { more.symbolView = newValue } } }
    var symbolFound: Bool { get { extras?.symbolFound ?? false } set { if newValue || extras != nil { more.symbolFound = newValue } } }
    var symbolKey: String? { get { extras?.symbolKey } set { if newValue != nil || extras != nil { more.symbolKey = newValue } } }
    var symbolRefusal: String? { get { extras?.symbolRefusal } set { if newValue != nil || extras != nil { more.symbolRefusal = newValue } } }
    var focusRing: CAShapeLayer? { get { extras?.focusRing } set { if newValue != nil || extras != nil { more.focusRing = newValue } } }
    var scrollEventQueued: Bool { get { extras?.scrollEventQueued ?? false } set { if newValue || extras != nil { more.scrollEventQueued = newValue } } }
    var lastScrollEvent: CGPoint { get { extras?.lastScrollEvent ?? .zero } set { if newValue != .zero || extras != nil { more.lastScrollEvent = newValue } } }
    var dispatchingScrollEvent: Bool { get { extras?.dispatchingScrollEvent ?? false } set { if newValue || extras != nil { more.dispatchingScrollEvent = newValue } } }
    var beforeLayoutScroll: CGPoint? { get { extras?.beforeLayoutScroll } set { if newValue != nil || extras != nil { more.beforeLayoutScroll = newValue } } }
    var hiddenScroll: CGPoint? { get { extras?.hiddenScroll } set { if newValue != nil || extras != nil { more.hiddenScroll = newValue } } }
    var followedScroll: (top: CGFloat, end: Bool)? { get { extras?.followedScroll } set { if newValue != nil || extras != nil { more.followedScroll = newValue } } }
    var followingEndAnimated: Bool { get { extras?.followingEndAnimated ?? false } set { if newValue || extras != nil { more.followingEndAnimated = newValue } } }
    var followsEndAfterInteraction: Bool { get { extras?.followsEndAfterInteraction ?? false } set { if newValue || extras != nil { more.followsEndAfterInteraction = newValue } } }
    var anchoredScrollTop: CGFloat? { get { extras?.anchoredScrollTop } set { if newValue != nil || extras != nil { more.anchoredScrollTop = newValue } } }
    var retainedScrollTop: CGFloat? { get { extras?.retainedScrollTop } set { if newValue != nil || extras != nil { more.retainedScrollTop = newValue } } }
    var materialKind: String? { get { extras?.materialKind } set { if newValue != nil || extras != nil { more.materialKind = newValue } } }
    var glassGroupView: GlassGroupView? { get { extras?.glassGroupView } set { if newValue != nil || extras != nil { more.glassGroupView = newValue } } }
    var glassSlot: GlassSlot? { get { extras?.glassSlot } set { if newValue != nil || extras != nil { more.glassSlot = newValue } } }
    var materialInteractive: Bool { get { extras?.materialInteractive ?? false } set { if newValue || extras != nil { more.materialInteractive = newValue } } }
    var readingAnchors: [(node: NodeView, y: CGFloat)] {
        get { extras?.readingAnchors ?? [] }
        set { if !newValue.isEmpty || extras != nil { more.readingAnchors = newValue } }
    }
    var activeReadingAnchor: NodeView? {
        get { extras?.activeReadingAnchor }
        set { if newValue != nil || extras != nil { more.activeReadingAnchor = newValue } }
    }
    var scrollAnchor: (node: NodeView, y: CGFloat)? {
        get { extras?.scrollAnchor }
        set { if newValue != nil || extras != nil { more.scrollAnchor = newValue } }
    }
    #if !os(tvOS)
    var swipeFeedback: UISelectionFeedbackGenerator { more.swipeFeedback }
    #endif
}
extension NodeView {
    var scrollOrigin: CGFloat { get { extras?.scrollOrigin ?? 0 } set { if newValue != 0 || extras != nil { more.scrollOrigin = newValue } } }
    var scrollCollapsed: CGFloat { get { extras?.scrollCollapsed ?? 0 } set { if newValue != 0 || extras != nil { more.scrollCollapsed = newValue } } }
    var scrollWritten: String? { get { extras?.scrollWritten } set { if newValue != nil || extras != nil { more.scrollWritten = newValue } } }
    var hatchReusable: Bool { get { extras?.hatchReusable ?? false } set { if newValue || extras != nil { more.hatchReusable = newValue } } }
    /// Where CSS's `scrollTop` 0 is in UIKit's offsets: past the scroller's
    /// top inset when a collapsing title's bar insets it — the scrollport's
    /// top is the bar's bottom, whatever its height (UIKit keeps the offset
    /// plus that inset fixed while the title collapses) — else 0.
    func scrollTopInset(_ sv: UIScrollView) -> CGFloat { scrollOrigin > 0 ? sv.adjustedContentInset.top : 0 }
}
extension NodeView {
    /// Whether a scroll animation ended at the running animation's target,
    /// clamped to the content as it is now (`CollectionHost.animationEnded`);
    /// nil with no target.
    func endedAtTarget(_ scrollView: UIScrollView) -> Bool? {
        presenter?.collections.animationTargets[id].map { t -> Bool in
            let o = scrollView.contentOffset, i = scrollView.adjustedContentInset
            let x = min(max(t.x, -i.left), max(-i.left, scrollView.contentSize.width + i.right - scrollView.bounds.width))
            let y = min(max(t.y, -i.top), max(-i.top, scrollView.contentSize.height + i.bottom - scrollView.bounds.height))
            return abs(o.x - x) + abs(o.y - y) <= 1
        }
    }
}
#endif
