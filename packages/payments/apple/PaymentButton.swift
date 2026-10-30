// `exact-payment-button` on Apple (LLP 1133.034 D4, D8): PassKit's own
// `PKPaymentButton`, whose look Apple's guidelines fix ("Apple Pay buttons":
// the type's words, the system's style for the appearance). A press is the
// view's `press` event, whose action makes the call.
import ExactPackage
import Foundation
import PassKit
#if os(macOS)
import AppKit
#else
import UIKit
#endif

private let types: [String: PKPaymentButtonType] = [
    "plain": .plain, "buy": .buy, "checkout": .checkout, "order": .order, "book": .book,
    "donate": .donate, "subscribe": .subscribe, "pay": .plain,
]

final class PaymentButtonView: NSObject, ExactPackageView {
    let view: ExactPackagePlatformView
    private let events: ExactPackageEvents
    private var button: PKPaymentButton?
    private var type = ""

    init(props: [String: String], events: ExactPackageEvents) {
        self.events = events
        #if os(macOS)
        view = NSView()
        #else
        view = UIView()
        #endif
        super.init()
        try? setProps(props)
    }

    func setProps(_ props: [String: String]) throws {
        let word = props["buttonType"] ?? "plain"
        guard let kind = types[word] else { throw ExactPackageRefusal("exact-payment-button: no buttonType \(word)") }
        let disabled = props["disabled"] == "true"
        if word != type { make(kind); type = word }
        button?.isEnabled = !disabled
    }

    /// A button of `kind`: PassKit fixes a button's type when it is made.
    private func make(_ kind: PKPaymentButtonType) {
        button?.removeFromSuperview()
        let b = PKPaymentButton(paymentButtonType: kind, paymentButtonStyle: .automatic)
        #if os(macOS)
        b.target = self
        b.action = #selector(pressed)
        #else
        b.addTarget(self, action: #selector(pressed), for: .touchUpInside)
        #endif
        b.translatesAutoresizingMaskIntoConstraints = false
        view.addSubview(b)
        NSLayoutConstraint.activate([
            b.leadingAnchor.constraint(equalTo: view.leadingAnchor), b.trailingAnchor.constraint(equalTo: view.trailingAnchor),
            b.topAnchor.constraint(equalTo: view.topAnchor), b.bottomAnchor.constraint(equalTo: view.bottomAnchor),
        ])
        button = b
    }

    @objc private func pressed() { events.press() }

    func destroy() { button?.removeFromSuperview() }
}
