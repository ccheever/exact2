// The synthetic store @exact/purchases keeps under the agent on Apple (LLP
// 1133.015 §9): the implementation is its own substitute, so a drive reaches
// no store and spends nothing. The store is the process's, as StoreKit's
// record is the account's: every session reads the same lists. It lives in
// the drive's scratch tree (`app:/data`, a named agent store), so it survives
// a relaunch as the store's servers do.
//
// It behaves as StoreKit does on Apple, on the agent clock: a subscription
// ends at its `expirationTime`, which `listPurchases` names as its stale
// time (D7); a renewal is a new transaction that enters the queue (D5); a
// pending purchase has no transaction until it is approved; a revoked
// transaction left unfinished is finished by the implementation and listed
// nowhere (D5's exception). Every time is Unix ms, from the agent clock's
// epoch (`EXACT_AGENT_EPOCH`, as the host reads it).
import ExactPackage
import Foundation

final class AgentStore {
    struct Transaction: Codable {
        var token: String
        var itemId: String
        /// `consumable`, `nonConsumable`, `autoRenewable`, `nonRenewable`.
        var kind: String
        var originalID: String
        var purchaseTime: Double
        var expirationTime: Double?
        var acknowledged = false
        var revoked: Double?
        var appAccountToken: String?
        var isUpgraded = false
    }

    /// A pending purchase (Ask to Buy): no transaction until approved.
    struct Ask: Codable {
        var id: String
        var itemId: String
        var appAccountToken: String?
    }

    struct State: Codable {
        var transactions: [Transaction] = []
        var asks: [Ask] = []
        var counter = 0
        /// The store events applied, by `op` and token or item, so one set
        /// again (or still set after a relaunch) is applied once.
        var applied: [String] = []
    }

    private(set) var state = State()
    private let file: URL?
    private let log: (String) -> Void
    /// The catalogue, from the app's manifest or a drive's `getDetails` preference.
    var catalog: [[String: Any]] = []

    init(file: URL?, log: @escaping (String) -> Void) {
        self.file = file
        self.log = log
        if let file, let data = try? Data(contentsOf: file), let saved = try? JSONDecoder().decode(State.self, from: data) {
            state = saved
        }
    }

    private func save() {
        guard let file, let data = try? JSONEncoder().encode(state) else { return }
        try? FileManager.default.createDirectory(at: file.deletingLastPathComponent(), withIntermediateDirectories: true)
        try? data.write(to: file, options: .atomic)
    }

    /// The agent clock as a Unix time: the host's epoch for the agent's zero
    /// plus the session clock.
    static func unix(_ now: Double) -> Double {
        let epoch = Double(ProcessInfo.processInfo.environment["EXACT_AGENT_EPOCH"] ?? "") ?? 1_767_225_600_000
        return epoch + now
    }

    func item(_ id: String) -> [String: Any]? { catalog.first { $0["itemId"] as? String == id } }

    /// The catalogue's StoreKit type for `item`: `apple.type` where given,
    /// else a non-consumable product or an auto-renewable subscription.
    func kind(_ item: [String: Any]) -> String {
        if let kind = (item["apple"] as? [String: Any])?["type"] as? String { return kind }
        return item["type"] as? String == "subscription" ? "autoRenewable" : "nonConsumable"
    }

    private func group(_ itemId: String) -> String? {
        guard let item = item(itemId) else { return nil }
        return (item["apple"] as? [String: Any])?["subscriptionGroupID"] as? String ?? item["group"] as? String
    }

    /// An ISO 8601 period (`P1M`, `P1Y`, `P7D`, `P1W`) in ms; months of 30 days, years of 365.
    static func period(_ iso: String?) -> Double? {
        guard let iso, iso.hasPrefix("P"), let unit = iso.last, let n = Double(iso.dropFirst().dropLast()) else { return nil }
        let day = 86_400_000.0
        switch unit {
        case "D": return n * day
        case "W": return n * 7 * day
        case "M": return n * 30 * day
        case "Y": return n * 365 * day
        default: return nil
        }
    }

    private func nextToken() -> String {
        state.counter += 1
        return String(2_000_000_000_000_000 + state.counter)
    }

    // MARK: the two facts (D3)

    /// What the account holds at `now`: each non-consumable and non-renewing
    /// subscription, each subscription transaction not yet expired and not
    /// superseded, each unfinished consumable; never a revoked one.
    func entitlements(at now: Double) -> [Transaction] {
        let live = state.transactions.filter { $0.revoked == nil }
        return live.filter { t in
            switch t.kind {
            case "consumable": return !t.acknowledged
            case "autoRenewable":
                guard let end = t.expirationTime, end > now else { return false }
                // The latest transaction of its group stands for it.
                return !live.contains { $0.kind == "autoRenewable" && $0.token != t.token && group($0.itemId) == group(t.itemId)
                    && ($0.expirationTime ?? 0) > end }
            default: return true
            }
        }
    }

    /// The queue: every transaction not finished. A revoked one is finished
    /// here, journaled by token, and never listed (D5).
    func unfinished() -> [Transaction] {
        var finished = false
        for i in state.transactions.indices where state.transactions[i].revoked != nil && !state.transactions[i].acknowledged {
            state.transactions[i].acknowledged = true
            finished = true
            log("purchases: finished revoked transaction \(state.transactions[i].token), never delivered")
        }
        if finished { save() }
        return state.transactions.filter { !$0.acknowledged && $0.revoked == nil }
    }

    /// The earliest expiration still to come: `listPurchases`' stale time (D7).
    func nextExpiry(after now: Double) -> Double? {
        entitlements(at: now).compactMap(\.expirationTime).filter { $0 > now }.min()
    }

    // MARK: what the person and the store do

    /// A purchase the person confirmed (`buy`), or one made elsewhere.
    func buy(_ itemId: String, at now: Double, account: String?) -> Transaction? {
        guard let item = item(itemId) else { return nil }
        let kind = self.kind(item)
        // A non-consumable owned already: StoreKit answers its transaction.
        if kind == "nonConsumable", let owned = state.transactions.first(where: { $0.itemId == itemId && $0.revoked == nil }) {
            return owned
        }
        let token = nextToken()
        var t = Transaction(token: token, itemId: itemId, kind: kind, originalID: token, purchaseTime: now, appAccountToken: account)
        if kind == "autoRenewable" {
            // Another plan of the group: the one it replaces is upgraded.
            for i in state.transactions.indices where state.transactions[i].kind == "autoRenewable" && group(state.transactions[i].itemId) == group(itemId)
                && (state.transactions[i].expirationTime ?? 0) > now && state.transactions[i].itemId != itemId {
                state.transactions[i].isUpgraded = true
                state.transactions[i].expirationTime = now
            }
            t.expirationTime = Self.period(item["subscriptionPeriod"] as? String).map { now + $0 }
        }
        state.transactions.append(t)
        save()
        log("purchases: transaction \(token) for \(itemId), unfinished")
        return t
    }

    func ask(_ itemId: String, account: String?) -> Ask {
        state.counter += 1
        let a = Ask(id: "ask-\(state.counter)", itemId: itemId, appAccountToken: account)
        state.asks.append(a)
        save()
        log("purchases: \(itemId) pending as \(a.id), no transaction until it is approved")
        return a
    }

    /// `acknowledge` or `consume` of `token`: StoreKit's `finish()`, which
    /// for a consumable is consuming it. False for a token the account does
    /// not hold.
    func finish(_ token: String) -> Bool {
        guard let i = state.transactions.firstIndex(where: { $0.token == token && $0.revoked == nil }) else { return false }
        if !state.transactions[i].acknowledged {
            state.transactions[i].acknowledged = true
            save()
            log("purchases: finished \(token)")
        }
        return true
    }

    /// A change the store makes on its own (`prefer extension
    /// purchases.storeEvent=…`), applied once. Whether anything changed.
    func apply(_ event: [String: Any], at now: Double) -> Bool {
        let op = event["op"] as? String ?? ""
        let token = event["purchaseToken"] as? String, itemId = event["itemId"] as? String
        let key = "\(op) \(token ?? "") \(itemId ?? "")"
        guard !state.applied.contains(key) else { return false }
        state.applied.append(key)
        defer { save() }
        switch op {
        case "approve":
            guard let i = state.asks.firstIndex(where: { $0.id == token || (token == nil && $0.itemId == itemId) }) else {
                log("purchases: storeEvent approve names no pending purchase")
                return false
            }
            let a = state.asks.remove(at: i)
            log("purchases: \(a.id) approved")
            return buy(a.itemId, at: now, account: a.appAccountToken) != nil
        case "renew":
            guard let old = state.transactions.last(where: { $0.token == token || $0.originalID == token }), old.kind == "autoRenewable",
                  let period = Self.period(item(old.itemId)?["subscriptionPeriod"] as? String) else {
                log("purchases: storeEvent renew names no subscription")
                return false
            }
            let start = max(old.expirationTime ?? now, now), renewal = nextToken()
            state.transactions.append(Transaction(token: renewal, itemId: old.itemId, kind: old.kind, originalID: old.originalID,
                                                  purchaseTime: start, expirationTime: start + period, appAccountToken: old.appAccountToken))
            log("purchases: \(old.originalID) renewed as \(renewal), unfinished")
            return true
        case "revoke":
            guard let i = state.transactions.firstIndex(where: { $0.token == token }) else {
                log("purchases: storeEvent revoke names no transaction")
                return false
            }
            state.transactions[i].revoked = now
            log("purchases: \(token ?? "") revoked")
            return true
        case "purchase":
            guard let itemId, let t = buy(itemId, at: now, account: nil) else {
                log("purchases: storeEvent purchase names no item of the catalogue")
                return false
            }
            log("purchases: \(t.token) bought outside the app")
            return true
        default:
            log("purchases: storeEvent \(op) is not approve, renew, revoke or purchase")
            return false
        }
    }

    /// The store as the journal discloses it: each transaction by token with
    /// its state, never an account.
    func disclosed(at now: Double) -> String {
        let rows = state.transactions.map { t in
            "\(t.token) \(t.itemId)\(t.acknowledged ? " finished" : " unfinished")\(t.revoked != nil ? " revoked" : "")"
                + (t.expirationTime.map { $0 > now ? " until \(Int($0))" : " expired" } ?? "")
        } + state.asks.map { "\($0.id) \($0.itemId) pending" }
        return rows.isEmpty ? "empty" : rows.joined(separator: "; ")
    }
}
