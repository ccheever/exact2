import AppKit
let base = URL(fileURLWithPath: CommandLine.arguments[1])
func emit(_ label: String,_ value: Any) { let bytes = try! JSONSerialization.data(withJSONObject:["label":label,"at":ISO8601DateFormatter().string(from:Date()),"value":value],options:[.sortedKeys]);print(String(data:bytes,encoding:.utf8)!);fflush(stdout) }
func wait(_ sec: Double) { let end=Date().addingTimeInterval(sec);while Date()<end {if let app=NSApp,let e=app.nextEvent(matching:.any,until:Date().addingTimeInterval(0.01),inMode:.default,dequeue:true){app.sendEvent(e)};RunLoop.current.run(until:Date().addingTimeInterval(0.005))} }
func call(_ t:T3Transport,_ req:[String:Any])->[String:Any] { var result:[String:Any]?;t.perform(req){result=$0};let end=Date().addingTimeInterval(35);while result == nil && Date()<end {wait(0.01)};guard let result else {fatalError("timeout")};if result["ok"] as? Bool != true {emit("FAIL",result)};return result["value"] as? [String:Any] ?? [:] }
func rpc(_ t:T3Transport,_ method:String,_ payload:[String:Any]=[:])->[String:Any] {call(t,["op":"request","method":method,"payload":payload])}
func token(_ port:Int)->String {let s=try! String(contentsOf:base.appendingPathComponent("\(port)/pair.txt"));return s.components(separatedBy:"\n").first{$0.hasPrefix("Token: ")}!.replacingOccurrences(of:"Token: ",with:"")}
let creds=T3Credentials(persistent:false),saved=T3SavedEnvironments(persistent:false)
let baseline=T3Transport(persistent:false,credentials:creds,savedEnvironments:saved,changed:{_ in})
let bMonitor=T3Transport(persistent:false,credentials:creds,savedEnvironments:saved,changed:{_ in})
emit("baseline-connect",call(baseline,["op":"connect","origin":"http://127.0.0.1:16845","credential":token(16845)]))
_ = rpc(baseline,"server.updateSettings",["patch":["providerHealthRefreshInterval":1000,"backgroundActivityProfile":"balanced"]])
func config(_ t:T3Transport)->Any {let c=rpc(t,"server.getConfig");return c["providers"] ?? c}
for i in 0...3 {emit("baseline-\(i)",config(baseline));wait(1.2)}
_ = call(baseline,["op":"disconnect"])
let app=NSApplication.shared;app.setActivationPolicy(.regular);app.finishLaunching()
let window=NSWindow(contentRect:NSRect(x:100,y:100,width:400,height:120),styleMask:[.titled,.closable],backing:.buffered,defer:false);window.title="Isolated activity acceptance";window.makeKeyAndOrderFront(nil);app.activate(ignoringOtherApps:true)
wait(0.5);emit("window-facts",["active":app.isActive,"hidden":app.isHidden,"windows":app.windows.map{["main":$0.canBecomeMain,"visible":$0.isVisible,"key":$0.isKeyWindow,"occluded":$0.occlusionState.contains(.visible)]}])
let reporter=T3ActivityReporter(persistent:false,dataDirectory:nil)
let a=T3Transport(persistent:false,credentials:creds,savedEnvironments:saved,activity:reporter,changed:{_ in})
let fleet=T3Fleet(persistent:false,credentials:creds,saved:saved,activity:reporter,changed:{_ in})
emit("active-connect",call(a,["op":"connect","origin":"http://127.0.0.1:16845"]))
let pair=call(a,["op":"pairEnvironment","origin":"http://127.0.0.1:16846","credential":token(16846)])
let bEnv=pair["environmentId"] as! String;let bKey="http://127.0.0.1:16846\n"+bEnv
func fleetCall(_ req:[String:Any])->[String:Any] {var r:[String:Any]?;fleet.perform(bKey,req){r=$0};let end=Date().addingTimeInterval(35);while r==nil && Date()<end {wait(0.01)};return r ?? [:]}
emit("fleet-connect",fleetCall(["op":"connect","origin":"http://127.0.0.1:16846"]))
wait(0.5)
for i in 0...3 {emit("active-\(i)",config(a));emit("policy-a-\(i)",rpc(a,"server.getBackgroundPolicy"));wait(1.2)}
emit("policy-b-on",fleetCall(["op":"request","method":"server.getBackgroundPolicy","payload":[:]]))
_ = call(bMonitor,["op":"connect","origin":"http://127.0.0.1:16846"])
emit("fleet-off",fleetCall(["op":"fleetStop"]))
wait(1)
emit("fleet-off-status",fleetCall(["op":"status"]))
emit("policy-b-off",rpc(bMonitor,"server.getBackgroundPolicy"))
wait(2)
emit("fleet-reconnect",fleetCall(["op":"connect","origin":"http://127.0.0.1:16846"]))
wait(0.5);emit("policy-b-reconnected",fleetCall(["op":"request","method":"server.getBackgroundPolicy","payload":[:]]))
_ = call(a,["op":"subscribe","key":"diagnostics-test","method":"subscribeResourceTelemetry","payload":[:]])
wait(0.5);emit("diagnostics-on",rpc(a,"server.getBackgroundPolicy"))
_ = call(a,["op":"unsubscribe","key":"diagnostics-test"])
wait(0.5);emit("diagnostics-off",rpc(a,"server.getBackgroundPolicy"))
app.hide(nil);emit("foreground-released",[:]);wait(1)
emit("background-policy",rpc(a,"server.getBackgroundPolicy"))
wait(47)
for i in 0...3 {emit("background-\(i)",config(a));emit("background-policy-\(i)",rpc(a,"server.getBackgroundPolicy"));wait(1.2)}
fleet.destroy();a.destroy();baseline.destroy();bMonitor.destroy();reporter.destroy();wait(0.5);emit("finished",[:])
