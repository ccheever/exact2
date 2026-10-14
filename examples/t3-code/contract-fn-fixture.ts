// Test helper: a one-line Contract `fn` of this app evaluated as JavaScript, so a test checks what the function
// returns rather than its text (usage-pr-pages.test.ts, browser-capture.test.ts).

const source = (file: string) => Bun.file(new URL(`./${file}`, import.meta.url)).text();

/** A one-line Contract `fn`'s body as JavaScript, with its non-method dependencies supplied by the caller. */
export const contractFn = async (file: string, name: string, params: string[], extra: Record<string, unknown> = {}) => {
  const body = new RegExp(`^fn ${name}\\([^)]*\\): [^=]+ = (.+)$`, 'm').exec(await source(file))?.[1];
  if (!body) throw new Error(`${file}: no fn ${name}`);
  const js = body.replace(/\band\b/g, '&&').replace(/\bor\b/g, '||').replace(/\bnot\b/g, '!');
  return new Function(...Object.keys(extra), ...params, `return ${js};`).bind(null, ...Object.values(extra)) as (...args: unknown[]) => unknown;
};
