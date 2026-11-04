// The site's public origin, such as "https://docs.example.com": the sitemap, canonical links and JSON-LD URLs
// are absolute, so they are written only when it is set. SITE_ORIGIN overrides it for one build.
const ORIGIN = '';
const PLACEHOLDER = 'https://docs.exact.example';

export const site = {
  origin: (process.env.SITE_ORIGIN ?? ORIGIN).replace(/\/+$/, ''),
  get docs() { return this.origin || PLACEHOLDER; },
  name: 'Exact Docs',
  description:
    'Exact is a cross-platform application runtime: an interface in Contract, data in TypeScript or Rust, on the web, macOS, iOS and Linux.',
};
