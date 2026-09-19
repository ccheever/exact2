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
        s.canvases.module = m; s.canvases.loadRequested = true
        let canvas = NodeView(id:100, kind:"canvas", presenter:s.presenter)
        let button = NodeView(id:101, kind:"button", presenter:s.presenter)
        canvas.frame = CGRect(x:0,y:0,width:200,height:200); button.frame=CGRect(x:0,y:0,width:100,height:100)
        canvas.addSubview(button); button.props["action"] = "jump"
        s.presenter.root.addSubview(canvas); s.presenter.views[100] = canvas; s.presenter.views[101] = button
        let e = Canvases.Entry(view:canvas, name:"world", values:[]); e.id=1; e.wantsInput=true
        s.canvases.entries[100]=e; canvas.canvasInput=CanvasInput(view:canvas)
        return (s, canvas, button)
    }
    #if os(macOS)
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
        withExtendedLifetime((window,canvas)) {}
    }
    #endif
    func testR13NamedAndEmptyArgumentsSurviveBatchBinding() {
        let (s,_,_)=fixture();defer {s.destroy()}
        for values: [String:Any] in [["restart":false,"seed":7,"paused":true], [:]] {
            boundObjects.removeAll()
            s.apply(Batch(ops:[["op":"surface","id":100,"name":"world","values":values]],timers:false,motion:false,clock:nil,error:nil))
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
        let e=Canvases.Entry(view:other,name:"other",values:[]); e.id=2; e.wantsInput=true; s.canvases.entries[200]=e
        XCTAssertTrue(button.control("down",id:1)); XCTAssertTrue(second.control("down",id:1))
        XCTAssertTrue(second.control("up",id:1)); XCTAssertNotNil(s.canvases.entries[100]!.controls[1]); XCTAssertNil(e.controls[1])
        XCTAssertTrue(second.control("down",id:1))
        XCTAssertEqual(s.canvases.releaseContact(["id":200,"contact":1,"phase":"cancel"])?["delivery"] as? String,"recognized")
        XCTAssertNotNil(s.canvases.entries[100]!.controls[1]); XCTAssertNil(e.controls[1])
        withExtendedLifetime((window,canvas)) {}
    }
    func testRestoredContactCancelsWhenItsControlUnmounts() {
        let (s, _, button) = fixture(); defer { s.destroy() }
        let m = s.canvases.module!, e = s.canvases.entries[100]!
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
        let e=s.canvases.entries[100]!,m=s.canvases.module!
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
        let e=Canvases.Entry(view:other,name:"other",values:[]);e.id=2;e.wantsInput=true;s.canvases.entries[200]=e
        XCTAssertTrue(button.control("down",id:7));XCTAssertTrue(second.control("up",id:7));XCTAssertTrue(s.canvases.entries[100]!.controls.isEmpty)
        XCTAssertTrue(button.control("down",id:8));other.addSubview(button)
        s.presenter.apply(Batch(ops:[],timers:false,motion:false,clock:nil,error:nil))
        XCTAssertTrue(s.canvases.entries[100]!.controls.isEmpty)
        withExtendedLifetime((window,canvas)) {}
    }
    func testCanvasRecreatedWhileRecoveryIsPendingGetsInitialClock() {
        let (s, canvas, _) = fixture(); defer { s.destroy() }
        let m=s.canvases.module!; m.canvases.add(s.canvases)
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
        let m = s.canvases.module!, e = s.canvases.entries[100]!
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
