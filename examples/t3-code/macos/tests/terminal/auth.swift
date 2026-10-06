import XCTest

final class TerminalAuthTests: XCTestCase {
    func testSerialChunksKeepUTF16AndResize() {
        var payloads: [[String: Any]] = [], replies: [(T3Failure?) -> Void] = [], failures: [Bool] = []
        let queue = T3TerminalAuth(send: { payload, reply in payloads.append(payload); replies.append(reply) }, failure: { failures.append($0) })
        queue.configure(instance: "provider", identity: "flow:terminal", readOnly: false)
        queue.input(String(repeating: "A", count: 4096) + String(repeating: "B", count: 4096) + String(repeating: "C", count: 1808))
        queue.input("", size: (93, 31))
        XCTAssertEqual(payloads.count, 1)
        replies.removeFirst()(nil); XCTAssertEqual(payloads.count, 2)
        replies.removeFirst()(nil); XCTAssertEqual(payloads.count, 3)
        replies.removeFirst()(nil); XCTAssertEqual(payloads.count, 4)
        replies.removeFirst()(nil)
        let responses = payloads.compactMap { $0["response"] as? [String: Any] }
        XCTAssertEqual(responses.compactMap { ($0["data"] as? String)?.utf16.count }, [4096, 4096, 1808, 0])
        XCTAssertEqual(responses.last?["size"] as? [String: Int], ["cols": 93, "rows": 31])
        XCTAssertTrue(payloads.allSatisfy { $0["instanceId"] as? String == "provider" && $0["flowId"] as? String == "flow" && $0["interactionId"] as? String == "terminal" })
        XCTAssertTrue(failures.isEmpty)
    }

    func testUnicodePairIsNotSplit() {
        var values: [String] = []
        let queue = T3TerminalAuth(send: { payload, reply in values.append((payload["response"] as? [String: Any])?["data"] as? String ?? ""); reply(nil) }, failure: { _ in })
        queue.configure(instance: "provider", identity: "flow:terminal", readOnly: false)
        let text = String(repeating: "a", count: 4095) + "😀한글"
        queue.input(text)
        XCTAssertEqual(values.joined(), text)
        XCTAssertEqual(values.map { $0.utf16.count }, [4095, 4])
    }

    func testSecondSliceFailureClearsRemainingAndSuccessfulInputRecovers() {
        var payloads: [[String: Any]] = [], replies: [(T3Failure?) -> Void] = [], failures: [Bool] = []
        let queue = T3TerminalAuth(send: { payload, reply in payloads.append(payload); replies.append(reply) }, failure: { failures.append($0) })
        queue.configure(instance: "provider", identity: "flow:terminal", readOnly: false)
        queue.input(String(repeating: "a", count: 10000)); replies.removeFirst()(nil)
        replies.removeFirst()(T3Failure(kind: "Fixture", message: "gone"))
        XCTAssertEqual(payloads.count, 2); XCTAssertEqual(failures, [true])
        queue.input("retry"); replies.removeFirst()(nil)
        XCTAssertEqual(payloads.count, 3); XCTAssertEqual(failures, [true, false])
    }

    func testReplacementIgnoresOldReplyAndUsesCurrentIdentity() {
        var payloads: [[String: Any]] = [], replies: [(T3Failure?) -> Void] = [], failures: [Bool] = []
        let queue = T3TerminalAuth(send: { payload, reply in payloads.append(payload); replies.append(reply) }, failure: { failures.append($0) })
        queue.configure(instance: "provider", identity: "old:terminal", readOnly: false)
        queue.input(String(repeating: "old", count: 4000))
        queue.configure(instance: "provider", identity: "new:terminal", readOnly: false)
        queue.input("replacement")
        XCTAssertEqual(payloads.compactMap { $0["flowId"] as? String }, ["old", "new"])
        replies.removeFirst()(T3Failure(kind: "Fixture", message: "cancelled"))
        replies.removeFirst()(nil)
        XCTAssertEqual(payloads.count, 2); XCTAssertTrue(failures.isEmpty)
    }

    func testReadOnlyAndUnmountDropPendingDataAndSize() {
        var payloads: [[String: Any]] = [], replies: [(T3Failure?) -> Void] = []
        let queue = T3TerminalAuth(send: { payload, reply in payloads.append(payload); replies.append(reply) }, failure: { _ in XCTFail("retired callback") })
        queue.configure(instance: "provider", identity: "flow:terminal", readOnly: true)
        queue.input("secret"); queue.input("", size: (80, 24)); XCTAssertTrue(payloads.isEmpty)
        queue.configure(instance: "provider", identity: "flow:terminal", readOnly: false)
        queue.input(String(repeating: "a", count: 10000)); queue.clear()
        replies.removeFirst()(nil); XCTAssertEqual(payloads.count, 1)
        queue.configure(instance: "provider", identity: "flow:terminal", readOnly: true)
        queue.input("later"); XCTAssertEqual(payloads.count, 1)
    }
}
