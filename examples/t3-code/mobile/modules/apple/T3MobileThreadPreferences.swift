// @ref llp/1107.009-mobile-settings.decision.md#thread-behavior-and-project-overview
// T3 Code 365aa87982: features/settings/components/AutoSettleDaysField.ios.tsx.
// App-owned popover and SwiftUI wheel. No server or preference owner lives here.
#if os(iOS)
import UIKit
import SwiftUI

final class T3MobileThreadPreferences: ExactNativeInstance {
    static let factory = ExactNativeFactory { props, events in
        let instance = T3MobileThreadPreferences(events: events)
        try instance.setProps(props)
        return instance
    }
    private let button = UIButton(type: .system)
    private var scope = ""
    private var amount = 3
    private var appearance: [String: Any] = [:]
    private var appearanceJSON = ""
    private var active = true
    private var openID: UUID?
    private var presented: UIViewController?
    private var dismissal: T3DaysDismissal?
    override var view: UIView { button }
    override var focusTarget: UIView? { button }
    override init(events: ExactNativeEvents) {
        super.init(events: events)
        button.addAction(UIAction { [weak self] _ in self?.open() }, for: .touchUpInside)
    }
    override func setProps(_ props: [String: String]) throws {
        guard let value = Int(props["days-value"] ?? ""), (1...90).contains(value),
              let nextScope = props["days-scope"], !nextScope.isEmpty,
              let json = props["days-appearance"], let bytes = json.data(using: .utf8),
              let nextAppearance = try? JSONSerialization.jsonObject(with: bytes) as? [String: Any] else {
            throw ExactNativeRefusal("The day picker requires a scope, a value from 1 to 90, and appearance preferences.")
        }
        let enabled = props["days-enabled"] == "true"
        if scope != nextScope || amount != value || !enabled || appearanceJSON != json { close() }
        scope = nextScope; amount = value; appearance = nextAppearance; appearanceJSON = json
        let mode = appearance["themeMode"] as? String
        button.overrideUserInterfaceStyle = mode == "dark" ? .dark : mode == "light" ? .light : .unspecified
        let palette = colors()
        var config = UIButton.Configuration.tinted()
        config.title = String(value)
        config.baseForegroundColor = palette.primary
        config.baseBackgroundColor = palette.primary.withAlphaComponent(0.12)
        config.cornerStyle = .small
        let size = (appearance["baseFontSize"] as? NSNumber)?.doubleValue ?? 16
        config.titleTextAttributesTransformer = UIConfigurationTextAttributesTransformer { incoming in
            var attributes = incoming; attributes.font = UIFont.systemFont(ofSize: size); return attributes
        }
        button.configuration = config; button.isEnabled = enabled
        button.accessibilityLabel = "Days before auto-settle: \(value)"
    }
    private func colors() -> T3DaysColors {
        // Dynamic providers resolve after attachment and during an open popover.
        // Capture immutable palette values, never the native instance or its events.
        let light = appearance["light"] as? [String: String] ?? [:]
        let dark = appearance["dark"] as? [String: String] ?? [:]
        func role(_ key: String) -> UIColor {
            UIColor { traits in
                T3DaysColors.color((traits.userInterfaceStyle == .dark ? dark : light)[key])
            }
        }
        return T3DaysColors(primary: role("primary"), foreground: role("foreground"), sheet: role("sheet"))
    }
    private func open() {
        guard active, button.isEnabled, button.window != nil, presented == nil else { return }
        var responder: UIResponder? = button
        while responder != nil && !(responder is UIViewController) { responder = responder?.next }
        guard let parent = responder as? UIViewController, parent.presentedViewController == nil,
              !parent.isBeingDismissed, !parent.isBeingPresented else { return }
        let id = UUID(), capturedScope = scope, initial = amount
        openID = id
        let controller = UIHostingController(rootView: T3DaysContent(initial: initial, colors: colors()) { [weak self] chosen in
            guard let self, self.active, self.openID == id else { return }
            let valid = self.button.isEnabled && self.scope == capturedScope && self.amount == initial
            self.close()
            guard valid, let chosen, chosen != initial,
                  let bytes = try? JSONSerialization.data(withJSONObject: ["scope": capturedScope, "initial": initial, "raw": String(chosen)]),
                  let event = String(data: bytes, encoding: .utf8) else { return }
            self.events.change(event)
        })
        controller.overrideUserInterfaceStyle = button.overrideUserInterfaceStyle
        controller.view.backgroundColor = colors().sheet
        controller.preferredContentSize = CGSize(width: 240, height: 240)
        controller.modalPresentationStyle = .popover
        let delegate = T3DaysDismissal { [weak self] in
            guard let self, self.openID == id else { return }; self.close()
        }
        dismissal = delegate
        controller.presentationController?.delegate = delegate
        if let popover = controller.popoverPresentationController {
            popover.delegate = delegate
            popover.sourceView = button; popover.sourceRect = button.bounds
            popover.permittedArrowDirections = .any
        }
        // UIKit supplies compact adaptation; its exact sizing awaits a native parity drive.
        presented = controller
        parent.present(controller, animated: true)
    }
    private func close() {
        openID = nil
        let old = presented; presented = nil
        old?.presentationController?.delegate = nil
        old?.popoverPresentationController?.delegate = nil
        dismissal = nil
        old?.dismiss(animated: true)
    }
    override func destroy() {
        active = false; close()
        button.isEnabled = false
    }
}

private final class T3DaysDismissal: NSObject, UIPopoverPresentationControllerDelegate {
    let dismissed: () -> Void
    init(_ dismissed: @escaping () -> Void) { self.dismissed = dismissed }
    func presentationControllerDidDismiss(_ presentationController: UIPresentationController) { dismissed() }
    func popoverPresentationControllerDidDismissPopover(_ popoverPresentationController: UIPopoverPresentationController) { dismissed() }
}
private struct T3DaysContent: View {
    @State private var draft: Int
    let colors: T3DaysColors
    let finish: (Int?) -> Void
    init(initial: Int, colors: T3DaysColors, finish: @escaping (Int?) -> Void) {
        _draft = State(initialValue: initial); self.colors = colors; self.finish = finish
    }
    var body: some View {
        VStack {
            Picker("Days before auto-settle", selection: $draft) {
                ForEach(1...90, id: \.self) { value in
                    Text("\(value) \(value == 1 ? "day" : "days")")
                        .foregroundStyle(Color(uiColor: colors.foreground)).tag(value)
                }
            }.pickerStyle(.wheel).frame(height: 180)
            HStack(spacing: 24) {
                Button("Cancel") { finish(nil) }
                Button("Done") { finish(draft) }
            }.foregroundStyle(Color(uiColor: colors.primary))
        }.padding(12).frame(width: 240).presentationBackground(Color(uiColor: colors.sheet))
    }
}
private struct T3DaysColors {
    let primary: UIColor
    let foreground: UIColor
    let sheet: UIColor
    static func color(_ value: String?) -> UIColor {
        let text = (value ?? "").trimmingCharacters(in: .whitespacesAndNewlines)
        if text.hasPrefix("#"), let hex = UInt64(text.dropFirst(), radix: 16) {
            let rgb = text.count == 9 ? hex >> 8 : hex
            return UIColor(red: CGFloat((rgb >> 16) & 255) / 255, green: CGFloat((rgb >> 8) & 255) / 255,
                           blue: CGFloat(rgb & 255) / 255, alpha: text.count == 9 ? CGFloat(hex & 255) / 255 : 1)
        }
        if text.hasPrefix("rgb"), let start = text.firstIndex(of: "("), let end = text.lastIndex(of: ")") {
            let numbers = text[text.index(after: start)..<end].split(separator: ",").compactMap { Double($0.trimmingCharacters(in: .whitespaces)) }
            if numbers.count >= 3 { return UIColor(red: numbers[0] / 255, green: numbers[1] / 255, blue: numbers[2] / 255, alpha: numbers.count > 3 ? numbers[3] : 1) }
        }
        return .label
    }
}
#endif
