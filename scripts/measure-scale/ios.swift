// Stage 11 B's measured cells, iOS (scripts/measure-scale.mjs builds and runs it on a simulator): UIKit's own
// spacing and corners, read from its API where it says them and from the inked shape where it only draws them.
// Prints one `measure <id> <value>` line each, then exits.
import UIKit

func say(_ id: String, _ v: CGFloat, _ note: String = "") { print("measure \(id) \(String(format: "%.2f", Double(v)))\(note.isEmpty ? "" : " # \(note)")"); fflush(stdout) }

/// The equivalent circular radius of a shape's top-left corner, in points: the inked bounding box, then the
/// distance along the 45° diagonal from its corner to the edge (alpha interpolated), as r = d / (√2 − 1).
func cornerRadius(_ view: UIView, in window: UIWindow, label: String) {
  view.layoutIfNeeded()
  let scale: CGFloat = 3
  let size = view.bounds.size
  let format = UIGraphicsImageRendererFormat(); format.scale = scale; format.opaque = false
  // As the screen shows it (a configuration's background is drawn by UIKit outside the layer tree's own render).
  let image = UIGraphicsImageRenderer(size: size, format: format).image { _ in _ = view.drawHierarchy(in: view.bounds, afterScreenUpdates: true) }
  guard let cg = image.cgImage else { return }
  let w = cg.width, h = cg.height
  var data = [UInt8](repeating: 0, count: w * h * 4)
  let space = CGColorSpaceCreateDeviceRGB()
  let c = CGContext(data: &data, width: w, height: h, bitsPerComponent: 8, bytesPerRow: w * 4, space: space, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
  c.draw(cg, in: CGRect(x: 0, y: 0, width: w, height: h))
  func a(_ x: Int, _ y: Int) -> CGFloat { CGFloat(data[(y * w + x) * 4 + 3]) / 255 }
  // A translucent fill (bordered, gray, tinted) is inked at its own level: the box by any ink, the edge at
  // half the level the top edge's middle has.
  var minX = w, minY = h, maxX = -1, maxY = -1
  for y in 0..<h { for x in 0..<w where a(x, y) > 0.02 { minX = min(minX, x); maxX = max(maxX, x); minY = min(minY, y); maxY = max(maxY, y) } }
  guard maxX >= 0 else { print("measure \(label) none"); return }
  let level = a((minX + maxX) / 2, minY + 2 * Int(scale)), half = level / 2
  // Along the diagonal from (minX, minY): the first step whose alpha passes half the level, interpolated.
  var k: CGFloat = 0, prev: CGFloat = 0
  for i in 0..<min(maxX - minX, maxY - minY) {
    let v = a(minX + i, minY + i)
    if v >= half { k = CGFloat(i) - 1 + (half - prev) / max(v - prev, 0.001); break }
    prev = v
  }
  let d = (k + 0.5) * 2.0.squareRoot()   // pixel centres
  let r = d / (2.0.squareRoot() - 1) / scale
  say(label, r, "inked \(String(format: "%.1f", Double(maxX - minX + 1) / scale))x\(String(format: "%.1f", Double(maxY - minY + 1) / scale)) pt")
}

func measure(_ window: UIWindow) {
  let root = window.rootViewController!
  print("os iOS \(UIDevice.current.systemVersion) \(UIDevice.current.model) \(window.bounds.width)x\(window.bounds.height)")
  // Layout margins.
  say("uiview-layout-margin", UIView().layoutMargins.left)
  say("vc-minimum-margin-leading", root.systemMinimumLayoutMargins.leading)
  say("vc-view-margin-leading", root.view.directionalLayoutMargins.leading)
  // System spacing between two labels, horizontally, and between two buttons.
  let host = root.view!
  func spacing(_ a: UIView, _ b: UIView) -> CGFloat {
    [a, b].forEach { $0.translatesAutoresizingMaskIntoConstraints = false; host.addSubview($0) }
    NSLayoutConstraint.activate([a.leadingAnchor.constraint(equalTo: host.leadingAnchor, constant: 40), a.topAnchor.constraint(equalTo: host.topAnchor, constant: 200),
      b.leadingAnchor.constraint(equalToSystemSpacingAfter: a.trailingAnchor, multiplier: 1), b.centerYAnchor.constraint(equalTo: a.centerYAnchor)])
    host.layoutIfNeeded(); defer { a.removeFromSuperview(); b.removeFromSuperview() }
    return b.frame.minX - a.frame.maxX
  }
  let l1 = UILabel(), l2 = UILabel(); l1.text = "A"; l2.text = "B"
  say("system-spacing-labels", spacing(l1, l2))
  // Vertically, label below label (baseline to baseline less a line).
  do {
    let a = UILabel(), b = UILabel(); a.text = "A"; b.text = "B"
    [a, b].forEach { $0.translatesAutoresizingMaskIntoConstraints = false; host.addSubview($0) }
    NSLayoutConstraint.activate([a.leadingAnchor.constraint(equalTo: host.leadingAnchor, constant: 40), a.topAnchor.constraint(equalTo: host.topAnchor, constant: 200),
      b.topAnchor.constraint(equalToSystemSpacingBelow: a.bottomAnchor, multiplier: 1), b.leadingAnchor.constraint(equalTo: a.leadingAnchor)])
    host.layoutIfNeeded(); say("system-spacing-below-labels", b.frame.minY - a.frame.maxY); a.removeFromSuperview(); b.removeFromSuperview()
  }
  do {
    let v = UILabel(); v.text = "A"; v.translatesAutoresizingMaskIntoConstraints = false; host.addSubview(v)
    NSLayoutConstraint.activate([v.leadingAnchor.constraint(equalToSystemSpacingAfter: host.leadingAnchor, multiplier: 1), v.topAnchor.constraint(equalToSystemSpacingBelow: host.safeAreaLayoutGuide.topAnchor, multiplier: 1)])
    host.layoutIfNeeded(); say("system-spacing-to-container-leading", v.frame.minX); v.removeFromSuperview()
  }
  // A child under a regular width: its minimum margins.
  do {
    let child = UIViewController(); root.addChild(child); host.addSubview(child.view); child.didMove(toParent: root)
    root.setOverrideTraitCollection(UITraitCollection(horizontalSizeClass: .regular), forChild: child)
    child.view.frame = CGRect(x: 0, y: 0, width: 800, height: 600); child.view.layoutIfNeeded()
    say("vc-minimum-margin-leading-regular", child.systemMinimumLayoutMargins.leading)
    child.willMove(toParent: nil); child.view.removeFromSuperview(); child.removeFromParent()
  }
  // The list's content configurations.
  let cell = UIListContentConfiguration.cell()
  say("list-cell-margin-leading", cell.directionalLayoutMargins.leading)
  say("list-cell-margin-top", cell.directionalLayoutMargins.top)
  say("list-cell-image-to-text", cell.imageToTextPadding)
  let sub = UIListContentConfiguration.subtitleCell()
  say("list-subtitle-text-gap", sub.textToSecondaryTextVerticalPadding)
  let value = UIListContentConfiguration.valueCell()
  say("list-value-text-gap", value.textToSecondaryTextHorizontalPadding)
  say("list-sidebar-image-to-text", UIListContentConfiguration.sidebarCell().imageToTextPadding)
  say("list-accompanied-subtitle-gap", UIListContentConfiguration.accompaniedSidebarSubtitleCell().textToSecondaryTextVerticalPadding)
  say("list-prominent-header-margin-leading", UIListContentConfiguration.prominentInsetGroupedHeader().directionalLayoutMargins.leading)
  say("list-prominent-header-margin-bottom", UIListContentConfiguration.prominentInsetGroupedHeader().directionalLayoutMargins.bottom)
  let header = UIListContentConfiguration.groupedHeader()
  say("list-grouped-header-margin-leading", header.directionalLayoutMargins.leading)
  say("list-grouped-header-margin-top", header.directionalLayoutMargins.top)
  // Buttons: their insets and paddings by size, and their corners as drawn.
  for (name, size) in [("small", UIButton.Configuration.Size.small), ("medium", .medium), ("large", .large)] {
    var c = UIButton.Configuration.filled(); c.buttonSize = size; c.title = "Button"; c.image = UIImage(systemName: "star")
    let b = UIButton(configuration: c); b.translatesAutoresizingMaskIntoConstraints = false
    host.addSubview(b); b.sizeToFit(); b.frame.origin = CGPoint(x: 20, y: 300); host.layoutIfNeeded()
    say("button-\(name)-inset-leading", b.configuration!.contentInsets.leading)
    say("button-\(name)-image-padding", b.configuration!.imagePadding)
    say("button-\(name)-inset-top", b.configuration!.contentInsets.top)
    say("button-\(name)-title-padding", b.configuration!.titlePadding)
    cornerRadius(b, in: window, label: "button-filled-\(name)-corner")
    b.removeFromSuperview()
  }
  for (name, style) in [("small", UIButton.Configuration.CornerStyle.small), ("medium", .medium), ("large", .large), ("dynamic", .dynamic), ("capsule", .capsule)] {
    var c = UIButton.Configuration.filled(); c.cornerStyle = style; c.title = "Button Button"
    for (w, h) in [(200, 80), (140, 44)] {
      let b = UIButton(configuration: c); host.addSubview(b); b.frame = CGRect(x: 20, y: 300, width: w, height: h); host.layoutIfNeeded()
      cornerRadius(b, in: window, label: "button-cornerstyle-\(name)-\(w)x\(h)")
      b.removeFromSuperview()
    }
  }
  for (name, cfg) in [("bordered", UIButton.Configuration.bordered()), ("gray", UIButton.Configuration.gray()), ("tinted", UIButton.Configuration.tinted())] {
    var c = cfg; c.title = "Button"
    let b = UIButton(configuration: c); host.addSubview(b); b.sizeToFit(); b.frame.origin = CGPoint(x: 20, y: 300); host.layoutIfNeeded()
    cornerRadius(b, in: window, label: "button-\(name)-medium-corner")
    b.removeFromSuperview()
  }
  // The inset-grouped list: its card's corner and its section spacing.
  var list = UICollectionLayoutListConfiguration(appearance: .insetGrouped); list.headerMode = .none
  let layout = UICollectionViewCompositionalLayout.list(using: list)
  let cv = UICollectionView(frame: host.bounds, collectionViewLayout: layout)
  let reg = UICollectionView.CellRegistration<UICollectionViewListCell, Int> { c, _, i in var t = c.defaultContentConfiguration(); t.text = "Row \(i)"; c.contentConfiguration = t }
  final class DS: NSObject, UICollectionViewDataSource {
    let reg: UICollectionView.CellRegistration<UICollectionViewListCell, Int>
    init(_ r: UICollectionView.CellRegistration<UICollectionViewListCell, Int>) { reg = r }
    func numberOfSections(in cv: UICollectionView) -> Int { 2 }
    func collectionView(_ cv: UICollectionView, numberOfItemsInSection s: Int) -> Int { 1 }
    func collectionView(_ cv: UICollectionView, cellForItemAt p: IndexPath) -> UICollectionViewCell { cv.dequeueConfiguredReusableCell(using: reg, for: p, item: p.section) }
  }
  let ds = DS(reg); cv.dataSource = ds; host.addSubview(cv); cv.layoutIfNeeded()
  if let a = cv.cellForItem(at: IndexPath(item: 0, section: 0)), let b = cv.cellForItem(at: IndexPath(item: 0, section: 1)) {
    say("inset-grouped-card-inset", a.frame.minX)
    say("inset-grouped-section-gap", b.frame.minY - a.frame.maxY)
    cornerRadius(a, in: window, label: "inset-grouped-card-corner")
  }
  withExtendedLifetime(ds) {}
}

final class Scene: UIResponder, UIWindowSceneDelegate {
  var window: UIWindow?
  func scene(_ scene: UIScene, willConnectTo session: UISceneSession, options: UIScene.ConnectionOptions) {
    let w = UIWindow(windowScene: scene as! UIWindowScene); w.rootViewController = UIViewController(); w.rootViewController!.view.backgroundColor = .systemBackground
    w.makeKeyAndVisible(); window = w
    DispatchQueue.main.asyncAfter(deadline: .now() + 1.5) { measure(w); print("done"); fflush(stdout); exit(0) }
  }
}
@main final class App: UIResponder, UIApplicationDelegate {
  func application(_ application: UIApplication, configurationForConnecting s: UISceneSession, options: UIScene.ConnectionOptions) -> UISceneConfiguration {
    let c = UISceneConfiguration(name: nil, sessionRole: s.role); c.delegateClass = Scene.self; return c
  }
}
