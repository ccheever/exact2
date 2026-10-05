import AppKit
enum ToolHook { case t3ToolIcon, t3TimelineTip }
enum ToolData { case toolIconLight, toolIconDark }
class ExactElement { var hook: ToolHook?; var view: NSView?; var data: [ToolData: String] = [:]; var clicks = 0; func click() { clicks += 1 } }
