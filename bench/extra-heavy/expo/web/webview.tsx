// react-native-webview has no web implementation. The web's own way to show an HTML string without a base
// URL is an <iframe srcdoc>; scrolling="no" is `scrollEnabled={false}`. (exact2's web build shows the same
// page in an <iframe src=…>: a bundled file per row.)
import { createElement } from 'react';

export function WebView({ source, style }: { source: { html: string }; style?: any; [k: string]: any }) {
  return createElement('iframe', {
    srcDoc: source.html,
    scrolling: 'no',
    style: { border: 0, width: '100%', height: '100%', display: 'block', background: 'transparent' },
  });
}
export default WebView;
