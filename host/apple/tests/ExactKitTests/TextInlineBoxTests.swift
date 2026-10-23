import XCTest
import CoreText
#if os(macOS)
import AppKit
#endif
@testable import ExactKit
@testable import ExactCore

final class TextInlineBoxTests: XCTestCase {
    func testAtomicRunReservesItsWidthAndRaisesOnlyItsLine() {
        let engine=TextEngine(resolve:{_ in nil})
        let base=Run(text:"",size:16,weight:400,family:0,italic:false,lineHeight:20,letterSpacing:0)
        var a=base; a.text="Hello "
        var object=base; object.text="\u{fffc}"; object.inlineBox=TextInlineBox(width:36,height:32,baseline:32)
        var b=base; b.text=" after the emoji"
        let spec=Spec(runs:[a,object,b],align:0,lineClamp:0,color:[0,0,0,255],strut:base)
        let paragraph=engine.paragraph(spec,width:110)
        XCTAssertGreaterThan(paragraph.lines.count,1)
        XCTAssertGreaterThanOrEqual(paragraph.firstBaseline,32)
        let source=engine.attributed(spec)
        let all=CTLineCreateWithAttributedString(source)
        let box=(CTLineGetGlyphRuns(all) as! [CTRun]).first { (CTRunGetAttributes($0) as NSDictionary)[kCTRunDelegateAttributeName] != nil }!
        XCTAssertEqual(CTRunGetTypographicBounds(box,CFRange(location:0,length:0),nil,nil,nil),36,accuracy:0.01)
        var bigger=spec; bigger.runs[1].inlineBox=TextInlineBox(width:80,height:50,baseline:50)
        XCTAssertNotEqual(TextMetricKey.hash(spec),TextMetricKey.hash(bigger))
        XCTAssertGreaterThanOrEqual(engine.paragraph(bigger,width:110).height,50)
    }
    func testFlowKeepsAnAtomicBoxOnItsSharedBaseline() throws {
        let engine = TextEngine(resolve: { _ in nil })
        let base = Run(text: "", size: 16, weight: 400, family: 0, italic: false, lineHeight: 20, letterSpacing: 0)
        var before = base; before.text = "Hello "
        var object = base; object.text = "\u{fffc}"
        object.inlineBox = TextInlineBox(width: 36, height: 32, baseline: 32)
        var after = base; after.text = " after the emoji"
        let spec = Spec(runs: [before, object, after], align: 0, lineClamp: 0, color: [0, 0, 0, 255], strut: base)
        let paragraph = engine.paragraph(spec, width: 200, flow: [TextFlowShape(kind: 0, x: 300, y: 1000, a: 10)])
        XCTAssertFalse(paragraph.fragments.isEmpty)
        XCTAssertFalse(paragraph.flowIncomplete)
        XCTAssertGreaterThanOrEqual(paragraph.firstBaseline, 32)
        let position = try XCTUnwrap(paragraph.atomicPositions(spec, width: 200).first)
        XCTAssertEqual(position.1.y + 32, paragraph.firstBaseline, accuracy: 0.01)
        XCTAssertGreaterThanOrEqual(paragraph.height, 32)
    }

    func testRtlObjectUsesItsVisualGlyphOrigin() {
        let engine=TextEngine(resolve:{_ in nil})
        let word=Run(text:"אבג ",size:16,weight:400,family:0,italic:false,lineHeight:20,letterSpacing:0)
        var box=word; box.text="\u{fffc}"; box.inlineBox=TextInlineBox(width:35,height:25,baseline:25)
        let spec=Spec(runs:[word,box],align:2,lineClamp:0,color:[0,0,0,255],direction:1,strut:word)
        let paragraph=engine.paragraph(spec,width:200)
        let positions=paragraph.atomicPositions(spec,width:200)
        XCTAssertEqual(positions.count,1)
        XCTAssertGreaterThanOrEqual(positions[0].1.x,0)
        XCTAssertLessThanOrEqual(positions[0].1.x+35,200.01)
        XCTAssertEqual(positions[0].1.y+25,paragraph.firstBaseline,accuracy:0.01)
    }

    #if os(macOS)
    func testTextWithOwnPressHandlerActivatesOnMouseUp() throws {
        _ = NSApplication.shared
        let presenter=Presenter()
        let window=NSWindow(contentRect:NSRect(x:0,y:0,width:200,height:100),styleMask:[.borderless],backing:.buffered,defer:false)
        window.isReleasedWhenClosed=false
        defer {window.close()}
        window.contentView=presenter.root
        let mention=NodeView(id:2,kind:"text",presenter:presenter)
        mention.frame=NSRect(x:10,y:10,width:100,height:30); mention.handlers=["press"]
        presenter.root.addSubview(mention); presenter.views[2]=mention
        var pressed: UInt32?
        presenter.onPress={pressed=$0}
        let point=mention.convert(NSPoint(x:50,y:15),to:nil)
        let down=try XCTUnwrap(NSEvent.mouseEvent(with:.leftMouseDown,location:point,modifierFlags:[],timestamp:0,windowNumber:window.windowNumber,context:nil,eventNumber:0,clickCount:1,pressure:1))
        let up=try XCTUnwrap(NSEvent.mouseEvent(with:.leftMouseUp,location:point,modifierFlags:[],timestamp:0.1,windowNumber:window.windowNumber,context:nil,eventNumber:1,clickCount:1,pressure:0))
        mention.mouseDown(with:down); mention.mouseUp(with:up)
        XCTAssertEqual(pressed,2)
    }

    func testAtomicMentionKeepsItsNativeHitTargetAndAccessibleText() {
        _ = NSApplication.shared
        let presenter=Presenter()
        let owner=NodeView(id:1,kind:"text",presenter:presenter)
        let mention=NodeView(id:2,kind:"text",presenter:presenter)
        mention.props["text"]="@Alice"
        presenter.views[1]=owner; presenter.views[2]=mention
        presenter.root.addSubview(owner); owner.addSubview(mention)
        var style=InlineStyle()
        style.run.inlineBox=TextInlineBox(width:60,height:24,baseline:18)
        let placeholder=InlineText(id:2,parent:1,props:["text":"\u{fffc}"],style:style,handlers:[],paints:true)
        presenter.applyParagraph(1,[placeholder])
        XCTAssertNil(presenter.inlineText(2),"the native view, not a text fragment, receives the press")
        XCTAssertTrue(presenter.textHost(2) === mention)
        XCTAssertTrue(owner.inlineText[0].isBox)
        XCTAssertEqual(owner.visibleParagraphText,"@Alice")
    }
    #endif

}
