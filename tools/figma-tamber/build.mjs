// Builds the Figma plugin (LLP 1121 §5 stage 10b): dist/code.js, and
// dist/ui.html with its script inline (a plugin's UI loads no file).
// bun tools/figma-tamber/build.mjs; then Figma › Plugins › Development ›
// Import plugin from manifest… (tools/figma-tamber/manifest.json).
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';

const here = new URL('.', import.meta.url).pathname;
mkdirSync(`${here}dist`, { recursive: true });
const bundle = async entry => {
  const out = await Bun.build({ entrypoints: [`${here}src/${entry}`], target: 'browser', format: 'iife', minify: false });
  if (!out.success) throw new AggregateError(out.logs, `figma-tamber: ${entry} did not build`);
  return out.outputs[0].text();
};
writeFileSync(`${here}dist/code.js`, await bundle('code.ts'));
const ui = (await bundle('ui.ts')).replaceAll('</script', '<\\/script');
writeFileSync(`${here}dist/ui.html`, readFileSync(`${here}src/ui.html`, 'utf8').replace('<script>/*UI*/</script>', () => `<script>${ui}</script>`));
console.log(`${here}dist: code.js, ui.html`);
