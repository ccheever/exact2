// GAP 001: bake cannot capture parent imports. Remove this copy when ancestor mounts work.
// Unchanged body from examples/t3-code/timeline-files.ts at 887b2491b182f851b11253655f6aa84fe2a26708.
// Pierre file-icon resolution (@pierre/trees createFileTreeIconResolver with
// T3's T3_PIERRE_ICONS overrides, MIT, see LICENSE-T3), tables generated from
// the built reference bundle: a path or fence language to the icon token that
// timeline-files.contract draws.

const BY_NAME: Record<string, string> = {".babelrc":"babel",".babelrc.json":"babel",".bash_profile":"bash",".bashrc":"bash",".browserslistrc":"browserslist",".dockerignore":"docker",".eslintignore":"eslint",".eslintrc":"eslint",".eslintrc.cjs":"eslint",".eslintrc.js":"eslint",".eslintrc.json":"eslint",".eslintrc.yaml":"eslint",".eslintrc.yml":"eslint",".gitattributes":"git",".gitignore":"git",".gitkeep":"git",".gitmodules":"git",".oxlintrc.json":"oxc",".postcssrc":"postcss",".postcssrc.json":"postcss",".postcssrc.yaml":"postcss",".postcssrc.yml":"postcss",".prettierignore":"prettier",".prettierrc":"prettier",".prettierrc.cjs":"prettier",".prettierrc.js":"prettier",".prettierrc.json":"prettier",".prettierrc.mjs":"prettier",".prettierrc.toml":"prettier",".prettierrc.yaml":"prettier",".prettierrc.yml":"prettier",".stylelintignore":"stylelint",".stylelintrc":"stylelint",".stylelintrc.cjs":"stylelint",".stylelintrc.js":"stylelint",".stylelintrc.json":"stylelint",".stylelintrc.mjs":"stylelint",".stylelintrc.yaml":"stylelint",".stylelintrc.yml":"stylelint",".terraform.lock.hcl":"terraform",".zprofile":"bash",".zshenv":"bash",".zshrc":"bash","babel.config.cjs":"babel","babel.config.js":"babel","babel.config.json":"babel","babel.config.mjs":"babel","biome.json":"biome","biome.jsonc":"biome","bootstrap.bundle.js":"bootstrap","bootstrap.bundle.min.js":"bootstrap","bootstrap.css":"bootstrap","bootstrap.js":"bootstrap","bootstrap.min.css":"bootstrap","bootstrap.min.js":"bootstrap","bun.lock":"bun","bun.lockb":"bun","bunfig.toml":"bun","claude.md":"claude","compose.yaml":"docker","compose.yml":"docker","docker-compose.override.yml":"docker","docker-compose.yaml":"docker","docker-compose.yml":"docker","dockerfile":"docker","eslint.config.cjs":"eslint","eslint.config.js":"eslint","eslint.config.mjs":"eslint","eslint.config.mts":"eslint","eslint.config.ts":"eslint","gemfile":"ruby","next.config.js":"nextjs","next.config.mjs":"nextjs","next.config.mts":"nextjs","next.config.ts":"nextjs","postcss.config.cjs":"postcss","postcss.config.js":"postcss","postcss.config.mjs":"postcss","postcss.config.ts":"postcss","prettier.config.cjs":"prettier","prettier.config.js":"prettier","prettier.config.mjs":"prettier","rakefile":"ruby","readme.md":"markdown","stylelint.config.cjs":"stylelint","stylelint.config.js":"stylelint","stylelint.config.mjs":"stylelint","svgo.config.cjs":"svgo","svgo.config.js":"svgo","svgo.config.mjs":"svgo","svgo.config.ts":"svgo","tailwind.config.cjs":"tailwind","tailwind.config.js":"tailwind","tailwind.config.mjs":"tailwind","tailwind.config.ts":"tailwind","vite.config.js":"vite","vite.config.mjs":"vite","vite.config.mts":"vite","vite.config.ts":"vite","webpack.config.babel.js":"webpack","webpack.config.cjs":"webpack","webpack.config.js":"webpack","webpack.config.mjs":"webpack","webpack.config.ts":"webpack"};
const BY_EXTENSION: Record<string, string> = {"7z":"zip","astro":"astro","AUTHORS":"text","avif":"image","bash":"bash","bmp":"image","bz2":"zip","c":"c","cc":"cpp","cfg":"text","CHANGELOG":"text","cjs":"javascript","code-workspace":"vscode","conf":"text","CONTRIBUTORS":"text","cpp":"cpp","csh":"bash","css":"css","csv":"table","cts":"typescript","cxx":"cpp","db":"database","editorconfig":"text","env":"text","env.development":"text","env.local":"text","env.production":"text","eot":"font","erb":"ruby","fish":"bash","gemspec":"ruby","gif":"image","go":"go","gql":"graphql","graphql":"graphql","gz":"zip","h":"c","hh":"cpp","hpp":"cpp","htm":"html","html":"html","hxx":"cpp","icns":"image","ico":"image","ini":"text","inl":"cpp","jar":"zip","jpeg":"image","jpg":"image","js":"javascript","json":"json","json5":"json","jsonc":"json","jsonl":"json","jsx":"javascript","ksh":"bash","less":"css","LICENSE":"text","log":"text","markdown":"markdown","mcp":"mcp","md":"markdown","mdx":"markdown","mdx.tsx":"markdown","mjs":"javascript","mm":"cpp","mts":"typescript","ods":"table","otf":"font","png":"image","postcss":"css","py":"python","pyi":"python","pyw":"python","pyx":"python","rake":"ruby","rar":"zip","rb":"ruby","rs":"rust","rst":"text","rtf":"text","sass":"css","scss":"css","sh":"bash","sql":"database","sqlite":"database","sqlite3":"database","styl":"css","svelte":"svelte","svg":"svg","swift":"swift","tar":"zip","tf":"terraform","tfstate":"terraform","tfvars":"terraform","tgz":"zip","tif":"image","tiff":"image","ts":"typescript","tsv":"table","tsx":"typescript","ttf":"font","txt":"text","vue":"vue","war":"zip","wasm":"wasm","wast":"wasm","wat":"wasm","webp":"image","woff":"font","woff2":"font","xhtml":"html","xls":"table","xlsx":"table","xz":"zip","yaml":"yml","yml":"yml","zig":"zig","zip":"zip","zsh":"bash"};
const COMPLETE: Record<string, string> = {"jsx":"react","sass":"sass","scss":"sass","tsx":"react"};
const LANGUAGE_EXTENSION: Record<string, string> = {"bash":"sh","csharp":"cs","javascript":"js","jsx":"jsx","markdown":"md","mdx":"mdx","plaintext":"txt","python":"py","ruby":"rb","rust":"rs","shell":"sh","shellscript":"sh","swift":"swift","typescript":"ts","tsx":"tsx","yaml":"yml"};
// T3_PIERRE_ICONS remaps: the icon's shape without a colour token, so they draw in the default grey.
const T3_NAMES: Record<string, string> = {"package.json":"npm-plain","tsconfig.json":"typescript-plain","agents.md":"agents","pnpm-lock.yaml":"pnpm","pnpm-workspace.yaml":"pnpm"};

/** The icon token for a file path ("default" when no specific icon exists). */
export function fileIconToken(path: string): string {
  const base = (path.split('/').pop() ?? path).toLowerCase();
  const t3 = T3_NAMES[base];
  if (t3) return t3;
  const named = BY_NAME[base];
  if (named) return named;
  const parts = base.split('.');
  for (let index = 1; index < parts.length; index++) {
    const extension = parts.slice(index).join('.');
    const token = COMPLETE[extension] ?? BY_EXTENSION[extension];
    if (token) return token;
  }
  return 'default';
}
/** MarkdownCodeBlockTitleContent: a fence language draws its icon only when it has a specific one. */
export function languageIconToken(language: string): string {
  const lower = language.trim().toLowerCase();
  if (!lower) return '';
  const token = fileIconToken(lower === 'dockerfile' ? 'Dockerfile' : `file.${LANGUAGE_EXTENSION[lower] ?? lower}`);
  return token === 'default' ? '' : token;
}
