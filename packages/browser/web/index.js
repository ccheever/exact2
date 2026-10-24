// @exact/browser on the web (LLP 1020.002 D4): a page cannot clear another
// origin's data, and a partition is not honoured here (`exactPage().iframePartition`
// is false), so `clearSiteData` is NotSupportedError.
export function implementation() {
  return {
    async clearSiteData() {
      return { error: 'NotSupportedError' };
    },
    release() {},
  };
}
