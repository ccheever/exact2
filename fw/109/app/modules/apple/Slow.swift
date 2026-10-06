// X14: `slow` replies after 500 ms. The first call announces topic `x` 5 times,
// every 300 ms, as a server's event stream would while a snapshot read is out.
import Foundation

final class Slow: ExactModule {
    private var calls = 0
    override func later(_ request: [String: Any], reply: ExactReply) {
        calls += 1
        let ask = request["ask"] as? Int ?? -1
        NSLog("x14 module: later #\(calls) (ask \(ask))")
        if calls == 1 {
            for i in 1...5 {
                DispatchQueue.main.asyncAfter(deadline: .now() + .milliseconds(300 * i)) {
                    NSLog("x14 module: changed x (\(i))"); self.context.changed("x")
                }
            }
        }
        DispatchQueue.main.asyncAfter(deadline: .now() + .milliseconds(500)) {
            NSLog("x14 module: reply to ask \(ask)"); reply.send(["ask": ask])
        }
    }
}
let exactModule: ExactModule.Type = Slow.self
