// X14 on the web page: the same module as modules/apple/Slow.swift.
let calls = 0;
let changed = () => {};
export function connect(ctx) { changed = ctx.changed; }
export async function later(request) {
  calls += 1;
  console.log(`x14 module: later #${calls} (ask ${request.ask})`);
  if (calls === 1) for (let i = 1; i <= 5; i++) setTimeout(() => { console.log(`x14 module: changed x (${i})`); changed('x'); }, 300 * i);
  await new Promise((r) => setTimeout(r, 500));
  console.log(`x14 module: reply to ask ${request.ask}`);
  return { ask: request.ask };
}
