// The suite writes the id `exact new` gave the app. The app reads no data of
// its own: every source is a module's.
export const appId = '__APP_ID__';
export const grants = 'device.microphone purpose.microphone\nfs.read app:/data/takes\nfs.write app:/data/takes';

export const answer = (source: string) => {
  throw new Error(`no data source ${source}`);
};
