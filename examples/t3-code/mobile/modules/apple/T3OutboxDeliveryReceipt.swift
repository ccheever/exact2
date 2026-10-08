#if os(iOS)
// @ref llp/1109.005-composer-and-transcript.decision.md#queued-command-construction
// Immutable command receipt and attempt state. Inline-asset persistence remains a separate stage.
import Foundation
import CoreFoundation

enum T3OutboxDeliveryReceipt {
    typealias Object = [String: Any]
    static func integer(_ raw: Any?, positive: Bool = false) -> Bool {
        guard let value = raw as? NSNumber, CFGetTypeID(value) != CFBooleanGetTypeID() else { return false }
        let n = value.doubleValue
        return n.isFinite && n.rounded() == n && n >= (positive ? 1 : 0) && n <= 9_007_199_254_740_991
    }
    private static func text(_ raw: Any?, limit: Int = 4096) -> Bool {
        guard let value = raw as? String else { return false }
        return !value.isEmpty && value.utf16.count <= limit && value == value.trimmingCharacters(in: .whitespacesAndNewlines)
    }
    private static func fields(_ value: Object, _ required: [String], _ optional: [String] = []) -> Bool {
        Set(required).isSubset(of: Set(value.keys)) && Set(value.keys).isSubset(of: Set(required + optional))
    }
    private static func member(_ raw: Any?, _ values: [String]) -> Bool {
        guard let raw = raw as? String else { return false }; return values.contains(raw)
    }
    private static func model(_ raw: Any?) -> Bool {
        guard let value = raw as? Object, fields(value, ["instanceId", "model"], ["options"]),
              text(value["instanceId"]), text(value["model"]) else { return false }
        if let options = value["options"] {
            guard let options = options as? [Object], options.allSatisfy({ option in
                guard fields(option, ["id", "value"]), text(option["id"]) else { return false }
                return text(option["value"]) || (option["value"] as? NSNumber).map { CFGetTypeID($0) == CFBooleanGetTypeID() } == true
            }) else { return false }
        }
        return true
    }
    private static func attachments(_ raw: Any?, record: Object) -> Bool {
        guard let values = raw as? [Object], let locals = record["attachments"] as? [Object],
              values.count == locals.count, values.count <= 100 else { return false }
        return zip(values, locals).allSatisfy { value, local in
            guard fields(value, ["type", "id", "name", "mimeType", "sizeBytes"], ["source"]),
                  text(value["id"], limit: 128), (value["id"] as! String).range(of: "^[a-zA-Z0-9_-]+$", options: .regularExpression) != nil,
                  text(value["name"], limit: 255), text(value["mimeType"], limit: 100), integer(value["sizeBytes"]),
                  value["id"] as? String == local["uploadId"] as? String,
                  local["uploadEnvironmentId"] as? String == record["environmentId"] as? String,
                  value["name"] as? String == local["name"] as? String,
                  value["sizeBytes"] as? Int == local["sizeBytes"] as? Int else { return false }
            let size = value["sizeBytes"] as! Int, mime = value["mimeType"] as! String
            if value["type"] as? String == "image" {
                guard size <= 10 * 1024 * 1024, mime.lowercased().hasPrefix("image/") else { return false }
                // Files-picked images can be promoted. The upload owner validated the full snapshot source.
                return value["source"] == nil || value["source"] is Object && T3MobileOutbox.jsonEqual(value["source"], local["source"])
            }
            guard value["type"] as? String == "file", local["kind"] as? String == "file", size >= 1, size <= 50 * 1024 * 1024 else { return false }
            return value["source"] == nil || T3MobileOutbox.jsonEqual(value["source"], ["_tag": "pasted-text"])
        }
    }
    private static func content(_ value: Object, record: Object) -> Bool {
        guard value["messageId"] as? String == record["messageId"] as? String,
              let body = value["text"] as? String, body.utf16.count <= 120_000,
              attachments(value["attachments"], record: record) else { return false }
        // Legacy context serialization changes text. Its source mapper owns that transformation.
        // Inline context must be the captured context with only adopted attachment IDs substituted.
        if let context = value["context"] {
            guard var expected = record["context"] as? Object, let originals = expected["records"] as? [Object] else { return false }
            let map = Dictionary(uniqueKeysWithValues: (record["attachments"] as! [Object]).map { ($0["id"] as! String, $0["uploadId"] as! String) })
            expected["records"] = originals.map { item -> Object in
                var item = item
                if let id = item["attachmentId"] as? String, let remote = map[id] { item["attachmentId"] = remote }
                return item
            }
            guard T3MobileOutbox.jsonEqual(context, expected) else { return false }
        }
        return true
    }
    private static func command(_ receipt: Object) -> Bool {
        guard let record = receipt["record"] as? Object, T3MobileOutbox.validateRecord(record),
              let payload = receipt["payload"] as? Object, let id = receipt["operationId"] as? String,
              payload["commandId"] as? String == id, payload["threadId"] as? String == record["threadId"] as? String else { return false }
        if receipt["stage"] as? String == "settings-sync" {
            guard receipt["method"] as? String == "orchestration.dispatchCommand", record["creation"] == nil else { return false }
            let base = record["commandId"] as! String
            if payload["type"] as? String == "thread.runtime-mode.set" {
                return fields(payload, ["type", "commandId", "threadId", "runtimeMode"]) && id == base + ":runtime-mode"
                    && member(payload["runtimeMode"], ["approval-required", "auto-accept-edits", "auto", "full-access"])
            }
            return fields(payload, ["type", "commandId", "threadId", "interactionMode"]) && id == base + ":interaction-mode"
                && payload["type"] as? String == "thread.interaction-mode.set" && member(payload["interactionMode"], ["default", "plan"])
        }
        guard receipt["stage"] as? String == "start-turn", id == record["commandId"] as? String,
              payload["creationSource"] as? String == "mobile", model(payload["modelSelection"]) else { return false }
        if receipt["method"] as? String == "orchestration.launchThread" {
            guard fields(payload, ["commandId", "threadId", "projectId", "title", "generateTitle", "creationSource", "modelSelection", "runtimeMode", "interactionMode", "workspaceStrategy", "initialMessage"]),
                  let creation = record["creation"] as? Object, payload["projectId"] as? String == creation["projectId"] as? String,
                  payload["title"] is String, (payload["generateTitle"] as? NSNumber).map({ CFGetTypeID($0) == CFBooleanGetTypeID() && $0.boolValue }) == true,
                  member(payload["runtimeMode"], ["approval-required", "auto-accept-edits", "auto", "full-access"]),
                  member(payload["interactionMode"], ["default", "plan"]), let workspace = payload["workspaceStrategy"] as? Object,
                  let initial = payload["initialMessage"] as? Object, fields(initial, ["messageId", "text", "attachments"], ["context"]),
                  content(initial, record: record) else { return false }
            switch workspace["type"] as? String {
            case "root": return fields(workspace, ["type"], ["branch"]) && creation["workspaceMode"] as? String == "local"
                && (creation["worktreePath"] is NSNull || creation["worktreePath"] as? String == "") && (workspace["branch"] == nil ? creation["branch"] is NSNull : T3MobileOutbox.jsonEqual(workspace["branch"], creation["branch"]))
            case "existing_worktree": return fields(workspace, ["type", "worktreePath"], ["branch"]) && creation["workspaceMode"] as? String == "local"
                && text(workspace["worktreePath"]) && T3MobileOutbox.jsonEqual(workspace["worktreePath"], creation["worktreePath"])
                && (workspace["branch"] == nil ? creation["branch"] is NSNull : T3MobileOutbox.jsonEqual(workspace["branch"], creation["branch"]))
            case "worktree": return fields(workspace, ["type", "baseRef", "branch"], ["startFromOrigin"]) && creation["workspaceMode"] as? String == "worktree"
                && text(workspace["branch"]) && text(workspace["baseRef"]) && T3MobileOutbox.jsonEqual(workspace["baseRef"], creation["branch"])
                && (workspace["startFromOrigin"] == nil ? creation["startFromOrigin"] as? Bool != true : T3MobileOutbox.jsonEqual(workspace["startFromOrigin"], true) && creation["startFromOrigin"] as? Bool == true)
            default: return false
            }
        }
        guard receipt["method"] as? String == "orchestration.dispatchCommand", record["creation"] == nil,
              fields(payload, ["type", "commandId", "threadId", "messageId", "text", "attachments", "createdBy", "creationSource", "dispatchMode", "modelSelection"], ["context", "deliveryIntent", "titleSeed"]),
              payload["type"] as? String == "message.dispatch", payload["createdBy"] as? String == "user", content(payload, record: record),
              let dispatch = payload["dispatchMode"] as? Object else { return false }
        if let intent = payload["deliveryIntent"], !member(intent, ["auto", "steer", "restart"]) { return false }
        if let title = payload["titleSeed"], !(title is String) { return false }
        if member(dispatch["type"], ["start_immediately", "queue_after_active"]) { return fields(dispatch, ["type"]) }
        return fields(dispatch, ["type", "targetRunId"]) && member(dispatch["type"], ["steer_active", "restart_active"]) && text(dispatch["targetRunId"])
    }
    static func valid(_ receipt: Object, id: String) -> Bool {
        guard fields(receipt, ["kind", "operationId", "revision", "origin", "environmentId", "messageId", "threadId", "rowToken", "rowRevision", "record", "stage", "method", "payload", "attachmentIDs", "state"], ["retiredRevision", "attemptRevision", "attemptPreviousState", "result", "error"]),
              receipt["kind"] as? String == "outbox", receipt["operationId"] as? String == id, text(id),
              integer(receipt["revision"], positive: true), integer(receipt["rowRevision"], positive: true), text(receipt["rowToken"]),
              member(receipt["state"], ["reserved", "retired", "issued", "uncertain", "acknowledged", "rejected"]), let record = receipt["record"] as? Object,
              ["origin", "environmentId", "messageId", "threadId"].allSatisfy({ T3MobileOutbox.jsonEqual(receipt[$0], record[$0]) }),
              let files = record["attachments"] as? [Object], let ids = receipt["attachmentIDs"] as? [String],
              ids == files.compactMap({ $0["id"] as? String }), JSONSerialization.isValidJSONObject(receipt) else { return false }
        let prior = receipt["retiredRevision"] as? Int ?? 0
        guard receipt["retiredRevision"] == nil || integer(receipt["retiredRevision"], positive: true),
              prior <= 9_007_199_254_740_989 else { return false }
        let state = receipt["state"] as! String, revision = receipt["revision"] as! Int
        if let rawAttempt = receipt["attemptRevision"] {
            guard integer(rawAttempt, positive: true), let attempt = rawAttempt as? Int, attempt > prior + 1,
                  let previous = receipt["attemptPreviousState"] as? String, ["reserved", "uncertain", "rejected"].contains(previous),
                  revision == attempt + (state == "issued" ? 0 : state == "retired" ? 2 : 1) else { return false }
            if ["reserved", "retired"].contains(state) && previous != "reserved" { return false }
            if state == "rejected" && previous == "uncertain" { return false }
            if previous == "rejected" && receipt["stage"] as? String != "settings-sync" { return false }
            if state == "issued" {
                guard receipt["result"] == nil, receipt["error"] == nil else { return false }
            } else if state == "acknowledged" {
                guard receipt["result"] != nil, receipt["error"] == nil else { return false }
            } else {
                guard receipt["result"] == nil, receipt["error"] is Object else { return false }
            }
        } else {
            guard ["reserved", "retired"].contains(state), receipt["attemptPreviousState"] == nil,
                  receipt["result"] == nil, receipt["error"] == nil,
                  revision == prior + (state == "reserved" ? 1 : 2) else { return false }
        }
        return command(receipt)
    }
    static func sameIdentity(_ a: Object, _ b: Object) -> Bool {
        T3MobileOutbox.jsonEqual(a.filter { !["revision", "state", "attemptRevision", "attemptPreviousState", "result", "error"].contains($0.key) }, b.filter { !["revision", "state", "attemptRevision", "attemptPreviousState", "result", "error"].contains($0.key) })
    }
    static func make(_ request: Object, origin: String, environment: String) throws -> Object {
        guard let record = request["record"] as? Object, let payload = request["payload"] as? Object,
              let id = payload["commandId"] as? String else { throw T3Failure(kind: "Outbox", message: "Choose a materialized outbox command.") }
        var receipt: Object = ["kind": "outbox", "operationId": id, "revision": 1, "origin": origin, "environmentId": environment,
            "messageId": request["messageId"] ?? NSNull(), "threadId": record["threadId"] ?? NSNull(),
            "rowToken": request["expectedToken"] ?? NSNull(), "rowRevision": request["expectedRevision"] ?? NSNull(), "record": record,
            "stage": request["stage"] ?? NSNull(), "method": request["method"] ?? NSNull(), "payload": payload,
            "attachmentIDs": (record["attachments"] as? [Object] ?? []).compactMap { $0["id"] as? String }, "state": "reserved"]
        if let prior = request["expectedRetiredRevision"] {
            guard integer(prior, positive: true), let revision = prior as? Int, revision <= 9_007_199_254_740_989 else {
                throw T3Failure(kind: "Outbox", message: "Choose the exact retired reservation revision.")
            }
            receipt["retiredRevision"] = revision; receipt["revision"] = revision + 1
        }
        guard valid(receipt, id: id) else { throw T3Failure(kind: "Outbox", message: "The outbox command is invalid or still needs attachment materialization.") }
        // Detach mutable Foundation objects owned by the caller before retaining them.
        return try JSONSerialization.jsonObject(with: JSONSerialization.data(withJSONObject: receipt, options: [.sortedKeys])) as! Object
    }
}
#endif
