// UIKit spring probe for LLP 1099. Builds as a one-scene simulator app; on
// launch it makes each UIKit / SwiftUI spring call, reads back what it
// resolves to (the CASpringAnimation UIKit adds, or SwiftUI's Spring
// fields), samples rendered curves on a paused layer, writes
// tmp/out.txt and exits. See probe.sh. Line formats are in the LLP, §3.
import UIKit
import SwiftUI

var out = ""
func p(_ s: String) { out += s + "\n" }
var v: UIView!
func springs() -> [CASpringAnimation] {
  (v.layer.animationKeys() ?? []).compactMap { v.layer.animation(forKey: $0) as? CASpringAnimation }
}
func line(_ tag: String, _ s: CASpringAnimation) {
  p("\(tag) mass=\(s.mass) stiffness=\(s.stiffness) damping=\(s.damping) v0=\(s.initialVelocity) duration=\(s.duration) settling=\(s.settlingDuration) additive=\(s.isAdditive) key=\(s.keyPath ?? "-") aod=\(s.allowsOverdamping)")
}
func reset() { UIView.performWithoutAnimation { v.transform = .identity; v.alpha = 1; v.center = CGPoint(x: 50, y: 50) }; v.layer.removeAllAnimations() }
func dump(_ tag: String, _ go: () -> Void) {
  reset(); go()
  let s = springs()
  if s.isEmpty { p("\(tag) NONE") }
  for a in s { line(tag, a) }
  reset()
}
// Samples a spring animation's presentation on a paused layer, every 10 ms
// from 0 to 30 ms past its duration, as a fraction of the way from 0 to 1.
func sample(_ tag: String, _ a: CASpringAnimation, _ root: CALayer) {
  let L = CALayer(); L.frame = CGRect(x: 0, y: 0, width: 10, height: 10); L.position = CGPoint(x: 1000, y: 0)
  root.addSublayer(L); L.speed = 0; L.timeOffset = 0
  a.beginTime = 1e-9
  L.add(a, forKey: "s"); CATransaction.flush()
  var row = "\(tag) k=\(a.stiffness) c=\(a.damping) v0=\(a.initialVelocity) dur=\(a.duration) aod=\(a.allowsOverdamping)"
  var t = 0.0
  while t <= a.duration + 0.03 {
    L.timeOffset = t + 1e-9; CATransaction.flush()
    let x = L.presentation()?.position.x ?? .nan
    row += " \(t):\(a.isAdditive ? (x - 1000) / 1000 + 1 : x / 1000)"
    t += 0.01
  }
  p(row); L.removeFromSuperlayer()
}

class App: UIResponder, UIApplicationDelegate {
  func application(_ app: UIApplication, configurationForConnecting s: UISceneSession, options: UIScene.ConnectionOptions) -> UISceneConfiguration {
    let c = UISceneConfiguration(name: nil, sessionRole: s.role); c.delegateClass = Scene.self; return c
  }
}
class Scene: UIResponder, UIWindowSceneDelegate {
  var window: UIWindow?
  func scene(_ scene: UIScene, willConnectTo session: UISceneSession, options o: UIScene.ConnectionOptions) {
    window = UIWindow(windowScene: scene as! UIWindowScene)
    window!.rootViewController = UIViewController()
    window!.makeKeyAndVisible()
    DispatchQueue.main.asyncAfter(deadline: .now() + 1) { self.run() }
  }
  func run() {
    let root = window!.rootViewController!.view!
    v = UIView(frame: CGRect(x: 0, y: 0, width: 100, height: 100)); root.addSubview(v)
    p("probe iOS \(UIDevice.current.systemVersion)")
    let durations: [Double] = [0.1, 0.2, 0.25, 0.3, 0.35, 0.4, 0.5, 0.6, 0.8, 1.0, 1.5, 2.0, 3.0]
    let zetas: [Double] = [0.01, 0.05, 0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.75, 0.8, 0.85, 0.9, 0.95, 0.99, 1.0, 1.01, 1.2, 1.5, 2.0, 5.0]
    let vels: [Double] = [0, 0.5, 1, 2, 5, 10, 20, -1, -5]

    // 1. UIView.animate(withDuration:delay:usingSpringWithDamping:initialSpringVelocity:)
    for d in durations { for z in zetas { for vel in vels {
      dump("A d=\(d) z=\(z) v=\(vel)") {
        UIView.animate(withDuration: d, delay: 0, usingSpringWithDamping: z, initialSpringVelocity: vel, options: [], animations: { v.transform = CGAffineTransform(scaleX: 0.2, y: 0.2) })
      }
    }}}
    // 1b. other properties / distances (does distance or key matter?)
    for (name, f) in [("center100", { v.center = CGPoint(x: 150, y: 50) }), ("center1000", { v.center = CGPoint(x: 1050, y: 50) }), ("alpha", { v.alpha = 0.3 })] as [(String, () -> Void)] {
      for vel in [0.0, 1.0, 5.0] {
        dump("A1b \(name) d=0.4 z=0.8 v=\(vel)") { UIView.animate(withDuration: 0.4, delay: 0, usingSpringWithDamping: 0.8, initialSpringVelocity: vel, options: [], animations: f) }
      }
    }
    // 2. UIViewPropertyAnimator(duration:dampingRatio:)
    for d in [0.2, 0.4, 1.0] { for z in [0.3, 0.8, 1.0, 1.5] {
      dump("B d=\(d) z=\(z) v=0") {
        let a = UIViewPropertyAnimator(duration: d, dampingRatio: z) { v.transform = CGAffineTransform(scaleX: 0.2, y: 0.2) }
        a.startAnimation()
      }
    }}
    // 3. UISpringTimingParameters(dampingRatio:initialVelocity:)
    for d in [0.2, 0.4, 1.0] { for z in [0.3, 0.8, 1.0] { for vel in [0.0, 1.0, 5.0] {
      dump("C d=\(d) z=\(z) v=\(vel)") {
        let a = UIViewPropertyAnimator(duration: d, timingParameters: UISpringTimingParameters(dampingRatio: z, initialVelocity: CGVector(dx: vel, dy: vel)))
        a.addAnimations { v.transform = CGAffineTransform(scaleX: 0.2, y: 0.2) }
        a.startAnimation()
      }
    }}}
    // 4. UISpringTimingParameters(mass:stiffness:damping:initialVelocity:)
    for d in [0.2, 1.0] { for (k, c) in [(300.0, 30.0), (100.0, 10.0), (500.0, 10.0)] {
      dump("D d=\(d) k=\(k) c=\(c) v=0") {
        let a = UIViewPropertyAnimator(duration: d, timingParameters: UISpringTimingParameters(mass: 1, stiffness: k, damping: c, initialVelocity: .zero))
        a.addAnimations { v.transform = CGAffineTransform(scaleX: 0.2, y: 0.2) }
        a.startAnimation()
      }
    }}
    // 5. iOS 17 UIView.animate(springDuration:bounce:initialSpringVelocity:)
    for d in [0.2, 0.4, 0.5, 1.0] { for b in [-0.5, -0.2, 0.0, 0.15, 0.3, 0.5, 0.8] { for vel in [0.0, 1.0] {
      dump("E d=\(d) b=\(b) v=\(vel)") {
        UIView.animate(springDuration: d, bounce: b, initialSpringVelocity: vel, delay: 0, options: [], animations: { v.transform = CGAffineTransform(scaleX: 0.2, y: 0.2) })
      }
    }}}
    // 5b. iOS 17 UISpringTimingParameters(duration:bounce:initialVelocity:)
    for d in [0.4, 1.0] { for b in [0.0, 0.3] {
      dump("E2 d=\(d) b=\(b) v=0") {
        let a = UIViewPropertyAnimator(duration: 99, timingParameters: UISpringTimingParameters(duration: d, bounce: b, initialVelocity: .zero))
        a.addAnimations { v.transform = CGAffineTransform(scaleX: 0.2, y: 0.2) }
        a.startAnimation()
      }
    }}
    // 6. SwiftUI Spring
    for r in [0.2, 0.4, 0.55, 1.0] { for z in [0.3, 0.825, 1.0, 1.5] {
      let s = Spring(response: r, dampingRatio: z)
      p("F response=\(r) z=\(z) mass=\(s.mass) stiffness=\(s.stiffness) damping=\(s.damping) settling=\(s.settlingDuration) duration=\(s.duration) bounce=\(s.bounce)")
    }}
    for d in [0.2, 0.4, 0.5, 1.0] { for b in [-0.5, 0.0, 0.15, 0.3, 0.8] {
      let s = Spring(duration: d, bounce: b)
      p("G duration=\(d) bounce=\(b) mass=\(s.mass) stiffness=\(s.stiffness) damping=\(s.damping) settling=\(s.settlingDuration) response=\(s.response) z=\(s.dampingRatio)")
    }}
    // 6b. SwiftUI Spring(settlingDuration:dampingRatio:)
    for d in [0.4, 1.0] { for z in [0.5, 0.8, 1.0] {
      let s = Spring(settlingDuration: d, dampingRatio: z)
      p("G2 settling=\(d) z=\(z) mass=\(s.mass) stiffness=\(s.stiffness) damping=\(s.damping) settlingOut=\(s.settlingDuration)")
    }}
    // 7. Additive retarget: two calls in a row
    reset()
    UIView.animate(withDuration: 0.4, delay: 0, usingSpringWithDamping: 0.8, initialSpringVelocity: 0, options: [], animations: { v.center = CGPoint(x: 150, y: 50) })
    UIView.animate(withDuration: 0.4, delay: 0, usingSpringWithDamping: 0.8, initialSpringVelocity: 0, options: [], animations: { v.center = CGPoint(x: 350, y: 50) })
    for k in v.layer.animationKeys() ?? [] { if let s = v.layer.animation(forKey: k) as? CASpringAnimation { p("H key=\(k) from=\(String(describing: s.fromValue)) to=\(String(describing: s.toValue)) by=\(String(describing: s.byValue)) additive=\(s.isAdditive) k=\(s.stiffness)") } }
    reset()
    UIView.animate(withDuration: 0.4, delay: 0, usingSpringWithDamping: 0.8, initialSpringVelocity: 0, options: [], animations: { v.center = CGPoint(x: 150, y: 50) })
    UIView.animate(withDuration: 0.4, delay: 0, usingSpringWithDamping: 0.8, initialSpringVelocity: 0, options: [.beginFromCurrentState], animations: { v.center = CGPoint(x: 350, y: 50) })
    for k in v.layer.animationKeys() ?? [] { if let s = v.layer.animation(forKey: k) as? CASpringAnimation { p("H2 key=\(k) from=\(String(describing: s.fromValue)) to=\(String(describing: s.toValue)) additive=\(s.isAdditive)") } }
    reset()

    // X. Edge cases: an overdamped physical timing parameter, a negative
    // bounce through the timing-parameter initialiser, damping ratio 0, a
    // call that does not change the value, and a velocity vector with dx != dy.
    dump("X phys-overdamped k=100 c=40") {
      let a = UIViewPropertyAnimator(duration: 1, timingParameters: UISpringTimingParameters(mass: 1, stiffness: 100, damping: 40, initialVelocity: .zero))
      a.addAnimations { v.transform = CGAffineTransform(scaleX: 0.2, y: 0.2) }; a.startAnimation()
    }
    for b in [-0.5, -0.2] {
      dump("X E2 d=0.4 b=\(b)") {
        let a = UIViewPropertyAnimator(duration: 99, timingParameters: UISpringTimingParameters(duration: 0.4, bounce: b, initialVelocity: .zero))
        a.addAnimations { v.transform = CGAffineTransform(scaleX: 0.2, y: 0.2) }; a.startAnimation()
      }
    }
    for vel in [0.0, 1.0] {
      dump("X zeta0 d=0.4 v=\(vel)") { UIView.animate(withDuration: 0.4, delay: 0, usingSpringWithDamping: 0, initialSpringVelocity: vel, options: [], animations: { v.transform = CGAffineTransform(scaleX: 0.2, y: 0.2) }) }
    }
    dump("X unchanged-center v=1") { UIView.animate(withDuration: 0.4, delay: 0, usingSpringWithDamping: 0.8, initialSpringVelocity: 1, options: [], animations: { v.center = CGPoint(x: 50, y: 50) }) }
    dump("X vector dx=1 dy=5 center") {
      let a = UIViewPropertyAnimator(duration: 0.4, timingParameters: UISpringTimingParameters(dampingRatio: 0.8, initialVelocity: CGVector(dx: 1, dy: 5)))
      a.addAnimations { v.center = CGPoint(x: 150, y: 150) }; a.startAnimation()
    }
    dump("X vector dx=5 dy=1 center") {
      let a = UIViewPropertyAnimator(duration: 0.4, timingParameters: UISpringTimingParameters(dampingRatio: 0.8, initialVelocity: CGVector(dx: 5, dy: 1)))
      a.addAnimations { v.center = CGPoint(x: 150, y: 150) }; a.startAnimation()
    }
    // W. Dense sweep at duration 1 s: velocity -5...20 by 0.05 (velocity x duration is the
    // only other dimensionless input; see the LLP).
    for z in [0.01, 0.05, 0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 0.95, 0.99, 1.0] { for i in -100...400 {
      let vel = Double(i) * 0.05
      reset()
      UIView.animate(withDuration: 1.0, delay: 0, usingSpringWithDamping: z, initialSpringVelocity: vel, options: [], animations: { v.transform = CGAffineTransform(scaleX: 0.2, y: 0.2) })
      if let a = springs().first { p("W z=\(z) u=\(vel) k=\(a.stiffness)") }
    }}
    reset()
    // U. The spring UIKit creates, sampled as Core Animation renders it.
    let calls: [(String, () -> Void)] = [
      ("A d=0.4 z=0.8 v=1", { UIView.animate(withDuration: 0.4, delay: 0, usingSpringWithDamping: 0.8, initialSpringVelocity: 1, options: [], animations: { v.center = CGPoint(x: 1050, y: 50) }) }),
      ("A d=0.2 z=0.06 v=0.8", { UIView.animate(withDuration: 0.2, delay: 0, usingSpringWithDamping: 0.06, initialSpringVelocity: 0.8, options: [], animations: { v.center = CGPoint(x: 1050, y: 50) }) }),
      ("A d=1 z=0.3 v=1", { UIView.animate(withDuration: 1, delay: 0, usingSpringWithDamping: 0.3, initialSpringVelocity: 1, options: [], animations: { v.center = CGPoint(x: 1050, y: 50) }) }),
      ("E d=0.4 b=-0.5 v=0", { UIView.animate(springDuration: 0.4, bounce: -0.5, initialSpringVelocity: 0, delay: 0, options: [], animations: { v.center = CGPoint(x: 1050, y: 50) }) }),
      ("E d=0.4 b=-0.2 v=1", { UIView.animate(springDuration: 0.4, bounce: -0.2, initialSpringVelocity: 1, delay: 0, options: [], animations: { v.center = CGPoint(x: 1050, y: 50) }) }),
      ("E d=0.4 b=0 v=0", { UIView.animate(springDuration: 0.4, bounce: 0, initialSpringVelocity: 0, delay: 0, options: [], animations: { v.center = CGPoint(x: 1050, y: 50) }) }),
      ("E d=0.4 b=0.3 v=0", { UIView.animate(springDuration: 0.4, bounce: 0.3, initialSpringVelocity: 0, delay: 0, options: [], animations: { v.center = CGPoint(x: 1050, y: 50) }) }),
    ]
    for (name, go) in calls {
      reset(); go()
      if let src = springs().first { let a = src.copy() as! CASpringAnimation; v.layer.removeAllAnimations(); sample("U \(name)", a, root.layer) }
    }
    reset()
    // S. Hand-built CASpringAnimations (Core Animation's own curve, independent of UIKit).
    for (k, c, v0, dur, aod) in [(497.5361505635245, 35.68882942101944, 1.0, 0.4, false), (80.89737200360506, 14.390874620023238, 0, 1.0, false), (300, 30, -2, 0.6, false), (246.74011002723395, 62.83185307179586, 1, 0.6, false), (246.74011002723395, 62.83185307179586, 1, 0.6, true), (246.74011002723395, 39.269908169872416, 0, 0.6, true), (100, 40, 3, 1.0, true)] as [(Double, Double, Double, Double, Bool)] {
      let a = CASpringAnimation(keyPath: "position.x")
      a.mass = 1; a.stiffness = k; a.damping = c; a.initialVelocity = v0; a.duration = dur; a.allowsOverdamping = aod
      a.fromValue = NSNumber(value: 0.0); a.toValue = NSNumber(value: 1000.0)
      sample("S", a, root.layer)
    }

    // R. Retargets: a second call 100 or 250 ms into the first, on a paused
    // container layer, sampled every 10 ms (position.x in points, or opacity).
    let box = UIView(frame: root.bounds); root.addSubview(box)
    let w = UIView(frame: CGRect(x: 0, y: 0, width: 100, height: 100)); box.addSubview(w)
    func run(_ name: String, at t1: Double, _ first: () -> Void, _ second: () -> Void, read: (CALayer) -> Double) {
      UIView.performWithoutAnimation { w.center = CGPoint(x: 50, y: 50); w.alpha = 1; w.transform = .identity }
      w.layer.removeAllAnimations()
      box.layer.speed = 0; box.layer.timeOffset = 0; CATransaction.flush()
      first(); CATransaction.flush()
      box.layer.timeOffset = t1; CATransaction.flush()
      second(); CATransaction.flush()
      var row = "R \(name) t1=\(t1)"
      for k in w.layer.animationKeys() ?? [] { if let a = w.layer.animation(forKey: k) as? CABasicAnimation { row += " [\(k) bt=\(a.beginTime) from=\(String(describing: a.fromValue)) to=\(String(describing: a.toValue)) add=\(a.isAdditive) dur=\(a.duration)\((a as? CASpringAnimation).map { " k=\($0.stiffness) v0=\($0.initialVelocity)" } ?? "")]" } }
      var t = 0.0
      while t <= 1.2 { box.layer.timeOffset = t; CATransaction.flush(); row += " \(t):\(read(w.layer.presentation() ?? w.layer))"; t += 0.01 }
      out += row + "\n"
      box.layer.speed = 1
    }
    let px: (CALayer) -> Double = { Double($0.position.x) }
    let op: (CALayer) -> Double = { Double($0.opacity) }
    for t1 in [0.1, 0.25] {
      run("center", at: t1, { UIView.animate(withDuration: 0.4, delay: 0, usingSpringWithDamping: 0.8, initialSpringVelocity: 0, options: [], animations: { w.center = CGPoint(x: 150, y: 50) }) },
                            { UIView.animate(withDuration: 0.4, delay: 0, usingSpringWithDamping: 0.8, initialSpringVelocity: 1, options: [], animations: { w.center = CGPoint(x: 350, y: 50) }) }, read: px)
      run("center-bfcs", at: t1, { UIView.animate(withDuration: 0.4, delay: 0, usingSpringWithDamping: 0.8, initialSpringVelocity: 0, options: [], animations: { w.center = CGPoint(x: 150, y: 50) }) },
                            { UIView.animate(withDuration: 0.4, delay: 0, usingSpringWithDamping: 0.8, initialSpringVelocity: 1, options: [.beginFromCurrentState], animations: { w.center = CGPoint(x: 350, y: 50) }) }, read: px)
      run("alpha", at: t1, { UIView.animate(withDuration: 0.4, delay: 0, usingSpringWithDamping: 0.8, initialSpringVelocity: 0, options: [], animations: { w.alpha = 0.2 }) },
                            { UIView.animate(withDuration: 0.4, delay: 0, usingSpringWithDamping: 0.8, initialSpringVelocity: 0, options: [], animations: { w.alpha = 1 }) }, read: op)
      run("alpha-v1-bfcs", at: t1, { UIView.animate(withDuration: 0.4, delay: 0, usingSpringWithDamping: 0.8, initialSpringVelocity: 0, options: [], animations: { w.alpha = 0.2 }) },
                            { UIView.animate(withDuration: 0.4, delay: 0, usingSpringWithDamping: 0.8, initialSpringVelocity: 1, options: [.beginFromCurrentState], animations: { w.alpha = 1 }) }, read: op)
      run("alpha-bfcs", at: t1, { UIView.animate(withDuration: 0.4, delay: 0, usingSpringWithDamping: 0.8, initialSpringVelocity: 0, options: [], animations: { w.alpha = 0.2 }) },
                            { UIView.animate(withDuration: 0.4, delay: 0, usingSpringWithDamping: 0.8, initialSpringVelocity: 0, options: [.beginFromCurrentState], animations: { w.alpha = 1 }) }, read: op)
    }
    // R (transforms). Scale and rotation retargets, read back from the
    // presentation transform: scale = |first column|, rotation = atan2 in degrees.
    let sc: (CALayer) -> Double = { let m = $0.transform; return Double(sqrt(m.m11 * m.m11 + m.m12 * m.m12)) }
    let rot: (CALayer) -> Double = { let m = $0.transform; return Double(atan2(m.m12, m.m11)) * 180 / .pi }
    func S(_ x: CGFloat) -> CGAffineTransform { CGAffineTransform(scaleX: x, y: x) }
    func Rz(_ deg: CGFloat) -> CGAffineTransform { CGAffineTransform(rotationAngle: deg * .pi / 180) }
    for t1 in [0.1, 0.25] {
      run("scale", at: t1, { UIView.animate(withDuration: 0.4, delay: 0, usingSpringWithDamping: 0.8, initialSpringVelocity: 0, options: [], animations: { w.transform = S(0.5) }) },
                           { UIView.animate(withDuration: 0.4, delay: 0, usingSpringWithDamping: 0.8, initialSpringVelocity: 1, options: [], animations: { w.transform = S(0.25) }) }, read: sc)
      run("scale-up", at: t1, { UIView.animate(withDuration: 0.4, delay: 0, usingSpringWithDamping: 0.8, initialSpringVelocity: 0, options: [], animations: { w.transform = S(2) }) },
                           { UIView.animate(withDuration: 0.4, delay: 0, usingSpringWithDamping: 0.8, initialSpringVelocity: 1, options: [], animations: { w.transform = S(3) }) }, read: sc)
      run("scale-to-zero", at: t1, { UIView.animate(withDuration: 0.4, delay: 0, usingSpringWithDamping: 0.8, initialSpringVelocity: 0, options: [], animations: { w.transform = S(0.5) }) },
                           { UIView.animate(withDuration: 0.4, delay: 0, usingSpringWithDamping: 0.8, initialSpringVelocity: 0, options: [], animations: { w.transform = S(0) }) }, read: sc)
      run("rotate", at: t1, { UIView.animate(withDuration: 0.4, delay: 0, usingSpringWithDamping: 0.8, initialSpringVelocity: 0, options: [], animations: { w.transform = Rz(60) }) },
                           { UIView.animate(withDuration: 0.4, delay: 0, usingSpringWithDamping: 0.8, initialSpringVelocity: 1, options: [], animations: { w.transform = Rz(150) }) }, read: rot)
    }
    try? out.write(toFile: NSTemporaryDirectory() + "out.txt", atomically: true, encoding: .utf8)
    exit(0)
  }
}
UIApplicationMain(CommandLine.argc, CommandLine.unsafeArgv, nil, NSStringFromClass(App.self))
