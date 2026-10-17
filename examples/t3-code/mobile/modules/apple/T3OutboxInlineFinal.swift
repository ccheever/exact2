#if os(iOS)
// @ref llp/1109.005-composer-and-transcript.decision.md#queued-command-construction
// Derive lifecycle payloads from the immutable native assets acknowledgment, never caller input.
import Foundation

enum T3OutboxInlineFinal {
    typealias Object = [String: Any]
    static func command(_ source: Object) -> Object? {
        guard let id = source["operationId"] as? String,
              T3OutboxInlineReceipt.valid(source, id: id), source["state"] as? String == "acknowledged",
              let template = source["template"] as? Object, let bindings = template["inline"] as? [Object],
              let response = source["result"] as? Object, let saved = response["attachments"] as? [Object],
              var command = template["commandTemplate"] as? Object,
              var payload = command["payload"] as? Object else { return nil }
        let launch = command["method"] as? String == "orchestration.launchThread"
        var message = launch ? payload["initialMessage"] as! Object : payload
        var attachments = message["attachments"] as! [Object]
        for (binding, attachment) in zip(bindings, saved) { attachments[binding["index"] as! Int] = attachment }
        // Source remaps only IDs present before persistence. Inline slots are idless;
        // all reference IDs stay unchanged, so the existing context remains exact.
        message["attachments"] = attachments
        if launch { payload["initialMessage"] = message } else { payload = message }
        command["payload"] = payload
        return command
    }
    static func link(_ source: Object) -> Object {
        ["operationId": source["operationId"] ?? NSNull(), "ackRevision": source["revision"] ?? NSNull(), "payloadDigest": source["payloadDigest"] ?? NSNull()]
    }
    static func matches(_ receipt: Object, source: Object?) -> Bool {
        guard let source, let command = command(source),
              let link = receipt["inlineSource"] as? Object, Set(link.keys) == Set(["operationId", "ackRevision", "payloadDigest"]),
              T3MobileOutbox.jsonEqual(link, self.link(source)),
              ["record", "rowToken", "rowRevision", "attachmentIDs", "origin", "environmentId", "threadId", "messageId"].allSatisfy({ T3MobileOutbox.jsonEqual(receipt[$0], source[$0]) }),
              receipt["operationId"] as? String == (source["record"] as? Object)?["commandId"] as? String else { return false }
        return ["stage", "method", "payload"].allSatisfy { T3MobileOutbox.jsonEqual(receipt[$0], command[$0]) }
    }
}
#endif
