// The view that presents a session on UIKit (LLP 1031 D1, D3): an ordinary
// UIView the containing app places. Its bounds are the session's viewport
// — the safe area of its own container, or the whole of it when the first
// root says `viewport-fit="cover"`, the safe-area insets then going to the
// kernel for its `env()` lengths (LLP 1008 §9) — and, under
// `interactive-widget="resizes-content"`, the keyboard's top. The plan
// boots at the first layout that has a size and follows every later size
// (a rotation, a split) and every change of the insets. Bounded
// containment: the session's page scrolls inside this view exactly as the
// standalone host's does.
#if os(iOS)
import UIKit

public final class ExactView: UIView {
    public let session: ExactSession
    private var fitPending = false
    private var lastSize = CGSize.zero
    private var lastInsets = UIEdgeInsets.zero
    private let keyboardProbe = UIView()
    /// The adapter's hook for the first root's `viewport-fit` and its
    /// canvas colour (the window's background under the safe areas is the
    /// window's business).
    public var onViewportFit: (() -> Void)?
    public var onCanvasColor: ((UIColor) -> Void)?

    public init(session: ExactSession) {
        self.session = session
        super.init(frame: .zero)
        backgroundColor = .white
        addSubview(session.presenter.viewport)
        // A zero-size dependent makes UIKit lay this view out as its keyboard
        // guide moves. Frame notifications alone omit interactive drag frames.
        keyboardProbe.isHidden = true
        keyboardProbe.translatesAutoresizingMaskIntoConstraints = false
        addSubview(keyboardProbe)
        NSLayoutConstraint.activate([
            keyboardProbe.topAnchor.constraint(equalTo: keyboardLayoutGuide.topAnchor),
            keyboardProbe.leadingAnchor.constraint(equalTo: leadingAnchor),
            keyboardProbe.widthAnchor.constraint(equalToConstant: 0),
            keyboardProbe.heightAnchor.constraint(equalToConstant: 0),
        ])
        session.view = self
        session.presenter.onViewportFit = { [weak self] in self?.setNeedsLayout(); self?.onViewportFit?() }
        session.presenter.onCanvasColor = { [weak self] color in self?.backgroundColor = color; self?.onCanvasColor?(color) }
        session.presenter.onKeyboardResize = { [weak self] in self?.fit() }
        session.presenter.observeKeyboard()
    }

    required init?(coder: NSCoder) { nil }

    /// The first root's `viewport-fit` prop (`"cover"` or nothing).
    public var viewportFit: String? { session.presenter.viewportFit }

    public override func layoutSubviews() {
        super.layoutSubviews()
        fit()
    }

    public override func willMove(toWindow newWindow: UIWindow?) {
        // Child didMoveToWindow callbacks can retry focus before our own
        // didMoveToWindow. Wait until their native owners have been installed.
        if newWindow != nil { session.presenter.navigation.willMount() }
        super.willMove(toWindow: newWindow)
    }

    public override func didMoveToWindow() {
        super.didMoveToWindow()
        if window == nil {
            session.presenter.menus.unmounted()
            session.presenter.modals.unmounted()
            session.presenter.navigation.unmounted()
        } else {
            // Install native ownership after UIKit finishes attaching this
            // view. A retained session may return under a different controller.
            DispatchQueue.main.async { [weak self] in
                guard let self, window != nil, session.state != .destroyed else { return }
                fit()
                session.presenter.navigation.mounted()
            }
        }
        // Mounted and visible participate in frame demand (D3): an unmounted
        // view wants no frames; a mounted one asks again.
        session.frames.run(window != nil && (session.frames.motion || session.canvases.wantsFrames))
    }

    /// Frame the viewport to the safe area or the whole view — and, under
    /// `interactive-widget="resizes-content"`, to the keyboard's top, where
    /// the bottom inset is the keyboard's and not the home indicator's (the
    /// web's rule) — and, once booted, tell the kernel about new insets or a
    /// new size. Called inside the keyboard's animation block, so the frames
    /// the batch sets animate with the keyboard (LLP 1008 §9).
    func fit() {
        let presenter = session.presenter
        // Containment may synchronously lay us out during a partial batch.
        // Let the next layout read the fully mounted tree before resizing it.
        if presenter.applying {
            if !fitPending {
                fitPending = true
                DispatchQueue.main.async { [weak self] in
                    guard let self else { return }
                    self.fitPending = false
                    self.fit()
                }
            }
            return
        }
        // UIKit moves the keyboard sideways with an interactive pop. Its
        // hide notification is not a request to drop the composer below
        // those moving keys; keep the current viewport until it settles.
        guard !presenter.navigation.preservesKeyboardViewport else { return }
        let container = presenter.modals.coordinateView ?? self
        let safe = container.safeAreaInsets
        let cover = presenter.viewportFit == "cover"
        var frame = cover ? container.bounds : container.bounds.inset(by: safe)
        var insets = cover ? safe : .zero
        if presenter.interactiveWidget == "resizes-content" {
            let top: CGFloat
            // A sheet's guide remains in its local coordinates as UIKit moves
            // the sheet, including during interactive dismissal.
            // A focused editor uses that same local guide. After rotation the
            // notification can include margin above the keys; after a cancelled
            // pop it can still announce hiding. Neither replaces the guide's
            // occupied geometry while this session retains its editor.
            if presenter.interactiveKeyboardDrag || presenter.modals.active ||
                presenter.hasKeyboardEditor {
                let guide = container.keyboardLayoutGuide.layoutFrame
                top = guide.height > safe.bottom + 1 ? guide.minY : .infinity
            } else if let edge = presenter.keyboardTop, let window {
                top = container.convert(CGPoint(x: 0, y: edge), from: window).y
            } else { top = .infinity }
            presenter.keyboardInset = min(max(0, frame.maxY - max(top, frame.minY)), frame.height)
            if top < frame.maxY {
                frame.size.height = max(0, top - frame.minY)
                insets.bottom = 0
            }
        }
        if presenter.viewport.frame != frame { presenter.viewport.frame = frame }
        let size = frame.size
        guard size.width > 0, size.height > 0 else { return }
        if !session.booted {
            lastSize = size
            lastInsets = insets
            session.boot(size: size)
            // The first batch made the roots: one that covers the screen is
            // framed to it now, before anything is drawn.
            fit()
            return
        }
        if insets != lastInsets {
            lastInsets = insets
            presenter.insets = insets
            session.insets(top: insets.top, right: insets.right, bottom: insets.bottom, left: insets.left)
        }
        if size != lastSize {
            lastSize = size
            session.resize(size)
        }
    }

    /// After a restart from a new plan (the dev loop): the new runner knows
    /// nothing of the insets — hand them over again, and fit the root.
    func rebooted() {
        if lastInsets != .zero { session.insets(top: lastInsets.top, right: lastInsets.right, bottom: lastInsets.bottom, left: lastInsets.left) }
        fit()
    }
}
#endif
