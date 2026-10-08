// Web entry (the web pair, ../web): Skia on the web needs CanvasKit loaded before any Skia import runs
// (LoadSkiaWeb, the documented way), and the bundled fonts need registering (the expo-font config plugin
// only embeds them natively). Then the same App as iOS.
import { LoadSkiaWeb } from '@shopify/react-native-skia/lib/module/web';
import * as Font from 'expo-font';
import { registerRootComponent } from 'expo';

const FONTS = {
  'AbrilFatface-Regular': require('./data/fonts/AbrilFatface-Regular.ttf'),
  'BebasNeue-Regular': require('./data/fonts/BebasNeue-Regular.ttf'),
  'CrimsonText-Italic': require('./data/fonts/CrimsonText-Italic.ttf'),
  'CrimsonText-Regular': require('./data/fonts/CrimsonText-Regular.ttf'),
  'DMSerifDisplay-Regular': require('./data/fonts/DMSerifDisplay-Regular.ttf'),
  'Inter-Regular': require('./data/fonts/Inter-Regular.ttf'),
  'Lobster-Regular': require('./data/fonts/Lobster-Regular.ttf'),
  'Pacifico-Regular': require('./data/fonts/Pacifico-Regular.ttf'),
  'PermanentMarker-Regular': require('./data/fonts/PermanentMarker-Regular.ttf'),
  'SpaceMono-Bold': require('./data/fonts/SpaceMono-Bold.ttf'),
  'SpaceMono-Regular': require('./data/fonts/SpaceMono-Regular.ttf'),
  'SpecialElite-Regular': require('./data/fonts/SpecialElite-Regular.ttf'),
};

Promise.all([LoadSkiaWeb({ locateFile: (f: string) => `/${f}` }), Font.loadAsync(FONTS)]).then(() => {
  registerRootComponent(require('./App').default);
});
