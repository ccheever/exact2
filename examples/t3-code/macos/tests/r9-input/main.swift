import AppKit
import XCTest

// Lane r9-input (R9Input.swift): the composer's focus reaches its node on a click, not only on
// typing; ⌘B bolds under a non-Latin source and while a syllable is composing; a press elsewhere
// ends composition first; the transcript remembers where a thread was left and restores it.
// Real AppKit views in a real window; compiled with every file in modules/apple.

private let r9Resolve: ExactHooks.ResolveFn = { _, _, _, _, _ in 0 }
private let r9Act: ExactHooks.ActFn = { _, _, _ in 0 }
private let r9Log: ExactHooks.LogFn = { _, _, _ in }
private let r9Delegate: ExactHooks.DelegateFn = { _, _, _ in }
private func makeHooks() -> ExactHooks {
    let table = UnsafeMutableRawPointer.allocate(byteCount: 40, alignment: 8)
    table.initializeMemory(as: UInt8.self, repeating: 0, count: 40)
    table.storeBytes(of: UInt32(40), as: UInt32.self)
    table.storeBytes(of: unsafeBitCast(r9Resolve, to: UnsafeRawPointer.self), toByteOffset: 8, as: UnsafeRawPointer.self)
    table.storeBytes(of: unsafeBitCast(r9Act, to: UnsafeRawPointer.self), toByteOffset: 16, as: UnsafeRawPointer.self)
    table.storeBytes(of: unsafeBitCast(r9Log, to: UnsafeRawPointer.self), toByteOffset: 24, as: UnsafeRawPointer.self)
    table.storeBytes(of: unsafeBitCast(r9Delegate, to: UnsafeRawPointer.self), toByteOffset: 32, as: UnsafeRawPointer.self)
    return ExactHooks(host: nil, table: UnsafeRawPointer(table))!
}

/// Stands in for Exact's node, the text view's real delegate: focus on begin-editing, blur on end.
final class Node: NSObject, NSTextViewDelegate {
    var events: [String] = []
    func textDidBeginEditing(_ notification: Notification) { events.append("focus") }
    func textDidEndEditing(_ notification: Notification) { events.append("blur") }
}
final class Flipped: NSView { override var isFlipped: Bool { true } }
private func tick(_ seconds: TimeInterval = 0.02) { let end = Date(timeIntervalSinceNow: seconds); while Date() < end { RunLoop.current.run(mode: .default, before: end) } }

final class ComposerFixture {
    let hooks = makeHooks()
    let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 420, height: 240), styleMask: [.titled], backing: .buffered, defer: false)
    let scroller = NSScrollView(frame: NSRect(x: 0, y: 0, width: 400, height: 120))
    let editor = NSTextView(frame: NSRect(x: 0, y: 0, width: 400, height: 120))
    let button = NSButton(frame: NSRect(x: 0, y: 160, width: 80, height: 30))
    let node = Node()
    let composer = T3Composer()
    let r9 = R9Input(agent: true)
    let element: ExactElement
    init() {
        _ = NSApplication.shared
        window.isReleasedWhenClosed = false
        editor.isEditable = true; editor.isRichText = false; editor.allowsUndo = true
        editor.delegate = node
        scroller.documentView = editor
        window.contentView?.addSubview(scroller); window.contentView?.addSubview(button)
        element = ExactElement(hook: .t3Composer, id: "composer", node: 1, hooks: hooks)
        element.view = scroller; element.platform = editor
        element.data = ExactData(["snapshot-owner": "owner-a"])
        composer.install(element) // the real delegate proxy goes in front of the node
        composer.editor.styler.richText = true
        r9.install(element)
    }
    func key(_ characters: String, _ ignoring: String, _ code: UInt16, _ flags: NSEvent.ModifierFlags = [.command]) -> NSEvent {
        NSEvent.keyEvent(with: .keyDown, location: .zero, modifierFlags: flags, timestamp: 0, windowNumber: window.windowNumber, context: nil,
            characters: characters, charactersIgnoringModifiers: ignoring, isARepeat: false, keyCode: code)!
    }
    func press(at point: NSPoint) -> NSEvent {
        NSEvent.mouseEvent(with: .leftMouseDown, location: point, modifierFlags: [], timestamp: 0, windowNumber: window.windowNumber, context: nil, eventNumber: 0, clickCount: 1, pressure: 1)!
    }
}

final class R9InputTests: XCTestCase {
    func testClickFocusReachesTheNodeOnce() {
        let f = ComposerFixture()
        f.window.makeFirstResponder(f.button); tick()
        XCTAssertEqual(f.node.events.filter { $0 == "focus" }.count, 0)
        // A click into an existing draft: first responder only, no edit.
        f.editor.string = "existing draft"
        f.window.makeFirstResponder(f.editor); tick()
        XCTAssertEqual(f.node.events.filter { $0 == "focus" }.count, 1, "focus reaches Exact's node through the proxy without typing")
        XCTAssertTrue(f.r9.composerFocused)
        f.window.makeFirstResponder(f.button); tick()
        XCTAssertEqual(f.node.events.last, "blur")
        XCTAssertFalse(f.r9.composerFocused)
        f.window.makeFirstResponder(f.editor); tick()
        XCTAssertEqual(f.node.events.filter { $0 == "focus" }.count, 2, "each return of the focus is a new focus")
        XCTAssertEqual(f.r9.bridged, 2)
    }

    func testChordKeyUnderAKoreanSource() {
        let f = ComposerFixture()
        XCTAssertEqual(R9Input.chordKey(f.key("b", "b", 11)), "b")
        XCTAssertEqual(R9Input.chordKey(f.key("ㅠ", "ㅠ", 11)), "b", "Korean 2-Set reports ㅠ for the B key")
        XCTAssertEqual(R9Input.chordKey(f.key("ㅑ", "ㅑ", 34)), "i")
        XCTAssertEqual(R9Input.chordKey(f.key("ㅠ", "ㅠ", 99)), "ㅠ", "an unmapped key keeps its characters")
    }

    func testCommandBBoldsAfterAClickAndUnderKorean() {
        let f = ComposerFixture()
        f.editor.string = "draft"
        f.window.makeFirstResponder(f.editor); tick()
        f.editor.setSelectedRange(NSRange(location: 0, length: 5))
        XCTAssertNil(f.composer.handle(f.key("b", "b", 11)), "⌘B is consumed")
        XCTAssertEqual(f.editor.string, "**draft**")
        f.editor.string = "word"; f.editor.setSelectedRange(NSRange(location: 0, length: 4))
        XCTAssertNil(f.composer.handle(f.key("ㅠ", "ㅠ", 11)), "⌘B under Korean 2-Set is consumed")
        XCTAssertEqual(f.editor.string, "**word**")
    }

    func testCommandBEndsCompositionThenBolds() {
        let f = ComposerFixture()
        f.window.makeFirstResponder(f.editor); tick()
        f.editor.string = ""
        f.editor.insertText("안", replacementRange: f.editor.selectedRange())
        f.editor.setMarkedText("녕", selectedRange: NSRange(location: 1, length: 0), replacementRange: NSRange(location: NSNotFound, length: 0))
        XCTAssertTrue(f.editor.hasMarkedText())
        XCTAssertNil(f.composer.handle(f.key("ㅠ", "ㅠ", 11)))
        XCTAssertFalse(f.editor.hasMarkedText(), "composition ended")
        XCTAssertTrue(f.editor.string.hasPrefix("안녕"), "the composed syllable stays: \(f.editor.string)")
        XCTAssertTrue(f.editor.string.contains("**"), "and the chord toggled bold: \(f.editor.string)")
    }

    func testPressElsewhereEndsComposition() {
        let f = ComposerFixture()
        f.window.makeFirstResponder(f.editor); tick()
        f.editor.string = ""
        f.editor.setMarkedText("한", selectedRange: NSRange(location: 1, length: 0), replacementRange: NSRange(location: NSNotFound, length: 0))
        f.r9.handle(f.press(at: NSPoint(x: 20, y: 60))) // inside the composer: composition goes on
        XCTAssertTrue(f.editor.hasMarkedText())
        f.r9.handle(f.press(at: NSPoint(x: 20, y: 175))) // on the button
        XCTAssertFalse(f.editor.hasMarkedText())
        XCTAssertEqual(f.editor.string, "한")
    }

    func testRemountedFieldKeepsItsCaret() {
        let f = ComposerFixture()
        let old = NSTextField(frame: NSRect(x: 100, y: 200, width: 200, height: 22)); old.stringValue = "set"
        f.window.contentView?.addSubview(old)
        f.window.makeFirstResponder(old); tick()
        (old.currentEditor() as? NSTextView)?.setSelectedRange(NSRange(location: 3, length: 0)); tick()
        // The appearance flips: the old subtree goes, the new one's field takes the focus on mount.
        old.removeFromSuperview()
        let new = NSTextField(frame: NSRect(x: 100, y: 200, width: 200, height: 22)); new.stringValue = "set"
        f.window.contentView?.addSubview(new)
        f.window.makeFirstResponder(new); tick(); tick()
        XCTAssertEqual((new.currentEditor() as? NSTextView)?.selectedRange(), NSRange(location: 3, length: 0), "the caret, not a select-all")
        XCTAssertEqual(f.r9.carried, 1)
        // A field focused afresh (another text, or the old one still in the window) keeps AppKit's select-all.
        let other = NSTextField(frame: NSRect(x: 100, y: 170, width: 200, height: 22)); other.stringValue = "else"
        f.window.contentView?.addSubview(other)
        f.window.makeFirstResponder(other); tick(); tick()
        XCTAssertEqual((other.currentEditor() as? NSTextView)?.selectedRange(), NSRange(location: 0, length: 4))
        XCTAssertEqual(f.r9.carried, 1)
    }

    // MARK: Transcript position

    final class TranscriptFixture {
        let hooks = makeHooks()
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 400, height: 400), styleMask: [.titled], backing: .buffered, defer: false)
        let scroll = NSScrollView(frame: NSRect(x: 0, y: 0, width: 400, height: 300))
        let document = Flipped(frame: NSRect(x: 0, y: 0, width: 400, height: 3000))
        let r9: R9Input
        let transcript: ExactElement
        var turns: [ExactElement] = []
        init(agent: Bool = true) {
            r9 = R9Input(agent: agent)
            _ = NSApplication.shared
            window.isReleasedWhenClosed = false
            scroll.documentView = document
            window.contentView?.addSubview(scroll)
            transcript = ExactElement(hook: .t3Transcript, id: "transcript", node: 1, hooks: hooks)
            transcript.view = scroll; transcript.platform = scroll
            owner("thread-a")
            rows(prefix: "a")
        }
        func owner(_ name: String) { transcript.data = ExactData(["timeline-owner": name]); r9.install(transcript) }
        func rows(prefix: String) {
            for element in turns { r9.remove(element); element.view?.removeFromSuperview() }
            turns = (0..<6).map { index in
                let row = Flipped(frame: NSRect(x: 0, y: CGFloat(index) * 500, width: 400, height: 480))
                document.addSubview(row)
                let element = ExactElement(hook: .t3Turn, id: "", node: UInt32(10 + index), hooks: hooks)
                element.view = row; element.data = ExactData(["turn": "\(prefix)\(index)"])
                r9.install(element)
                return element
            }
        }
        /// A reader's scroll: small steps, as a wheel's frames are.
        func glide(_ y: CGFloat) {
            var at = top
            while abs(at - y) > 0.5 { at += max(-150, min(150, y - at)); scrollTo(at) }
        }
        func scrollTo(_ y: CGFloat) { scroll.contentView.scroll(to: NSPoint(x: 0, y: y)); scroll.reflectScrolledClipView(scroll.contentView); r9.sample(); r9.sample(); tick() }
        var top: CGFloat { scroll.contentView.bounds.minY }
        /// A wheel event over the transcript, as the local monitor sees it (`momentum`: the CG
        /// momentum phase, 2 continue, 3 end). A synthesized event carries no window, so its
        /// location is given in screen coordinates and the window is passed alongside.
        func wheel(momentum: Int64 = 0, phase: Int64 = 0) {
            let event = CGEvent(scrollWheelEvent2Source: nil, units: .pixel, wheelCount: 1, wheel1: -10, wheel2: 0, wheel3: 0)!
            event.setIntegerValueField(.scrollWheelEventMomentumPhase, value: momentum)
            event.setIntegerValueField(.scrollWheelEventScrollPhase, value: phase)
            event.location = CGPoint(x: 100, y: (NSScreen.screens.first?.frame.height ?? 0) - 100)
            r9.handle(NSEvent(cgEvent: event)!, window: window)
        }
        /// One move without a run-loop turn after it (the host's layout pass, then the next batch).
        func move(_ y: CGFloat) { scroll.contentView.scroll(to: NSPoint(x: 0, y: y)); scroll.reflectScrolledClipView(scroll.contentView); r9.sample() }
        func restore() { for _ in 0..<12 where r9.isRestoring { r9.step() } }
    }

    /// A real (non-agent) run: the wheel's own frames are kept, its momentum tail and the layout
    /// settling after its last event are kept too, and the thread left keeps that settled position
    /// even when the switch's rows land before the owner changes.
    func testRealWheelTailIsKeptWhenTheThreadIsLeft() {
        let f = TranscriptFixture(agent: false)
        var clock: TimeInterval = 100
        f.r9.now = { clock }
        f.scrollTo(2700)
        XCTAssertNil(f.r9.remembered("thread-a"), "a programmatic move outside a gesture is not the reader's")
        f.wheel(phase: 1); f.glide(1800)
        XCTAssertEqual(f.r9.remembered("thread-a")?.scrollOffset, 1800)
        clock += 0.4; f.wheel(momentum: 2); f.glide(1500)
        clock += 0.4; f.wheel(momentum: 3); f.glide(1298)
        XCTAssertEqual(f.r9.remembered("thread-a")?.scrollOffset, 1298)
        // 1.5 s after the momentum's last event the view settles 64 pt further (past the 0.8 s
        // gesture window, where a real run used to drop it).
        clock += 1.5; f.move(1234); tick()
        XCTAssertTrue(f.r9.isSettling)
        // The switch: the next thread's rows and its jump to the end land first, then the owner.
        f.rows(prefix: "b"); f.move(2700); f.owner("thread-b")
        XCTAssertFalse(f.r9.isSettling)
        XCTAssertEqual(f.r9.remembered("thread-a"), R9Input.Position(rowId: "a2", offsetWithinRow: 234, scrollOffset: 1234, atEnd: false))
        tick()
        clock += 5
        f.owner("thread-a"); f.rows(prefix: "a"); f.scrollTo(2700)
        XCTAssertTrue(f.r9.isRestoring)
        f.restore()
        XCTAssertEqual(f.top, 1234, accuracy: 0.5, "back where the wheel left it, not 64 pt off")
    }

    /// Once the momentum has ended and nothing moves, the settled position is committed on its own;
    /// a jump in the tail (Edit from here, a jump to the end) is not the reader's and ends it.
    func testMomentumEndCommitsAndAJumpEndsTheTail() {
        let f = TranscriptFixture(agent: false)
        var clock: TimeInterval = 100
        f.r9.now = { clock }
        f.wheel(phase: 1); f.glide(900)
        clock += 0.3; f.wheel(momentum: 3); f.glide(840)
        clock += 1.0; f.move(776); tick()
        XCTAssertEqual(f.r9.remembered("thread-a")?.scrollOffset, 840, "pending until quiet")
        tick(0.5)
        XCTAssertEqual(f.r9.remembered("thread-a")?.scrollOffset, 776, "committed once the motion is quiet")
        XCTAssertEqual(f.r9.remembered("thread-a")?.rowId, "a1")
        // Still in the tail: a programmatic jump is not kept and ends the tail.
        clock += 0.5; f.move(2700); tick(0.5)
        XCTAssertFalse(f.r9.isSettling)
        XCTAssertEqual(f.r9.remembered("thread-a")?.scrollOffset, 776)
        // Past the tail, small programmatic moves stay unrecorded as before.
        clock += 5; f.move(2650); tick(0.5)
        XCTAssertEqual(f.r9.remembered("thread-a")?.scrollOffset, 776)
    }

    /// A press in the transcript commits the tail at once (the reader stopped there).
    func testPressCommitsTheTail() {
        let f = TranscriptFixture(agent: false)
        var clock: TimeInterval = 100
        f.r9.now = { clock }
        f.wheel(phase: 1); f.glide(1500)
        clock += 1.2; f.move(1436); tick()
        let down = NSEvent.mouseEvent(with: .leftMouseDown, location: NSPoint(x: 100, y: 100), modifierFlags: [], timestamp: 0, windowNumber: f.window.windowNumber, context: nil, eventNumber: 0, clickCount: 1, pressure: 1)!
        f.r9.handle(down)
        XCTAssertFalse(f.r9.isSettling)
        XCTAssertEqual(f.r9.remembered("thread-a")?.scrollOffset, 1436)
    }

    func testLeftScrolledUpRestoresTheRowAndOffset() {
        let f = TranscriptFixture()
        f.scrollTo(2700) // the end
        f.glide(1234)
        XCTAssertEqual(f.r9.remembered("thread-a"), R9Input.Position(rowId: "a2", offsetWithinRow: 234, scrollOffset: 1234, atEnd: false))
        // Another thread opens at its end.
        f.owner("thread-b"); f.rows(prefix: "b"); f.scrollTo(2700); f.glide(2400); f.glide(2700)
        XCTAssertFalse(f.r9.isRestoring)
        XCTAssertEqual(f.r9.remembered("thread-b")?.atEnd, true)
        // Back to thread A: the host first lands at the end (scrollTop jump), then the row returns.
        f.owner("thread-a"); f.rows(prefix: "a"); f.scrollTo(2700)
        XCTAssertTrue(f.r9.isRestoring)
        for _ in 0..<12 where f.r9.isRestoring { f.r9.step() }
        XCTAssertFalse(f.r9.isRestoring)
        XCTAssertEqual(f.top, 1234, accuracy: 0.5)
    }

    func testRowMovedAboveKeepsItsOffset() {
        let f = TranscriptFixture()
        f.glide(1234)
        f.owner("thread-b"); f.rows(prefix: "b")
        // History grew above: thread A's rows now start 1000pt lower.
        f.owner("thread-a"); f.rows(prefix: "a")
        for element in f.turns { element.view?.frame.origin.y += 1000 }
        for _ in 0..<12 where f.r9.isRestoring { f.r9.step() }
        XCTAssertEqual(f.top, 2234, accuracy: 0.5, "the saved row, not the saved offset")
    }

    func testASwitchNeverRecordsUnderTheOldOwner() {
        let f = TranscriptFixture()
        f.glide(1234)
        // The new thread's rows land (a pure scroll to its end), then the owner changes in the same turn.
        f.scroll.contentView.scroll(to: NSPoint(x: 0, y: 2700)); f.scroll.reflectScrolledClipView(f.scroll.contentView); f.r9.sample()
        f.owner("thread-b")
        tick()
        XCTAssertEqual(f.r9.remembered("thread-a")?.rowId, "a2", "thread A keeps where it was left")
        // selectThread's jump to the end lands before the rows change: not the reader's position either.
        f.owner("thread-a"); for _ in 0..<12 where f.r9.isRestoring { f.r9.step() }
        f.scrollTo(2700)
        XCTAssertEqual(f.r9.remembered("thread-a")?.rowId, "a2")
        XCTAssertEqual(f.r9.remembered("thread-a")?.atEnd, false)
    }

    func testAtEndAndPressCancel() {
        let f = TranscriptFixture()
        f.glide(2700)
        XCTAssertEqual(f.r9.remembered("thread-a")?.atEnd, true)
        f.owner("thread-b"); f.rows(prefix: "b")
        f.owner("thread-a"); f.rows(prefix: "a")
        XCTAssertFalse(f.r9.isRestoring, "a thread left at its end opens at its end")
        f.glide(600)
        f.owner("thread-b"); f.rows(prefix: "b")
        f.owner("thread-a"); f.rows(prefix: "a")
        XCTAssertTrue(f.r9.isRestoring)
        let down = NSEvent.mouseEvent(with: .leftMouseDown, location: NSPoint(x: 100, y: 100), modifierFlags: [], timestamp: 0, windowNumber: f.window.windowNumber, context: nil, eventNumber: 0, clickCount: 1, pressure: 1)!
        f.r9.handle(down)
        XCTAssertFalse(f.r9.isRestoring, "a press in the transcript cancels the restoration")
    }
}

let suite = XCTestSuite(forTestCaseClass: R9InputTests.self)
suite.run()
let run = suite.testRun!
print("Executed \(run.executionCount) tests, with \(run.failureCount) failures")
exit(run.hasSucceeded ? 0 : 1)
