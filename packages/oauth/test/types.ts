// index.d.ts declares what index.ts implements (LLP 1146 D5), read as an app's
// checker reads it, through the package's `types` export: beside index.ts, an
// import of `../index.d.ts` resolves to index.ts itself. Each pair is one type,
// as `Same` compares them: a member either side lacks, an optional one too,
// fails the check, which one-way assignability would let by.
import type * as declared from '@exact/oauth';
import * as implemented from '../index.ts';

type Same<A, B> = (<T>() => T extends A ? 1 : 2) extends (<T>() => T extends B ? 1 : 2) ? true : false;
const same = <A, B>(proof: Same<A, B>): Same<A, B> => proof;

export const checks = [
  same<declared.OAuthConfig, implemented.OAuthConfig>(true),
  same<declared.OAuthSession, implemented.OAuthSession>(true),
  same<declared.OAuthResult, implemented.OAuthResult>(true),
  same<declared.OAuthStore, implemented.OAuthStore>(true),
  same<declared.OAuthProvider, implemented.OAuthProvider>(true),
  same<declared.OAuthError, implemented.OAuthError>(true),
  same<typeof declared.OAuthError, typeof implemented.OAuthError>(true),
  same<typeof declared.oauth, typeof implemented.oauth>(true),
];
