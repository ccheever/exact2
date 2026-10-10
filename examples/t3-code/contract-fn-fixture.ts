// Test helper: a one-line Contract `fn` of this app evaluated as JavaScript, so a test checks what the function
// returns rather than its text (usage-pr-pages.test.ts, browser-capture.test.ts).

const source = (file: string) => Bun.file(new URL(`./${file}`, import.meta.url)).text();

/** A one-line Contract `fn`'s body as JavaScript, over the standard calls it uses. */
export const contractFn = async (file: string, name: string, params: string[], extra: Record<string, unknown> = {}) => {
  const body = new RegExp(`^fn ${name}\\([^)]*\\): [^=]+ = (.+)$`, 'm').exec(await source(file))?.[1];
  if (!body) throw new Error(`${file}: no fn ${name}`);
  const js = body.replace(/\band\b/g, '&&').replace(/\bor\b/g, '||').replace(/\bnot\b/g, '!');
  const std = {
    includes: (within: string | unknown[], item: unknown) => (within as unknown[]).includes(item as never),
    toLowerCase: (text: string) => text.toLowerCase(), trim: (text: string) => text.trim(),
    slice: (items: unknown[], start: number, end?: number) => items.slice(start, end),
    concat: (a: unknown[], b: unknown[]) => [...a, ...b], filter: (items: unknown[], keep: (item: never) => boolean) => items.filter(keep as never),
    ...extra,
  };
  return new Function(...Object.keys(std), ...params, `return ${js};`).bind(null, ...Object.values(std)) as (...args: unknown[]) => unknown;
};
