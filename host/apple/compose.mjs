// The Swift that composes an app's module with its packages' implementations (LLP 1133.001 D5), which
// host/apple/build.mjs writes beside the module's sources and never commits.

/** The app's module composed with its packages' implementations: an adapter over each package's class (its shape,
 * host/apple/modules/ExactPackage.swift; the adapter's, ExactExtensions.swift) with that session's context, the table
 * that names them, and, for an app with no module of its own, an empty one. */
export function composedModule(packages, ownModule) {
  const lines = ['// Written by host/apple/build.mjs (LLP 1133.001 D5); never edited.', ...(packages.length ? ['import ExactPackage'] : []), ...packages.map(p => `import ${p.module}`)];
  for (const p of packages) lines.push(
    `final class ExactAdapter_${p.module}: ExactExtensionAdapter {`,
    `    let impl: ${p.className}, context: ExactPackageContext`,
    `    init(_ module: ExactModuleContext, session: String) {`,
    `        context = ExactPackageContext(agent: module.agent && !ExactExtensions.real.contains(${JSON.stringify(p.extension)}), session: session, changed: { [weak module] topic in module?.changed(topic) }, log: { [weak module] line in module?.diagnostics.log(line) }, now: { [weak module] in module?.now() ?? 0 })`,
    `        impl = ${p.className}(context: context)`,
    '    }',
    '    override func later(_ name: String, _ args: [String: Any], preferred: Any?, reply: ExactReply) { impl.later(name, args, preferred: preferred) { reply.send($0) } }',
    '    override func call(_ name: String, _ args: [String: Any]) throws -> [String: Any] { try impl.call(name, args) }',
    '    override func release(_ name: String) { impl.release(name) }',
    '    override func release(_ name: String, args: [String: Any]) { impl.release(name, args: args) }',
    '    override func preferred(_ values: [String: Any]) { impl.preferred(values) }',
    '    override func dispose(_ ids: [String]) { impl.dispose(ids) }',
    '    override func foreground(_ shown: Bool) { context.foreground = shown; impl.foreground(shown) }',
    '    override func destroy(_ done: @escaping () -> Void) { impl.destroy(done) }',
    '}');
  lines.push(`let exactExtensions: [String: (ExactModuleContext, String) -> ExactExtensionAdapter] = [${packages.map(p => `${JSON.stringify(p.extension)}: { ExactAdapter_${p.module}($0, session: $1) }`).join(', ') || ':'}]`);
  if (!ownModule) lines.push('final class ExactComposedModule: ExactModule {}', 'let exactModule: ExactModule.Type = ExactComposedModule.self');
  return lines.join('\n') + '\n';
}
