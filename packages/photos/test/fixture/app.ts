// The suite writes the id `exact new` gave the app. The list is read only
// once the suite asks, since the first read while `prompt` asks (D7).
import { photoAssets } from '@exact/photos';

export const appId = '__APP_ID__';
export const grants = 'device.photos purpose.read\ndevice.photos-add purpose.add\nfs.read app:/tmp\nfs.write app:/tmp';

const sources: Record<string, (args: any) => Promise<unknown>> = {
  assets: async ([read]: [boolean]) => (read ? await photoAssets({ limit: 24 }) : { assets: [] }),
};

export const answer = ((source: string, args: unknown) => sources[source](args)) as any;
