#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

/// A display frame that advances to a deadline whose commit shows nothing
/// skips the presenter's pass, as a timer's does: a `sent` reader is
/// answered at each MIDI message's time (LLP 1125 D6), and euxy's activity
/// log, off screen while the sequencer plays, cost the pass at every note.
final class FrameAdvanceMacTests: XCTestCase {
    private static let plan: Result<Data, Error> = Result {
        let dir = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: dir) }
        let source = """
            shape Message
              output: string
              data: number[]
              timeStamp: number

            shape MIDI
              state: string
              sent: Message[]

            component App
              resource midi = exactMIDI() as shape MIDI
              action send
                sendMIDI("", [144, 60, 100], at=performanceNow() + 50)
                sendMIDI("", [128, 60, 0], at=performanceNow() + 150)
                sendMIDI("", [144, 62, 100], at=performanceNow() + 250)
              view
                column
                  button press=send testId="send"
                    text "Send"
                  text (midi.sent.length >= 3 ? "three" : "") testId="three"

            """
        try source.write(to: dir.appendingPathComponent("app.contract"), atomically: true, encoding: .utf8)
        let compiler = Process()
        compiler.executableURL = URL(fileURLWithPath: try XCTUnwrap(ProcessInfo.processInfo.environment["EXACT_CONTRACT"]))
        compiler.arguments = ["build", dir.appendingPathComponent("app.contract").path, "-o", dir.appendingPathComponent("app.plan").path]
        try compiler.run(); compiler.waitUntilExit()
        XCTAssertEqual(compiler.terminationStatus, 0)
        return try Data(contentsOf: dir.appendingPathComponent("app.plan"))
    }

    private func sent(_ session: ExactSession) throws -> Int {
        let state = try XCTUnwrap(try JSONSerialization.jsonObject(with: Data(session.runtime.agent("{\"op\":\"state\"}").utf8)) as? [String: Any])
        let midi = (state["resources"] as? [String: Any])?["midi"] as? [String: Any]
        return try XCTUnwrap(midi?["sent"] as? [Any]).count
    }

    func testAFrameAdvanceThatShowsNothingSkipsThePass() throws {
        _ = NSApplication.shared
        let session = ExactApp.shared.makeSession(label: "frame-advance")
        let window = NSWindow(contentRect: NSRect(x: 100, y: 100, width: 400, height: 300), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        let view = ExactView(session: session)
        window.contentView = view
        defer { window.close(); session.destroy() }
        XCTAssertNil(session.boot(plan: try Self.plan.get(), size: CGSize(width: 400, height: 300)).error)
        let send = try XCTUnwrap(session.presenter.views.values.first { $0.props["testId"] == "send" })
        // The press under a held clock, so a slow pass cannot reach the
        // deadline before the frame does; then an explicitly supplied frame
        // advances this internal MIDI deadline while native clock wakes are pending.
        session.clock = session.now()
        session.apply(session.runtime.press(send.id, now: session.now()))
        session.clock = nil
        let due = try XCTUnwrap(session.timerDue, "the message's time answers the reader")
        session.frames.timerSoon = true
        XCTAssertEqual(try sent(session), 0)
        while session.now() < due { usleep(100) }
        let before = session.appliedBatches
        let link = view.displayLink(target: session.frames, selector: #selector(Frames.tick(_:)))
        defer { link.invalidate() }
        session.frames.tick(link)
        XCTAssertGreaterThanOrEqual(try sent(session), 1, "the frame advanced: the reader was answered")
        XCTAssertEqual(session.appliedBatches, before, "and its batch, which shows nothing, skipped the pass")
    }

}
#endif
