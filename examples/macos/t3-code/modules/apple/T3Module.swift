// Contract/TypeScript owns presentation and domain projection. This optional
// module owns one authenticated connection; Exact's core knows nothing of T3.
import Foundation

final class T3Module: ExactModule {
    private let transport: T3Transport
    private let composer = T3Composer()
    private let chrome = T3WindowChrome()

    required init(context: ExactModuleContext) {
        transport = T3Transport(persistent: !context.agent, dataDirectory: context.data, changed: context.changed)
        super.init(context: context)
    }
    override func later(_ request: [String: Any], reply: ExactReply) {
        transport.perform(request) { reply.send($0) }
    }
    override func element(_ element: ExactElement) { composer.install(element); chrome.install(element) }
    override func elementEnded(_ element: ExactElement) { composer.remove(element) }
    override func destroy() { composer.destroy(); chrome.destroy(); transport.destroy() }
}

let exactModule: ExactModule.Type = T3Module.self
