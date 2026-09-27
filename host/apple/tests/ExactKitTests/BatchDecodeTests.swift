import Foundation
import XCTest
@testable import ExactKit
#if os(macOS)
import AppKit
#endif

// Existing JSON-building fixtures keep their call sites and go through the
// production decoder. OracleBatch has its own, unshadowed Decodable entry.
extension JSONDecoder {
    func decode(_ type: NodeStyle.Type, from data: Data) throws -> NodeStyle {
        try data.withUnsafeBytes {
            var reader = BatchReader(bytes: $0.bindMemory(to: UInt8.self))
            let style = try reader.values()
            try reader.end()
            return style
        }
    }
    func decode(_ type: Batch.Type, from data: Data) throws -> Batch {
        let batch = Batch.decode(data)
        guard batch.error != "unreadable batch" else { throw BatchReader.Invalid.wire }
        return batch
    }
    func decode(_ type: [InlineText].Type, from data: Data) throws -> [InlineText] {
        try data.withUnsafeBytes {
            var reader = BatchReader(bytes: $0.bindMemory(to: UInt8.self))
            let rows = try reader.array { try $0.inline() }
            try reader.end()
            return rows
        }
    }
}

final class BatchDecodeTests: XCTestCase {
    private var compared = 0
    private var operations = 0
    private var paragraphs = 0

    @discardableResult
    func compare(_ data: Data, decoded: Batch? = nil, file: StaticString = #filePath, line: UInt = #line) throws -> Batch {
        let old = try JSONDecoder().decode(OracleBatch.self, from: data)
        let new = decoded ?? Batch.decode(data)
        XCTAssertEqual(new.error, old.error, file: file, line: line)
        XCTAssertEqual(new.timers, old.timers, file: file, line: line)
        XCTAssertEqual(new.motion, old.motion, file: file, line: line)
        XCTAssertEqual(new.pending, old.pending, file: file, line: line)
        XCTAssertEqual(new.clock?.bitPattern, old.clock?.bitPattern, file: file, line: line)
        XCTAssertEqual(new.timerDueMs?.bitPattern, old.timerDueMs?.bitPattern, file: file, line: line)
        XCTAssertEqual(new.ops.count, old.ops.count, file: file, line: line)
        for (a, b) in zip(new.ops, old.ops) {
            XCTAssertEqual(a.op.rawValue, b.op.rawValue, file: file, line: line)
            XCTAssertEqual(a.nodeID, b.nodeID, file: file, line: line)
            XCTAssertEqual(a.kind, b.kind, file: file, line: line)
            XCTAssertEqual(a.props, b.props, file: file, line: line)
            XCTAssertEqual(a.clear, b.clear, file: file, line: line)
            XCTAssertEqual(a.handlers, b.handlers, file: file, line: line)
            XCTAssertEqual(a.ids, b.ids, file: file, line: line)
            XCTAssertEqual(a.property, b.property, file: file, line: line)
            XCTAssertEqual([a.x, a.y, a.w, a.h].map(\.bitPattern), [b.x, b.y, b.w, b.h].map(\.bitPattern), file: file, line: line)
            XCTAssertEqual(a.style, b.style.mapValues(convert), file: file, line: line)
            XCTAssertTrue(NSDictionary(dictionary: a.payload).isEqual(to: b.payload), file: file, line: line)
            XCTAssertEqual(a.runs.count, b.runs.count, file: file, line: line)
            for (x, y) in zip(a.runs, b.runs) {
                XCTAssertEqual(x.id, y.id, file: file, line: line)
                XCTAssertEqual(x.parent, y.parent, file: file, line: line)
                XCTAssertEqual(x.props, y.props, file: file, line: line)
                XCTAssertEqual(x.handlers, y.handlers, file: file, line: line)
                XCTAssertEqual(x.paints, y.paints, file: file, line: line)
                XCTAssertEqual(x.hasSchemeColor, y.hasSchemeColor, file: file, line: line)
                XCTAssertEqual(x.range, y.range, file: file, line: line)
                XCTAssertEqual(x.run(dark: false), y.run(dark: false), file: file, line: line)
                XCTAssertEqual(x.run(dark: true), y.run(dark: true), file: file, line: line)
            }
            if a.op == .paragraph { paragraphs += 1 }
        }
        compared += 1; operations += new.ops.count
        return new
    }
    private func convert(_ value: OracleBatchValue) -> BatchValue {
        switch value {
        case .number(let n): return .number(n)
        case .string(let s): return .string(s)
        case .bool(let b): return .bool(b)
        case .array(let a): return .array(a.map(convert))
        case .object(let o): return .object(o.mapValues(convert))
        case .null: return .null
        }
    }
    private func compare(_ text: String) throws -> Batch { try compare(Data(text.utf8)) }

    func testEveryOperationAndAdapterPayload() throws {
        let rows = [
            #"{"op":"create","id":4294967295,"kind":"text","props":{"text":"α 👩‍🚀 é\n\"\\\u0001"},"style":{"opacity":0.5,"text_color":[[1,2,3,4],[5,6,7,8]],"future":{"b":true,"n":null}},"handlers":["press","press","change"]}"#,
            #"{"op":"props","id":1,"set":{"href":"/a"},"clear":["text"]}"#,
            #"{"op":"style","id":1,"style":{"a":true,"b":false,"c":null,"d":"x","e":[1,"x",{}]}}"#,
            #"{"op":"children","id":1,"ids":[0,1,4294967295]}"#,
            #"{"op":"roots","ids":[1,2]}"#,
            #"{"op":"paragraph","id":1,"runs":[{"id":2,"parent":1,"paint":true,"props":{"text":"a","href":"/"},"style":{"line_height":1.25,"font_size":17.3,"font_weight":600,"font_family":1,"letter_spacing":0.1,"font_style":"italic","text_decoration_line":"underline","text_color":[[1,2,3,4],[5,6,7,8]],"ignored":[true]},"handlers":["press"]},{"id":3,"parent":2,"style":{"line_height":"21px","text_color":[1,2,3,4]}}]}"#,
            #"{"op":"frame","id":1,"x":-0,"y":0.1,"w":1e3,"h":1.7976931348623157e308}"#,
            #"{"op":"content","id":1,"w":2,"h":3}"#,
            #"{"op":"present","id":1,"property":"translate","x":-2.5,"y":0.125}"#,
            #"{"op":"destroy","id":1,"ignored":{"deep":[null,true,"x"]}}"#,
            #"{"op":"flow","id":1,"shapes":[{"rect":[0,1,2,3]}]}"#,
            #"{"op":"surface","id":1,"name":"map","values":[1,true,null,["x"]]}"#,
            #"{"op":"command","name":"openURL","args":["https://example.com",false,null]}"#,
            #"{"op":"hold","token":"18446744073709551615","x":1,"y":2}"#,
            #"{"op":"height-drag","id":1,"handleKey":"18446744073709551615","target":null,"targetKey":null}"#,
            #"{"op":"transform-drag","id":1,"runtime":"1","handleKey":"2","target":3,"targetKey":"4","clip":5,"clipKey":"6"}"#,
            #"{"op":"retire-motion","id":1,"property":"translate","runtime":"2","token":"3"}"#,
            #"{"op":"collections","items":[{"id":1,"rows":[]}] }"#,
            #"{"op":"region","id":1,"nested":{"number":0.1,"boolean":true,"null":null}}"#,
            #"{"op":"router","top":2,"url":"/path","removed":[1]}"#,
            #"{"op":"title","title":"A question"}"#,
            #"{"op":"title","title":null}"#,
            #"{"op":"future","id":1,"any":[true,null,{"x":"y"}]}"#
        ]
        for row in rows {
            _ = try compare("{\"ops\":[\(row)],\"timers\":true,\"timer_due_ms\":123.25,\"clock\":-0.0,\"motion\":true,\"pending\":true}")
            // Move op behind all other keys without a JSON round trip changing
            // number spellings. This exercises the one-pass reordered path.
            let comma = try XCTUnwrap(row.firstIndex(of: ","))
            let head = row[row.index(after: row.startIndex)..<comma]
            let tail = row[row.index(after: comma)..<row.index(before: row.endIndex)]
            _ = try compare("{\"ops\":[{\(tail),\(head)}]}")
        }
        XCTAssertEqual(compared, rows.count * 2)
    }

    func testStringsNumbersDefaultsAndUnknownKeys() throws {
        for text in [#"{}"#, #"{"ops":null,"timers":null,"motion":null,"clock":null,"error":null,"pending":null,"timer_due_ms":null}"#,
                     #"{"unknown":{"a":[{},[],true,false,null,1e999]},"ops":[{"op":"destroy","unused":1e999}]}"#,
                     #"{"error":"\"\\\/\b\f\n\r\t\u0000\u007F\u0080\u07FF\u0800\uFFFF\uD83D\uDE80","ops":[]}"#] {
            _ = try compare(text)
        }
        let numbers = ["-0", "0", "-0.0", "1.0000000000000002", "9007199254740993", "18446744073709551615",
                       "5e-324", "2.2250738585072014e-308", "1.7976931348623157e308", "0.84551240822557006", "1e+20", "0e-400"]
        for number in numbers { _ = try compare("{\"clock\":\(number),\"ops\":[{\"op\":\"style\",\"style\":{\"n\":\(number)}}]}") }
        // A nonzero literal below the smallest subnormal is refused by both, as one past the largest is.
        for number in ["1e-324", "-2.4e-324", "1.8e308"] {
            let data = Data("{\"clock\":\(number)}".utf8)
            XCTAssertThrowsError(try JSONDecoder().decode(OracleBatch.self, from: data))
            XCTAssertEqual(Batch.decode(data).error, "unreadable batch")
        }
        // Deterministic finite f64 round trips exercise decimal rounding across
        // the exponent range, with exact bits (including the sign of zero).
        var bits: UInt64 = 1044
        for _ in 0..<2000 {
            bits = bits &* 6364136223846793005 &+ 1442695040888963407
            let value = Double(bitPattern: bits)
            if value.isFinite { _ = try compare("{\"clock\":\(value)}") }
        }
        _ = try compare(#"{"ops":[{"op":"roots","ids":[1.0,1e2,-0,4294967295.0]}]}"#)
    }

    func testMalformedBatchesFailAsOneUnreadableBatch() {
        let malformed = ["", "[]", "null", "{}x", "{", #"{"ops":[{}]}"#,
            #"{"ops":[{"op":"create","id":4294967296}]}"#, #"{"ops":[{"op":"roots","ids":[1.2]}]}"#,
            #"{"ops":[{"op":"create","props":{"text":null}}]}"#, #"{"timers":1}"#,
            #"{"clock":01}"#, #"{"clock":1.}"#, #"{"clock":1e}"#, #"{"clock":NaN}"#, #"{"clock":1e999}"#,
            #"{"error":"\q"}"#, #"{"error":"\uD800"}"#, #"{"error":"\uDC00"}"#, #"{"error":"\uD800\u0041"}"#,
            #"{"ops":[{"op":"paragraph","runs":[{"id":1}]}]}"#,
            #"{"ops":[{"op":"paragraph","runs":[{"id":1,"parent":0,"style":{"text_color":[1,2,3]}}]}]}"#]
        for text in malformed {
            XCTAssertThrowsError(try JSONDecoder().decode(OracleBatch.self, from: Data(text.utf8)), text)
            XCTAssertEqual(Batch.decode(Data(text.utf8)).error, "unreadable batch", text)
            XCTAssertTrue(Batch.decode(Data(text.utf8)).ops.isEmpty, text)
        }
        for text in [#"{"error":"a"}"#, #"{"ops":[{"op":"paragraph","runs":[{"id":1,"parent":0}]}]}"#] {
            let bytes = Data(text.utf8)
            for end in 0..<bytes.count { XCTAssertEqual(Batch.decode(bytes.prefix(end)).error, "unreadable batch") }
        }
        XCTAssertEqual(Batch.decode(Data([123,34,101,114,114,111,114,34,58,34,0xff,34,125])).error, "unreadable batch")
        XCTAssertEqual(Batch.decode(Data(("{\"unknown\":" + String(repeating: "[", count: 513) + "0" + String(repeating: "]", count: 513) + "}").utf8)).error, "unreadable batch")
    }

    func testTransformReplyUsesTheSameBatchReader() throws {
        let wire = #"{"accepted":true,"committed":true,"runtime":"18446744073709551615","geometrySequence":"2","translateToken":"3","scaleToken":"4","value":[1,2,3],"batch":{"ops":[{"op":"present","id":1,"x":0.1}],"timer_due_ms":123.25,"timers":true}}"#
        let reply = try XCTUnwrap(TransformDragReply(Data(wire.utf8)))
        XCTAssertTrue(reply.accepted); XCTAssertTrue(reply.committed)
        XCTAssertEqual(reply.runtime, UInt64.max); XCTAssertEqual(reply.sequence, 2)
        XCTAssertEqual(reply.translate, 3); XCTAssertEqual(reply.scale, 4)
        XCTAssertEqual(reply.value?.values, [1,2,3]); XCTAssertEqual(reply.batch.timerDueMs, 123.25)
        XCTAssertNil(TransformDragReply(Data(#"{"batch":{"ops":[{}]}}"#.utf8)))
    }
}

#if os(macOS)
extension BatchDecodeTests {
    func testEveryBatchFromRealSession() throws {
        _ = NSApplication.shared
        let session = ExactApp.shared.makeSession(label: "batch-equivalence")
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 900, height: 700),
                              styleMask: [], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = session.presenter.viewport
        session.clock = 0
        session.runtime.observeBatch = { [unowned self] data, batch in
            do { try self.compare(data, decoded: batch) }
            catch { XCTFail("Foundation refused real session batch: \(error)") }
        }
        defer { session.runtime.observeBatch = nil; session.destroy(); window.close() }
        XCTAssertNil(session.boot(size: CGSize(width: 900, height: 700)).error)
        func drain() { RunLoop.current.run(until: Date(timeIntervalSinceNow: 0.002)) }
        func node(_ id: String) -> NodeView? { session.presenter.views.values.first { $0.props["testId"] == id } }
        if node("open-file") != nil {
            // The command running this test can name the actual probe corpus
            // and dense document; normal host tests use repository documents.
            let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
                .deletingLastPathComponent().deletingLastPathComponent()
                .deletingLastPathComponent().deletingLastPathComponent()
            let paths = ProcessInfo.processInfo.environment["EXACT_BATCH_DOCUMENTS"]?.components(separatedBy: "\n")
                ?? [root.appendingPathComponent("README.md").path,
                    root.appendingPathComponent("llp/1044.000-text-performance-claude.plan.md").path]
            for path in paths {
                let before = compared, beforeOps = operations, beforeParagraphs = paragraphs
                XCTAssertTrue(FileManager.default.fileExists(atPath: path), path)
                XCTAssertTrue(session.change(testId: "open-file", value: path))
                let deadline = Date(timeIntervalSinceNow: 30)
                let name = URL(fileURLWithPath: path).lastPathComponent
                while node("document-name")?.props["text"] != name && Date() < deadline {
                    session.apply(session.runtime.pump(now: session.now())); drain()
                }
                XCTAssertEqual(node("document-name")?.props["text"], name)
                let list = try XCTUnwrap(node("document"))
                let scroll = try XCTUnwrap(list.scroll)
                for fraction in (0...128).map({ CGFloat($0) / 128 }) + [0.5, 0.1, 0.9, 0] {
                    let extent = max(0, (scroll.documentView?.frame.height ?? 0) - scroll.contentSize.height)
                    scroll.contentView.scroll(to: CGPoint(x: 0, y: extent * fraction))
                    session.presenter.scrolled()
                    session.presenter.syncLists()
                    session.presenter.pump()
                    drain()
                }
                for width: CGFloat in [640, 1200, 900] {
                    session.resize(CGSize(width: width, height: 700)); drain()
                }
                XCTAssertGreaterThan(paragraphs - beforeParagraphs, 0)
                print("BATCH EQUIVALENCE \(name): batches=\(compared - before) ops=\(operations - beforeOps) paragraphs=\(paragraphs - beforeParagraphs)")
            }
        } else {
            // Caltrain is the other real archive used by this same test target.
            let change = try XCTUnwrap(node("change-station"))
            session.apply(session.runtime.press(change.id, now: 0))
            XCTAssertTrue(session.change(testId: "station-search", value: "Palo"))
            for _ in 0..<8 { session.apply(session.runtime.pump(now: 0)); drain() }
            session.apply(session.runtime.advance(now: 60_000))
            for width: CGFloat in [390, 900] { session.resize(CGSize(width: width, height: 700)); drain() }
            print("BATCH EQUIVALENCE Caltrain: batches=\(compared) ops=\(operations) paragraphs=\(paragraphs)")
        }
        XCTAssertGreaterThan(compared, 4)
        XCTAssertGreaterThan(operations, 20)
    }
}
#endif
