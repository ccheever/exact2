// A moved or removed page's old path, and where it lives now. tools/gen.ts checks every entry and gives it
// to app.ts (client: the not-found page replaces the URL); tools/postbuild.ts writes dist/_redirects (301 on
// hosts that read it) and a fallback page at each old path (serve and any static host).
export const redirects: Record<string, string> = {
  '/learn/contract-for-humans/contents': '/learn/contract-for-humans',
  '/start-here': '/start',
};
