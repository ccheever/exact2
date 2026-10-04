#if os(macOS)
import AppKit
#else
import UIKit
#endif
import XCTest
@testable import ExactKit

private let recoveryReply = UnsafeMutablePointer<UInt8>.allocate(capacity: 128)
private var recoveryLength: UInt32 = 0
private var replacementLost = true
private var replacements = 0
private var controlEvents: [[String: Any]] = []
private var boundObjects: [NSDictionary] = []
#if os(macOS)
private final class ContactEventWindow: NSWindow {
    var events: [NSEvent] = []
    override func sendEvent(_ event: NSEvent) { events.append(event) }
}
#endif
final class SurfaceControlTests: XCTestCase {
    private func fixture() -> (ExactSession, NodeView, NodeView) {
        #if os(macOS)
        _ = NSApplication.shared
        #endif
        let s = ExactApp.shared.makeSession(label: "control-test")
        let m = GpuModule(create: { _, _, _, _, _ in 1 }, bind: { _, p, n in
                if let p, let object = try? JSONSerialization.jsonObject(with: Data(bytes:p, count:n)) as? NSDictionary { boundObjects.append(object) }; return 0
            },
            render: { _, _, _, _, _ in 0 }, dirty: { _ in 0 }, destroy: { _ in },
            texture: { _, _, _, _, _ in 0 }, textureMetal: nil, sync: nil,
            childrenMode: { _ in 0 }, readback: { _, _, _, _, _, _, _ in 0 },
            child: { _, _, _, _, _, _, _, _, _, _, _, _ in 0 }, childrenCount: { _, _ in 0 }, placement: { _, _, _, _ in 0 },
            shader: nil, validateShader: nil, clearShaders: nil, errorLen: { 0 }, errorPtr: { nil },
            wantsInput: { _ in 1 }, input: { _, p, n in
                controlEvents.append((try? JSONSerialization.jsonObject(with: Data(bytes:p!, count:n))) as? [String: Any] ?? [:]); return 0
            }, messages: nil, published: nil, agent: { _, _, _ in recoveryLength }, outPtr: { UnsafePointer(recoveryReply) })
        s.canvases.modules[""] = m; s.canvases.attempted = [""]
        let canvas = NodeView(id:100, kind:"canvas", presenter:s.presenter)
        let button = NodeView(id:101, kind:"button", presenter:s.presenter)
        canvas.frame = CGRect(x:0,y:0,width:200,height:200); button.frame=CGRect(x:0,y:0,width:100,height:100)
        canvas.addSubview(button); button.props["action"] = "jump"
        s.presenter.root.addSubview(canvas); s.presenter.views[100] = canvas; s.presenter.views[101] = button
        let e = Canvases.Entry(view:canvas, name:"world", values:[]); e.id=1; e.wantsInput=true; e.module=m
        s.canvases.entries[100]=e; canvas.canvasInput=CanvasInput(view:canvas)
        return (s, canvas, button)
    }
    #if os(macOS)
    func testE11ActionAndSliderKeepPointerFocus() {
        let (s, _, button) = fixture(); defer { s.destroy() }
        let window=NSWindow(contentRect:NSRect(x:0,y:0,width:400,height:200),styleMask:[.borderless],backing:.buffered,defer:false)
        window.contentView=s.presenter.viewport
        XCTAssertTrue(window.makeFirstResponder(button))
        button.finishPointerPress()
        XCTAssertTrue(window.firstResponder === button)
        button.props.removeValue(forKey:"action");button.props["accessibilityRole"]="slider"
        button.finishPointerPress()
        XCTAssertTrue(window.firstResponder === button)
        withExtendedLifetime(window) {}
    }
    func testE11NewAutofocusRunsAfterCanvasPointerHandoff() {
        let (s, canvas, button) = fixture(); defer { s.destroy() }
        button.props.removeValue(forKey:"action")
        let window=NSWindow(contentRect:NSRect(x:0,y:0,width:400,height:200),styleMask:[.borderless],backing:.buffered,defer:false)
        window.contentView=s.presenter.viewport
        XCTAssertTrue(window.makeFirstResponder(button))
        let next=NodeView(id:102,kind:"button",presenter:s.presenter)
        next.frame=CGRect(x:0,y:100,width:100,height:50);next.props["autofocus"]="true"
        canvas.addSubview(next);s.presenter.views[102]=next
        s.presenter.syncAccessibility()
        button.finishPointerPress()
        XCTAssertTrue(window.firstResponder === next)
        // Explicit focus selected by the committed route cannot be stolen.
        button.finishPointerPress();XCTAssertTrue(window.firstResponder === next)
        withExtendedLifetime(window) {}
    }
    func testR15PointerButtonReturnsSpaceToCanvasAndKeyboardKeepsFocus() {
        let (s, canvas, button) = fixture(); defer { s.destroy() }
        button.props.removeValue(forKey:"action"); button.handlers.insert("press")
        let window=NSWindow(contentRect:NSRect(x:0,y:0,width:400,height:200),styleMask:[.borderless],backing:.buffered,defer:false)
        window.contentView=s.presenter.viewport
        let key=NSEvent.keyEvent(with:.keyDown,location:.zero,modifierFlags:[],timestamp:1,windowNumber:window.windowNumber,context:nil,characters:" ",charactersIgnoringModifiers:" ",isARepeat:false,keyCode:49)!
        XCTAssertTrue(window.makeFirstResponder(button))
        button.pressed=true
        let point=button.convert(NSPoint(x:10,y:10),to:nil)
        let up=NSEvent.mouseEvent(with:.leftMouseUp,location:point,modifierFlags:[],timestamp:0,windowNumber:window.windowNumber,context:nil,eventNumber:0,clickCount:1,pressure:0)!
        button.mouseUp(with:up)
        XCTAssertFalse(window.firstResponder === button)
        controlEvents.removeAll()
        (window.firstResponder as? NodeView)?.keyDown(with:key)
        XCTAssertTrue(controlEvents.contains { $0["code"] as? String == "Space" && $0["down"] as? Bool == true })
        canvas.nextKeyView=button; window.selectNextKeyView(canvas)
        XCTAssertTrue(window.firstResponder === button)
        controlEvents.removeAll(); button.keyDown(with:key)
        XCTAssertTrue(window.firstResponder === button)
        XCTAssertFalse(controlEvents.contains { $0["code"] as? String == "Space" })
        let ordinary=NodeView(id:102,kind:"button",presenter:s.presenter)
        ordinary.frame=CGRect(x:220,y:0,width:100,height:100); ordinary.handlers.insert("press")
        s.presenter.root.addSubview(ordinary); s.presenter.views[102]=ordinary
        XCTAssertTrue(window.makeFirstResponder(ordinary)); ordinary.pressed=true
        let ordinaryPoint=ordinary.convert(NSPoint(x:10,y:10),to:nil)
        let ordinaryUp=NSEvent.mouseEvent(with:.leftMouseUp,location:ordinaryPoint,modifierFlags:[],timestamp:0,windowNumber:window.windowNumber,context:nil,eventNumber:0,clickCount:1,pressure:0)!
        ordinary.mouseUp(with:ordinaryUp)
        XCTAssertTrue(window.firstResponder === ordinary)
        controlEvents.removeAll(); ordinary.keyDown(with:key)
        XCTAssertTrue(window.firstResponder === ordinary)
        XCTAssertFalse(controlEvents.contains { $0["code"] as? String == "Space" })
        withExtendedLifetime((window,canvas)) {}
    }
    func testDetachedPointerButtonReturnsRawKeyToOriginalCanvas() {
        let (s, canvas, button) = fixture(); defer { s.destroy() }
        button.props.removeValue(forKey:"action"); button.handlers.insert("press")
        let window=NSWindow(contentRect:NSRect(x:0,y:0,width:400,height:200),styleMask:[.borderless],backing:.buffered,defer:false)
        window.contentView=s.presenter.viewport
        s.presenter.onPress = { _ in
            s.presenter.apply(wireBatch([["op":"destroy","id":Int(button.id)]]))
        }
        XCTAssertTrue(window.makeFirstResponder(button)); button.pressed=true
        let point=button.convert(NSPoint(x:10,y:10),to:nil)
        let event=NSEvent.mouseEvent(with:.leftMouseUp,location:point,modifierFlags:[],timestamp:0,windowNumber:window.windowNumber,context:nil,eventNumber:0,clickCount:1,pressure:0)!
        button.mouseUp(with:event)
        XCTAssertTrue(window.firstResponder === canvas)
        controlEvents=[]
        let raw=NSEvent.keyEvent(with:.keyDown,location:.zero,modifierFlags:[],timestamp:1,windowNumber:window.windowNumber,context:nil,characters:"x",charactersIgnoringModifiers:"x",isARepeat:false,keyCode:7)!
        window.sendEvent(raw)
        XCTAssertTrue(controlEvents.contains { $0["code"] as? String == "KeyX" && $0["down"] as? Bool == true })
        withExtendedLifetime(window) {}
    }
    func testDetachedKeyboardButtonReturnsEnterAndSpaceToOriginalCanvas() {
        for (characters,keyCode): (String,UInt16) in [("\r",36),(" ",49)] {
            let (s, canvas, button) = fixture()
            button.props.removeValue(forKey:"action"); button.handlers.insert("press")
            let window=NSWindow(contentRect:NSRect(x:0,y:0,width:400,height:200),styleMask:[.borderless],backing:.buffered,defer:false)
            window.contentView=s.presenter.viewport
            XCTAssertTrue(window.makeFirstResponder(button))
            s.presenter.onPress = { _ in
                s.presenter.apply(wireBatch([["op":"destroy","id":Int(button.id)]]))
            }
            let event=NSEvent.keyEvent(with:.keyDown,location:.zero,modifierFlags:[],timestamp:1,windowNumber:window.windowNumber,context:nil,characters:characters,charactersIgnoringModifiers:characters,isARepeat:false,keyCode:keyCode)!
            window.sendEvent(event)
            XCTAssertTrue(window.firstResponder === canvas)
            s.destroy(); withExtendedLifetime(window) {}
        }
    }
    func testDetachedButtonDoesNotStealExplicitFocusOrReviveRemovedCanvas() {
        do {
            let (s, canvas, button) = fixture(); defer { s.destroy() }
            button.props.removeValue(forKey:"action"); button.handlers.insert("press")
            let window=NSWindow(contentRect:NSRect(x:0,y:0,width:400,height:200),styleMask:[.borderless],backing:.buffered,defer:false)
            window.contentView=s.presenter.viewport
            let explicit=NodeView(id:102,kind:"button",presenter:s.presenter); explicit.frame=CGRect(x:220,y:0,width:100,height:100)
            s.presenter.root.addSubview(explicit); s.presenter.views[102]=explicit
            XCTAssertTrue(window.makeFirstResponder(button))
            s.presenter.onPress = { _ in
                s.presenter.apply(wireBatch([["op":"destroy","id":Int(button.id)]]))
                XCTAssertTrue(window.makeFirstResponder(explicit))
            }
            let enter=NSEvent.keyEvent(with:.keyDown,location:.zero,modifierFlags:[],timestamp:1,windowNumber:window.windowNumber,context:nil,characters:"\r",charactersIgnoringModifiers:"\r",isARepeat:false,keyCode:36)!
            window.sendEvent(enter)
            XCTAssertTrue(window.firstResponder === explicit)
            withExtendedLifetime((window,canvas)) {}
        }
        do {
            let (s, canvas, button) = fixture(); defer { s.destroy() }
            button.props.removeValue(forKey:"action"); button.handlers.insert("press")
            let window=NSWindow(contentRect:NSRect(x:0,y:0,width:400,height:200),styleMask:[.borderless],backing:.buffered,defer:false)
            window.contentView=s.presenter.viewport
            XCTAssertTrue(window.makeFirstResponder(button))
            s.presenter.onPress = { _ in
                s.presenter.apply(wireBatch([["op":"destroy","id":Int(button.id)],["op":"destroy","id":Int(canvas.id)]]))
            }
            let space=NSEvent.keyEvent(with:.keyDown,location:.zero,modifierFlags:[],timestamp:1,windowNumber:window.windowNumber,context:nil,characters:" ",charactersIgnoringModifiers:" ",isARepeat:false,keyCode:49)!
            window.sendEvent(space)
            XCTAssertFalse(window.firstResponder === canvas)
            XCTAssertNil(canvas.window)
            withExtendedLifetime(window) {}
        }
    }
    #endif
    func testR13NamedAndEmptyArgumentsSurviveBatchBinding() {
        let (s,_,_)=fixture();defer {s.destroy()}
        for values: [String:Any] in [["restart":false,"seed":7,"paused":true], [:]] {
            boundObjects.removeAll()
            s.apply(wireBatch([["op":"surface","id":100,"name":"world","values":values]]))
            XCTAssertEqual(boundObjects.last,values as NSDictionary)
        }
    }
    func testContactIdentityIncludesCanvasAndReleaseRequest() {
        let (s, canvas, button) = fixture(); defer { s.destroy() }
        #if os(macOS)
        let window=NSWindow(contentRect:NSRect(x:0,y:0,width:400,height:200),styleMask:[.borderless],backing:.buffered,defer:false)
        window.contentView=s.presenter.viewport
        #else
        let window=UIWindow(frame:CGRect(x:0,y:0,width:400,height:200)); window.addSubview(s.presenter.viewport)
        #endif
        let other=NodeView(id:200,kind:"canvas",presenter:s.presenter), second=NodeView(id:201,kind:"button",presenter:s.presenter)
        s.presenter.root.addSubview(other); other.addSubview(second); second.props["action"]="jump"
        s.presenter.views[200]=other; s.presenter.views[201]=second
        other.canvasInput=CanvasInput(view:other)
        let e=Canvases.Entry(view:other,name:"other",values:[]); e.id=2; e.wantsInput=true; e.module=s.canvases.modules[""]; s.canvases.entries[200]=e
        XCTAssertTrue(button.control("down",id:1)); XCTAssertTrue(second.control("down",id:1))
        XCTAssertTrue(second.control("up",id:1)); XCTAssertNotNil(s.canvases.entries[100]!.controls[1]); XCTAssertNil(e.controls[1])
        XCTAssertTrue(second.control("down",id:1))
        XCTAssertEqual(s.canvases.releaseContact(["id":200,"contact":1,"phase":"cancel"])?["delivery"] as? String,"recognized")
        XCTAssertNotNil(s.canvases.entries[100]!.controls[1]); XCTAssertNil(e.controls[1])
        withExtendedLifetime((window,canvas)) {}
    }
    func testRestoredContactCancelsWhenItsControlUnmounts() {
        let (s, _, button) = fixture(); defer { s.destroy() }
        let m = s.canvases.modules[""]!, e = s.canvases.entries[100]!
        let bytes = Array("{\"world\":{\"restored\":true,\"input\":{\"controlContacts\":[{\"id\":7,\"action\":\"jump\"}]}}}".utf8)
        bytes.withUnsafeBufferPointer { recoveryReply.update(from:$0.baseAddress!,count:$0.count) }
        recoveryLength = UInt32(bytes.count)
        e.restorePending = true; s.canvases.finishRestore(m, e)
        XCTAssertEqual(e.controls[7]?.node, button.id)
        controlEvents = []; button.forget()
        XCTAssertTrue(e.controls.isEmpty)
        XCTAssertEqual(controlEvents.last?["phase"] as? String, "cancel")
    }
    func testR12BlurClearsRestoredOwnership() {
        let (s,canvas,button)=fixture(); defer {s.destroy()}
        #if os(macOS)
        let window=NSWindow(contentRect:NSRect(x:0,y:0,width:400,height:200),styleMask:[.borderless],backing:.buffered,defer:false);window.contentView=s.presenter.viewport
        #else
        let window=UIWindow(frame:CGRect(x:0,y:0,width:400,height:200));window.addSubview(s.presenter.viewport)
        #endif
        defer {withExtendedLifetime(window) {}}
        let e=s.canvases.entries[100]!
        e.controls[7]=SurfaceControl(node:button.id,name:"jump",offset:.zero,position:.zero)
        controlEvents=[];canvas.canvasInput!.blur()
        XCTAssertTrue(e.controls.isEmpty)
        XCTAssertEqual(controlEvents.map {$0["phase"] as? String ?? $0["t"] as? String ?? ""},["cancel","blur"])
    }
    func testR12DuplicateActionsDoNotGuessRestoredNode() {
        let (s,canvas,first)=fixture(); defer {s.destroy()}
        let second=NodeView(id:102,kind:"button",presenter:s.presenter)
        second.props["action"]="jump";canvas.addSubview(second);s.presenter.views[102]=second
        let e=s.canvases.entries[100]!,m=s.canvases.modules[""]!
        let bytes=Array("{\"world\":{\"restored\":true,\"input\":{\"controlContacts\":[{\"id\":7,\"action\":\"jump\"}]}}}".utf8)
        bytes.withUnsafeBufferPointer {recoveryReply.update(from:$0.baseAddress!,count:$0.count)};recoveryLength=UInt32(bytes.count)
        e.restorePending=true;s.canvases.finishRestore(m,e)
        XCTAssertEqual(e.controls[7]?.node,canvas.id)
        second.forget();second.removeFromSuperview();s.presenter.views.removeValue(forKey:second.id);s.canvases.cancelMovedControls();XCTAssertNotNil(e.controls[7])
        first.forget();first.removeFromSuperview();s.presenter.views.removeValue(forKey:first.id);s.canvases.cancelMovedControls();XCTAssertNil(e.controls[7])
    }
    func testR12ReparentAndCrossViewRelease() {
        let (s,canvas,button)=fixture(); defer {s.destroy()}
        #if os(macOS)
        let window=NSWindow(contentRect:NSRect(x:0,y:0,width:400,height:200),styleMask:[.borderless],backing:.buffered,defer:false);window.contentView=s.presenter.viewport
        #else
        let window=UIWindow(frame:CGRect(x:0,y:0,width:400,height:200));window.addSubview(s.presenter.viewport)
        #endif
        let other=NodeView(id:200,kind:"canvas",presenter:s.presenter),second=NodeView(id:201,kind:"button",presenter:s.presenter)
        s.presenter.root.addSubview(other);other.addSubview(second);second.props["action"]="jump"
        s.presenter.views[200]=other;s.presenter.views[201]=second;other.canvasInput=CanvasInput(view:other)
        let e=Canvases.Entry(view:other,name:"other",values:[]);e.id=2;e.wantsInput=true;e.module=s.canvases.modules[""];s.canvases.entries[200]=e
        XCTAssertTrue(button.control("down",id:7));XCTAssertTrue(second.control("up",id:7));XCTAssertTrue(s.canvases.entries[100]!.controls.isEmpty)
        XCTAssertTrue(button.control("down",id:8));other.addSubview(button)
        s.presenter.apply(wireBatch([]))
        XCTAssertTrue(s.canvases.entries[100]!.controls.isEmpty)
        withExtendedLifetime((window,canvas)) {}
    }
    func testCanvasRecreatedWhileRecoveryIsPendingGetsInitialClock() {
        let (s, canvas, _) = fixture(); defer { s.destroy() }
        let m=s.canvases.modules[""]!; m.canvases.add(s.canvases)
        replacementLost=true
        let bytes=Array("{\"status\":\"recovered\"}".utf8)
        bytes.withUnsafeBufferPointer { recoveryReply.update(from:$0.baseAddress!,count:$0.count) }; recoveryLength=UInt32(bytes.count)
        m.deviceLost={ replacementLost }; m.recover={ replacementLost=false; return recoveryLength }
        m.recoverDevice()
        let fresh=Canvases.Entry(view:canvas,name:"fresh",values:[]); fresh.id=2; s.canvases.entries[100]=fresh
        let done=expectation(description:"recovery")
        DispatchQueue.main.asyncAfter(deadline:.now()+0.05) { done.fulfill() }; wait(for:[done],timeout:2)
        XCTAssertEqual(m.deliveryClock(fresh,now:300)["now"] as? Double,300)
    }
    #if os(macOS)
    func testControlPreservesTextEditor() {
        let (s,_,button)=fixture(); defer { s.destroy() }
        let window=NSWindow(contentRect:NSRect(x:0,y:0,width:200,height:200),styleMask:[.borderless],backing:.buffered,defer:false)
        window.contentView=s.presenter.viewport
        let editor=NSTextView(frame:NSRect(x:0,y:100,width:100,height:50)); s.presenter.root.addSubview(editor)
        XCTAssertTrue(window.makeFirstResponder(editor)); XCTAssertTrue(button.control("down",id:7))
        XCTAssertTrue(window.firstResponder === editor)
        XCTAssertTrue(button.controlKey("Space",down:true))
        XCTAssertTrue(window.firstResponder === editor)
        XCTAssertNotNil(s.canvases.entries[100]!.controls[4294967294])
        XCTAssertTrue(button.controlKey("Space",down:false))
        controlEvents=[]
        let reply=s.agentInstance.type(["id":Int(button.id),"key":"Space"])
        XCTAssertNil(reply["error"])
        XCTAssertEqual(controlEvents.filter {$0["t"] as? String == "control"}.map {$0["phase"] as? String ?? ""},["down","up"])
        XCTAssertTrue(window.firstResponder === editor)
        withExtendedLifetime(window) {}
    }
    func testRawCanvasPointerPreservesTextEditor() {
        let (s, canvas, _) = fixture(); defer { s.destroy() }
        let window=NSWindow(contentRect:NSRect(x:0,y:0,width:200,height:200),styleMask:[.borderless],backing:.buffered,defer:false)
        window.contentView=s.presenter.viewport
        let editor=NSTextView(frame:NSRect(x:0,y:100,width:100,height:50)); s.presenter.root.addSubview(editor)
        XCTAssertTrue(window.makeFirstResponder(editor))
        XCTAssertTrue(canvas.focusSurfacePointer())
        XCTAssertTrue(window.firstResponder === editor); withExtendedLifetime(window) {}
    }
    #endif
    func testResultThreeRetriesReplacementLossWithoutDeviceObserverAndScopesRedelivery() {
        let (s, canvas, _) = fixture(); defer { s.destroy() }
        let m = s.canvases.modules[""]!, e = s.canvases.entries[100]!
        m.canvases.add(s.canvases); replacements = 0; replacementLost = true
        let bytes = Array("{\"status\":\"recovered\"}".utf8)
        bytes.withUnsafeBufferPointer { recoveryReply.update(from:$0.baseAddress!,count:$0.count) }; recoveryLength=UInt32(bytes.count)
        m.deviceLost = { replacementLost }
        m.recover = { replacements += 1; replacementLost = replacements == 1; return recoveryLength }
        s.canvases.rendered(e, 3); s.canvases.rendered(e, 3)
        let done = expectation(description:"replacement retry")
        DispatchQueue.main.asyncAfter(deadline:.now()+0.3) { done.fulfill() }
        wait(for:[done],timeout:2)
        XCTAssertEqual(replacements,2); XCTAssertEqual(m.lossGeneration,1)
        XCTAssertNil(m.deliveryClock(e,now:100)["now"])
        XCTAssertNotNil(m.deliveryClock(e,now:200)["now"])
        let fresh = Canvases.Entry(view:canvas,name:"new",values:[])
        XCTAssertEqual(m.deliveryClock(fresh,now:300)["now"] as? Double,300)
    }
    #if os(macOS)
    func testAgentDragCarriesDeviceMotionForLockedCanvases() {
        let (s, _, _) = fixture(); defer { s.destroy() }
        let window = ContactEventWindow(contentRect: NSRect(x: 0, y: 0, width: 400, height: 300), styleMask: [.borderless], backing: .buffered, defer: false)
        window.contentView = s.presenter.viewport
        let agent = s.agentInstance
        agent.contact = CGPoint(x: 50, y: 50)
        agent.contactClock = 10
        XCTAssertNil(agent.contact("move", ["dx": 100.0, "dy": -20.0, "ms": 48.0])["error"])
        XCTAssertEqual(window.events.count, 3)
        XCTAssertEqual(window.events.reduce(0) { $0 + $1.deltaX }, 100)
        XCTAssertEqual(window.events.reduce(0) { $0 + $1.deltaY }, -20)
        XCTAssertTrue(window.events.allSatisfy { $0.type == .leftMouseDragged && $0.windowNumber == window.windowNumber })
        XCTAssertEqual(window.events.last!.timestamp, 10.048, accuracy: 0.000001)
        let clip = s.presenter.viewport.contentView
        let at = clip.convert(NSPoint(x: 150 + clip.bounds.origin.x, y: 30 + clip.bounds.origin.y), to: nil)
        XCTAssertEqual(window.events.last!.locationInWindow, at)
        XCTAssertNil(agent.contact("up", [:])["error"])
        XCTAssertEqual(window.events.last!.deltaX, 0)
        XCTAssertEqual(window.events.last!.deltaY, 0)
        withExtendedLifetime(window) {}
    }
    func testAgentCancelClearsMouseContact() {
        let (s,_,_) = fixture(); defer { s.destroy() }
        let window=NSWindow(contentRect:NSRect(x:0,y:0,width:200,height:200),styleMask:[.borderless],backing:.buffered,defer:false)
        window.contentView=s.presenter.viewport
        let agent=s.agentInstance; agent.contact=CGPoint(x:50,y:50)
        let reply=agent.contact("cancel",[:])
        XCTAssertEqual(reply["delivery"] as? String,"platform"); XCTAssertNil(agent.contact)
        withExtendedLifetime(window) {}
    }
    #else
    func testAgentHoldDoesNotInjectMoveUntilPositionChanges() {
        let (s,_,button) = fixture(); defer { s.destroy() }
        let window=UIWindow(frame:CGRect(x:0,y:0,width:200,height:200)); window.addSubview(s.presenter.viewport)
        let agent=s.agentInstance; agent.contact=CGPoint(x:50,y:50); agent.canvasContact=button
        XCTAssertTrue(button.control("down",point:CGPoint(x:50,y:50)));controlEvents=[]
        let held = agent.canvasTap(["phase":"hold"]); XCTAssertNotNil(held); XCTAssertNil(held?["error"]);XCTAssertTrue(controlEvents.isEmpty)
        let moved = agent.canvasTap(["phase":"hold","dx":10.0]); XCTAssertNotNil(moved); XCTAssertNil(moved?["error"]);XCTAssertEqual(controlEvents.count,1)
        XCTAssertEqual(controlEvents.last?["phase"] as? String,"move")
        withExtendedLifetime(window) {}
    }
    #endif
    func testImmutableControlBindingAndClearCancel() {
        let (s, canvas, button) = fixture()
        #if os(macOS)
        let window = NSWindow(contentRect:NSRect(x:0,y:0,width:200,height:200),styleMask:[.borderless],backing:.buffered,defer:false)
        window.contentView=s.presenter.viewport
        #else
        let window = UIWindow(frame:CGRect(x:0,y:0,width:200,height:200)); window.addSubview(s.presenter.viewport)
        #endif
        defer { s.destroy(); withExtendedLifetime(window) {} }
        controlEvents=[]
        XCTAssertTrue(button.control("down",id:7,point:CGPoint(x:10,y:20)))
        #if os(macOS)
        XCTAssertTrue(window.firstResponder === button)
        #else
        XCTAssertTrue(button.isFirstResponder)
        #endif
        button.props["action"]="light"
        XCTAssertTrue(button.control("up",id:7))
        XCTAssertEqual(controlEvents.last?["name"] as? String,"jump")
        XCTAssertTrue(button.control("down",id:8))
        button.applyProps(set:[:],clear:["action"])
        XCTAssertEqual(controlEvents.last?["phase"] as? String,"cancel")
        XCTAssertEqual(controlEvents.last?["name"] as? String,"light")
        button.props["action"]="jump"; XCTAssertTrue(button.control("down",id:9)); button.forget()
        XCTAssertEqual(controlEvents.last?["phase"] as? String,"cancel")
        s.presenter.autofocusProcessed.insert(ObjectIdentifier(button)); s.presenter.reset()
        XCTAssertTrue(s.presenter.autofocusProcessed.isEmpty)
        withExtendedLifetime(canvas) {}
    }
}
