// T3TimelineMermaid against a served T3 web client: discovers the reference's
// Mermaid chunk, renders a flowchart in both themes and a syntax error, and
// checks the flattened result. Usage: mermaid-tests <origin> (README recipe).
import AppKit

let origin = CommandLine.arguments.dropFirst().first ?? "http://127.0.0.1:14822"
let flow = "graph TD\n  A[Start] --> B{Is it working?}\n  B -->|Yes| C[Ship it]\n  B -.->|No| D[Debug]\n  D --> A"
let broken = "graph TD\n  A -->"
var failures = 0
func check(_ condition: Bool, _ message: String) { if !condition { failures += 1; print("FAIL: \(message)") } else { print("ok: \(message)") } }

_ = NSApplication.shared
var announced = 0
let mermaid = T3TimelineMermaid(changed: { _ in announced += 1 })
let request: [String: Any] = ["op": "mermaidRender", "origin": origin, "generation": 1,
    "diagrams": [["source": flow, "theme": "light"], ["source": flow, "theme": "dark"], ["source": broken, "theme": "light"]]]
var latest: [[String: Any]] = []
func ask() { mermaid.perform(request) { reply in latest = (reply["value"] as? [String: Any])?["items"] as? [[String: Any]] ?? [] } }
ask()
check(latest.count == 3 && latest.allSatisfy { ($0["json"] as? String) == "" }, "first answer is three pending diagrams")
let deadline = Date().addingTimeInterval(60)
while Date() < deadline && announced < 3 { RunLoop.main.run(until: Date().addingTimeInterval(0.1)) }
ask()
func decode(_ item: [String: Any]) -> [String: Any] {
    guard let json = item["json"] as? String, let data = json.data(using: .utf8) else { return [:] }
    return (try? JSONSerialization.jsonObject(with: data)) as? [String: Any] ?? [:]
}
let light = decode(latest[0]), dark = decode(latest[1]), error = decode(latest[2])
check(light["status"] as? String == "rendered", "light flowchart rendered (\(light["message"] ?? ""))")
let items = light["items"] as? [[String: Any]] ?? []
check(items.count > 15, "flowchart flattened into \(items.count) primitives")
check(items.contains { $0["kind"] as? String == "text" && $0["text"] as? String == "Ship" } && items.contains { $0["kind"] as? String == "text" && $0["text"] as? String == "working?" }, "labels keep their words, one run per word")
check(items.filter { $0["kind"] as? String == "path" }.count >= 9, "nodes, edges and arrowheads are paths")
print("light width: \(light["width"] ?? "-") viewBox: \(light["viewBox"] ?? "-")")
check((light["width"] as? Double ?? 0) > 100 && (light["viewBox"] as? String ?? "").split(separator: " ").count == 4, "the diagram has its natural size and view box")
check(dark["status"] as? String == "rendered" && (dark["items"] as? [[String: Any]])?.count == items.count, "the dark theme has the same geometry")
let lightFill = items.first { $0["kind"] as? String == "path" && ($0["fill"] as? String ?? "") != "" }?["fill"] as? String
let darkFill = (dark["items"] as? [[String: Any]])?.first { $0["kind"] as? String == "path" && ($0["fill"] as? String ?? "") != "" }?["fill"] as? String
check(lightFill != nil && lightFill != darkFill, "themes paint differently (\(lightFill ?? "-") / \(darkFill ?? "-"))")
check(error["status"] as? String == "error" && (error["message"] as? String ?? "").contains("Parse error"), "a syntax error reports Mermaid's message")
check(error["retryable"] as? Bool == false, "a syntax error is not retryable")
if let out = ProcessInfo.processInfo.environment["T3_MERMAID_TEST_DIR"] {
    try? (latest[0]["json"] as? String ?? "").write(toFile: out + "/flow-light.json", atomically: true, encoding: .utf8)
    try? (latest[1]["json"] as? String ?? "").write(toFile: out + "/flow-dark.json", atomically: true, encoding: .utf8)
}
print(failures == 0 ? "PASS" : "FAILED \(failures)")
exit(failures == 0 ? 0 : 1)
