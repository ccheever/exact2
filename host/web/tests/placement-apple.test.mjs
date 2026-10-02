import {test, expect} from 'bun:test';
import {readFileSync} from 'node:fs';
import {spawnSync} from 'node:child_process';

const unavailable=process.platform==='darwin' ? null : 'the extracted Swift placement harness requires macOS and xcrun';
if(unavailable) console.warn(`SKIP: ${unavailable}`);
const check=unavailable ? test.skip : test;

// Execute each production capture loop's empty-child branch with Swift values.
// The remainder of the loop is a stand-in for a successful bitmap upload.
for (const host of ['Mac','IOS']) check(`${host} clears zero-sized and display-none captures${unavailable ? ` — ${unavailable}` : ''}`,()=>{
  const file=`host/apple/Sources/ExactKit/${host}/Gpu${host}.swift`;
  const source=readFileSync(process.env.R7_APPLE_SOURCE ?? file,'utf8');
  const start=source.indexOf('        for (i, child) in children.enumerated() {');
  const end=source.indexOf('            let hidden = child.isHidden',start);
  const branch=source.slice(start,end);
  const swift=`struct Rect { var width: Double; var height: Double }
enum BatchValue { case string(String); var string: String? { if case .string(let s) = self { return s }; return nil } } // ExactKit's style value, reduced
struct Child { var frame: Rect; var style: [String:BatchValue]; var props: [String:String] = [:] }
struct Entry { var id: UInt32 = 1 }
final class Module {
 var cleared: [UInt32] = []
 func child(_ id: UInt32,_ i: UInt32,_ name: String,_ x: Float,_ y: Float,_ w: Float,_ h: Float,_ pw: UInt32,_ ph: UInt32,_ bytes: UnsafePointer<UInt8>?,_ n: Int) -> UInt32 {
  precondition([x,y,w,h] == [0,0,0,0] && pw == 0 && ph == 0 && bytes == nil && n == 0)
  cleared.append(i); return 0
 }
}
let m=Module(), e=Entry()
let children=[Child(frame:Rect(width:20,height:20),style:[:]),
 Child(frame:Rect(width:0,height:0),style:[:]),Child(frame:Rect(width:20,height:20),style:["display":.string("none")])]
var captured=0
func capture() -> Bool {
${branch}
 captured += 1
 }
 return true
}
precondition(capture()); precondition(m.cleared == [1,2]); precondition(captured == 1)
`;
  const env={...process.env,DEVELOPER_DIR:'/Applications/Xcode.app/Contents/Developer'};delete env.SDKROOT;
  const r=spawnSync('xcrun',['swift','-'],{input:swift,encoding:'utf8',env,timeout:60000});
  expect(r.status, r.stderr).toBe(0);
},65000);

for(const host of ['Mac','IOS']) check(`${host} hidden placement box is zero, not the kernel frame${unavailable ? ` — ${unavailable}` : ''}`,()=>{
  const source=readFileSync(process.env.R7_APPLE_AGENT ?? `host/apple/Sources/ExactKit/${host}/Agent${host}.swift`,'utf8');
  const start=source.indexOf('        if (v as? NodeView)?.placedAncestor?.placementHidden == true');
  expect(start).toBeGreaterThan(-1);
  const guard=source.slice(start,source.indexOf('\n',start));
  const swift=`struct Rect: Equatable {var width:Int;static let zero=Rect(width:0)}
class View {}
class NodeView: View {var placedAncestor:NodeView?;var placementHidden=false}
func box(_ v:View)->Rect {${guard}\nreturn Rect(width:100)}
let parent=NodeView(), child=NodeView();child.placedAncestor=parent
precondition(box(child).width==100);parent.placementHidden=true
precondition(box(child)==Rect.zero)
`;
  const env={...process.env,DEVELOPER_DIR:'/Applications/Xcode.app/Contents/Developer'};delete env.SDKROOT;
  const r=spawnSync('xcrun',['swift','-'],{input:swift,encoding:'utf8',env,timeout:60000});
  expect(r.status,r.stderr).toBe(0);
},65000);
