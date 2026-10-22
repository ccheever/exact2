/** The app's module composed with its packages' implementations (LLP 1133.001 D5): an adapter over each package's
 * class (its shape, host/apple/modules/ExactExtensions.swift), the table that names them, and, for an app with no
 * module of its own, an empty one. */
export function composedModule(packages, ownModule) {
  const lines = ['// Written by host/apple/build.mjs (LLP 1133.001 D5); never edited.', ...packages.map(p => `import ${p.module}`)];
  for (const p of packages) lines.push(
    `final class ExactAdapter_${p.module}: ExactExtensionAdapter {`,
    `    let impl: ${p.className}`,
    // An interface that declares events hands its implementation `emit` and `activation` too (LLP 1133.004 D6).
    `    init(_ context: ExactModuleContext) { impl = ${p.className}(changed: { [weak context] topic in context?.changed(topic) }, agent: context.agent, log: { [weak context] line in context?.diagnostics.log(line) }${p.events ? `, emit: { [weak context] name, args in context?.emit(${JSON.stringify(p.extension)}, name, args) }, activation: { [weak context] name, args, navigate in context?.activation(${JSON.stringify(p.extension)}, name, args, navigate: navigate) ?? [:] }` : ''}) }`,
    '    override func later(_ name: String, _ args: [String: Any], reply: ExactReply) { impl.later(name, args) { reply.send($0) } }',
    '    override func ask(_ name: String, _ args: [String: Any], answer: @escaping ([String: Any]) -> Void) { impl.later(name, args, reply: answer) }',
    '    override func call(_ name: String, _ args: [String: Any]) throws -> [String: Any] { try impl.call(name, args) }',
    '    override func release(_ name: String) { impl.release(name) }',
    '}');
  lines.push(`let exactExtensions: [String: (ExactModuleContext) -> ExactExtensionAdapter] = [${packages.map(p => `${JSON.stringify(p.extension)}: { ExactAdapter_${p.module}($0) }`).join(', ') || ':'}]`);
  // What the runner admits each one's sends by (LLP 1133.006 D8 items 3 and 4), as JSON, which the roster carries.
  lines.push(`let exactExtensionAdmission: [String: String] = [${packages.map(p => `${JSON.stringify(p.extension)}: ${JSON.stringify(JSON.stringify(p.admission ?? {}))}`).join(', ') || ':'}]`);
  if (!ownModule) lines.push('final class ExactComposedModule: ExactModule {}', 'let exactModule: ExactModule.Type = ExactComposedModule.self');
  return lines.join('\n') + '\n';
}
