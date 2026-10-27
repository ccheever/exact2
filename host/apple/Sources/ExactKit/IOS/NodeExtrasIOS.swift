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

/// A paragraph's specs and raster, apart from its view: three `Spec`s
/// inline made every `NodeView` about 1 KB larger, text or not (2112
/// bytes, a 2.5 KB block); the raster's fields fit in the same block.
final class TextSpecs {
    var rasterKey: TextRasterKey?
    var rasterSpec: Spec?
    var cached: Spec? { didSet { headroom = nil } }
    var headroom: Float?
    var raster: CGImage?
    var rasterLayer: CALayer?
    var rasterFrame = CGRect.zero
    var rasterScale: CGFloat = 1
    var flowShapes: [TextFlowShape] = []
    var layout: (width: CGFloat, paragraph: Paragraph)?
}

/// What some nodes set and most do not, apart from the view, so every view
/// is smaller: a transform's parts, an image, paint ghosts, a scroll's
/// extent. Made on the first write that is not a default (`NodeView.aside`).
final class NodeAside {
    var translatePx = CGPoint.zero, translatePercent = CGPoint.zero
    var scale: CGFloat = 1
    var rotate: CGFloat = 0
    var content = CGSize.zero
    var image: UIImage?
    var raster: NativeRasterLease?
    var imageLayer: CALayer?
    var imageSource: String?
    var loadGeneration = 0
    var buttonRasterView: UInt32?
    var paintGhosts = 0
    weak var ghostParent: NodeView?
}

/// A box's gradient and shadows, apart from `NodeExtras`: most nodes that
/// have one need none of its other fields, and an extras object is 1.5 KB
/// (euxy's launch: 43 of its 47 extras held only these).
final class NodePaintExtras {
    var boxFill: CALayer?
    /// A `background-image` gradient Core Animation paints (LLP 1066).
    var boxGradient: CAGradientLayer?
    /// What a fixed gradient was last aimed with (`reaimFixedGradient`).
    var aimedGradient: AimedGradient?
    /// `box-shadow` and the clip it casts outside (`BoxShadow.swift`).
    var shadowCaster: ShadowCaster?
    /// Inset `box-shadow`s (LLP 1077 D4).
    var insetCaster: InsetShadowCaster?
}

final class NodeExtras {
    var pressContact: PressContact?
    var inlinePressed: UInt32?
    /// A Markdown link pressed in this text: its target, followed on release over it.
    var linkPressed: String?
    var svgPressed: UInt32?
    var clipPath: CGPath?
    var clipRule: CGPathFillRule = .winding
    var clipBox: PlainView?
    /// The box layout moved it from (LLP 1063).
    var layoutOffset: CGPoint = .zero
    /// How far its scroller's scroll moves a sticky box (LLP 1083, `Sticky.swift`).
    var stickyOffset: CGPoint = .zero
    var keyboardLift: CGFloat = 0
    /// How far its surface's size stands from the laid-out size (LLP 1063).
    var layoutResize: CGSize = .zero
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
    var heightHold: DragInput?
    var contextTransform = CGAffineTransform.identity
    var flightLook: FlightLook?
    var columnRecord: ColumnRecord?
    var liveText: String?
    var reorderPan: UIPanGestureRecognizer?
    var reorderPress: UILongPressGestureRecognizer?
    var reorderHold: AnyObject?
    var reorderOrigin: CGPoint = .zero
    var transformRecognizer: UIPanGestureRecognizer?
    var transformHold: DragInput?
    var transformContact: TransformContact?
    var swipeOrigin: Double = 0
    var contextRecognizer: UILongPressGestureRecognizer?
    var doubleRecognizer: UITapGestureRecognizer?
    #if !os(tvOS)
    var hoverRecognizer: UIHoverGestureRecognizer?
    var lookHoverRecognizer: UIHoverGestureRecognizer?
    #endif
    var textArea: UITextView?
    var field: UITextField?
    var fieldWaits = false
    weak var headerField: UITextField?
    #if os(iOS)
    /// A number input's stepper (`NumberStepperIOS.swift`).
    var stepper: FieldStepper?
    #endif
    var toldText: String?
    var pendingValue: String?
    /// A native field in a grouped list's row (`isCellField`).
    var fieldInCell = false
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
    var listSymbol = false
    var listRow = false
    var scrollCornered = false
    var focusRing: CAShapeLayer?
    var scrollEventQueued: Bool = false
    var lastScrollEvent: (offset: CGPoint, topInset: CGFloat) = (.zero, 0)
    var dispatchingScrollEvent: Bool = false
    var scrollEnd: ScrollEnd?
    var beforeLayoutScroll: (offset: CGPoint, topInset: CGFloat)?
    /// Logical coordinates survive hiding and physical backend replacement.
    var hiddenScroll: CGPoint?
    var followedScroll: (top: CGFloat, end: Bool, start: Bool)?
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
    var pendingScrollTransfer = false
    /// A collapsing title's scroller (LLP 1075.003 Stage 3): its expanded
    /// title's inset, where `scrollTop` 0 rests; 0 for every other scroller.
    var controlRide: CGFloat = 0
    var scrollOrigin: CGFloat = 0
    /// …and the smallest inset it has had: its title collapsed.
    var scrollCollapsed: CGFloat = 0
    /// What the style last said of the scroll view's indicators and deceleration.
    weak var nativeScrollView: UIScrollView?
    weak var nativeScrollDelegate: (any UIScrollViewDelegate)?
    /// Content room implemented as native insets, not a smaller CSS viewport.
    var nativeScrollContentInsets = UIEdgeInsets.zero
    /// The status bar's and the home indicator's bands its scroll view
    /// reaches into past its box, inset by as much (LLP 1130 D3).
    var pageBand = UIEdgeInsets.zero
    /// The part of the band its scroll view's `contentInset` holds (LLP 1131 D2).
    var bandInset = UIEdgeInsets.zero
    /// A control bar over a scroller's content (LLP 1131 D3, D4): the scroller's paint rank, which it is placed
    /// above, and UIKit's edge effect.
    var bandOver: Int64?
    var edgeInteraction: AnyObject?
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
    var textMore: TextSpecs {
        if let specs { return specs }
        let made = TextSpecs(); specs = made
        return made
    }
    /// The words and style `textRaster` paints, which the paragraph's may
    /// have left behind since (`invalidateText` keeps the old pixels up).
    var textRasterSpec: Spec? { get { specs?.rasterSpec } set { if newValue != nil || specs != nil { textMore.rasterSpec = newValue } } }
    var cachedTextSpec: Spec? { get { specs?.cached } set { if newValue != nil || specs != nil { textMore.cached = newValue } } }
    var cachedTextHeadroom: Float? { get { specs?.headroom } set { if newValue != nil || specs != nil { textMore.headroom = newValue } } }
    var textRaster: CGImage? { get { specs?.raster } set { if newValue != nil || specs != nil { textMore.raster = newValue } } }
    var textRasterLayer: CALayer? { get { specs?.rasterLayer } set { if newValue != nil || specs != nil { textMore.rasterLayer = newValue } } }
    var textRasterFrame: CGRect { get { specs?.rasterFrame ?? .zero } set { if newValue != .zero || specs != nil { textMore.rasterFrame = newValue } } }
    var textRasterScale: CGFloat { get { specs?.rasterScale ?? 1 } set { if newValue != 1 || specs != nil { textMore.rasterScale = newValue } } }
    var flowShapes: [TextFlowShape] { get { specs?.flowShapes ?? [] } set { if !newValue.isEmpty || specs != nil { textMore.flowShapes = newValue } } }
    var cachedTextLayout: (width: CGFloat, paragraph: Paragraph)? { get { specs?.layout } set { if newValue != nil || specs != nil { textMore.layout = newValue } } }

    var asideMade: NodeAside {
        if let aside { return aside }
        let made = NodeAside(); aside = made
        return made
    }
    /// `translate`: its lengths, and its percentages of the box (chess diary #4).
    var translatePx: CGPoint { get { aside?.translatePx ?? .zero } set { if newValue != .zero || aside != nil { asideMade.translatePx = newValue } } }
    var translatePercent: CGPoint { get { aside?.translatePercent ?? .zero } set { if newValue != .zero || aside != nil { asideMade.translatePercent = newValue } } }
    package var scale: CGFloat { get { aside?.scale ?? 1 } set { if newValue != 1 || aside != nil { asideMade.scale = newValue } } }
    package var rotate: CGFloat { get { aside?.rotate ?? 0 } set { if newValue != 0 || aside != nil { asideMade.rotate = newValue } } }
    /// A scroll container's content extent (the `content` op), before the
    /// axes that do not scroll are held to the box.
    var content: CGSize { get { aside?.content ?? .zero } set { if newValue != .zero || aside != nil { asideMade.content = newValue } } }
    /// An image node's picture, once loaded (decoded off the main thread),
    /// the source it came from, and which load is current: a completion
    /// from an older load, or for a view that was destroyed, is dropped.
    var buttonRasterView: UInt32? { get { aside?.buttonRasterView } set { if newValue != nil || aside != nil { asideMade.buttonRasterView = newValue } } }
    var image: UIImage? { get { aside?.image } set { if newValue != nil || aside != nil { asideMade.image = newValue } } }
    var raster: NativeRasterLease? {
        get { aside?.raster }
        set {
            if newValue != nil || aside != nil { asideMade.raster = newValue }
            if newValue == nil, let l = imageLayer { l.removeFromSuperlayer(); imageLayer = nil }
            if isNativeButton { presenter?.controls.refreshRaster(self) }
        }
    }
    /// An image's pixels as a sublayer's contents (`applyImageLayer`).
    var imageLayer: CALayer? { get { aside?.imageLayer } set { if newValue != nil || aside != nil { asideMade.imageLayer = newValue } } }
    var imageSource: String? { get { aside?.imageSource } set { if newValue != nil || aside != nil { asideMade.imageSource = newValue } } }
    var loadGeneration: Int { get { aside?.loadGeneration ?? 0 } set { if newValue != 0 || aside != nil { asideMade.loadGeneration = newValue } } }
    var paintGhosts: Int { get { aside?.paintGhosts ?? 0 } set { if newValue != 0 || aside != nil { asideMade.paintGhosts = newValue } } }
    var ghostParent: NodeView? { get { aside?.ghostParent } set { if newValue != nil || aside != nil { asideMade.ghostParent = newValue } } }

    var more: NodeExtras {
        if let extras { return extras }
        let made = NodeExtras(); extras = made
        return made
    }
    var inlinePressed: UInt32? { get { extras?.inlinePressed } set { if newValue != nil || extras != nil { more.inlinePressed = newValue } } }
    var pressContact: PressContact? { get { extras?.pressContact } set { if newValue != nil || extras != nil { more.pressContact = newValue } } }
    var linkPressed: String? { get { extras?.linkPressed } set { if newValue != nil || extras != nil { more.linkPressed = newValue } } }
    var svgPressed: UInt32? { get { extras?.svgPressed } set { if newValue != nil || extras != nil { more.svgPressed = newValue } } }
    var clipPath: CGPath? { get { extras?.clipPath } set { if newValue != nil || extras != nil { more.clipPath = newValue } } }
    var clipRule: CGPathFillRule { get { extras?.clipRule ?? .winding } set { if newValue != .winding || extras != nil { more.clipRule = newValue } } }
    var fieldInCell: Bool { get { extras?.fieldInCell ?? false } set { if newValue || extras != nil { more.fieldInCell = newValue } } }
    var paintMore: NodePaintExtras {
        if let paintExtras { return paintExtras }
        let made = NodePaintExtras(); paintExtras = made
        return made
    }
    var boxFill: CALayer? { get { paintExtras?.boxFill } set { if newValue != nil || paintExtras != nil { paintMore.boxFill = newValue } } }
    var boxGradient: CAGradientLayer? { get { paintExtras?.boxGradient } set { if newValue != nil || paintExtras != nil { paintMore.boxGradient = newValue } } }
    var aimedGradient: AimedGradient? { get { paintExtras?.aimedGradient } set { if newValue != nil || paintExtras != nil { paintMore.aimedGradient = newValue } } }
    var shadowCaster: ShadowCaster? { get { paintExtras?.shadowCaster } set { if newValue != nil || paintExtras != nil { paintMore.shadowCaster = newValue } } }
    var insetCaster: InsetShadowCaster? { get { paintExtras?.insetCaster } set { if newValue != nil || paintExtras != nil { paintMore.insetCaster = newValue } } }
    var clipBox: PlainView? { get { extras?.clipBox } set { if newValue != nil || extras != nil { more.clipBox = newValue } } }
    var layoutOffset: CGPoint { get { extras?.layoutOffset ?? .zero } set { if newValue != .zero || extras != nil { more.layoutOffset = newValue } } }
    var stickyOffset: CGPoint { get { extras?.stickyOffset ?? .zero } set { if newValue != .zero || extras != nil { more.stickyOffset = newValue } } }
    /// How far a keyboard toolbar rides up with the keyboard (KeyboardToolbarIOS).
    var keyboardLift: CGFloat { get { extras?.keyboardLift ?? 0 } set { if newValue != 0 || extras != nil { more.keyboardLift = newValue } } }
    var layoutResize: CGSize { get { extras?.layoutResize ?? .zero } set { if newValue != .zero || extras != nil { more.layoutResize = newValue } } }
    var surface: SurfaceLayer? { get { extras?.surface } set { if newValue != nil || extras != nil { more.surface = newValue } } }
    var layoutPanRecognizer: UIPanGestureRecognizer? { get { extras?.layoutPanRecognizer } set { if newValue != nil || extras != nil { more.layoutPanRecognizer = newValue } } }
    var layoutPanOrigin: CGPoint { get { extras?.layoutPanOrigin ?? .zero } set { if newValue != .zero || extras != nil { more.layoutPanOrigin = newValue } } }
    var swipeRecognizer: UIPanGestureRecognizer? { get { extras?.swipeRecognizer } set { if newValue != nil || extras != nil { more.swipeRecognizer = newValue } } }
    var swipeDownX: CGFloat? { get { extras?.swipeDownX } set { if newValue != nil || extras != nil { more.swipeDownX = newValue } } }
    var swipeArmed: Bool { get { extras?.swipeArmed ?? false } set { if newValue || extras != nil { more.swipeArmed = newValue } } }
    var swipeHold: SwipeHold? { get { extras?.swipeHold } set { if newValue != nil || extras != nil { more.swipeHold = newValue } } }
    package var heightRecognizer: UIPanGestureRecognizer? { get { extras?.heightRecognizer } set { if newValue != nil || extras != nil { more.heightRecognizer = newValue } } }
    package var heightHold: DragInput? { get { extras?.heightHold } set { if newValue != nil || extras != nil { more.heightHold = newValue } } }
    package var reorderPan: UIPanGestureRecognizer? { get { extras?.reorderPan } set { if newValue != nil || extras != nil { more.reorderPan = newValue } } }
    package var reorderPress: UILongPressGestureRecognizer? { get { extras?.reorderPress } set { if newValue != nil || extras != nil { more.reorderPress = newValue } } }
    package var reorderHold: AnyObject? { get { extras?.reorderHold } set { if newValue != nil || extras != nil { more.reorderHold = newValue } } }
    package var reorderOrigin: CGPoint { get { extras?.reorderOrigin ?? .zero } set { if newValue != .zero || extras != nil { more.reorderOrigin = newValue } } }
    package var transformRecognizer: UIPanGestureRecognizer? { get { extras?.transformRecognizer } set { if newValue != nil || extras != nil { more.transformRecognizer = newValue } } }
    package var transformHold: DragInput? { get { extras?.transformHold } set { if newValue != nil || extras != nil { more.transformHold = newValue } } }
    package var transformContact: TransformContact? { get { extras?.transformContact } set { if newValue != nil || extras != nil { more.transformContact = newValue } } }
    /// A projection's transform over its layout (PresenterIOS), identity for every other node.
    package var contextTransform: CGAffineTransform {
        get { extras?.contextTransform ?? .identity }
        set {
            if !newValue.isIdentity || extras != nil { more.contextTransform = newValue }
            if newValue.isIdentity { presenter?.contextNodes.remove(id) } else { presenter?.contextNodes.insert(id) }
        }
    }
    /// While it flies as a shared element (LLP 1013.000 D4): where its image is drawn.
    var flightLook: FlightLook? { get { extras?.flightLook } set { if newValue != nil || extras != nil { more.flightLook = newValue } } }
    /// LLP 1093 D7: fragments or columns.
    var columnRecord: ColumnRecord? { get { extras?.columnRecord } set { if newValue != nil || extras != nil { more.columnRecord = newValue } } }
    var liveText: String? { get { extras?.liveText } set { if newValue != nil || extras != nil { more.liveText = newValue } } }
    var swipeOrigin: Double { get { extras?.swipeOrigin ?? 0 } set { if newValue != 0 || extras != nil { more.swipeOrigin = newValue } } }
    var contextRecognizer: UILongPressGestureRecognizer? { get { extras?.contextRecognizer } set { if newValue != nil || extras != nil { more.contextRecognizer = newValue } } }
    var doubleRecognizer: UITapGestureRecognizer? { get { extras?.doubleRecognizer } set { if newValue != nil || extras != nil { more.doubleRecognizer = newValue } } }
    #if !os(tvOS)
    var hoverRecognizer: UIHoverGestureRecognizer? { get { extras?.hoverRecognizer } set { if newValue != nil || extras != nil { more.hoverRecognizer = newValue } } }
    var lookHoverRecognizer: UIHoverGestureRecognizer? { get { extras?.lookHoverRecognizer } set { if newValue != nil || extras != nil { more.lookHoverRecognizer = newValue } } }
    #endif
    package var textArea: UITextView? { get { extras?.textArea } set { if newValue != nil || extras != nil { more.textArea = newValue } } }
    package var field: UITextField? { get { extras?.field } set { if newValue != nil || extras != nil { more.field = newValue } } }
    /// An `input` whose field waits for its closed box (`makeWaitingField`).
    var fieldWaits: Bool { get { extras?.fieldWaits ?? false } set { if newValue || extras != nil { more.fieldWaits = newValue } } }
    /// UIKit's field standing in for this one while it shows: the bar's
    /// search field (`HeaderSearch`), a prompt's (`PromptField`).
    var headerField: UITextField? { get { extras?.headerField } set { if newValue != nil || extras != nil { more.headerField = newValue } } }
    var selectionField: UITextField? { headerField ?? field }
    /// The editor's text as the app was last told it; nil once the app has set the value since (`FieldComposition.swift`).
    var toldText: String? { get { extras?.toldText } set { if newValue != nil || extras != nil { more.toldText = newValue } } }
    /// A `value` that arrived while text was marked, applied when the composition ends.
    var pendingValue: String? { get { extras?.pendingValue } set { if newValue != nil || extras != nil { more.pendingValue = newValue } } }
    var video: VideoView? { get { extras?.video } set { if newValue != nil || extras != nil { more.video = newValue } } }
    var web: UIView? { get { extras?.web } set { if newValue != nil || extras != nil { more.web = newValue } } }
    package var metal: MetalView? { get { extras?.metal } set { if newValue != nil || extras != nil { more.metal = newValue } } }
    package var canvasInput: CanvasInput? { get { extras?.canvasInput } set { if newValue != nil || extras != nil { more.canvasInput = newValue } } }
    package var overlay: PlainView? { get { extras?.overlay } set { if newValue != nil || extras != nil { more.overlay = newValue } } }
    package var needsCapture: Bool { get { extras?.needsCapture ?? false } set { if newValue || extras != nil { more.needsCapture = newValue } } }
    package var paintedThisTurn: Bool { get { extras?.paintedThisTurn ?? false } set { if newValue || extras != nil { more.paintedThisTurn = newValue } } }
    package var placement: [Double]? { get { extras?.placement } set { if newValue != nil || extras != nil { more.placement = newValue }; if newValue != nil { Self.placedViews.add(self) } } }
    var symbolView: UIImageView? { get { extras?.symbolView } set { if newValue != nil || extras != nil { more.symbolView = newValue } } }
    var symbolFound: Bool { get { extras?.symbolFound ?? false } set { if newValue || extras != nil { more.symbolFound = newValue } } }
    var symbolKey: String? { get { extras?.symbolKey } set { if newValue != nil || extras != nil { more.symbolKey = newValue } } }
    var symbolRefusal: String? { get { extras?.symbolRefusal } set { if newValue != nil || extras != nil { more.symbolRefusal = newValue } } }
    /// A grouped list's custom row's leading symbol, drawn as UIKit's list
    /// draws a cell's image (`GroupedCustomRowsIOS.swift`).
    package var listSymbol: Bool { get { extras?.listSymbol ?? false } set { guard newValue != listSymbol else { return }; more.listSymbol = newValue; symbolKey = nil; updateSymbol() } }
    /// What a grouped list's custom row presses (`GroupedCustomRowsIOS.swift`):
    /// a list cell, which UIKit gives the button trait whatever it opens, so
    /// a `link` row is a button to VoiceOver there, as a hand-built cell is.
    package var listRow: Bool {
        get { extras?.listRow ?? false }
        set {
            guard newValue != listRow else { return }
            more.listRow = newValue
            guard actsAsButton, props["accessibilityRole"] == "link" else { return }
            accessibilityTraits.insert(newValue ? .button : .link); accessibilityTraits.remove(newValue ? .link : .button)
        }
    }
    /// Whether its scroll view holds a default surface's corner (`syncDefaultSurfaceScroll`).
    var scrollCornered: Bool { get { extras?.scrollCornered ?? false } set { if newValue || extras != nil { more.scrollCornered = newValue } } }
    var focusRing: CAShapeLayer? { get { extras?.focusRing } set { if newValue != nil || extras != nil { more.focusRing = newValue } } }
    var scrollEventQueued: Bool { get { extras?.scrollEventQueued ?? false } set { if newValue || extras != nil { more.scrollEventQueued = newValue } } }
    var lastScrollEvent: (offset: CGPoint, topInset: CGFloat) { get { extras?.lastScrollEvent ?? (.zero, 0) } set { if newValue.offset != .zero || newValue.topInset != 0 || extras != nil { more.lastScrollEvent = newValue } } }
    var dispatchingScrollEvent: Bool { get { extras?.dispatchingScrollEvent ?? false } set { if newValue || extras != nil { more.dispatchingScrollEvent = newValue } } }
    var scrollEnd: ScrollEnd? { get { extras?.scrollEnd } set { if newValue != nil || extras != nil { more.scrollEnd = newValue } } }
    var beforeLayoutScroll: (offset: CGPoint, topInset: CGFloat)? { get { extras?.beforeLayoutScroll } set { if newValue != nil || extras != nil { more.beforeLayoutScroll = newValue } } }
    var hiddenScroll: CGPoint? { get { extras?.hiddenScroll } set { if newValue != nil || extras != nil { more.hiddenScroll = newValue } } }
    var followedScroll: (top: CGFloat, end: Bool, start: Bool)? { get { extras?.followedScroll } set { if newValue != nil || extras != nil { more.followedScroll = newValue } } }
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
    var controlRide: CGFloat { get { extras?.controlRide ?? 0 } set { if newValue != 0 || extras != nil { more.controlRide = newValue } } }
    var scrollOrigin: CGFloat { get { extras?.scrollOrigin ?? 0 } set { if newValue != 0 || extras != nil { more.scrollOrigin = newValue } } }
    var scrollCollapsed: CGFloat { get { extras?.scrollCollapsed ?? 0 } set { if newValue != 0 || extras != nil { more.scrollCollapsed = newValue } } }
    var scrollWritten: String? { get { extras?.scrollWritten } set { if newValue != nil || extras != nil { more.scrollWritten = newValue } } }
    var hatchReusable: Bool { get { extras?.hatchReusable ?? false } set { if newValue || extras != nil { more.hatchReusable = newValue } } }
    /// Where CSS's `scrollTop` 0 is in UIKit's offsets: past the scroller's
    /// top inset when UIKit insets it — a collapsing title's bar, or the bar
    /// over a route's bottom-edge scroller (`reachBottom`), which stays
    /// UIKit's to inset once the title collapses another: the scrollport's
    /// top is the bar's bottom, whatever its height (UIKit keeps the offset
    /// plus that inset fixed while the title collapses), or past native content room.
    func scrollTopInset(_ sv: UIScrollView) -> CGFloat {
        scrollOrigin > 0 || sv.contentInsetAdjustmentBehavior != .never
            ? sv.adjustedContentInset.top : (extras?.nativeScrollContentInsets.top ?? 0) + pageBand.top
    }
    /// Its page band (`NodeExtras.pageBand`), zero for most.
    var pageBand: UIEdgeInsets { extras?.pageBand ?? .zero }
    var bandInset: UIEdgeInsets { extras?.bandInset ?? .zero }
    var bandOver: Int64? { get { extras?.bandOver } set { if newValue != nil || extras != nil { more.bandOver = newValue } } }
    var edgeInteraction: AnyObject? { get { extras?.edgeInteraction } set { if newValue != nil || extras != nil { more.edgeInteraction = newValue } } }
    /// Its scroll view's frame: its bounds, and the band past them.
    package var scrollFrame: CGRect { bounds.inset(by: UIEdgeInsets(top: -pageBand.top, left: 0, bottom: -pageBand.bottom, right: 0)) }
    /// Native padding and footer space are content, unlike a bar or keyboard.
    package func scrollPortInsets(_ sv: UIScrollView) -> UIEdgeInsets {
        let inset = sv.adjustedContentInset, content = extras?.nativeScrollContentInsets ?? .zero
        return UIEdgeInsets(top: inset.top - content.top, left: inset.left - content.left,
                            bottom: inset.bottom - content.bottom, right: inset.right - content.right)
    }
    /// Its scrollport in `sv`'s bounds: `sv` less those insets, empty where they overlap (CSS's `clientWidth` and
    /// `clientHeight` never go below zero).
    func scrollPort(_ sv: UIScrollView) -> CGRect {
        let b = sv.bounds, p = scrollPortInsets(sv)
        return CGRect(x: b.minX + min(p.left, b.width), y: b.minY + min(p.top, b.height),
                      width: max(0, b.width - p.left - p.right), height: max(0, b.height - p.top - p.bottom))
    }
    /// The least content height UIKit takes: the box, or for a scroller
    /// UIKit insets, the port between the insets the kernel lays its content
    /// out in (LLP 1116.000 D1.2), so a page shorter than that port does not
    /// scroll by the bar and the home indicator, as a table view's does not.
    func scrollFloor(_ sv: UIScrollView) -> CGFloat {
        guard case .inset(let top, let bottom)? = presenter?.navigation.covers[id] else { return sv.bounds.height - pageBand.top - pageBand.bottom }
        return max(0, sv.bounds.height - top - bottom - pageBand.top - pageBand.bottom)
    }
}
extension UIColor {
    /// LLP 1115 D2: what an unset background is, the platform's —
    /// `systemBackground` (tvOS has none; white stands in) — or the theme's
    /// where it replaces the page's ground (LLP 1095 D4).
    /// `view`'s presenter's theme (LLP 1121 stage 5).
    package static func platformBackground(in view: UIView?) -> UIColor {
        #if os(tvOS)
        return ThemeRoles.color("background", in: view) ?? ThemeRoles.color("Canvas", in: view, or: .white)
        #else
        return ThemeRoles.color("background", in: view) ?? ThemeRoles.color("Canvas", in: view, or: .systemBackground)
        #endif
    }
}
extension NodeView {
    /// Whether this is a modal route on the system background the compiler
    /// gave it (LLP 1116.009 D3), its author having said none: on iOS that
    /// ground is its sheet's surface, which the presentation paints
    /// (`ModalController.paintSurface`), so that below its large detent the
    /// sheet is UIKit's glass (LLP 1141.001 D3).
    var groundIsSheets: Bool {
        #if os(iOS)
        guard props["navigationPresentation"] == "modal", !wrote("background_color"),
              case .object(let o)? = style["background_color"] else { return false }
        return o["sys"]?.string == "systemBackgroundColor"
        #else
        return false
        #endif
    }
    /// LLP 1115.000 D2: whether the author wrote the row `key` on this node,
    /// as an attribute or a row of its class. The style says so row by row
    /// (`written`); it is false of a value the node only inherits and of a
    /// row the compiler implied (a sheet's, a heading's, a tag's own).
    package func wrote(_ key: String) -> Bool {
        guard case .array(let rows)? = style["written"] else { return false }
        return rows.contains(.string(key))
    }
    /// LLP 1115 §2, "what the author writes wins": the colour row `key` when
    /// its author wrote it on this node, a platform colour and one equal to
    /// its parent's included. Nil leaves the control the platform's (D4).
    func ownColor(_ key: String) -> BatchValue? { wrote(key) ? style[key] : nil }
    /// A field's text: its own `color`, else the design's `FieldText`, else
    /// the platform's; an inherited colour never tints a field (LLP 1115 D4,
    /// LLP 1121 D9).
    /// A field of the author's own (`appearance="none"`) is their box, and
    /// inherits as text does.
    var fieldInk: UIColor {
        showsOwnTextColor || !isNativeTextControl ? color("text_color", SystemColor.canvasText) : ThemeRoles.color("FieldText", in: self, or: SystemColor.canvasText)
    }
    /// `ownColor` as this node draws it now, or nil.
    func ownUIColor(_ key: String) -> UIColor? {
        // As written, never greyed: its callers hand it to UIKit (TintDimmingIOS.swift)
        ownColor(key) == nil ? nil : undimmedCGColor(key).map { UIColor(cgColor: $0) }
    }
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
