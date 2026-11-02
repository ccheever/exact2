import Foundation

/// Only native generated and strictly decoded opaque payload bytes enter this encoder.
enum T3QuestionCommandWire {
    static func request(id: String, payloadUTF8: String) throws -> String {
        guard let payload = try JSONSerialization.jsonObject(with: Data(payloadUTF8.utf8)) as? [String: Any],
              payload["commandId"] is String else { throw T3QuestionCommands.bad() }
        let scalar = try JSONSerialization.data(withJSONObject: id, options: [.fragmentsAllowed])
        let text = "{\"_tag\":\"Request\",\"id\":\(String(decoding: scalar, as: UTF8.self)),\"tag\":\"orchestration.dispatchCommand\",\"payload\":\(payloadUTF8),\"headers\":[[\"x-t3-orchestration-protocol\",\"2\"]]}"
        guard text.utf8.count <= T3QuestionCommands.limit else { throw T3QuestionCommands.bad("The question request is too large.") }
        return text
    }
}
