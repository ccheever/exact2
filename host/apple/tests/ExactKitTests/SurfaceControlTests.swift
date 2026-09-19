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
final class SurfaceControlTests: XCTestCase {
    private func fixture() -> (ExactSession, NodeView, NodeView) {
        #if os(macOS)
        _ = NSApplication.shared
        #endif
        let s = ExactApp.shared.makeSession(label: "control-test")
        let m = GpuModule(create: { _, _, _, _, _ in 1 }, bind: { _, _, _ in 0 },
            render: { _, _, _, _, _ in 0 }, dirty: { _ in 0 }, destroy: { _ in },
            texture: { _, _, _, _, _ in 0 }, textureMetal: nil, sync: nil,
            childrenMode: { _ in 0 }, readback: { _, _, _, _, _, _, _ in 0 },
            child: { _, _, _, _, _, _, _, _, _, _ in 0 }, childrenCount: { _, _ in 0 }, placement: { _, _, _, _ in 0 },
            shader: nil, validateShader: nil, clearShaders: nil, errorLen: { 0 }, errorPtr: { nil },
            wantsInput: { _ in 1 }, input: { _, p, n in
                controlEvents.append((try? JSONSerialization.jsonObject(with: Data(bytes:p!, count:n))) as? [String: Any] ?? [:]); return 0
            }, messages: nil, published: nil, agent: nil, outPtr: { UnsafePointer(recoveryReply) })
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
