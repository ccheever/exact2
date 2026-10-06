import Foundation
// Compile against the unchanged production session registry/output buffer. Only the wire and view
// are substituted, so a lost RPC and a connection restoration can be scheduled deterministically.
struct T3Failure { let kind: String; let message: String }
final class T3TerminalView {
 static func json(_ value: Any) -> String { String(data: try! JSONSerialization.data(withJSONObject:value),encoding:.utf8)! }
 func sessionChanged(_ session:T3TerminalSession) {}
 func systemMessage(_ message:String) {}
}
final class T3Transport {
 var terminalConnection: ((Bool)->Void)?
 var connected = true
 var calls: [[String:Any]] = []
 var serverGrid = (cols:80,rows:24)
 var ended: ((T3Failure?,Bool)->Void)?
 var attaches = 0
 func terminalDetach(_ id:String) {}
 func terminalAttach(payload:[String:Any],receive: @escaping ([Any], @escaping ()->Void)->Void,ended: @escaping (T3Failure?,Bool)->Void,opened: @escaping (String?,T3Failure?)->Void) {
  self.ended=ended
  guard connected else { opened(nil,T3Failure(kind:"Disconnected",message:"offline"));return }
  attaches += 1;opened("attach-\(attaches)",nil)
  receive([["type":"snapshot","snapshot":["history":"","status":"running"]]]) {}
 }
 func terminalCall(_ method:String,payload:[String:Any],done:@escaping(T3Failure?)->Void) {
  calls.append(["method":method,"payload":payload,"connected":connected])
  guard connected else {done(T3Failure(kind:"Disconnected",message:"offline"));return}
  if method == "terminal.resize" {serverGrid=(payload["cols"] as! Int,payload["rows"] as! Int)}
  done(nil)
 }
 func disconnect(){ connected=false;ended?(nil,true);terminalConnection?(false) }
 func reconnect(){ connected=true;terminalConnection?(true) }
}
func drain(){RunLoop.main.run(until:Date().addingTimeInterval(0.05))}
let transport=T3Transport(),view=T3TerminalView()
let sessions=T3TerminalSessions(transport:transport)
let session=sessions.bind(view,environment:"env",thread:"thread",terminal:"term-1",cwd:"/repo",worktreePath:"",env:[:])
drain()
sessions.resize(session,cols:100,rows:30);drain()
precondition(transport.serverGrid == (100,30))
transport.disconnect();drain()
sessions.resize(session,cols:120,rows:40);drain()
transport.reconnect();drain()
// Even a fresh fit reporting the same target size fails to repair the old server grid.
sessions.resize(session,cols:120,rows:40);drain()
print(T3TerminalView.json(["calls":transport.calls,"attaches":transport.attaches,"lastSize":[session.lastSize.cols,session.lastSize.rows],"serverGrid":[transport.serverGrid.cols,transport.serverGrid.rows],"expectedGrid":[120,40],"pass":transport.serverGrid == (120,40)]))
exit(transport.serverGrid == (120,40) ? 0 : 1)
