// @exact/payments on Apple (LLP 1133.034 D5, D6, D10, D11): Apple Pay through
// `PKPaymentAuthorizationController`, one sheet at a time, presented over the
// session's window. The app's composed module calls it outside the agent and
// under the agent when bound (`--bind payments`); otherwise the runner's
// substitute answers and only the button is drawn here.
//
// - The request is the W3C call's words in PassKit's: the Apple Pay method's
//   `data` (`merchantIdentifier`, `merchantCapabilities`, `supportedNetworks`,
//   `countryCode`, the contact fields), the details as summary items (the
//   display items, then the total, whose label is the merchant's), the
//   shipping options as shipping methods, and the options as required
//   contact fields. The `paymentRequest` hatch sees it last.
// - The sheet's delegate calls the app's handlers through `invoke`, by the
//   call's `request:<n>` handle, each with D5's budget. An update past it
//   leaves the sheet as it was; an authorization past it fails.
// - The final handler's answer is D6's table: `success` → `.success`;
//   `fail` and `unknown` → `.failure`; `retry` → `.failure` with its errors,
//   and the sheet stays for another authorization.
// - A call let go is dismissed unless its final handler runs; its answer then
//   completes the sheet (D16 item 2).
// - A Mac build signed without a profile has no merchant entitlement, so
//   Apple Pay reads unavailable there.
import Contacts
import ExactPackage
import Foundation
import PassKit
#if os(macOS)
import AppKit
#else
import UIKit
#endif

private let applePay = "https://apple.com/apple-pay"
private let budget: TimeInterval = 25

/// `value` as a JSON object: an object, or JSON text of one (a Contract call's `json<…>`).
private func object(_ value: Any?) -> [String: Any]? {
    if let o = value as? [String: Any] { return o }
    guard let text = value as? String, let data = text.data(using: .utf8) else { return nil }
    return (try? JSONSerialization.jsonObject(with: data)) as? [String: Any]
}

private func decimal(_ amount: Any?) -> NSDecimalNumber {
    let value = (amount as? [String: Any])?["value"] as? String ?? "0"
    let n = NSDecimalNumber(string: value)
    return n == .notANumber ? .zero : n
}

private func items(_ list: Any?) -> [PKPaymentSummaryItem] {
    ((list as? [[String: Any]]) ?? []).map {
        let item = PKPaymentSummaryItem(label: $0["label"] as? String ?? "", amount: decimal($0["amount"]))
        if $0["pending"] as? Bool == true { item.type = .pending }
        return item
    }
}

private func methods(_ list: Any?) -> [PKShippingMethod] {
    ((list as? [[String: Any]]) ?? []).map {
        let m = PKShippingMethod(label: $0["label"] as? String ?? "", amount: decimal($0["amount"]))
        m.identifier = $0["id"] as? String
        return m
    }
}

private let networks: [String: PKPaymentNetwork] = [
    "amex": .amex, "discover": .discover, "masterCard": .masterCard, "visa": .visa, "jcb": .JCB,
    "chinaUnionPay": .chinaUnionPay, "interac": .interac, "maestro": .maestro, "eftpos": .eftpos,
    "electron": .electron, "elo": .elo, "mada": .mada, "privateLabel": .privateLabel, "vPay": .vPay,
]

private func capabilities(_ words: [String]) -> PKMerchantCapability {
    var c: PKMerchantCapability = []
    for w in words {
        switch w {
        case "supports3DS": c.insert(.threeDSecure)
        case "supportsEMV": c.insert(.emv)
        case "supportsCredit": c.insert(.credit)
        case "supportsDebit": c.insert(.debit)
        default: break
        }
    }
    return c.isEmpty ? .threeDSecure : c
}

private func contactFields(_ words: Any?) -> Set<PKContactField> {
    var out = Set<PKContactField>()
    for w in (words as? [String]) ?? [] {
        switch w {
        case "postalAddress": out.insert(.postalAddress)
        case "name": out.insert(.name)
        case "email": out.insert(.emailAddress)
        case "phone": out.insert(.phoneNumber)
        case "phoneticName": out.insert(.phoneticName)
        default: break
        }
    }
    return out
}

/// A contact's address as the W3C's `ContactAddress` (`toJSON()`).
private func address(_ contact: PKContact?) -> [String: Any]? {
    guard let a = contact?.postalAddress else { return nil }
    var out: [String: Any] = [
        "country": a.isoCountryCode.uppercased(), "region": a.state, "city": a.city,
        "dependentLocality": a.subLocality, "postalCode": a.postalCode, "sortingCode": "",
        "addressLine": a.street.split(separator: "\n").map(String.init), "organization": "",
    ]
    if let name = contact?.name { out["recipient"] = PersonNameComponentsFormatter().string(from: name) }
    if let phone = contact?.phoneNumber?.stringValue { out["phone"] = phone }
    return out
}

private func methodJSON(_ m: PKPaymentMethod) -> [String: Any] {
    let type: String
    switch m.type {
    case .debit: type = "debit"
    case .credit: type = "credit"
    case .prepaid: type = "prepaid"
    case .store: type = "store"
    case .eMoney: type = "eMoney"
    default: type = "unknown"
    }
    return ["displayName": m.displayName ?? "", "network": m.network?.rawValue ?? "", "type": type]
}

/// The W3C's errors as PassKit's (`retry`, `shippingAddressErrors`).
private func errors(_ by: [String: Any]?) -> [Error] {
    var out: [Error] = []
    let keys: [String: String] = [
        "addressLine": CNPostalAddressStreetKey, "city": CNPostalAddressCityKey, "country": CNPostalAddressISOCountryCodeKey,
        "postalCode": CNPostalAddressPostalCodeKey, "region": CNPostalAddressStateKey, "dependentLocality": CNPostalAddressSubLocalityKey,
    ]
    for (field, text) in (by?["shippingAddress"] as? [String: Any]) ?? [:] {
        guard let text = text as? String, let key = keys[field] else { continue }
        out.append(PKPaymentRequest.paymentShippingAddressInvalidError(withKey: key, localizedDescription: text))
    }
    for (field, text) in (by?["payer"] as? [String: Any]) ?? [:] {
        guard let text = text as? String else { continue }
        let f: PKContactField = field == "email" ? .emailAddress : field == "phone" ? .phoneNumber : .name
        out.append(PKPaymentRequest.paymentContactInvalidError(withContactField: f, localizedDescription: text))
    }
    return out
}

public final class PaymentsModule: NSObject, ExactPackage, PKPaymentAuthorizationControllerDelegate {
    /// The app's access hatch (D11): each request, after the module's words are set.
    public static var paymentRequest: ((PKPaymentRequest) -> Void)?

    private let context: ExactPackageContext

    /// The sheet up: its call's number, handlers, request and result.
    private final class Sheet {
        let request: Int, id: String, args: [String: Any], controller: PKPaymentAuthorizationController
        let reply: ([String: Any]) -> Void
        var authorizing = false, letGo = false
        var result: [String: Any]?
        var payment: PKPayment?
        init(request: Int, id: String, args: [String: Any], controller: PKPaymentAuthorizationController, reply: @escaping ([String: Any]) -> Void) {
            self.request = request; self.id = id; self.args = args; self.controller = controller; self.reply = reply
        }
    }
    private var sheet: Sheet?

    public init(context: ExactPackageContext) { self.context = context }

    public func later(_ name: String, _ args: [String: Any], preferred: Any?, reply: @escaping ([String: Any]) -> Void) {
        switch name {
        case "paymentRequestStatus": reply(status(args))
        case "showPaymentRequest": show(args, reply)
        default: reply(["error": "NotSupportedError"])
        }
    }

    /// The Apple Pay method's `data`, from a call's method data.
    private func appleData(_ args: [String: Any]) -> [String: Any]? {
        let list = (args["methodData"] as? [[String: Any]]) ?? []
        return list.first { $0["supportedMethods"] as? String == applePay }.map { object($0["data"]) ?? [:] }
    }

    /// Whether this build was signed with a merchant ID: a Mac app reads its own entitlement.
    private var entitled: Bool {
        #if os(macOS)
        guard let task = SecTaskCreateFromSelf(nil) else { return false }
        return SecTaskCopyValueForEntitlement(task, "com.apple.developer.in-app-payments" as CFString, nil) != nil
        #else
        return true
        #endif
    }

    private func status(_ args: [String: Any]) -> [String: Any] {
        let available = entitled && PKPaymentAuthorizationController.canMakePayments()
        guard available, let data = appleData(args) else {
            return ["available": available, "canMakePayment": false, "hasEnrolledInstrument": false]
        }
        let nets = ((data["supportedNetworks"] as? [String]) ?? []).compactMap { networks[$0] }
        let enrolled = PKPaymentAuthorizationController.canMakePayments(
            usingNetworks: nets, capabilities: capabilities((data["merchantCapabilities"] as? [String]) ?? []))
        return ["available": true, "canMakePayment": true, "hasEnrolledInstrument": enrolled, "methodName": applePay]
    }

    private func show(_ args: [String: Any], _ reply: @escaping ([String: Any]) -> Void) {
        guard context.origin == "app" else { return reply(["error": "NotAllowedError", "message": "person-required"]) }
        guard sheet == nil else { return reply(["error": "AbortError", "message": "a payment sheet is up"]) }
        guard entitled, let data = appleData(args), let merchant = data["merchantIdentifier"] as? String else {
            return reply(["error": "NotSupportedError"])
        }
        let details = (args["details"] as? [String: Any]) ?? [:], options = (args["options"] as? [String: Any]) ?? [:]
        let total = (details["total"] as? [String: Any]) ?? [:]
        let request = PKPaymentRequest()
        request.merchantIdentifier = merchant
        request.countryCode = data["countryCode"] as? String ?? "US"
        request.currencyCode = (total["amount"] as? [String: Any])?["currency"] as? String ?? "USD"
        request.supportedNetworks = ((data["supportedNetworks"] as? [String]) ?? []).compactMap { networks[$0] }
        request.merchantCapabilities = capabilities((data["merchantCapabilities"] as? [String]) ?? [])
        request.paymentSummaryItems = summary(details)
        request.shippingMethods = methods(details["shippingOptions"])
        var shipping = contactFields(data["requiredShippingContactFields"])
        if options["requestShipping"] as? Bool == true { shipping.insert(.postalAddress) }
        if options["requestPayerName"] as? Bool == true { shipping.insert(.name) }
        if options["requestPayerEmail"] as? Bool == true { shipping.insert(.emailAddress) }
        if options["requestPayerPhone"] as? Bool == true { shipping.insert(.phoneNumber) }
        request.requiredShippingContactFields = shipping
        var billing = contactFields(data["requiredBillingContactFields"])
        if options["requestBillingAddress"] as? Bool == true { billing.insert(.postalAddress) }
        request.requiredBillingContactFields = billing
        switch options["shippingType"] as? String {
        case "delivery": request.shippingType = .delivery
        case "pickup": request.shippingType = .storePickup
        default: break
        }
        PaymentsModule.paymentRequest?(request)
        let controller = PKPaymentAuthorizationController(paymentRequest: request)
        controller.delegate = self
        let id = details["id"] as? String ?? UUID().uuidString
        let s = Sheet(request: context.request ?? 0, id: id, args: args, controller: controller, reply: reply)
        sheet = s
        context.log("payments: \(id): shown, \(applePay)")
        controller.present { [weak self] presented in
            guard !presented, let self, self.sheet === s else { return }
            self.sheet = nil
            reply(["error": "NotSupportedError", "message": "Apple Pay did not present its sheet"])
        }
    }

    /// The details as summary items: the display items, then the total, whose label is the merchant's.
    private func summary(_ details: [String: Any]) -> [PKPaymentSummaryItem] {
        let total = (details["total"] as? [String: Any]) ?? [:]
        let last = PKPaymentSummaryItem(label: total["label"] as? String ?? "", amount: decimal(total["amount"]))
        if total["pending"] as? Bool == true { last.type = .pending }
        return items(details["displayItems"]) + [last]
    }

    /// The app's handler for the sheet up, its answer parsed; nil past the budget or on an error.
    private func ask(_ s: Sheet, _ keyword: String, _ arg: [String: Any], _ done: @escaping ([String: Any]?) -> Void) {
        guard let handler = s.args[keyword] as? String else { return done(nil) }
        var answered = false
        let finish: ([String: Any]?) -> Void = { value in
            guard !answered else { return }
            answered = true
            done(value)
        }
        DispatchQueue.main.asyncAfter(deadline: .now() + budget) { [weak self] in
            guard !answered else { return }
            self?.context.log("payments: \(handler) for \(s.id): past its budget")
            finish(nil)
        }
        context.invoke("request:\(s.request)", handler, arg) { answer in
            guard let text = answer["text"] as? String, let data = text.data(using: .utf8),
                  let value = (try? JSONSerialization.jsonObject(with: data)) as? [String: Any] else { return finish(nil) }
            finish(value)
        }
    }

    /// An update's summary items and shipping methods, laid over the request as it was.
    private func updated(_ s: Sheet, _ update: [String: Any]?) -> (items: [PKPaymentSummaryItem], methods: [PKShippingMethod]?, errors: [Error]) {
        var details = (s.args["details"] as? [String: Any]) ?? [:]
        for key in ["total", "displayItems", "shippingOptions"] where update?[key] != nil { details[key] = update?[key] }
        var all = errors(["shippingAddress": update?["shippingAddressErrors"] as Any, "payer": update?["payerErrors"] as Any])
        if let text = update?["error"] as? String, all.isEmpty {
            all.append(PKPaymentRequest.paymentShippingAddressUnserviceableError(withLocalizedDescription: text))
        }
        let shipping = update?["shippingOptions"] != nil ? methods(details["shippingOptions"]) : nil
        return (summary(details), shipping, all)
    }

    public func paymentAuthorizationController(_ controller: PKPaymentAuthorizationController, didSelectShippingContact contact: PKContact,
                                               handler completion: @escaping (PKPaymentRequestShippingContactUpdate) -> Void) {
        guard let s = sheet, !s.letGo else { return completion(PKPaymentRequestShippingContactUpdate(paymentSummaryItems: [])) }
        // Apple redacts the contact before authorization, as the W3C's rule does.
        ask(s, "onshippingaddresschange", ["requestId": s.id, "shippingAddress": address(contact) as Any, "methodName": applePay]) { [weak self] value in
            guard let self else { return }
            let u = self.updated(s, value)
            completion(PKPaymentRequestShippingContactUpdate(errors: u.errors, paymentSummaryItems: u.items, shippingMethods: u.methods ?? []))
        }
    }

    public func paymentAuthorizationController(_ controller: PKPaymentAuthorizationController, didSelectShippingMethod method: PKShippingMethod,
                                               handler completion: @escaping (PKPaymentRequestShippingMethodUpdate) -> Void) {
        guard let s = sheet, !s.letGo else { return completion(PKPaymentRequestShippingMethodUpdate(paymentSummaryItems: [])) }
        ask(s, "onshippingoptionchange", ["requestId": s.id, "shippingOption": method.identifier as Any, "methodName": applePay]) { [weak self] value in
            guard let self else { return }
            completion(PKPaymentRequestShippingMethodUpdate(paymentSummaryItems: self.updated(s, value).items))
        }
    }

    public func paymentAuthorizationController(_ controller: PKPaymentAuthorizationController, didSelectPaymentMethod method: PKPaymentMethod,
                                               handler completion: @escaping (PKPaymentRequestPaymentMethodUpdate) -> Void) {
        guard let s = sheet, !s.letGo, s.args["onpaymentmethodchange"] is String else {
            return completion(PKPaymentRequestPaymentMethodUpdate(paymentSummaryItems: summary((sheet?.args["details"] as? [String: Any]) ?? [:])))
        }
        ask(s, "onpaymentmethodchange", ["requestId": s.id, "methodName": applePay, "methodDetails": methodJSON(method)]) { [weak self] value in
            guard let self else { return }
            completion(PKPaymentRequestPaymentMethodUpdate(paymentSummaryItems: self.updated(s, value).items))
        }
    }

    public func paymentAuthorizationController(_ controller: PKPaymentAuthorizationController, didAuthorizePayment payment: PKPayment,
                                               handler completion: @escaping (PKPaymentAuthorizationResult) -> Void) {
        guard let s = sheet, !s.letGo else { return completion(PKPaymentAuthorizationResult(status: .failure, errors: nil)) }
        s.authorizing = true
        s.payment = payment
        let token = payment.token
        let paymentData = (try? JSONSerialization.jsonObject(with: token.paymentData)) ?? [:]
        let response: [String: Any] = [
            "requestId": s.id, "methodName": applePay,
            "details": ["token": ["paymentData": paymentData, "paymentMethod": methodJSON(token.paymentMethod), "transactionIdentifier": token.transactionIdentifier]],
            "shippingAddress": address(payment.shippingContact) as Any, "shippingOption": payment.shippingMethod?.identifier as Any,
            "payerName": payment.shippingContact?.name.map { PersonNameComponentsFormatter().string(from: $0) } as Any,
            "payerEmail": payment.shippingContact?.emailAddress as Any, "payerPhone": payment.shippingContact?.phoneNumber?.stringValue as Any,
        ]
        ask(s, "onpaymentauthorized", response) { [weak self] value in
            s.authorizing = false
            if let retry = value?["retry"] as? [String: Any] {
                return completion(PKPaymentAuthorizationResult(status: .failure, errors: errors(retry)))
            }
            let result = (value?["result"] as? String).flatMap { ["success", "fail", "unknown"].contains($0) ? $0 : nil } ?? "fail"
            var out: [String: Any] = [
                "requestId": s.id, "methodName": applePay, "result": result,
                "shippingAddress": address(payment.shippingContact) as Any, "shippingOption": payment.shippingMethod?.identifier as Any,
                "payerName": response["payerName"] as Any, "payerEmail": response["payerEmail"] as Any, "payerPhone": response["payerPhone"] as Any,
                "apple": ["paymentMethod": methodJSON(token.paymentMethod)],
            ]
            if value == nil { out["message"] = "authorize-timeout" }
            if let data = value?["data"], let text = try? JSONSerialization.data(withJSONObject: data) { out["data"] = String(decoding: text, as: UTF8.self) }
            s.result = out
            self?.context.log("payments: \(s.id): \(result)")
            completion(PKPaymentAuthorizationResult(status: result == "success" ? .success : .failure, errors: nil))
        }
    }

    public func paymentAuthorizationControllerDidFinish(_ controller: PKPaymentAuthorizationController) {
        controller.dismiss()
        guard let s = sheet, s.controller === controller else { return }
        sheet = nil
        if s.letGo { return context.log("payment: closed after its call ended (\(s.id), last result \(s.result?["result"] as? String ?? "none"))") }
        s.reply(s.result ?? ["error": "AbortError"])
    }

    #if os(macOS)
    public func presentationWindow(for controller: PKPaymentAuthorizationController) -> NSWindow? { context.presentation?.window }
    #else
    public func presentationWindow(for controller: PKPaymentAuthorizationController) -> UIWindow? { context.presentation?.window }
    #endif

    /// The call was let go: the sheet is dismissed unless its final handler runs (D6 phase 2).
    public func cancel(_ request: Int) {
        guard let s = sheet, s.request == request else { return }
        s.letGo = true
        if !s.authorizing { s.controller.dismiss() }
    }

    public func destroy(_ done: @escaping () -> Void) {
        guard let s = sheet else { return done() }
        s.letGo = true
        s.controller.dismiss { done() }
    }

    public func view(_ tag: String, id: String, props: [String: String], events: ExactPackageEvents) throws -> ExactPackageView {
        guard tag == "exact-payment-button" else { throw ExactPackageRefusal("PaymentsModule serves no view \(tag)") }
        return PaymentButtonView(props: props, events: events)
    }
}
