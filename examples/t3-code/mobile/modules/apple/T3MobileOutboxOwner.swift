#if os(iOS)
// @ref llp/1109.005-composer-and-transcript.decision.md#local-outbox-storage
// FIFO invocation owner. Its lock is the queued-edit coordinator's existing mutex.
import Foundation
import CoreFoundation

struct T3OutboxReplaceFailure: Error { let replaced: Bool; let cause: Error }

final class T3MobileOutboxOwner: @unchecked Sendable {
    typealias Object = [String: Any]
    typealias Answer = (Result<Object, Error>) -> Void
    private struct Row { var record: Object?; var revision: Int; var token: String; var confirmed: Bool }
    private let lock: NSLock
    private let disk: T3MobileOutbox
    private let release: ([Object]) throws -> Void
    private let transferAdmission: (Object) throws -> Void
    private let transferEvidence: (Object) throws -> Void
    private let deliveryCleanupEvidence: (Object) throws -> Void
    private var acceptedTransfers: [String: Object] = [:]
    private let worker = DispatchQueue(label: "t3.mobile.outbox.persistence", qos: .userInitiated)
    private let epoch = UUID().uuidString.lowercased()
    private var floor = 0
    private var loaded = false
    private var rows: [String: Row] = [:]
    private var cache: [String: Object] = [:]
    private var holds: [String: Set<String>] = [:]
    private var accepted: [String: Object] = [:]
    private var listeners: [String: [Answer]] = [:]
    private var errors: [Object] = []
    private var unresolved: [String: String] = [:]
    private var uncertainResults = Set<String>()
    init(lock: NSLock, disk: T3MobileOutbox, release: @escaping ([Object]) throws -> Void, transferEvidence: @escaping (Object) throws -> Void, transferAdmission: @escaping (Object) throws -> Void, deliveryCleanupEvidence: @escaping (Object) throws -> Void = { _ in throw T3Failure(kind: "Persistence", message: "Delivery cleanup evidence is unavailable.") }) {
        self.lock = lock; self.disk = disk; self.release = release; self.transferEvidence = transferEvidence; self.transferAdmission = transferAdmission; self.deliveryCleanupEvidence = deliveryCleanupEvidence
    }
    private func synced<T>(_ body: () throws -> T) rethrows -> T { lock.lock(); defer { lock.unlock() }; return try body() }
    private func fail(_ message: String) -> T3Failure { T3Failure(kind: "Persistence", message: message) }
    private func text(_ value: Any?) -> String? { guard let value = value as? String, !value.isEmpty else { return nil }; return value }
    private func fingerprint(_ request: Object) throws -> String {
        let fields = ["mutationId", "messageId", "operation", "record", "expectedRevision", "expectedToken", "requireUnheld", "transfer", "deliveryCleanup"]
        let value = request.filter { fields.contains($0.key) }
        return String(data: try JSONSerialization.data(withJSONObject: value, options: [.sortedKeys]), encoding: .utf8)!
    }
    private func matches(_ request: Object, _ row: Row) -> Bool {
        if let token = request["expectedToken"] as? String, token != row.token { return false }
        if let revision = request["expectedRevision"] as? Int, revision != row.revision { return false }
        return true
    }
    /// Caller holds the coordinator mutex through journal persistence. Never reacquire it here.
    func deliveryRecordLocked(_ request: Object) throws -> Object {
        guard loaded, errors.isEmpty, request["ownerEpoch"] as? String == epoch,
              let id = request["messageId"] as? String, let row = rows[id], let record = row.record,
              row.confirmed, unresolved[id] == nil,
              !accepted.values.contains(where: { $0["messageId"] as? String == id }),
              (holds[id] ?? []).isEmpty,
              request["expectedToken"] as? String == row.token,
              T3OutboxDeliveryReceipt.integer(request["expectedRevision"], positive: true),
              request["expectedRevision"] as? Int == row.revision,
              let captured = request["record"] as? Object, T3MobileOutbox.jsonEqual(captured, record) else {
            throw fail("The queued message changed or is not durably available for delivery.")
        }
        return record
    }
    /// Caller holds the coordinator mutex until the replacement cleanup identity is saved.
    func deliveryCleanupRetiredLocked(_ message: String, mutation: String) throws {
        // save publishes cache only after disk success; a cold read re-syncs before publishing.
        // Failed acknowledgments retain their old cached owner, even after a visible rename.
        guard loaded, errors.isEmpty, let value = cache[message], value["state"] as? String == "active",
              unresolved[message] == nil, !uncertainResults.contains(mutation),
              !accepted.values.contains(where: { $0["messageId"] as? String == message }),
              (value["outcomes"] as? [String: Object])?[mutation] == nil,
              (value["removals"] as? [String: Object])?[mutation] == nil else {
            throw fail("Durably acknowledge the previous cleanup outcome and removal before retrying.")
        }
    }
    /// Exact replay owns its old payload; a currently open editor still defers network work.
    func deliveryRetryUnheldLocked(_ message: String) -> Bool {
        (holds[message] ?? []).isEmpty
    }
    // Called with the coordinator lock already held by byte removal.
    func protects(_ identifier: String) -> Bool {
        let records = accepted.values.compactMap { $0["record"] as? Object } + rows.values.compactMap(\.record) + cache.values.flatMap(disk.payloads)
        return records.contains { ($0["attachments"] as? [Object] ?? []).contains { ($0["id"] as? String)?.lowercased() == identifier } }
    }
    func submit(_ input: Object, answer: @escaping Answer) { submit(input, delivery: false, answer: answer) }
    func submitDeliveryCleanup(_ input: Object, answer: @escaping Answer) { submit(input, delivery: true, answer: answer) }
    /// All earlier accepted mutations settle before this identity inspection runs.
    func inspectDeliveryCleanup(_ message: String, mutation: String, answer: @escaping Answer) {
        worker.async {
            let result: Object = self.synced {
                guard self.loaded, self.errors.isEmpty else { return ["status": "uncertain", "outcome": NSNull()] }
                if let outcome = (self.cache[message]?["outcomes"] as? [String: Object])?[mutation]?["result"] as? Object {
                    return ["status": "outcome", "outcome": self.decorated(outcome)]
                }
                if self.unresolved[message] != nil || self.accepted.values.contains(where: { $0["messageId"] as? String == message }) {
                    return ["status": "uncertain", "outcome": NSNull()]
                }
                return ["status": "not-started", "outcome": NSNull()]
            }
            answer(.success(result))
        }
    }
    private func submit(_ input: Object, delivery: Bool, answer originalAnswer: @escaping Answer) {
        var request = input
        var answer = originalAnswer
        do {
            try synced {
                var action = request["action"] as? String ?? ""
                if action == "read" { worker.async { self.read(answer) }; return }
                if action == "mutate" && request["transfer"] != nil { throw fail("Captured drafts must enter through enqueueTransfer.") }
                if request["deliveryCleanup"] != nil && !delivery { throw fail("Delivery removal requires its acknowledged journal owner.") }
                guard loaded else { throw fail("Read the outbox before changing it.") }
                if action == "resumeUpdate" {
                    guard request["ownerEpoch"] as? String == epoch else { throw fail("The outbox owner changed. Read it again.") }
                    let captured = request
                    worker.async { do { answer(.success(try self.resumeUpdate(captured))) } catch { answer(.failure(error)) } }
                    return
                }
                if ["mutate", "enqueueTransfer", "hold", "releaseHold", "confirmQueued"].contains(action), request["ownerEpoch"] as? String != epoch {
                    throw fail("The outbox owner changed. Read it again.")
                }
                if ["transferLookup", "transferStatus", "completeTransfer", "releaseFailedTransfer"].contains(action) {
                    let captured = request
                    worker.async { do { originalAnswer(.success(try self.transferControl(captured))) } catch { originalAnswer(.failure(error)) } }
                    return
                }
                if action == "enqueueTransfer" {
                    guard errors.isEmpty, let record = request["record"] as? Object, let capture = request["capture"] as? Object,
                          T3MobileOutbox.validateCapture(capture, record: record), let mutation = text(request["mutationId"]),
                          mutation.hasPrefix(epoch + ":"), let sequence = Int(mutation.dropFirst(epoch.count + 1)), sequence > 0,
                          sequence <= 9_007_199_254_740_991 else { throw fail("Read a complete outbox and provide an owned draft capture.") }
                    let draft = capture["draft"] as! Object, key = draft["key"] as! String, digest = try T3MobileOutbox.captureFingerprint(capture)
                    let candidates = transfers().filter { $0["draftKey"] as? String == key }.sorted { ($0["transferId"] as! String) < ($1["transferId"] as! String) }
                    let existing = candidates.first { claim in
                        let state = claim["state"] as! String
                        return unresolved[claim["transferId"] as! String] != nil || !["released", "completed"].contains(state)
                    } ?? candidates.first { $0["state"] as? String == "completed" && $0["fingerprint"] as? String == digest }
                    if let existing {
                        let id = existing["transferId"] as! String
                        let disposition = existing["fingerprint"] as? String == digest ? "existing" : "conflict"
                        worker.async { originalAnswer(.success(self.synced { self.transferResponse(id, disposition: disposition) })) }
                        return
                    }
                    let id = record["messageId"] as! String
                    guard cache[id] == nil, rows[id] == nil, acceptedTransfers[id] == nil else { throw fail("A new transfer needs unused task identities.") }
                    try transferAdmission(record)
                    let claim: Object = ["transferId": id, "draftKey": key, "fingerprint": digest, "messageId": id,
                        "threadId": record["threadId"]!, "commandId": record["commandId"]!, "mutationId": mutation,
                        "state": "prepared", "record": record, "capture": capture, "outcome": NSNull()]
                    request = ["action": "mutate", "ownerEpoch": epoch, "mutationId": mutation, "messageId": id,
                        "operation": "enqueue", "record": record, "transfer": claim]
                    action = "mutate"
                    answer = { result in
                        switch result {
                        case .failure(let error): originalAnswer(.failure(error))
                        case .success: originalAnswer(.success(self.synced { self.transferResponse(id, disposition: "created") }))
                        }
                    }
                }
                guard let id = text(request["messageId"]) else { throw fail("Choose an outbox message.") }
                if action == "hold" || action == "releaseHold" {
                    guard let owner = text(request["owner"]) else { throw fail("Choose an editor owner.") }
                    if action == "releaseHold" {
                        let released = holds[id]?.remove(owner) != nil
                        worker.async { answer(.success(["released": released])) }; return
                    }
                    let row = rows[id]
                    let held = row?.record != nil && matches(request, row!) && (request["expectedToken"] != nil || request["expectedRevision"] != nil)
                    if held { holds[id, default: []].insert(owner) }
                    worker.async { answer(.success(["held": held])) }; return
                }
                if action == "mutate" {
                    guard let mutation = text(request["mutationId"]), mutation.hasPrefix(epoch + ":"),
                          let sequence = Int(mutation.dropFirst(epoch.count + 1)), sequence > 0, sequence <= 9_007_199_254_740_991,
                          let operation = request["operation"] as? String, ["enqueue", "update", "remove"].contains(operation) else { throw fail("The outbox mutation identity is invalid.") }
                    if operation != "remove" {
                        guard let record = request["record"] as? Object, record["messageId"] as? String == id, T3MobileOutbox.validateRecord(record) else { throw fail("The outbox record is invalid.") }
                    }
                    if let expected = request["expectedRevision"] {
                        guard let n = expected as? NSNumber, CFGetTypeID(n) != CFBooleanGetTypeID(), n.doubleValue.isFinite,
                              n.doubleValue >= 0, n.doubleValue <= 9_007_199_254_740_991, n.doubleValue.rounded() == n.doubleValue else { throw fail("The expected revision is invalid.") }
                    }
                    if let expected = request["expectedToken"], text(expected) == nil { throw fail("The expected token is invalid.") }
                    guard T3MobileOutbox.validateMutation(request, id: id) else { throw fail("The outbox mutation is invalid.") }
                    let digest = try fingerprint(request)
                    if let existing = accepted[mutation] {
                        guard try fingerprint(existing) == digest else { throw fail("A mutation identity cannot change its request.") }
                        listeners[mutation, default: []].append(answer); return
                    }
                    if let outcome = (cache[id]?["outcomes"] as? [String: Object])?[mutation] {
                        guard outcome["request"] as? String == digest else { throw fail("A mutation identity cannot change its request.") }
                        worker.async { answer(.success(self.synced { self.decorated(outcome["result"] as! Object) })) }; return
                    }
                    guard sequence > floor else { throw fail("This mutation identity was already consumed. Read its current state.") }
                    guard unresolved[id] == nil else { throw fail("Resolve this message's interrupted storage mutation first.") }
                    floor = max(floor, sequence)
                    accepted[mutation] = request; listeners[mutation] = [answer]
                    if let transfer = request["transfer"] as? Object { acceptedTransfers[id] = transfer }
                    if operation == "enqueue" {
                        let revision = (rows[id]?.revision ?? 0) + 1
                        rows[id] = Row(record: request["record"] as? Object, revision: revision, token: mutation, confirmed: false)
                        accepted[mutation]?["acceptedRevision"] = revision
                    }
                    let captured = request
                    worker.async { self.mutate(captured) }; return
                }
                guard ["status", "confirmQueued", "recover", "acknowledge", "completeRemoval"].contains(action) else { throw fail("Unknown outbox operation.") }
                let captured = request, callback = answer
                worker.async {
                    do { callback(.success(try self.control(captured))) } catch { callback(.failure(error)) }
                }
            }
        } catch { answer(.failure(error)) }
    }
    private func read(_ answer: Answer) {
        var inventory = disk.envelopes()
        var claimOwners: [String: String] = [:]
        for value in inventory.values {
            guard let claim = value["transfer"] as? Object, !["completed", "released"].contains(claim["state"] as! String) else { continue }
            let key = claim["draftKey"] as! String, id = claim["transferId"] as! String
            if let previous = claimOwners[key], previous != id {
                inventory.errors.append(["messageId": id, "message": "Multiple active outbox transfers claim the same draft."])
            }
            claimOwners[key] = id
        }
        var failedSync = Set<String>(), completedSync = Set<String>()
        do {
            // Visible terminal JSON alone does not prove the prior directory fsync completed.
            for value in inventory.values where synced({ rows[value["messageId"] as! String] == nil || unresolved[value["messageId"] as! String] != nil || !(value["outcomes"] as? [String: Object] ?? [:]).keys.filter({ uncertainResults.contains($0) }).isEmpty }) {
                do { try disk.save(value); completedSync.insert(value["messageId"] as! String) } catch {
                    let id = value["messageId"] as! String; failedSync.insert(id)
                    inventory.errors.append(["messageId": id, "message": "The saved outbox outcome still needs durability recovery."])
                }
            }
        }
        let result: Object = synced {
            errors = inventory.errors
            for value in inventory.values {
                let id = value["messageId"] as! String
                let ids = (value["outcomes"] as? [String: Object] ?? [:]).keys
                if failedSync.contains(id) {
                    uncertainResults.formUnion(ids)
                    if let claim = value["transfer"] as? Object { unresolved[id] = claim["mutationId"] as? String; uncertainResults.insert(claim["mutationId"] as! String) }
                }
                if completedSync.contains(id) {
                    cache[id] = value
                    if let claim = value["transfer"] as? Object { uncertainResults.remove(claim["mutationId"] as! String) }
                    for mutation in ids { uncertainResults.remove(mutation) }
                    if value["state"] as? String != "pending" {
                        unresolved.removeValue(forKey: id)
                        if rows[id]?.token == value["token"] as? String { rows[id]?.confirmed = value["state"] as? String == "active" && value["confirmed"] as? Bool == true }
                    }
                }
            }
            do {
                for value in inventory.values where rows[value["messageId"] as! String] == nil {
                    let id = value["messageId"] as! String
                    cache[id] = value
                    if value["state"] as? String == "pending" || failedSync.contains(id) {
                        unresolved[id] = (value["mutation"] as? Object)?["mutationId"] as? String ?? value["token"] as? String ?? ""
                    }
                    let token = value["state"] as? String == "pending" && value["proposed"] is Object
                        ? (value["mutation"] as! Object)["mutationId"] as! String : value["token"] as! String
                    rows[id] = Row(record: value["record"] as? Object ?? value["proposed"] as? Object ?? value["previous"] as? Object,
                        revision: value["sourceRevision"] as! Int, token: token,
                        confirmed: unresolved[id] == nil && value["state"] as? String == "active" && value["confirmed"] as? Bool == true)
                }
                loaded = true
            }
            return ["ownerEpoch": epoch, "sequenceFloor": floor, "complete": errors.isEmpty, "errors": errors,
                "revisions": rows.mapValues(\.revision), "tokens": rows.mapValues(\.token), "records": rows.values.compactMap { row -> Object? in
                    guard let record = row.record else { return nil }
                    return ["record": record, "revision": row.revision, "token": row.token, "pending": !row.confirmed,
                        "held": !(holds[record["messageId"] as! String] ?? []).isEmpty]
                }, "outcomes": cache.values.flatMap { ($0["outcomes"] as? [String: Object] ?? [:]).values.compactMap { entry -> Object? in
                    guard let outcome = entry["result"] as? Object else { return nil }; return decorated(outcome)
                } },
                "mutations": cache.values.filter { $0["state"] as? String == "pending" }, "transfers": transfers().map(publicClaim)]
        }
        answer(.success(result))
    }
    private func result(_ request: Object, _ status: String, revision: Int, record: Object? = nil, removed: Object? = nil, message: String = "") -> Object {
        ["mutationId": request["mutationId"]!, "messageId": request["messageId"]!, "status": status, "revision": revision,
         "record": record.map { $0 as Any } ?? NSNull(), "removed": removed.map { $0 as Any } ?? NSNull(), "message": message]
    }
    private func blank(_ id: String) -> Object {
        ["version": 1, "messageId": id, "revision": 0, "sourceRevision": 0, "token": "", "confirmed": false,
         "state": "deleted", "record": NSNull(), "outcomes": [String: Object](), "removals": [String: Object]()]
    }
    private func terminal(_ base: Object, request: Object, row: Row, outcome: Object, owners: [Object]) throws -> Object {
        var value = base
        value["state"] = row.record == nil ? "deleted" : "active"; value["record"] = row.record.map { $0 as Any } ?? NSNull()
        value["sourceRevision"] = row.revision; value["token"] = row.token; value["confirmed"] = row.confirmed
        for key in ["previous", "proposed", "mutation"] { value.removeValue(forKey: key) }
        var outcomes = value["outcomes"] as? [String: Object] ?? [:]
        outcomes[request["mutationId"] as! String] = ["request": try fingerprint(request), "result": outcome, "owners": owners]
        value["outcomes"] = outcomes
        if var transfer = value["transfer"] as? Object, transfer["mutationId"] as? String == request["mutationId"] as? String {
            transfer["outcome"] = outcome
            transfer["state"] = outcome["status"] as? String == "committed" ? "queued" : "failed"
            value["transfer"] = transfer
        }
        if let removed = outcome["removed"] as? Object {
            var removals = value["removals"] as? [String: Object] ?? [:]; removals[request["mutationId"] as! String] = removed; value["removals"] = removals
        }
        return value
    }
    private func save(_ input: Object) throws {
        var value = input
        let id = value["messageId"] as! String
        value["sourceRevision"] = synced { max(value["sourceRevision"] as? Int ?? 0, rows[id]?.revision ?? 0) }
        try disk.save(value)
        synced { cache[id] = value }
    }
    // Called under the coordinator lock: operation payload and current row are deliberately distinct.
    private func decorated(_ outcome: Object) -> Object {
        let id = outcome["messageId"] as! String, row = rows[id]
        var value = outcome
        if unresolved[id] == outcome["mutationId"] as? String || uncertainResults.contains(outcome["mutationId"] as? String ?? "") {
            value["status"] = "uncertain"
            // A prepared mutation has no terminal result yet, but the bridge still needs a complete Outcome.
            if value["revision"] == nil { value["revision"] = row?.revision ?? 0 }
            if value["record"] == nil { value["record"] = NSNull() }
            if value["removed"] == nil { value["removed"] = NSNull() }
        }
        value["ownerEpoch"] = epoch; value["sequenceFloor"] = floor
        value["current"] = ["record": row?.record.map { $0 as Any } ?? NSNull(), "revision": row?.revision ?? 0,
                            "token": row?.token ?? "", "pending": row?.confirmed != true]
        return value
    }
    private func finish(_ mutation: String, outcome: Object) {
        let completed: ([Answer], Object) = synced {
            if let request = accepted.removeValue(forKey: mutation), request["transfer"] != nil { acceptedTransfers.removeValue(forKey: request["messageId"] as! String) }
            return (listeners.removeValue(forKey: mutation) ?? [], decorated(outcome))
        }
        for answer in completed.0 { answer(.success(completed.1)) }
    }
    private func deliveryRemovalMatchesLocked(_ request: Object, _ row: Row, recovering: Bool = false) -> Bool {
        guard let guardValue = request["deliveryCleanup"] as? Object else { return true }
        let id = request["messageId"] as! String, mutation = request["mutationId"] as! String
        return errors.isEmpty && (row.confirmed || recovering) && matches(request, row)
            && T3MobileOutbox.jsonEqual(row.record, guardValue["record"])
            && (holds[id] ?? []).isEmpty
            && !accepted.values.contains { $0["messageId"] as? String == id && $0["mutationId"] as? String != mutation }
    }
    private func mutate(_ request: Object) {
        let id = request["messageId"] as! String, mutation = request["mutationId"] as! String, operation = request["operation"] as! String
        let previous = synced { cache[id] ?? blank(id) }
        let initial = synced { rows[id] ?? Row(record: nil, revision: 0, token: "", confirmed: false) }
        let enqueueRevision = synced { accepted[mutation]?["acceptedRevision"] as? Int ?? initial.revision }
        var pending = previous
        if let transfer = request["transfer"] as? Object { pending["transfer"] = transfer }
        pending["revision"] = (previous["revision"] as? Int ?? 0) + 1
        pending["state"] = "pending"; pending["previous"] = previous["record"] ?? NSNull(); pending.removeValue(forKey: "record")
        pending["proposed"] = request["record"] ?? NSNull(); pending["mutation"] = request
        let owners = [previous["record"] as? Object, request["record"] as? Object].compactMap { $0 }
        func admitted(_ row: Row) -> Bool {
            row.record != nil && matches(request, row) && (request["requireUnheld"] as? Bool != true || synced { (holds[id] ?? []).isEmpty })
                && synced { deliveryRemovalMatchesLocked(request, row) }
        }
        if synced({ unresolved[id] != nil }) {
            blocked(request, previous: previous); return
        }
        if operation != "enqueue" && !admitted(initial) {
            let outcome = result(request, "stale", revision: initial.revision)
            do { try save(terminal(pending, request: request, row: initial, outcome: outcome, owners: [])); finish(mutation, outcome: outcome) }
            catch { uncertain(request, pending: pending, message: "The stale result could not be saved.") }
            return
        }
        var installedProposal = false
        do {
            try save(pending)
            var target = operation == "enqueue" ? Row(record: request["record"] as? Object, revision: enqueueRevision, token: mutation, confirmed: true)
                : Row(record: operation == "remove" ? nil : request["record"] as? Object, revision: initial.revision + 1, token: mutation, confirmed: true)
            var outcome = result(request, "committed", revision: target.revision, record: target.record, removed: operation == "remove" ? initial.record : nil)
            if request["deliveryCleanup"] == nil {
                try save(terminal(pending, request: request, row: target, outcome: outcome, owners: owners))
                installedProposal = true
            }
            if operation != "enqueue" {
                let published: (Row, Bool) = synced {
                    let current = rows[id]!
                    let valid = current.record != nil && matches(request, current)
                        && (request["requireUnheld"] as? Bool != true || (holds[id] ?? []).isEmpty)
                        && deliveryRemovalMatchesLocked(request, current)
                    if !valid { return (current, false) }
                    target.revision = current.revision + 1; rows[id] = target
                    return (target, true)
                }
                target = published.0
                if request["deliveryCleanup"] != nil { installedProposal = true }
                outcome = result(request, published.1 ? "committed" : "stale", revision: target.revision,
                    record: published.1 ? target.record : nil, removed: published.1 && operation == "remove" ? initial.record : nil)
                // One bounded compensation/final-outcome write, like source; later enqueues own their own write.
                try save(terminal(pending, request: request, row: target, outcome: outcome, owners: owners))
            } else {
                synced { if rows[id]?.token == mutation { rows[id]?.confirmed = true } }
            }
            finish(mutation, outcome: outcome)
        } catch {
            if !installedProposal, let failure = error as? T3OutboxReplaceFailure, !failure.replaced {
                let current: Row = synced {
                    if operation == "enqueue", rows[id]?.token == mutation { rows[id]?.record = nil; rows[id]?.confirmed = false }
                    return rows[id] ?? initial
                }
                let outcome = result(request, "failed", revision: current.revision, message: "The outbox mutation could not be saved.")
                do { try save(terminal(pending, request: request, row: current, outcome: outcome, owners: owners)); finish(mutation, outcome: outcome) }
                catch { uncertain(request, pending: pending, message: "The failed mutation cleanup could not be confirmed.") }
            } else { uncertain(request, pending: pending, message: "The outbox mutation may have reached disk. Resolve its exact outcome.") }
        }
    }

    private func blocked(_ request: Object, previous: Object) {
        let id = request["messageId"] as! String, mutation = request["mutationId"] as! String
        let row: Row = synced {
            if request["operation"] as? String == "enqueue", rows[id]?.token == mutation { rows[id]?.record = nil; rows[id]?.confirmed = false }
            return rows[id] ?? Row(record: nil, revision: 0, token: "", confirmed: false)
        }
        let outcome = result(request, "failed", revision: row.revision, message: "This mutation did not write because an earlier storage outcome needs recovery.")
        var value = previous, outcomes = previous["outcomes"] as? [String: Object] ?? [:]
        do {
            outcomes[mutation] = ["request": try fingerprint(request), "result": outcome, "owners": [request["record"] as? Object].compactMap { $0 }]
            value["outcomes"] = outcomes
            try save(value)
            finish(mutation, outcome: outcome)
        } catch {
            synced { cache[id] = value; uncertainResults.insert(mutation) }
            finish(mutation, outcome: result(request, "uncertain", revision: row.revision, message: "The no-write outcome could not be saved."))
        }
    }
    private func uncertain(_ request: Object, pending: Object, message: String) {
        let id = request["messageId"] as! String
        // Reread captures a terminal rename as well as a surviving prepared marker; neither proves fsync success.
        let read = try? disk.load(id)
        let hasOutcome = ((read?["outcomes"] as? [String: Object])?[request["mutationId"] as! String]) != nil
        let visible = hasOutcome || read?["state"] as? String == "pending" ? read! : pending
        synced { cache[id] = visible; unresolved[id] = request["mutationId"] as? String; uncertainResults.insert(request["mutationId"] as! String); rows[id]?.confirmed = false }
        finish(request["mutationId"] as! String, outcome: result(request, "uncertain", revision: synced { rows[id]?.revision ?? 0 }, message: message))
    }
    private func requireDeliveryCleanupEvidence(_ outcome: Object) throws {
        guard let raw = outcome["request"] as? String, let data = raw.data(using: .utf8),
              let request = try JSONSerialization.jsonObject(with: data) as? Object else { throw fail("The mutation receipt is invalid.") }
        if request["deliveryCleanup"] != nil { try deliveryCleanupEvidence(request) }
    }
    /// Saved editor updates keep their original identity across native epochs. This
    /// bounded recovery owns the coordinator mutex through durable publication.
    private func resumeUpdate(_ input: Object) throws -> Object {
        try synced {
            guard loaded, errors.isEmpty, input["ownerEpoch"] as? String == epoch,
                  let owner = text(input["holdOwner"]), let request = input["request"] as? Object,
                  Set(request.keys) == Set(["ownerEpoch", "mutationId", "messageId", "operation", "record", "expectedToken", "expectedRevision", "requireUnheld"]),
                  let originalEpoch = text(request["ownerEpoch"]), let mutation = text(request["mutationId"]),
                  mutation.hasPrefix(originalEpoch + ":"), let sequence = Int(mutation.dropFirst(originalEpoch.count + 1)),
                  sequence > 0, sequence <= 9_007_199_254_740_991, let id = text(request["messageId"]),
                  request["operation"] as? String == "update", request["requireUnheld"] as? Bool == false,
                  text(request["expectedToken"]) != nil, request["expectedRevision"] != nil,
                  T3MobileOutbox.validateMutation(request, id: id), let proposed = request["record"] as? Object,
                  holds[id]?.contains(owner) == true else { throw fail("Restore the exact pending editor hold before resuming its saved update.") }
            guard accepted[mutation] == nil,
                  !cache.contains(where: { $0.key != id && ($0.value["outcomes"] as? [String: Object])?[mutation] != nil }),
                  !rows.contains(where: { $0.key != id && $0.value.token == mutation }) else {
                throw fail("The saved mutation identity already belongs to admitted work.")
            }
            guard !accepted.values.contains(where: { $0["messageId"] as? String == id }) else {
                throw fail("An admitted mutation must settle before this editor update can resume.")
            }
            let digest = try fingerprint(request), previous = cache[id] ?? blank(id)
            let initial = rows[id] ?? Row(record: nil, revision: 0, token: "", confirmed: false)
            if let receipt = (previous["outcomes"] as? [String: Object])?[mutation] {
                guard receipt["request"] as? String == digest, let outcome = receipt["result"] as? Object else {
                    throw fail("A saved update identity cannot change its request.")
                }
                if unresolved[id] == mutation || uncertainResults.contains(mutation) {
                    do { try disk.save(previous) }
                    catch { throw T3Failure(kind: "Persistence", message: "The saved update still needs durability recovery.", uncertain: true) }
                    if initial.token == previous["token"] as? String || matches(request, initial) {
                        rows[id] = Row(record: previous["record"] as? Object, revision: max(initial.revision, previous["sourceRevision"] as! Int),
                            token: previous["token"] as! String, confirmed: previous["confirmed"] as? Bool == true)
                    }
                    unresolved.removeValue(forKey: id); uncertainResults.remove(mutation)
                }
                return decorated(outcome)
            }
            var pending = previous
            if previous["state"] as? String == "pending" {
                guard let original = previous["mutation"] as? Object, try fingerprint(original) == digest,
                      unresolved[id] == mutation, previous["sourceRevision"] as? Int == request["expectedRevision"] as? Int,
                      matches(request, initial) || initial.token == mutation && T3MobileOutbox.jsonEqual(initial.record, proposed) else {
                    throw fail("The saved update does not own this interrupted mutation.")
                }
            } else {
                guard unresolved[id] == nil, initial.confirmed else { throw fail("Resolve the pending task's existing storage outcome first.") }
                pending["revision"] = (previous["revision"] as? Int ?? 0) + 1
                pending["state"] = "pending"; pending["previous"] = previous["record"] ?? NSNull()
                pending.removeValue(forKey: "record"); pending["proposed"] = proposed; pending["mutation"] = request
            }
            // The original request may have been reserved before a higher sequence,
            // or by a prior native owner. Only this held, captured API permits it.
            if originalEpoch == epoch { floor = max(floor, sequence) }
            let sameOwner = ["origin", "environmentId", "threadId", "messageId", "commandId"].allSatisfy {
                T3MobileOutbox.jsonEqual(initial.record?[$0], proposed[$0])
            }
            let committing = previous["state"] as? String == "pending" || sameOwner && matches(request, initial)
            let target = committing ? Row(record: proposed, revision: initial.revision + 1, token: mutation, confirmed: true) : initial
            let outcome = result(request, committing ? "committed" : "stale", revision: target.revision, record: committing ? proposed : nil)
            let owners = [previous["record"] as? Object, previous["previous"] as? Object, proposed].compactMap { $0 }
            do {
                if committing { try disk.save(pending); cache[id] = pending }
                let saved = try terminal(pending, request: request, row: target, outcome: outcome, owners: owners)
                try disk.save(saved); cache[id] = saved; rows[id] = target
                unresolved.removeValue(forKey: id); uncertainResults.remove(mutation)
                return decorated(outcome)
            } catch {
                let visible = (try? disk.load(id))
                let hasOutcome = ((visible?["outcomes"] as? [String: Object])?[mutation]) != nil
                if !committing && !hasOutcome {
                    return decorated(result(request, "unknown", revision: initial.revision, message: "The stale update outcome could not be saved. Resume its exact identity."))
                }
                cache[id] = hasOutcome || (visible?["mutation"] as? Object)?["mutationId"] as? String == mutation ? visible! : pending
                unresolved[id] = mutation; uncertainResults.insert(mutation); rows[id]?.confirmed = false
                return decorated(result(request, "uncertain", revision: initial.revision, message: "The saved editor update may have reached disk. Resume its exact identity."))
            }
        }
    }
    private func control(_ request: Object) throws -> Object {
        let id = request["messageId"] as! String, action = request["action"] as! String
        var value = synced { cache[id] ?? blank(id) }
        if action == "confirmQueued" {
            return synced {
                let row = rows[id] ?? Row(record: nil, revision: 0, token: "", confirmed: false)
                return ["current": row.record != nil && row.confirmed && matches(request, row) && row.token == request["token"] as? String && (holds[id] ?? []).isEmpty,
                        "revision": row.revision]
            }
        }
        guard let mutation = text(request["mutationId"]) else { throw fail("Choose an exact outbox mutation.") }
        var outcomes = value["outcomes"] as? [String: Object] ?? [:]
        if action == "status" {
            return synced { decorated(outcomes[mutation]?["result"] as? Object ?? ["mutationId": mutation, "messageId": id, "status": "unknown", "message": "No terminal outcome is recorded."]) }
        }
        if action == "recover" {
            let originalToken = value["token"] as? String ?? ""
            if value["state"] as? String == "pending" {
                guard let original = value["mutation"] as? Object, original["mutationId"] as? String == mutation,
                      ["commit", "rollback"].contains(request["decision"] as? String ?? "") else { throw fail("Choose commit or rollback for the exact pending mutation.") }
                let commit = request["decision"] as? String == "commit"
                if original["deliveryCleanup"] != nil {
                    return try synced {
                        guard errors.isEmpty, !accepted.values.contains(where: { $0["messageId"] as? String == id }),
                              let current = rows[id] else { throw fail("Resolve accepted work before delivery cleanup recovery.") }
                        let originalRow = Row(record: value["previous"] as? Object,
                            revision: original["expectedRevision"] as! Int, token: original["expectedToken"] as! String, confirmed: true)
                        let capturedStillCurrent = matches(original, current)
                            && T3MobileOutbox.jsonEqual(current.record, originalRow.record)
                        // In-process CAS may already have removed the row before its terminal save failed.
                        let publishedOwnRemoval = current.token == mutation && current.record == nil
                        guard capturedStillCurrent || publishedOwnRemoval else { throw fail("The delivery cleanup row changed; recovery cannot overwrite it.") }
                        if commit {
                            guard (holds[id] ?? []).isEmpty else { throw fail("Close this message editor before delivery cleanup recovery.") }
                        }
                        let row = commit ? Row(record: nil, revision: max(current.revision, originalRow.revision + 1), token: mutation, confirmed: true) : originalRow
                        let outcome = result(original, commit ? "committed" : "failed", revision: row.revision,
                            record: nil, removed: commit ? originalRow.record : nil)
                        let saved = try terminal(value, request: original, row: row, outcome: outcome, owners: disk.payloads(value))
                        // This rare explicit recovery holds the shared mutex through disk publication.
                        // No new editor/row admission can invalidate its captured CAS during the write.
                        do { try disk.save(saved) }
                        catch { throw T3Failure(kind: "Persistence", message: "Delivery cleanup recovery may have reached disk.", uncertain: true) }
                        cache[id] = saved; rows[id] = row; unresolved.removeValue(forKey: id); uncertainResults.remove(mutation)
                        return decorated(outcome)
                    }
                }
                let record = !commit && original["operation"] as? String == "enqueue" ? nil : value[commit ? "proposed" : "previous"] as? Object
                let revision = (value["sourceRevision"] as? Int ?? 0) + 1
                let row = Row(record: record, revision: revision, token: commit || original["operation"] as? String == "enqueue" ? mutation : originalToken, confirmed: commit || original["operation"] as? String != "enqueue")
                let outcome = result(original, commit ? "committed" : "failed", revision: revision, record: record,
                    removed: commit && record == nil ? value["previous"] as? Object : nil)
                value = try terminal(value, request: original, row: row, outcome: outcome, owners: disk.payloads(value))
            } else {
                guard request["decision"] as? String == "retry", outcomes[mutation] != nil || value["token"] as? String == mutation else { throw fail("Retry durability for an exact terminal outcome.") }
            }
            try save(value)
            synced {
                if unresolved[id] == mutation { unresolved.removeValue(forKey: id) }
                uncertainResults.remove(mutation)
                let current = rows[id]
                if current == nil || current?.token == originalToken || current?.token == value["token"] as? String || current?.token == mutation {
                    rows[id] = Row(record: value["record"] as? Object, revision: max(value["sourceRevision"] as! Int, current?.revision ?? 0),
                                   token: value["token"] as! String, confirmed: value["confirmed"] as? Bool == true)
                }
            }
            return synced { decorated((value["outcomes"] as? [String: Object])?[mutation]?["result"] as? Object
                ?? ["mutationId": mutation, "messageId": id, "status": "unknown", "message": "Durability restored; this outcome was already acknowledged."]) }
        }
        if action == "acknowledge" {
            guard let outcome = outcomes[mutation], value["state"] as? String != "pending", synced({ unresolved[id] != mutation && !uncertainResults.contains(mutation) }) else { return ["acknowledged": false] }
            try requireDeliveryCleanupEvidence(outcome)
            try release(outcome["owners"] as? [Object] ?? [])
            outcomes.removeValue(forKey: mutation); value["outcomes"] = outcomes; try save(value)
            return ["acknowledged": true]
        }
        guard synced({ unresolved[id] == nil }) else { return ["completed": false] }
        var removals = value["removals"] as? [String: Object] ?? [:]
        guard let removed = removals[mutation] else { return ["completed": false] }
        if let outcome = outcomes[mutation] { try requireDeliveryCleanupEvidence(outcome) }
        try release([removed]); removals.removeValue(forKey: mutation); value["removals"] = removals; try save(value)
        return ["completed": true]
    }
    // These helpers run under the existing coordinator mutex unless noted otherwise.
    private func transfers() -> [Object] {
        var values = cache.compactMapValues { $0["transfer"] as? Object }
        for (id, claim) in acceptedTransfers { values[id] = claim }
        return values.values.sorted { ($0["transferId"] as! String) < ($1["transferId"] as! String) }
    }
    private func transfer(_ id: String) -> Object? { acceptedTransfers[id] ?? cache[id]?["transfer"] as? Object }
    private func publicClaim(_ claim: Object) -> Object {
        var value = claim; value.removeValue(forKey: "outcome")
        if !["completed", "released"].contains(value["state"] as! String),
           unresolved[value["transferId"] as! String] == value["mutationId"] as? String || uncertainResults.contains(value["mutationId"] as! String) { value["state"] = "prepared" }
        return value
    }
    private func transferOutcome(_ claim: Object) -> Any {
        if let outcome = claim["outcome"] as? Object { return decorated(outcome) }
        let id = claim["transferId"] as! String, mutation = claim["mutationId"] as! String
        if unresolved[id] == mutation || uncertainResults.contains(mutation) {
            return decorated(["messageId": id, "mutationId": mutation, "status": "unknown", "message": "Resolve the captured enqueue's durability."])
        }
        return NSNull()
    }
    private func transferResponse(_ id: String, disposition: String? = nil) -> Object {
        let claim = transfer(id)
        var response: Object = ["claim": claim.map { publicClaim($0) as Any } ?? NSNull(), "outcome": claim.map { transferOutcome($0) } ?? NSNull()]
        if let disposition { response["disposition"] = disposition }
        return response
    }
    // FIFO worker. Preference evidence and byte release use coordinator callbacks with its mutex.
    private func transferControl(_ request: Object) throws -> Object {
        let action = request["action"] as! String
        if action == "transferLookup" {
            guard let key = text(request["draftKey"]), key.hasPrefix("new-task:") else { throw fail("Choose a full draft identity.") }
            var digest: Any = NSNull()
            if let raw = request["capture"] {
                guard let capture = raw as? Object, T3MobileOutbox.validateCapture(capture),
                      (capture["draft"] as! Object)["key"] as? String == key else { throw fail("The draft capture is invalid.") }
                digest = try T3MobileOutbox.captureFingerprint(capture)
            }
            return synced { ["complete": errors.isEmpty, "fingerprint": digest,
                "claims": transfers().filter { $0["draftKey"] as? String == key }.map(publicClaim)] }
        }
        guard let id = text(request["transferId"]) else { throw fail("Choose an exact draft transfer.") }
        if action == "transferStatus" { return synced { transferResponse(id) } }
        guard var claim = synced({ transfer(id) }), claim["fingerprint"] as? String == request["fingerprint"] as? String else { throw fail("The draft transfer fingerprint changed.") }
        let state = claim["state"] as! String, mutation = claim["mutationId"] as! String
        guard synced({ unresolved[id] != mutation && !uncertainResults.contains(mutation) }), let outcome = claim["outcome"] as? Object else { throw fail("Resolve this transfer's exact durable outcome first.") }
        let releasing = action == "releaseFailedTransfer", finalState = releasing ? "released" : "completed"
        if state == finalState { return [releasing ? "released" : "completed": true, "claim": publicClaim(claim)] }
        guard state == (releasing ? "failed" : "queued"), outcome["status"] as? String == (releasing ? "failed" : "committed") else { throw fail("The transfer has not reached the required durable outcome.") }
        if !releasing { try transferEvidence(claim) }
        // Queue release work before retirement; a failed retirement leaves capture ownership intact.
        if let record = claim["record"] as? Object { try release([record]) }
        var savedOutcome = outcome; savedOutcome["record"] = NSNull(); savedOutcome["removed"] = NSNull()
        claim["state"] = finalState; claim["record"] = NSNull(); claim["capture"] = NSNull(); claim["outcome"] = savedOutcome
        var value = synced { cache[id]! }; value["transfer"] = claim
        do { try save(value) }
        catch {
            synced { unresolved[id] = mutation; uncertainResults.insert(mutation) }
            throw T3Failure(kind: "Persistence", message: "Transfer retirement may have reached disk. Read its exact outcome before retrying.", uncertain: true)
        }
        return [releasing ? "released" : "completed": true, "claim": publicClaim(claim)]
    }

}
#endif
