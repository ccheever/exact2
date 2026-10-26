#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

extension PopoverSheetIOSTests {
    func page(session: ExactSession? = nil) throws -> Presenter {
        let scene = try XCTUnwrap(UIApplication.shared.connectedScenes.compactMap { $0 as? UIWindowScene }.first)
        self.session = session
        let p = session?.presenter ?? Presenter(), window = UIWindow(windowScene: scene)
        kept = p
        // Agent's observations need a session, but the fixture owns this wire tree.
        p.session = nil
        p.onIntrinsic = nil; p.onCovers = nil; p.onClipboard = nil; p.onClose = nil
        p.onPress = nil; p.onFocus = nil; p.onBlur = nil; p.onFocusEvent = nil; p.onKey = nil
        window.frame = scene.screen.bounds
        let controller = UIViewController()
        window.rootViewController = controller
        window.makeKeyAndVisible()
        self.window = window
        p.viewport.frame = controller.view.bounds
        controller.view.addSubview(p.viewport)
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view"],
            ["op": "create", "id": 2, "kind": "view", "props": ["id": "form", "semanticTag": "dialog", "closedby": "any"]],
            ["op": "create", "id": 3, "kind": "view", "handlers": ["press", "pointerdown", "pointerup"]],
            ["op": "create", "id": 4, "kind": "button", "props": ["id": "opener", "popovertarget": "choices"]],
            ["op": "create", "id": 5, "kind": "view", "props": ["id": "choices", "popover": "auto"], "style": ["position_area": "bottom"]],
            ["op": "create", "id": 6, "kind": "text", "props": ["text": "Choice"]],
            ["op": "create", "id": 7, "kind": "button", "props": ["id": "sub-opener", "popovertarget": "submenu"]],
            ["op": "create", "id": 8, "kind": "view", "props": ["id": "submenu", "popover": "auto"]],
            ["op": "create", "id": 9, "kind": "text", "props": ["text": "Submenu content"]],
            ["op": "create", "id": 10, "kind": "text", "props": ["text": "Form content"]],
            ["op": "create", "id": 20, "kind": "view", "props": ["id": "second", "semanticTag": "dialog"]],
            ["op": "create", "id": 21, "kind": "text", "props": ["text": "Second dialog content"]],
            ["op": "children", "id": 8, "ids": [9]],
            ["op": "children", "id": 5, "ids": [6, 7, 8]],
            ["op": "children", "id": 3, "ids": [4, 5]],
            ["op": "children", "id": 2, "ids": [3, 10]],
            ["op": "children", "id": 20, "ids": [21]],
            ["op": "children", "id": 1, "ids": [2, 20]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0, "y": 0, "w": window.bounds.width, "h": window.bounds.height],
            ["op": "frame", "id": 2, "x": 0, "y": 200, "w": 300, "h": 360],
            ["op": "frame", "id": 3, "x": 0, "y": 0, "w": 300, "h": 260],
            ["op": "frame", "id": 4, "x": 100, "y": 40, "w": 100, "h": 40],
            ["op": "frame", "id": 5, "x": 10, "y": 100, "w": 200, "h": 120],
            ["op": "frame", "id": 6, "x": 0, "y": 0, "w": 200, "h": 120],
            ["op": "frame", "id": 7, "x": 150, "y": 10, "w": 40, "h": 30],
            ["op": "frame", "id": 8, "x": 0, "y": 0, "w": 100, "h": 60],
            ["op": "frame", "id": 9, "x": 0, "y": 0, "w": 100, "h": 60],
            ["op": "frame", "id": 10, "x": 20, "y": 280, "w": 200, "h": 40],
            ["op": "frame", "id": 20, "x": 0, "y": 0, "w": 280, "h": 240],
            ["op": "frame", "id": 21, "x": 20, "y": 20, "w": 200, "h": 100],
        ]))
        p.viewport.layoutIfNeeded()
        return p
    }

    func show(_ name: String = "form", in p: Presenter) throws -> DialogSheetController {
        p.dialogCommand("showModal", name)
        settle { !p.menus.inTransition }
        let sheet = try XCTUnwrap(p.menus.sheets.top as? DialogSheetController)
        XCTAssertNotNil(sheet.view.window)
        sheet.view.layoutIfNeeded()
        return sheet
    }

    func assertAbove(_ popover: NodeView, _ sheet: DialogSheetController,
                             file: StaticString = #filePath, line: UInt = #line) throws {
        let container = try XCTUnwrap(sheet.presentationController?.containerView, file: file, line: line)
        let layer = try XCTUnwrap(popover.superview, file: file, line: line)
        XCTAssertTrue(layer.superview === container, "the layer escapes the sheet's card", file: file, line: line)
        var branch: UIView = sheet.view
        while let parent = branch.superview, parent !== container { branch = parent }
        let cardIndex = try XCTUnwrap(container.subviews.firstIndex(of: branch), file: file, line: line)
        let popoverIndex = try XCTUnwrap(container.subviews.firstIndex(of: layer), file: file, line: line)
        XCTAssertGreaterThan(popoverIndex, cardIndex, file: file, line: line)
        let point = popover.convert(CGPoint(x: popover.bounds.midX, y: popover.bounds.midY), to: window)
        let hit = try XCTUnwrap(window?.hitTest(point, with: nil), file: file, line: line)
        XCTAssertTrue(hit.isDescendant(of: popover), "the window reaches popover content, got \(type(of: hit))", file: file, line: line)
    }

    func presentRoute(_ p: Presenter, authoredInRoute: Bool = true) throws -> UIViewController {
        p.apply(wireBatch([
            ["op": "create", "id": 30, "kind": "view", "props": ["navigationPresentation": "modal"]],
            ["op": "frame", "id": 30, "x": 0, "y": 0, "w": 300, "h": 600],
            ["op": "children", "id": 30, "ids": authoredInRoute ? [2, 20] : [20]],
            ["op": "children", "id": 1, "ids": authoredInRoute ? [30] : [2, 30]],
        ]))
        let route = try XCTUnwrap(p.views[30]), owner = try XCTUnwrap(window?.rootViewController)
        let navigation = UIViewController()
        navigation.view.addSubview(route)
        p.modals.present(route, navigation: navigation, from: owner, preceding: nil, owner: owner)
        settle { !p.modals.inTransition }
        return try XCTUnwrap(p.modals.owner)
    }

    func flushCallbacks() {
        var done = false
        DispatchQueue.main.async { done = true }
        settle { done }
    }

}

final class SheetPresenter: UIViewController {
    let owner: UIViewController
    let transition: SheetDismissal?
    init(_ owner: UIViewController, transition: SheetDismissal? = nil) {
        self.owner = owner; self.transition = transition
        super.init(nibName: nil, bundle: nil)
    }
    required init?(coder: NSCoder) { nil }
    override func present(_ controller: UIViewController, animated: Bool, completion: (() -> Void)? = nil) {
        controller.modalPresentationStyle = .fullScreen
        controller.transitioningDelegate = transition
        owner.present(controller, animated: animated, completion: completion)
    }
}

/// UIKit's real cancellation coordinator, driven without synthesizing a finger.
final class SheetDismissal: NSObject, UIViewControllerTransitioningDelegate, UIViewControllerAnimatedTransitioning {
    let interaction = UIPercentDrivenInteractiveTransition()
    var interactive = true
    var started = false
    func animationController(forDismissed dismissed: UIViewController) -> UIViewControllerAnimatedTransitioning? { self }
    func interactionControllerForDismissal(using animator: UIViewControllerAnimatedTransitioning) -> UIViewControllerInteractiveTransitioning? {
        interactive ? interaction : nil
    }
    func transitionDuration(using context: UIViewControllerContextTransitioning?) -> TimeInterval { 0.2 }
    func animateTransition(using context: UIViewControllerContextTransitioning) {
        guard let view = context.view(forKey: .from) else { return context.completeTransition(false) }
        started = true
        UIView.animate(withDuration: transitionDuration(using: context), animations: {
            view.transform = CGAffineTransform(translationX: 0, y: view.bounds.height)
        }) { _ in
            view.transform = .identity
            context.completeTransition(!context.transitionWasCancelled)
        }
    }
}

enum PopoverScreenModule {
    nonisolated(unsafe) static weak var made: UIViewController?
    static let roster = strdup(#"{"popover-screen":{"snapshot":false}}"#)!
    static let table: [UInt8] = {
        var bytes = [UInt8](repeating: 0, count: 168)
        func put<T>(_ value: T, at offset: Int) {
            withUnsafeBytes(of: value) { for (i, byte) in $0.enumerated() { bytes[offset + i] = byte } }
        }
        let create: @convention(c) (UnsafeMutableRawPointer?, UnsafePointer<UInt8>?, UInt32, UnsafePointer<UInt8>?, UInt32,
                                   UnsafeMutableRawPointer?, UnsafeMutableRawPointer?, UnsafeMutableRawPointer?, UInt32,
                                   UnsafeMutablePointer<UInt8>?, UInt32) -> UnsafeMutableRawPointer? = { _, _, _, _, _, _, _, _, _, _, _ in
            let controller = UIViewController()
            PopoverScreenModule.made = controller
            return Unmanaged.passRetained(controller).toOpaque()
        }
        let view: @convention(c) (UnsafeMutableRawPointer?) -> UnsafeMutableRawPointer? = {
            let controller = Unmanaged<UIViewController>.fromOpaque($0!).takeUnretainedValue()
            return Unmanaged.passUnretained(controller.view!).toOpaque()
        }
        let controller: @convention(c) (UnsafeMutableRawPointer?) -> UnsafeMutableRawPointer? = { $0 }
        let set: @convention(c) (UnsafeMutableRawPointer?, UnsafePointer<UInt8>?, UInt32, UnsafeMutablePointer<UInt8>?, UInt32) -> Int32 = { _, _, _, _, _ in 0 }
        let destroy: @convention(c) (UnsafeMutableRawPointer?) -> Void = { Unmanaged<UIViewController>.fromOpaque($0!).release() }
        let none: @convention(c) (UnsafeMutableRawPointer?) -> Void = { _ in }
        put(UInt32(3), at: 0); put(UInt32(168), at: 4); put(UnsafeRawPointer(roster), at: 8)
        put(unsafeBitCast(create, to: UnsafeRawPointer.self), at: 16)
        put(unsafeBitCast(view, to: UnsafeRawPointer.self), at: 24)
        put(unsafeBitCast(set, to: UnsafeRawPointer.self), at: 32)
        put(unsafeBitCast(destroy, to: UnsafeRawPointer.self), at: 48)
        // The test supplies the module instance; only its no-op destroy runs.
        for offset in [72, 80, 88, 96] { put(unsafeBitCast(none, to: UnsafeRawPointer.self), at: offset) }
        put(unsafeBitCast(controller, to: UnsafeRawPointer.self), at: 160)
        return bytes
    }()
}
#endif
