// Web build only (the web pair, ../web): the two native-only modules resolve to their web counterparts in
// ./web on the web platform; iOS resolves exactly as before. Everything else is Expo's default config.
const { getDefaultConfig } = require('expo/metro-config');
const path = require('path');
const config = getDefaultConfig(__dirname);
const WEB = {
  'react-native-maps': path.resolve(__dirname, 'web/maps.tsx'),
  'react-native-webview': path.resolve(__dirname, 'web/webview.tsx'),
  // Skia's web Platform turns a bundled image's asset id into a URL through React Native's asset registry;
  // on the web that registry is react-native-web's (Expo refuses react-native internals on web).
  'react-native/Libraries/Image/AssetRegistry': path.resolve(__dirname, 'node_modules/react-native-web/dist/modules/AssetRegistry/index.js'),
};
config.resolver.resolveRequest = (context, moduleName, platform) => {
  if (platform === 'web' && WEB[moduleName]) return { type: 'sourceFile', filePath: WEB[moduleName] };
  return context.resolveRequest(context, moduleName, platform);
};
module.exports = config;
