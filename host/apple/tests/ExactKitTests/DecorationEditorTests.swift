#if os(macOS)
import AppKit
import CoreText
import CExact
import XCTest
@testable import ExactKit
@testable import ExactMarkdown

final class DecorationEditorTests: XCTestCase {
    private var window: NSWindow!
    private var session: ExactSession!
    func testIntrinsicMetricsUseTheSameDecoratedTextKitStorage() throws {
        let text = "A heading"
        let engine = TextEngine(resolve:{_ in nil})
        func measure(_ size:Int,_ value:String = "A heading",width:Float = 300) throws -> ExactMetrics? {
            let decorated = try record(value,blocks:[["name":"heading","start":0,"end":value.utf16.count]])
            var source = value
            return source.withUTF8 { bytes in
                var run = ExactTextRun()
                run.text = bytes.baseAddress; run.len = bytes.count
                run.font_size = 16; run.font_weight = 400
                var request = ExactMeasureRequest()
                request.strut = run; request.width = width; request.count = 1; request.white_space = 1
                return withUnsafePointer(to:&run) { pointer in
                    request.runs = pointer
                    return DecorationMeasurement.measure(engine,request:request,record:decorated,styles:"{\"heading\":[[64,\(size)]]}",root:16,scale:1)
                }
            }
        }
        let plain = try XCTUnwrap(measure(16)), heading = try XCTUnwrap(measure(32))
        XCTAssertGreaterThan(heading.height,plain.height*1.5)
        XCTAssertGreaterThan(heading.width,plain.width*1.5)
        let minimum = try XCTUnwrap(measure(32,width:-2)), maximum = try XCTUnwrap(measure(32,width:-1))
        XCTAssertLessThan(minimum.width,maximum.width,"min-content measures unbreakable styled pieces")
        let trailing = try XCTUnwrap(measure(16,text + "\n"))
        XCTAssertGreaterThan(trailing.height,plain.height,"a trailing newline reserves its caret line")
        XCTAssertEqual(text,"A heading")
    }
    override func setUp() { _ = NSApplication.shared; ExactMarkdown.install() }
    override func tearDown() { window?.close(); session?.destroy(); window = nil; session = nil }

    private func record(_ source: String, spans: [[String: Any]] = [], blocks: [[String: Any]] = [], hidden: [[String: Any]] = [], atoms: [[String: Any]] = []) throws -> String {
        String(decoding: try JSONSerialization.data(withJSONObject: ["value":source,"spans":spans,"blocks":blocks,"hidden":hidden,"atoms":atoms]), as: UTF8.self)
    }
    private func fixture(_ source: String, record: String) throws -> (Presenter, NodeView, TextArea, DecorationEditor) {
        session = ExactApp.shared.makeSession(label: "decoration-test")
        let presenter = session.presenter
        presenter.apply(wireBatch([
            ["op":"create","id":1,"kind":"view"],
            ["op":"create","id":2,"kind":"textarea","props":["id":"editor","value":source,"decorations":record,"decorationStyles":"{\"strong\":[[65,700]],\"chip\":[]}"],"handlers":["input","selectionchange"]],
            ["op":"children","id":1,"ids":[2]], ["op":"roots","ids":[1]],
            ["op":"frame","id":1,"x":0,"y":0,"w":400,"h":300],
            ["op":"frame","id":2,"x":0,"y":0,"w":180,"h":200],
        ]))
        let node = try XCTUnwrap(presenter.views[2]), field = try XCTUnwrap(node.textArea as? TextArea)
        window = NSWindow(contentRect: NSRect(x:0,y:0,width:400,height:300),styleMask:[.titled],backing:.buffered,defer:false)
        window.isReleasedWhenClosed = false; window.contentView = presenter.viewport
        window.makeFirstResponder(field)
        node.restyleMarkup()
        return (presenter,node,field,try XCTUnwrap(field.decorations as? DecorationEditor))
    }

    func testHiddenSourceHasZeroAdvanceCaretStopsAndAccessibilityProjection() throws {
        let source = "**hello** @file"
        let decorated = try record(source, spans:[["name":"strong","start":2,"end":7]], hidden:[["start":0,"end":2],["start":7,"end":9]], atoms:[["name":"chip","start":10,"end":15,"label":"File"]])
        let (_,node,field,editor) = try fixture(source,record:decorated)
        XCTAssertEqual(field.string,source)
        let storage = try XCTUnwrap(field.textStorage)
        let strong = try XCTUnwrap(storage.attribute(.font,at:2,effectiveRange:nil) as? NSFont)
        XCTAssertTrue(strong.fontDescriptor.symbolicTraits.contains(.bold))
        let font = try XCTUnwrap(storage.attribute(.font,at:0,effectiveRange:nil) as? NSFont)
        var chars: [UniChar] = [42], glyphs: [CGGlyph] = [0], advance = CGSize.zero
        CTFontGetGlyphsForCharacters(font,&chars,&glyphs,1)
        CTFontGetAdvancesForGlyphs(font,.horizontal,&glyphs,&advance,1)
        XCTAssertEqual(advance.width,0)
        field.setSelectedRange(NSRange(location:0,length:0)); field.setSelectedRange(NSRange(location:1,length:0))
        XCTAssertEqual(field.selectedRange().location,2)
        field.setSelectedRange(NSRange(location:6,length:0)); field.setSelectedRange(NSRange(location:8,length:0))
        XCTAssertEqual(field.selectedRange().location,9)
        XCTAssertEqual(editor.projection?.value,"hello File")
        XCTAssertEqual(field.accessibilityValue(),"hello File")
        XCTAssertEqual(field.accessibilityNumberOfCharacters(),10)
        let line = field.accessibilityRange(forLine:0)
        XCTAssertEqual(field.accessibilityString(for:line),"hello File")
        XCTAssertEqual(field.accessibilityLine(for:9),0)
        XCTAssertEqual(node.fieldText,source)
    }

    func testDecorationBaseUsesTheSameNativeFieldInkAsAnUndecoratedField() throws {
        let (presenter,node,field,_) = try fixture("plain",record:record("plain"))
        presenter.apply(wireBatch([["op":"style","id":2,"style":["text_color":[255,0,0,255]]]]))
        node.restyleMarkup()
        let ink = try XCTUnwrap(field.textStorage?.attribute(.foregroundColor,at:0,effectiveRange:nil) as? NSColor)
        XCTAssertEqual(ink,node.fieldInk,"an inherited prose color does not replace the native FieldText role")
    }

    func testTouchingRevealShowsSyntaxAndCompositionDefersMetadata() throws {
        let source = "**hello**"
        let decorated = try record(source, hidden:[["start":0,"end":2,"reveal":["start":0,"end":9]],["start":7,"end":9,"reveal":["start":0,"end":9]]])
        let (_,node,field,editor) = try fixture(source,record:decorated)
        field.setSelectedRange(NSRange(location:4,length:0)); node.restyleMarkup()
        XCTAssertEqual(editor.hidden,[])
        field.setMarkedText("に",selectedRange:NSRange(location:1,length:0),replacementRange:NSRange(location:NSNotFound,length:0))
        let before = editor.state["source"] as? String
        node.props["decorations"] = try record(field.string)
        node.restyleMarkup()
        XCTAssertEqual(editor.state["source"] as? String,before)
        XCTAssertTrue(field.hasMarkedText())
        field.unmarkText(); node.restyleMarkup()
        XCTAssertEqual(editor.state["source"] as? String,field.string)
        XCTAssertEqual(field.typingAttributes[.font] as? NSFont,node.text?.font(size:node.number("font_size",PageFacts.defaultRootFontSize),weight:400,family:0,italic:false))
    }

    func testProgrammaticUndoIsOneStepRestoresSourceAndSelectionAndKeepsTypingHistory() throws {
        let source = "hello"
        let (p,node,field,_) = try fixture(source,record:record(source))
        let manager = try XCTUnwrap(field.undoManager)
        manager.groupsByEvent = false
        var replacements: [String] = []
        p.fieldSelections.onReplacement = { _,value,_ in replacements.append(value) }
        field.setSelectedRange(NSRange(location:1,length:3))
        p.fieldSelections.setRangeText(["editor","XY","undo"])
        XCTAssertEqual(field.string,"hXYo")
        XCTAssertEqual(replacements,["hXYo"])
        XCTAssertTrue(manager.canUndo)
        manager.undo()
        XCTAssertEqual(field.string,"hello")
        XCTAssertEqual(field.selectedRange(),NSRange(location:1,length:3))
        XCTAssertFalse(manager.canUndo,"one requested replacement is one step")
        manager.redo()
        XCTAssertEqual(field.string,"hXYo")
        XCTAssertEqual(p.fieldSelections.current(node).range,NSRange(location:1,length:2))
        XCTAssertEqual(replacements,["hXYo"],"undo is history input, not another app replacement")
        manager.undo()
        manager.beginUndoGrouping()
        field.setSelectedRange(NSRange(location:5,length:0))
        field.insertText("!",replacementRange:field.selectedRange())
        field.breakUndoCoalescing()
        manager.endUndoGrouping()
        field.setSelectedRange(NSRange(location:1,length:3))
        p.fieldSelections.setRangeText(["editor","Z","undo"])
        XCTAssertEqual(field.string,"hZo!")
        manager.undo()
        XCTAssertEqual(field.string,"hello!")
        XCTAssertTrue(manager.canUndo,"the app edit preserves preceding native typing")
        manager.undo()
        XCTAssertEqual(field.string,"hello")
    }

    func testRangeGeometryWrapsScrollsAndBecomesUnavailableWhenDetached() throws {
        let source = String(repeating:"A wrapping field ",count:20)
        let (p,node,field,_) = try fixture(source,record:record(source))
        let first = try XCTUnwrap(p.rangeFrame(2,start:0,end:5))
        let all = try XCTUnwrap(p.rangeFrame(2,start:0,end:UInt32(source.utf16.count)))
        XCTAssertGreaterThan(all.height,first.height)
        node.textAreaScroll?.contentView.scroll(to:NSPoint(x:0,y:30))
        let moved = try XCTUnwrap(p.rangeFrame(2,start:0,end:5))
        XCTAssertLessThan(moved.minY,first.minY)
        XCTAssertNotNil(p.rangeFrame(2,start:0,end:0))
        XCTAssertNil(p.rangeFrame(2,start:0,end:UInt32(source.utf16.count + 1)))
        XCTAssertNotNil(field.textLayoutManager)
        node.superview?.isHidden = true
        XCTAssertNil(p.rangeFrame(2,start:0,end:5))
        node.superview?.isHidden = false
        node.removeFromSuperview()
        XCTAssertNil(p.rangeFrame(2,start:0,end:5))
    }

    func testBoundSourceUndoAndSelectionRevealRemeasureTheActualField() throws {
        let source = #"""
        component App
          state draft = "**hello world**\nend"
          state inputType = "ready"
          derive ranges = TextDecorations(value="**hello world**\nend",spans=[],blocks=[],hidden=[TextDecorationHidden(start=0,end=2,reveal=some(TextDecorationRange(start=0,end=15))),TextDecorationHidden(start=13,end=15,reveal=some(TextDecorationRange(start=0,end=15)))],atoms=[])
          action edit(value: string,e: InputEvent)
            draft = value
            inputType = e.inputType
          view
            column width=400 height=400
              textarea id="editor" testId="editor" value=draft input=edit decorations=ranges appearance="none" field-sizing="content" width=100 font-size=16
              text `${draft}|${inputType}` testId="bound"
        """#
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at:directory,withIntermediateDirectories:true)
        defer { try? FileManager.default.removeItem(at:directory) }
        let entry = directory.appendingPathComponent("app.contract"), plan = directory.appendingPathComponent("app.plan")
        try source.write(to:entry,atomically:true,encoding:.utf8)
        let compiler = Process()
        compiler.executableURL = URL(fileURLWithPath:try XCTUnwrap(ProcessInfo.processInfo.environment["EXACT_CONTRACT"]))
        compiler.arguments = ["build",entry.path,"-o",plan.path]
        try compiler.run(); compiler.waitUntilExit()
        XCTAssertEqual(compiler.terminationStatus,0)
        session = ExactApp.shared.makeSession(label:"bound-decorations")
        window = NSWindow(contentRect:NSRect(x:0,y:0,width:400,height:400),styleMask:[.titled],backing:.buffered,defer:false)
        window.isReleasedWhenClosed = false; window.contentView = session.presenter.viewport
        XCTAssertNil(session.boot(plan:try Data(contentsOf:plan),size:CGSize(width:400,height:400)).error)
        let presenter = session.presenter
        let node = try XCTUnwrap(presenter.views.values.first { $0.props["testId"] == "editor" })
        let bound = try XCTUnwrap(presenter.views.values.first { $0.props["testId"] == "bound" })
        let field = try XCTUnwrap(node.textArea as? TextArea)
        window.makeFirstResponder(field)
        func drainLayout() {
            let drained = expectation(description:"selection-triggered native measure completed")
            DispatchQueue.main.async { drained.fulfill() }
            wait(for:[drained],timeout:2)
        }
        field.setSelectedRange(NSRange(location:19,length:0)); drainLayout()
        let compact = node.frame.height
        func waitForLayout(_ changed: () -> Bool) {
            let deadline = Date().addingTimeInterval(2)
            while !changed() && Date() < deadline { RunLoop.main.run(mode:.default,before:deadline) }
        }
        field.setSelectedRange(NSRange(location:7,length:0))
        waitForLayout { node.frame.height > compact }
        XCTAssertGreaterThan(node.frame.height,compact,"revealed syntax participates in content sizing")
        field.setSelectedRange(NSRange(location:19,length:0))
        waitForLayout { abs(node.frame.height - compact) < 0.5 }
        XCTAssertEqual(node.frame.height,compact,accuracy:0.5)
        presenter.fieldSelections.setRangeText(["editor","Hi",2,7,"select","undo"])
        XCTAssertEqual(bound.paragraphText,"**Hi world**\nend|insertReplacementText")
        field.undo(nil)
        XCTAssertEqual(bound.paragraphText,"**hello world**\nend|historyUndo")
        field.redo(nil)
        XCTAssertEqual(bound.paragraphText,"**Hi world**\nend|historyRedo")
    }
}
#endif
