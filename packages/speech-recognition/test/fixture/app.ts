// The suite writes the id `exact new` gave the app. The app reads no data of
// its own: every source is a module's.
export const appId = '__APP_ID__';
export const grants = 'device.microphone purpose.microphone\ndevice.speech-recognition purpose.recognition\nfs.read app:/data\nfs.write app:/data';

export const answer = (source: string) => {
  throw new Error(`no data source ${source}`);
};
