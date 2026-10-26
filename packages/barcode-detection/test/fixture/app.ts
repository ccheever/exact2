// The suite writes the id `exact new` gave the app. The app reads no data of
// its own: every source is a module's.
export const appId = '__APP_ID__';
export const grants = 'device.camera purpose.camera\nfs.read app:/tmp/picked';

export const answer = (source: string) => {
  throw new Error(`no data source ${source}`);
};
