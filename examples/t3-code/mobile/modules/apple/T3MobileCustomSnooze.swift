#if os(iOS)
// @ref llp/1107.004-home-projection.decision.md#custom-snooze-input
// Pinned CustomSnoozeSheet.ios: transient picker state, one native reply, no transport owner.
import UIKit
import SwiftUI

final class T3MobileCustomSnooze: NSObject, UIPopoverPresentationControllerDelegate {
    private final class Row {
        weak var element: ExactElement?
        var context: T3CustomSnoozeContext
        let surface: String
        init(_ element: ExactElement, _ context: T3CustomSnoozeContext, _ surface: String) {
            self.element = element; self.context = context; self.surface = surface
        }
        var shown: Bool {
            guard let element, element.isLive, let view = element.view, view.window != nil else { return false }
            var next: UIView? = view
            while let value = next { if value.isHidden || value.alpha == 0 || !value.isUserInteractionEnabled { return false }; next = value.superview }
            return surface == "sidebar-list" ? context.sidebarVisible : context.homeVisible
        }
    }
    private var rows: [ObjectIdentifier: Row] = [:]
    private var owner: Row?
    private var captured: T3CustomSnoozeContext?
    private var invocation = "menu"
    private var opening: UUID?
    private var presentation: T3CustomSnoozePopover?
    private var completion: (([String: Any]) -> Void)?
    private var sidebarVisible = false
    private var alive = true
    private let now: () -> Date
    init(now: @escaping () -> Date = { Date() }) { self.now = now; super.init() }

    func configure(_ element: ExactElement) {
        let key = ObjectIdentifier(element)
        guard let raw = element.data[.mobileHomeMenu], let context = try? JSONDecoder().decode(T3CustomSnoozeContext.self, from: Data(raw.utf8)),
              let surface = element.data[.mobileHomeSurface], ["home-list", "sidebar-list"].contains(surface) else { end(element); return }
        if let row = rows[key], row.surface == surface { row.context = context }
        else { rows[key] = Row(element, context, surface) }
        if let owner, owner.element === element, !current() { finish(nil) }
    }
    func end(_ element: ExactElement) {
        if owner?.element === element { finish(nil) }
        rows.removeValue(forKey: ObjectIdentifier(element))
    }
    func setSidebarVisible(_ visible: Bool) {
        sidebarVisible = visible
        if !visible && owner?.surface == "sidebar-list" { finish(nil) }
    }
    func routeEnded(_ route: ExactRoute) {
        if captured?.requestRoute == route.key || (owner?.surface == "sidebar-list" && route.key == "t3-workspace-sidebar") { finish(nil) }
    }
    private func current() -> Bool {
        guard alive, let owner, let captured, let element = owner.element,
              rows[ObjectIdentifier(element)] === owner, owner.shown, owner.context.eligible(invocation: invocation),
              owner.context.sameOwner(as: captured, invocation: invocation), owner.surface != "sidebar-list" || sidebarVisible else { return false }
        return true
    }
    func present(_ request: [String: Any], reply: @escaping ([String: Any]) -> Void) throws {
        guard alive, opening == nil else { throw ExactNativeRefusal("A custom snooze picker is already open.") }
        guard let invocation = request["invocation"] as? String, ["menu", "swipe"].contains(invocation),
              let row = rows.values.first(where: { $0.shown && $0.context.eligible(invocation: invocation) && $0.context.matches(request) && ($0.surface != "sidebar-list" || sidebarVisible) }),
              let element = row.element, let view = element.view else { reply(["choice": "cancel"]); return }
        var responder: UIResponder? = view
        while responder != nil && !(responder is UIViewController) { responder = responder?.next }
        guard let presenter = responder as? UIViewController, presenter.presentedViewController == nil,
              !presenter.isBeingDismissed else { throw ExactNativeRefusal("Close the current presentation before choosing a custom snooze.") }
        let token = UUID(), colors = T3CustomSnoozeColors(element)
        owner = row; captured = row.context; self.invocation = invocation; opening = token; completion = reply
        let clock = now
        let model = T3CustomSnoozeDraft(initial: T3CustomSnoozeValue.milliseconds(clock()))
        let content = T3CustomSnoozeContent(model: model, colors: colors)
        let screen = T3CustomSnoozeController(rootView: content)
        screen.view.backgroundColor = .clear
        screen.navigationItem.style = .navigator
        let title = UILabel()
        title.text = "Custom snooze"; title.textColor = colors.foreground; title.font = .systemFont(ofSize: 17, weight: .bold)
        title.textAlignment = .center; title.accessibilityTraits = .header; title.translatesAutoresizingMaskIntoConstraints = false
        screen.navigationItem.leftBarButtonItem = UIBarButtonItem(image: UIImage(systemName: "xmark"), primaryAction: UIAction { [weak self] _ in self?.finish(nil, token: token) })
        screen.navigationItem.leftBarButtonItem?.accessibilityLabel = "Cancel custom snooze"
        screen.navigationItem.rightBarButtonItem = UIBarButtonItem(title: "Snooze", primaryAction: UIAction { [weak self, weak model] _ in
            guard let self, let model, self.opening == token else { return }
            guard self.current() else { self.finish(nil, token: token); return }
            let current = T3CustomSnoozeValue.milliseconds(clock()), zone = TimeZone.current
            let local = T3CustomSnoozeValue.local(model.date, zone: zone)
            guard let wake = T3CustomSnoozeValue.resolve(mode: model.mode, date: local.date, time: local.time,
                                                       amount: model.amount, unit: model.unit, now: current, zone: zone) else {
                model.error = model.mode == "date" ? "Choose a date and time in the future." : "Enter a positive duration."
                self.resize(error: true); return
            }
            self.finish(T3CustomSnoozeValue.iso(wake), confirmedAt: Int64((current.timeIntervalSince1970 * 1000).rounded()), token: token)
        })
        let navigation = UINavigationController(rootViewController: screen)
        let appearance = UINavigationBarAppearance()
        if #available(iOS 26.0, *) { appearance.configureWithTransparentBackground() }
        else { appearance.configureWithOpaqueBackground(); appearance.backgroundColor = colors.background }
        appearance.shadowColor = .clear; appearance.titleTextAttributes = [.foregroundColor: colors.foreground, .font: UIFont.systemFont(ofSize: 17, weight: .bold)]
        navigation.navigationBar.standardAppearance = appearance; navigation.navigationBar.scrollEdgeAppearance = appearance
        navigation.navigationBar.tintColor = colors.foreground
        navigation.overrideUserInterfaceStyle = view.traitCollection.userInterfaceStyle
        // Source nests its navigation controller inside the fixed-size popover content.
        // Presenting a UINavigationController itself adds its bar to the proposed size.
        let presentation = T3CustomSnoozePopover(navigation)
        presentation.modalPresentationStyle = .popover; self.presentation = presentation
        navigation.navigationBar.addSubview(title)
        NSLayoutConstraint.activate([title.centerXAnchor.constraint(equalTo: navigation.navigationBar.centerXAnchor),
                                     title.centerYAnchor.constraint(equalTo: navigation.navigationBar.centerYAnchor)])
        screen.resized = { [weak self, weak model] in self?.resize(error: model?.error != nil) }
        model.errorChanged = { [weak self] error in self?.resize(error: error) }
        resize(error: false)
        if let popover = presentation.popoverPresentationController {
            popover.sourceView = view; popover.sourceRect = view.bounds; popover.permittedArrowDirections = .any; popover.delegate = self
        }
        presenter.present(presentation, animated: true)
        presentation.presentationController?.delegate = self
    }
    private func resize(error: Bool) {
        guard let presentation, let bounds = owner?.element?.view?.window?.bounds else { return }
        let size = CGSize(width: min(360, max(0, bounds.width - 32)), height: min(error ? 364 : 324, max(0, bounds.height - 96)))
        // This is the complete popover height, including its navigation bar.
        if presentation.preferredContentSize != size { presentation.preferredContentSize = size }
    }
    private func finish(_ iso: String?, confirmedAt: Int64? = nil, token: UUID? = nil) {
        guard let opening, token == nil || token == opening else { return }
        let reply = completion, old = presentation
        self.opening = nil; completion = nil; presentation = nil; owner = nil; captured = nil; invocation = "menu"
        old?.presentationController?.delegate = nil; old?.popoverPresentationController?.delegate = nil; old?.dismiss(animated: true)
        if let iso, let confirmedAt { reply?(["choice": "snooze", "snoozedUntil": iso, "confirmedAt": confirmedAt]) }
        else { reply?(["choice": "cancel"]) }
    }
    func adaptivePresentationStyle(for controller: UIPresentationController) -> UIModalPresentationStyle { .none }
    func presentationControllerDidDismiss(_ presentationController: UIPresentationController) { if presentation === presentationController.presentedViewController { finish(nil) } }
    func popoverPresentationControllerDidDismissPopover(_ popoverPresentationController: UIPopoverPresentationController) { if presentation === popoverPresentationController.presentedViewController { finish(nil) } }
    func destroy() { alive = false; finish(nil); rows.removeAll() }
}

// A public containment boundary fixes the entire source popover, including its bar.
private final class T3CustomSnoozePopover: UIViewController {
    let navigation: UINavigationController
    init(_ navigation: UINavigationController) { self.navigation = navigation; super.init(nibName: nil, bundle: nil) }
    required init?(coder: NSCoder) { fatalError("init(coder:) is unavailable") }
    override func viewDidLoad() {
        super.viewDidLoad(); view.backgroundColor = .clear
        addChild(navigation); view.addSubview(navigation.view)
        navigation.view.frame = view.bounds; navigation.view.autoresizingMask = [.flexibleWidth, .flexibleHeight]
        navigation.didMove(toParent: self)
    }
}

private final class T3CustomSnoozeDraft: ObservableObject {
    @Published var mode = "date" { didSet { error = nil } }
    @Published var date: Date { didSet { error = nil } }
    @Published var amount = 2 { didSet { error = nil } }
    @Published var unit = "hours" { didSet { error = nil } }
    @Published var error: String? { didSet { errorChanged?(error != nil) } }
    var errorChanged: ((Bool) -> Void)?
    init(initial: Date) { date = initial.addingTimeInterval(3600) }
}
private struct T3CustomSnoozeColors {
    let foreground: UIColor, primary: UIColor, danger: UIColor, background: UIColor
    init(_ element: ExactElement) {
        foreground = T3HomeChrome.color(element.data[.mobileHomeForeground] ?? "")
        primary = T3HomeChrome.color(element.data[.mobileHomePrimary] ?? "")
        danger = T3HomeChrome.color(element.data[.mobileHomeDanger] ?? "")
        background = T3HomeChrome.color(element.data[.mobileHomeBackground] ?? "")
    }
}
private struct T3CustomSnoozeContent: View {
    @ObservedObject var model: T3CustomSnoozeDraft
    let colors: T3CustomSnoozeColors
    var body: some View {
        ScrollView {
            VStack(spacing: 16) {
                Picker("Snooze mode", selection: $model.mode) { Text("Date and time").tag("date"); Text("Duration").tag("duration") }
                    .pickerStyle(.segmented).accessibilityIdentifier("home-snooze-mode")
                if model.mode == "date" {
                    DatePicker("Snooze until", selection: $model.date, displayedComponents: [.date, .hourAndMinute])
                        .datePickerStyle(.wheel).labelsHidden().frame(maxWidth: .infinity).frame(height: 180).accessibilityIdentifier("home-snooze-date")
                } else {
                    HStack(spacing: 0) {
                        Picker("Duration amount", selection: $model.amount) { ForEach(1...99, id: \.self) { Text(String($0)).tag($0) } }
                            .pickerStyle(.wheel).labelsHidden().frame(minWidth: 0, maxWidth: .infinity).frame(height: 180).clipped().accessibilityIdentifier("home-snooze-amount")
                        Picker("Duration unit", selection: $model.unit) { Text("Minutes").tag("minutes"); Text("Hours").tag("hours"); Text("Days").tag("days") }
                            .pickerStyle(.wheel).labelsHidden().frame(minWidth: 0, maxWidth: .infinity).frame(height: 180).clipped().accessibilityIdentifier("home-snooze-unit")
                    }
                }
                if let error = model.error { Text(error).font(.footnote).foregroundStyle(Color(uiColor: colors.danger)).accessibilityIdentifier("home-snooze-error") }
            }.padding(16).foregroundStyle(Color(uiColor: colors.foreground))
        }.scrollIndicators(.hidden).tint(Color(uiColor: colors.primary))
    }
}
private final class T3CustomSnoozeController: UIHostingController<T3CustomSnoozeContent> {
    var resized: (() -> Void)?
    override func viewDidLayoutSubviews() { super.viewDidLayoutSubviews(); resized?() }
}
#endif
