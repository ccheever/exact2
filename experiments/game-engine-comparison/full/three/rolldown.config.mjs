import { cp, mkdir, readFile, rm, writeFile } from 'node:fs/promises';
import { defineConfig } from 'rolldown';

export default defineConfig({
  input: 'src/main.js',
  output: {
    dir: 'dist',
    entryFileNames: 'game-[hash].js',
    assetFileNames: 'assets/[name]-[hash][extname]',
    format: 'es',
    sourcemap: true,
  },
  plugins: [{
    name: 'lanterns-static-files',
    async buildStart() {
      await rm('dist', { recursive: true, force: true });
    },
    async writeBundle(_options, bundle) {
      await mkdir('dist/assets', { recursive: true });
      await cp('index.html', 'dist/index.html');
      await cp('src/styles.css', 'dist/styles.css');
      await cp('assets/Fox.glb', 'dist/assets/Fox.glb');
      await cp('assets/ATTRIBUTION.md', 'dist/assets/ATTRIBUTION.md');
      await cp('assets/Fox-LICENSE.md', 'dist/assets/Fox-LICENSE.md');
      const entry = Object.values(bundle).find((item) => item.type === 'chunk' && item.isEntry);
      let html = await readFile('dist/index.html', 'utf8');
      html = html.replace('__GAME_SCRIPT__', `./${entry.fileName}`);
      await writeFile('dist/index.html', html);
    },
  }],
});
