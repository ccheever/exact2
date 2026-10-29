// @exact/purchases on Apple (LLP 1133.015): StoreKit 2, answered in the
// Digital Goods API's shapes. Under the agent the implementation is its own
// substitute (§9): it reaches no store and keeps a synthetic one
// (AgentStore.swift); `--bind purchases` reaches StoreKit, which in a
// development build is Xcode's local environment or the sandbox.
//
// - Made when the module loads (`"start": "load"`, D6), not at its first
//   call: the first session starts the process's listener over
//   `Transaction.updates`, `Status.updates` and `Storefront.updates`, then
//   sweeps `Transaction.unfinished`. A call that arrives before the sweep
//   has finished waits for it. The last session's end stops the listener.
// - Two facts (D3): `listPurchases` is `currentEntitlements` with the
//   unfinished consumables, `unfinishedPurchases` is `Transaction.unfinished`,
//   both verified only. Each store change, return to the foreground and
//   finished call re-answers both in every session.
// - `listPurchases` names its earliest future `expirationTime` as its stale
//   time (D7), the grace period's end where a subscription is in one.
// - Nothing is finished but by the app (`acknowledge`, `consume`), except a
//   revoked transaction left unfinished, which is finished and journaled (D5).
// - Tier 3 (D11): `PurchasesModule.hatch` adds options to each purchase's
//   set after the module's own.
import ExactPackage
import Foundation
import StoreKit
#if os(macOS)
import AppKit
#else
import UIKit
#endif

public final class PurchasesModule: ExactPackage {
    /// The app's access hatch (D11): each purchase's option set, after the
    /// module's words (`appAccountToken`, `quantity`) are in it.
    public static var hatch: ((Product, inout Set<Product.PurchaseOption>) -> Void)?

    let context: ExactPackageContext
    /// Whether the session's first `unfinishedPurchases` answer has gone,
    /// which starts the minute after which a forgotten row is journaled.
    private var watched = false

    public init(context: ExactPackageContext) {
        self.context = context
        Shared.join(self)
    }

    var agent: Bool { context.agent }
    private func say(_ line: String) { context.log("purchases: \(line)") }
    var unixNow: Double { agent ? AgentStore.unix(context.now()) : Date().timeIntervalSince1970 * 1000 }

    public func later(_ name: String, _ args: [String: Any], preferred: Any?, reply: @escaping ([String: Any]) -> Void) {
        if agent { return agentLater(name, args, preferred as? [String: Any], reply) }
        Shared.afterSweep { [self] in
            Task { @MainActor in reply(await realLater(name, args)) }
        }
    }

    public func call(_ name: String, _ args: [String: Any]) throws -> [String: Any] {
        name == "purchasesStatusSync" ? status() : ["error": "NotSupportedError"]
    }

    public func preferred(_ values: [String: Any]) {
        guard agent, let store = Shared.agentStore(context) else { return }
        store.catalog = values["getDetails"] as? [[String: Any]] ?? values["catalog"] as? [[String: Any]] ?? []
        Shared.fault = values["fault"] as? String
        if let event = values["storeEvent"] as? [String: Any], store.apply(event, at: unixNow) {
            say("store: \(store.disclosed(at: unixNow))")
            Shared.changed()
        }
    }

    /// Apple asks for both reads again at a return to the foreground (D3):
    /// the account may have changed.
    public func foreground(_ shown: Bool) {
        if shown { Shared.changed() }
    }

    public func destroy(_ done: @escaping () -> Void) {
        Shared.leave(self)
        done()
    }

    // MARK: the status

    func status() -> [String: Any] {
        if agent {
            return ["available": true, "store": "app-store", "canMakePayments": true, "storefront": "US",
                    "apple": ["storefront": "USA", "unverified": [String]()]]
        }
        // Apple's storefront is an ISO 3166-1 alpha-3 code, which `apple.storefront` carries.
        return ["available": true, "store": "app-store", "canMakePayments": AppStore.canMakePayments, "storefront": NSNull(),
                "apple": ["storefront": Shared.storefront ?? NSNull(), "unverified": Shared.unverified.sorted()]]
    }

    // MARK: under the agent (§9)

    private func agentLater(_ name: String, _ args: [String: Any], _ sheet: [String: Any]?, _ reply: @escaping ([String: Any]) -> Void) {
        guard let store = Shared.agentStore(context) else { return reply(["error": "OperationError", "message": "no scratch tree for the synthetic store"]) }
        let now = unixNow
        switch name {
        case "purchasesStatus": reply(status())
        case "getDetails":
            let ids = args["itemIds"] as? [String] ?? []
            reply(["exact:answer": ids.compactMap { store.item($0).map(Self.details) }])
        case "listPurchases":
            var out: [String: Any] = ["exact:answer": store.entitlements(at: now).map { agentDetails($0, store) }]
            if let stale = store.nextExpiry(after: now) { out["exact:stale"] = stale }
            reply(out)
        case "unfinishedPurchases":
            reply(["exact:answer": store.unfinished().map { agentDetails($0, store) }])
            watchForgotten()
        case "purchase":
            if let refused = Self.refuseAccount(args) { return reply(refused) }
            let itemId = args["itemId"] as? String ?? "", account = args["accountToken"] as? String
            guard store.item(itemId) != nil else { return reply(["error": "ITEM_UNAVAILABLE", "message": "\(itemId) is not in the catalogue"]) }
            switch sheet?["tap"] as? String {
            case "buy":
                guard let t = store.buy(itemId, at: now, account: account) else { return reply(["error": "ITEM_UNAVAILABLE"]) }
                say("store: \(store.disclosed(at: now))")
                reply(agentDetails(t, store))
                Shared.changed()
            case "pending":
                let ask = store.ask(itemId, account: account)
                reply(["itemId": ask.itemId, "purchaseToken": "", "purchaseState": "pending", "purchaseTime": now, "acknowledged": false,
                       "willAutoRenew": NSNull(), "quantity": 1, "expirationTime": NSNull(), "apple": NSNull(), "android": NSNull()])
            case "unavailable": reply(["error": "ITEM_UNAVAILABLE"])
            case "fail": reply(["error": "NETWORK"])
            default: reply(["error": "AbortError"])
            }
        case "restorePurchases":
            guard sheet?["tap"] as? String == "allow" else { return reply(["error": "AbortError"]) }
            reply(["exact:answer": store.entitlements(at: now).map { agentDetails($0, store) }])
            Shared.changed()
        case "acknowledge", "consume":
            let token = args["purchaseToken"] as? String ?? ""
            // The drive's fault point (`prefer extension purchases.fault="finish"`): the process dies before the
            // store hears the finish, so the row stays in the queue for the next launch (§10, drives 7 to 9).
            if Shared.fault == "finish" {
                say("\(name) \(token): refused at the drive's fault point; it stays unfinished")
                return reply(["error": "OperationError", "message": "fault point"])
            }
            guard store.finish(token) else { return reply(["error": "NOT_OWNED", "message": "this account holds no \(token)"]) }
            say("store: \(store.disclosed(at: now))")
            reply([:])
            Shared.changed()
        default: reply(["error": "NotSupportedError"])
        }
    }

    private func agentDetails(_ t: AgentStore.Transaction, _ store: AgentStore) -> [String: Any] {
        let subscription = t.kind == "autoRenewable"
        return ["itemId": t.itemId, "purchaseToken": t.token, "purchaseState": "purchased", "purchaseTime": t.purchaseTime,
                "acknowledged": t.acknowledged, "willAutoRenew": subscription ? true : NSNull(), "quantity": 1,
                "expirationTime": t.expirationTime ?? NSNull(), "android": NSNull(),
                "apple": ["jwsRepresentation": NSNull(), "originalID": t.originalID, "ownershipType": "purchased", "environment": "agent",
                          "revocationDate": t.revoked ?? NSNull(), "isUpgraded": t.isUpgraded, "appAccountToken": t.appAccountToken ?? NSNull()]
                    as [String: Any]]
    }

    /// A catalogue item as `ItemDetails`, its absent fields the platform's.
    static func details(_ item: [String: Any]) -> [String: Any] {
        var out = item
        out["description"] = item["description"] ?? ""
        out["iconURLs"] = item["iconURLs"] ?? [String]()
        out["familyShareable"] = item["familyShareable"] ?? false
        if out["displayPrice"] == nil, let price = item["price"] as? [String: Any] {
            out["displayPrice"] = "\(price["currency"] as? String ?? "") \(price["value"] as? String ?? "")"
        }
        for key in ["subscriptionPeriod", "freeTrialPeriod", "introductoryPricePeriod", "introductoryPrice", "introductoryPriceCycles", "android"]
            where out[key] == nil { out[key] = NSNull() }
        var apple = item["apple"] as? [String: Any] ?? [:]
        apple["type"] = apple["type"] ?? (item["type"] as? String == "subscription" ? "autoRenewable" : "nonConsumable")
        apple["subscriptionGroupID"] = apple["subscriptionGroupID"] ?? item["group"] ?? NSNull()
        apple["isEligibleForIntroOffer"] = apple["isEligibleForIntroOffer"] ?? NSNull()
        out["apple"] = apple
        out["group"] = nil
        return out
    }

    /// `accountToken` is a UUID, which fits both stores' account fields (D8).
    static func refuseAccount(_ args: [String: Any]) -> [String: Any]? {
        guard let token = args["accountToken"] as? String, UUID(uuidString: token) == nil else { return nil }
        return ["error": "OperationError", "message": "accountToken-not-uuid: \(token) is not a UUID"]
    }

    /// A minute after the session's first queue answer, each purchased row
    /// still unfinished is journaled by token (D5): Apple delivers it again
    /// at every launch until the app finishes it.
    private func watchForgotten() {
        guard !watched else { return }
        watched = true
        context.after(60_000) { [weak self] in
            guard let self else { return }
            if self.agent {
                guard let store = Shared.agentStore(self.context) else { return }
                for t in store.unfinished() { self.say("\(t.token) (\(t.itemId)) is still unfinished a minute on: acknowledge or consume it") }
                return
            }
            Task { @MainActor in
                for await result in Transaction.unfinished {
                    if case .verified(let t) = result { self.say("\(t.id) (\(t.productID)) is still unfinished a minute on: acknowledge or consume it") }
                }
            }
        }
    }

    // MARK: StoreKit (stage 1)

    @MainActor
    private func realLater(_ name: String, _ args: [String: Any]) async -> [String: Any] {
        do {
            switch name {
            case "purchasesStatus": return status()
            case "getDetails":
                let ids = args["itemIds"] as? [String] ?? []
                let products = try await Product.products(for: ids)
                var out: [[String: Any]] = []
                for id in ids { if let p = products.first(where: { $0.id == id }) { out.append(await Self.details(p)) } }
                return ["exact:answer": out]
            case "listPurchases":
                let unfinished = await Shared.unfinishedIDs()
                var rows: [[String: Any]] = [], stale: Double?
                let now = Date().timeIntervalSince1970 * 1000
                for await result in Transaction.currentEntitlements {
                    guard case .verified(let t) = result, t.productType != .consumable else { continue }
                    let row = await Self.details(t, acknowledged: !unfinished.contains(t.id))
                    if let end = row["expirationTime"] as? Double, end > now { stale = min(stale ?? end, end) }
                    rows.append(row)
                }
                for await result in Transaction.unfinished {
                    guard case .verified(let t) = result, t.productType == .consumable, t.revocationDate == nil else { continue }
                    rows.append(await Self.details(t, acknowledged: false))
                }
                var out: [String: Any] = ["exact:answer": rows]
                if let stale { out["exact:stale"] = stale }
                return out
            case "unfinishedPurchases":
                var rows: [[String: Any]] = []
                for await result in Transaction.unfinished {
                    guard case .verified(let t) = result, t.revocationDate == nil else { continue }
                    rows.append(await Self.details(t, acknowledged: false))
                }
                watchForgotten()
                return ["exact:answer": rows]
            case "purchase": return try await purchase(args)
            case "restorePurchases":
                try await AppStore.sync()
                Shared.changed()
                var rows: [[String: Any]] = []
                let unfinished = await Shared.unfinishedIDs()
                for await result in Transaction.currentEntitlements {
                    if case .verified(let t) = result { rows.append(await Self.details(t, acknowledged: !unfinished.contains(t.id))) }
                }
                return ["exact:answer": rows]
            case "acknowledge", "consume": return await finish(args["purchaseToken"] as? String ?? "")
            default: return ["error": "NotSupportedError"]
            }
        } catch {
            return Self.failure(error)
        }
    }

    @MainActor
    private func purchase(_ args: [String: Any]) async throws -> [String: Any] {
        if let refused = Self.refuseAccount(args) { return refused }
        let itemId = args["itemId"] as? String ?? ""
        guard let product = try await Product.products(for: [itemId]).first else {
            return ["error": "ITEM_UNAVAILABLE", "message": "the store answers no \(itemId)"]
        }
        var options = Set<Product.PurchaseOption>()
        if let token = (args["accountToken"] as? String).flatMap(UUID.init(uuidString:)) { options.insert(.appAccountToken(token)) }
        if let quantity = args["quantity"] as? Int, quantity > 1 { options.insert(.quantity(quantity)) }
        if (args["apple"] as? [String: Any])?["simulatesAskToBuyInSandbox"] as? Bool == true { options.insert(.simulatesAskToBuyInSandbox(true)) }
        Self.hatch?(product, &options)
        guard let anchor = context.presentation else {
            return ["error": "NotAllowedError", "message": "no window of the session is in front"]
        }
        let result: Product.PurchaseResult
        #if os(macOS)
        if #available(macOS 15.2, *) {
            result = try await product.purchase(confirmIn: anchor.window, options: options)
        } else {
            result = try await product.purchase(options: options)
        }
        #else
        result = try await product.purchase(confirmIn: anchor.scene, options: options)
        #endif
        switch result {
        case .success(.verified(let t)):
            say("transaction \(t.id) for \(t.productID), unfinished")
            Shared.changed()
            return await Self.details(t, acknowledged: false)
        case .success(.unverified(let t, let why)):
            say("transaction \(t.id) failed verification (\(why)); left unfinished")
            Shared.unverified.insert(String(t.id))
            return ["error": "VERIFICATION_FAILED", "message": "\(why)"]
        case .pending:
            return ["itemId": itemId, "purchaseToken": "", "purchaseState": "pending", "purchaseTime": Date().timeIntervalSince1970 * 1000,
                    "acknowledged": false, "willAutoRenew": NSNull(), "quantity": args["quantity"] ?? 1, "expirationTime": NSNull(),
                    "apple": NSNull(), "android": NSNull()]
        case .userCancelled: return ["error": "AbortError"]
        @unknown default: return ["error": "OperationError"]
        }
    }

    /// StoreKit's `finish()`: of an unfinished transaction, else answered at
    /// once for one the account holds already finished, else `NOT_OWNED`.
    @MainActor
    private func finish(_ token: String) async -> [String: Any] {
        guard let id = UInt64(token) else { return ["error": "NOT_OWNED", "message": "\(token) is not a transaction id"] }
        for await result in Transaction.unfinished {
            guard case .verified(let t) = result, t.id == id else { continue }
            await t.finish()
            say("finished \(token)")
            Shared.changed()
            return [:]
        }
        for await result in Transaction.all {
            if case .verified(let t) = result, t.id == id { return [:] }
        }
        return ["error": "NOT_OWNED", "message": "this account holds no \(token)"]
    }

    /// StoreKit's errors in the interface's codes (D1).
    static func failure(_ error: Error) -> [String: Any] {
        let message = "\(error)"
        if let e = error as? StoreKitError {
            switch e {
            case .userCancelled: return ["error": "AbortError"]
            case .networkError: return ["error": "NETWORK", "message": message]
            case .notAvailableInStorefront: return ["error": "ITEM_UNAVAILABLE", "message": message]
            case .unsupported: return ["error": "NotSupportedError", "message": message]
            case .notEntitled: return ["error": "NOT_OWNED", "message": message]
            default: return ["error": "OperationError", "message": message, "apple": ["code": message]]
            }
        }
        if let e = error as? Product.PurchaseError {
            switch e {
            case .productUnavailable: return ["error": "ITEM_UNAVAILABLE", "message": message]
            case .purchaseNotAllowed: return ["error": "PURCHASE_NOT_ALLOWED", "message": message]
            default: return ["error": "OperationError", "message": message, "apple": ["code": message]]
            }
        }
        return ["error": "OperationError", "message": message]
    }

    static func ms(_ date: Date?) -> Any { date.map { $0.timeIntervalSince1970 * 1000 } ?? NSNull() }

    static func iso(_ period: Product.SubscriptionPeriod) -> String {
        let unit: String
        switch period.unit {
        case .day: unit = "D"
        case .week: unit = "W"
        case .month: unit = "M"
        case .year: unit = "Y"
        @unknown default: unit = "D"
        }
        return "P\(period.value)\(unit)"
    }

    static func kind(_ type: Product.ProductType) -> String {
        switch type {
        case .consumable: "consumable"
        case .nonConsumable: "nonConsumable"
        case .autoRenewable: "autoRenewable"
        default: "nonRenewable"
        }
    }

    static func details(_ p: Product) async -> [String: Any] {
        let subscription = p.subscription, intro = subscription?.introductoryOffer
        let free = intro?.paymentMode == .freeTrial
        let currency = p.priceFormatStyle.currencyCode
        var apple: [String: Any] = ["type": kind(p.type), "subscriptionGroupID": subscription?.subscriptionGroupID ?? NSNull(),
                                    "isEligibleForIntroOffer": NSNull()]
        if let subscription { apple["isEligibleForIntroOffer"] = await subscription.isEligibleForIntroOffer }
        return ["itemId": p.id, "title": p.displayName, "price": ["currency": currency, "value": "\(p.price)"],
                "type": subscription != nil || p.type == .nonRenewable ? "subscription" : "product",
                "description": p.description, "iconURLs": [String](),
                "subscriptionPeriod": subscription.map { iso($0.subscriptionPeriod) } ?? NSNull(),
                "freeTrialPeriod": free ? intro.map { iso($0.period) } ?? NSNull() : NSNull(),
                "introductoryPricePeriod": intro != nil && !free ? intro.map { iso($0.period) } ?? NSNull() : NSNull(),
                "introductoryPrice": intro != nil && !free ? intro.map { ["currency": currency, "value": "\($0.price)"] } ?? NSNull() : NSNull(),
                "introductoryPriceCycles": intro != nil && !free ? intro?.periodCount ?? NSNull() : NSNull(),
                "displayPrice": p.displayPrice, "familyShareable": p.isFamilyShareable, "apple": apple, "android": NSNull()]
    }

    /// A verified transaction as `PurchaseDetails`. A subscription's
    /// `expirationTime` is its grace period's end while it is in one (D7).
    static func details(_ t: Transaction, acknowledged: Bool) async -> [String: Any] {
        var end = ms(t.expirationDate), renews: Any = NSNull()
        if t.productType == .autoRenewable, let group = t.subscriptionGroupID,
           let statuses = try? await Product.SubscriptionInfo.status(for: group),
           let status = statuses.first(where: { (try? $0.transaction.payloadValue.originalID) == t.originalID }),
           case .verified(let renewal) = status.renewalInfo {
            renews = renewal.willAutoRenew
            if status.state == .inGracePeriod, let grace = renewal.gracePeriodExpirationDate { end = ms(grace) }
        }
        let environment: String
        switch t.environment {
        case .production: environment = "production"
        case .sandbox: environment = "sandbox"
        default: environment = "xcode"
        }
        return ["itemId": t.productID, "purchaseToken": String(t.id), "purchaseState": "purchased", "purchaseTime": ms(t.purchaseDate),
                "acknowledged": acknowledged, "willAutoRenew": renews, "quantity": t.purchasedQuantity, "expirationTime": end, "android": NSNull(),
                "apple": ["jwsRepresentation": NSNull(), "originalID": String(t.originalID),
                          "ownershipType": t.ownershipType == .familyShared ? "familyShared" : "purchased", "environment": environment,
                          "revocationDate": ms(t.revocationDate), "isUpgraded": t.isUpgraded,
                          "appAccountToken": t.appAccountToken?.uuidString ?? NSNull()] as [String: Any]]
    }
}

/// What the process shares among sessions: the synthetic store under the
/// agent, the StoreKit listener outside it, and the sessions to tell. Main
/// thread only, as every entry of a package is.
enum Shared {
    private final class Weak { weak var module: PurchasesModule?; init(_ m: PurchasesModule) { module = m } }
    private static var sessions: [Weak] = []
    private static var store: AgentStore?
    private static var listeners: [Task<Void, Never>] = []
    private static var swept = false
    private static var waiting: [() -> Void] = []
    static var storefront: String?
    /// The drive's fault point under the agent, `finish` or none.
    static var fault: String?
    static var unverified = Set<String>()

    static func join(_ m: PurchasesModule) {
        sessions.removeAll { $0.module == nil }
        sessions.append(Weak(m))
        if !m.agent && listeners.isEmpty { start(m.context.log) }
    }

    static func leave(_ m: PurchasesModule) {
        sessions.removeAll { $0.module == nil || $0.module === m }
        guard sessions.isEmpty else { return }
        listeners.forEach { $0.cancel() }
        listeners = []
        swept = false
    }

    /// Both facts' readers in every live session are asked again.
    static func changed() {
        for m in sessions.compactMap(\.module) {
            m.context.changed("purchases.listPurchases")
            m.context.changed("purchases.unfinishedPurchases")
            m.context.changed("purchases.purchasesStatus")
        }
    }

    static func agentStore(_ context: ExactPackageContext) -> AgentStore? {
        if let store { return store }
        guard let file = context.file("app:/data/exact-agent-purchases.json") else { return nil }
        store = AgentStore(file: file, log: context.log)
        return store
    }

    /// `body` once the process's first sweep has finished (D6).
    static func afterSweep(_ body: @escaping () -> Void) {
        if swept { body() } else { waiting.append(body) }
    }

    static func unfinishedIDs() async -> Set<UInt64> {
        var ids = Set<UInt64>()
        for await result in Transaction.unfinished { if case .verified(let t) = result { ids.insert(t.id) } }
        return ids
    }

    /// The listener over the store's three sequences, then the sweep: a
    /// verified revoked transaction left unfinished is finished (D5).
    private static func start(_ log: @escaping (String) -> Void) {
        listeners.append(Task { @MainActor in
            for await result in Transaction.updates {
                if case .unverified(let t, _) = result { unverified.insert(String(t.id)) }
                changed()
            }
        })
        listeners.append(Task { @MainActor in
            for await _ in Product.SubscriptionInfo.Status.updates { changed() }
        })
        listeners.append(Task { @MainActor in
            for await front in Storefront.updates {
                storefront = front.countryCode
                changed()
            }
        })
        Task { @MainActor in
            storefront = await Storefront.current?.countryCode
            for await result in Transaction.unfinished {
                guard case .verified(let t) = result, t.revocationDate != nil else { continue }
                await t.finish()
                log("purchases: finished revoked transaction \(t.id), never delivered")
            }
            swept = true
            let ready = waiting
            waiting = []
            ready.forEach { $0() }
        }
    }
}

/// An optional field as JSON: its value, or null.
func ?? <T>(value: T?, null: NSNull) -> Any { value.map { $0 as Any } ?? null as Any }
