// The suite writes the id `exact new` gave the app. The app reads no data of
// its own, and speaking needs no grant.
export const appId = '__APP_ID__';
export const grants = '';

export const answer = (source: string) => {
  throw new Error(`no data source ${source}`);
};
