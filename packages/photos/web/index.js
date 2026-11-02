// @exact/photos on the web: absent. The web has no photo
// library API; a page reaches the person's photos only through the file
// input the picker already is, and saves through a download
// or a share. So the status says the library is not available here and
// every other name is `NotSupportedError`. The page runtime
// (host/web-js/modules.js) calls this outside the agent; under the agent the
// declared substitute answers the same (`web`, `unsupported`).

const STATUS = { available: false, state: 'unknown', access: 'none', addState: 'unknown', apple: null, android: null };
const absent = async () => ({ error: 'NotSupportedError' });

export function implementation() {
  return {
    async photoLibraryStatus() { return STATUS; },
    photoAssets: absent,
    copyPhotoAsset: absent,
    addToPhotoLibrary: absent,
    managePhotoSelection: absent,
    // A picture of this scheme is never drawn here.
    image: absent,
  };
}
