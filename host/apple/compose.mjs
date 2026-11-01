// The Swift that composes an app's module with its packages' implementations (LLP 1133.001 D5), which
// host/apple/build.mjs writes beside the module's sources and never commits.

/** The module side's fixed Swift, in `root`: the table, its view entries, the hatch bindings and the extensions. */
export const moduleSeam = (resolve, root) => ['ExactNativeModule.swift', 'ExactNativeViews.swift', 'ExactTamber.swift', 'ExactExtensions.swift']
  .map(f => resolve(root, 'host/apple/modules', f));

/** Link declared system frameworks even when only an Objective-C category uses them. Frameworks introduced
 * above the app's minimum OS are weak-linked; the package checks `#available` before using them (LLP 1133.016 D15). */
export function systemFrameworks(packages, os, minimum) {
  const above = (version) => { const [a, b] = [version, minimum ?? '0'].map(v => String(v).split('.').map(Number)); for (let i = 0; i < 3; i++) if ((a[i] ?? 0) !== (b[i] ?? 0)) return (a[i] ?? 0) > (b[i] ?? 0); return false; };
  const names = new Map();
  for (const p of packages) for (const [name, at] of Object.entries(p.frameworks ?? {})) {
    if (at?.[os]) names.set(name, names.get(name) || above(at[os]));
  }
  return [...names].sort(([a], [b]) => a.localeCompare(b)).flatMap(([name, weak]) => weak
    ? ['-Xlinker', '-weak_framework', '-Xlinker', name] : ['-framework', name]);
}

/** The app's module composed with its packages' implementations: an adapter over each package's class (its shape,
 * host/apple/modules/ExactPackage.swift; the adapter's, ExactExtensions.swift) with that session's context, the table
 * that names them, each view a package serves (`views`) made by that session's adapter (LLP 1133.008 D8 item 1), and,
 * for an app with no module of its own, an empty one. */
export function composedModule(packages, ownModule) {
  const authenticator = packages.find(p => p.authenticator);
  const lines = ['// Written by host/apple/build.mjs (LLP 1133.001 D5); never edited.', ...(packages.length ? ['import CoreGraphics', 'import ExactPackage'] : []), ...(authenticator ? ['import Security'] : []), ...packages.map(p => `import ${p.module}`)];
  for (const p of packages) lines.push(
    `final class ExactAdapter_${p.module}: ExactExtensionAdapter {`,
    `    let impl: ${p.className}, context: ExactPackageContext`,
    `    init(_ module: ExactModule, session: String) {`,
    `        context = ExactExtensions.context(module, ${JSON.stringify(p.extension)}, session: session)`,
    `        impl = ${p.className}(context: context)`,
    '    }',
    '    override func later(_ name: String, _ args: [String: Any], preferred: Any?, origin: String, request: Int?, reply: ExactReply) { context.origin = origin; context.request = request; impl.later(name, args, preferred: preferred) { reply.send($0) } }',
    '    override func ask(_ name: String, _ args: [String: Any], answer: @escaping ([String: Any]) -> Void) { impl.later(name, args, preferred: nil, reply: answer) }',
    '    override func call(_ name: String, _ args: [String: Any]) throws -> [String: Any] { try impl.call(name, args) }',
    '    override func release(_ name: String) { impl.release(name) }',
    '    override func release(_ name: String, args: [String: Any]) { impl.release(name, args: args) }',
    '    override func preferred(_ values: [String: Any]) { impl.preferred(values) }',
    '    override func dispose(_ ids: [String]) { impl.dispose(ids) }',
    '    override func cancel(_ request: Int) { impl.cancel(request) }',
    '    override func foreground(_ shown: Bool) { context.foreground = shown; impl.foreground(shown) }',
    '    override func destroy(_ done: @escaping () -> Void) { impl.destroy(done) }',
    '    override func view(_ tag: String, id: String, props: [String: String], events: ExactNativeEvents) throws -> ExactNativeInstance {',
    '        let sent = ExactPackageEvents(send: { kind, text in events.send(kind, text) }, size: { events.intrinsicSize($0) })',
    '        return ExactPackageViewInstance(try impl.view(tag, id: id, props: props, events: sent), events: events)',
    '    }',
    '    override func displayed(_ id: String, _ shown: Bool, origin: String) { impl.displayed(id, shown, origin: origin) }',
    '    override func bound(_ id: String, _ prop: String, origin: String) { impl.bound(id, prop, origin: origin) }',
    '    override func image(_ source: String, width: Int, height: Int, fit: String, owned: Bool, reply: @escaping (CGImage?, String?) -> Void) { impl.image(source, width: width, height: height, fit: fit, owned: owned, reply: reply) }',
    '    override func permissions(_ states: [String: String]) { impl.permissions(states) }',
    '    override func displayed(surface: String, _ shown: Bool, origin: String) { impl.displayed(surface: surface, shown, origin: origin) }',
    '    override func orientation(_ angle: Double) { impl.orientation(angle) }',
    '    override func due(_ wake: [String: Any], done: @escaping (Bool) -> Void) {',
    '        impl.woke(ExactWake(wake, context: context, done: done))',
    '    }',
    // The authenticator entry (LLP 1133.010 D13): the class must conform, which compiling this checks.
    ...(p.authenticator ? [
      '    override func evaluate(_ accessControl: SecAccessControl, _ request: [String: Any], reply: @escaping (AnyObject?, [String: Any]?) -> Void) {',
      '        let origin = request["origin"] as? String ?? "app"',
      '        context.origin = origin',
      '        impl.evaluate(accessControl: accessControl, reason: request["reason"] as? String ?? "", policy: request["policy"] as? String ?? "", origin: origin, reply: reply)',
      '    }'] : []),
    '}');
  if (packages.length) lines.push(
    '/// A package\'s view as the table holds an instance.',
    'final class ExactPackageViewInstance: ExactNativeInstance {',
    '    let made: ExactPackageView',
    '    init(_ made: ExactPackageView, events: ExactNativeEvents) { self.made = made; super.init(events: events) }',
    '    override var view: ExactNativeView { made.view }',
    '    override func setProps(_ props: [String: String]) throws { try made.setProps(props) }',
    '    override func destroy() { made.destroy() }',
    '}');
  lines.push(`let exactAuthenticator: String? = ${authenticator ? JSON.stringify(authenticator.extension) : 'nil'}`);
  lines.push(`let exactStartAtLoad: [String] = [${packages.filter(p => p.start).map(p => JSON.stringify(p.extension)).join(', ')}]`);
  lines.push(`let exactExtensions: [String: (ExactModule, String) -> ExactExtensionAdapter] = [${packages.map(p => `${JSON.stringify(p.extension)}: { ExactAdapter_${p.module}($0, session: $1) }`).join(', ') || ':'}]`);
  const views = packages.flatMap(p => (p.views ?? []).map(tag => `${JSON.stringify(tag)}: ExactNativeFactory(package: { try ExactExtensions.view($0, ${JSON.stringify(p.extension)}, ${JSON.stringify(tag)}, id: $1, props: $2, events: $3) })`));
  lines.push(`let exactPackageViews: [String: ExactNativeFactory] = [${views.join(', ') || ':'}]`);
  if (!ownModule) lines.push('final class ExactComposedModule: ExactModule {}', 'let exactModule: ExactModule.Type = ExactComposedModule.self');
  return lines.join('\n') + '\n';
}
