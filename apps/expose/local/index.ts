// The baked OpenRouter key when no build has written one: none. A native build with EXPOSE_OPENROUTER_KEY set writes
// apps/expose/local.ts (gitignored), which `./local` resolves to before this file, on every resolver that reads
// app.ts; so a clean checkout builds with nothing generated, and the key never enters git.
export const bakedKey: string = '';
