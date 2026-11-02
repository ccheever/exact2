import { parseSync } from 'rolldown/utils';

// Select declarations, not text between neighbouring functions: a new neighbour
// must not silently become part of a fixture's scope.
export function glueDeclarations(source, names) {
  const parsed = parseSync('glue.js', source);
  if (parsed.errors.length) throw new Error(`glue.js fixture: ${JSON.stringify(parsed.errors)}`);
  const declarations = new Map();
  for (const statement of parsed.program.body) {
    if (statement.type === 'FunctionDeclaration') declarations.set(statement.id.name, source.slice(statement.start, statement.end));
    if (statement.type === 'VariableDeclaration') for (const declaration of statement.declarations) {
      if (declaration.id.type === 'Identifier') declarations.set(declaration.id.name, `${statement.kind} ${source.slice(declaration.start, declaration.end)};`);
    }
  }
  return names.map(name => {
    if (!declarations.has(name)) throw new Error(`glue.js fixture: missing declaration ${name}`);
    return declarations.get(name);
  }).join('\n');
}

export function checkedGlueCall(call) {
  return (...args) => {
    try { return call(...args); }
    catch (error) {
      if (error instanceof ReferenceError) throw new Error(`glue.js fixture: supply the module binding used by ${call.name}: ${error.message}`, { cause: error });
      throw error;
    }
  };
}
