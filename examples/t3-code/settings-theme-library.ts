// The built-in themes' full role sets (MIT T3 Code 1e2ecbd975, LICENSE-T3: packages/shared/src/themePalettes.ts
// T3_CHAT_THEME, GROVE_THEME, OCEAN_THEME, EMBER_THEME, IRIS_THEME: `colors` for light and `variants.dark`), every
// THEME_COLOR_ROLES value converted from OKLCH to hex by settings-themes.ts toHex. The theme editor seeds a copy or a
// theme created from one of them with these (ThemeEditorPanel's sourceTheme colours), so its Advanced roles show the
// source's own colours (settings-appearance-and-skill-chip, desktop audit S1-5). The app's painting keeps its own
// palette table (settings-appearance.ts).

const ROLES = (
  'canvas chrome toolbar toolbarForeground toolbarBorder toolbarControl toolbarControlForeground toolbarControlHover surface surfaceRaised surfaceOverlay ' +
  'text textMuted border input focus accent accentForeground secondary secondaryForeground muted mutedForeground placeholder secondaryLabel iconMuted ' +
  'error errorForeground errorSurface warning warningForeground warningSurface update updateForeground updateSurface accentSurface ' +
  'accentSurfaceForeground messageSurface messageForeground messageAction messageActionForeground messageActionHover codeBackground codeForeground ' +
  'sidebar sidebarForeground sidebarMutedForeground sidebarControlSurface sidebarRowHover sidebarRowActive sidebarRowSelected sidebarBorder ' +
  'terminalBackground terminalForeground terminalCursor terminalSelection terminalScrollbar terminalScrollbarHover'
).split(' ');
const LIBRARY: Record<string, { light: string; dark: string }> = {
  't3-chat': {
    light: '#fdf7fd #fdf7fd #fdf7fd #501854 #efbdeb #f3e6f5 #501854 #eccfe3 #faf3fb #fdfafd #ffffff #501854 #ac1668 #eee1ed #e7c1dc #db2777 #db2777 #ffffff #f1c4e6 #77347c #eaa7cb #8d1255 #8b5f90 #ac1668 #ac1668 #f7086c #9d174d #fde4f1 ' +
      '#f59e0b #b05109 #fcf0ea #db2777 #ac1668 #fadfef #f3e6f5 #454554 #f7def2 #492c61 #db2777 #ffffff #c12269 #f5ecf9 #673c8b #f2e1f4 #454554 #ac1668 #f8f8f7 #f8f8f7 #f8f8f7 #f8f8f7 #eceae9 #fdf7fd #501854 #db2777 #f1c4e6 #e7c1dc #eaa7cb',
    dark: '#1f1a24 #1f1a24 #1f1a24 #f9f8fb #27242c #362d3d #d4c7e1 #463753 #29232d #2c2631 #100a0e #f9f8fb #e7d0dd #27242c #302029 #db2777 #a3004c #fbd0e8 #362d3d #d4c7e1 #423a45 #e7d0dd #968d9f #e7d0dd #d4c7e1 #9d174d #fbd0e8 #331a2b ' +
      '#f59e0b #fbbf24 #412f20 #a3004c #fbd0e8 #37152b #463753 #f8f1f5 #2b2431 #f2ebfa #a3004c #fbd0e8 #a2004c #1f1a24 #d8c3ef #171018 #f4f4f5 #e7d0dd #261922 #261922 #261922 #261922 #322028 #1f1a24 #f9f8fb #db2777 #362d3d #302029 #423a45',
  },
  'grove': {
    light: '#f3f7f4 #f3f7f4 #f3f7f4 #241523 #d5e6dd #e2ede7 #241523 #d5e6dd #f3f7f4 #ecefed #e7e9e8 #241523 #746c73 #cbd5d1 #becbc5 #1b7d50 #1b7d50 #fffaff #e2ede7 #241523 #e6f0ea #6e696f #716971 #746c73 #746c73 #fb2c36 #c10007 #f4e7e5 ' +
      '#fe9a00 #b64a00 #f4f0e1 #1b7d50 #125134 #d0e3da #d5e6dd #241523 #cce1d7 #241523 #8f6410 #fffaff #7e580e #eef1ef #241523 #e2ede7 #241523 #6b666c #d3dcd8 #cae0d5 #bad7c9 #b2d2c3 #cbd3d0 #f3f7f4 #241523 #1b7d50 #cce1d7 #c5c5c6 #b1afb1',
    dark: '#1b2821 #1b2821 #1b2821 #fffaff #36654c #2a4b39 #fffaff #325c46 #1b2821 #36413c #444d49 #fffaff #919595 #415f4f #4f725f #69d69a #69d69a #241523 #2a4b39 #fffaff #253e31 #9da5a2 #a9abab #919595 #919595 #fb414a #ff6668 #3f2c28 ' +
      '#fe9a00 #ffb900 #3f3a1c #69d69a #9ee4be #345f48 #325c46 #fffaff #37664d #fffaff #e3b34e #241523 #e6bc63 #28342e #fffaff #21362b #fffaff #9da3a2 #45554d #2a4938 #2f5641 #325c46 #6f7a75 #1b2821 #fffaff #69d69a #36654c #7b807e #989b9b',
  },
  'ocean': {
    light: '#f5f7f8 #f5f7f8 #f5f7f8 #241523 #d8e4ee #e4ecf2 #241523 #d8e4ee #f5f7f8 #edeff1 #e8e9eb #241523 #746c75 #cdd4dc #c0c9d4 #2672af #2672af #fffaff #e4ecf2 #241523 #e8eff4 #6f6873 #716972 #746c75 #746c75 #fb2c36 #c10007 #f5e6e9 ' +
      '#fe9a00 #b74b00 #f6efe4 #2672af #194a72 #d4e1ed #d8e4ee #241523 #d0dfeb #241523 #0a6f75 #fffaff #096267 #f0f1f3 #241523 #e4ecf2 #241523 #6c6570 #d5dbe2 #cdddea #bed4e5 #b7cfe2 #cdd2d9 #f5f7f8 #241523 #2672af #d0dfeb #c7c5c9 #b2aeb4',
    dark: '#17212b #17212b #17212b #fffaff #36566f #293f52 #fffaff #324e66 #17212b #333b45 #414851 #fffaff #8d8f97 #405567 #4f677b #70b9ee #70b9ee #241523 #293f52 #fffaff #233544 #969ca6 #a4a4ac #8d8f97 #8d8f97 #fb414a #ff6467 #3c2630 ' +
      '#fe9a00 #ffb900 #3c3424 #70b9ee #a2d2f4 #345269 #324e66 #fffaff #375871 #fffaff #5bd0d6 #241523 #6fd6db #252e38 #fffaff #1e2d3b #fffaff #989ca5 #424e5a #283e50 #2f495f #324f66 #6d757f #17212b #fffaff #70b9ee #36566f #797c84 #9798a0',
  },
  'ember': {
    light: '#f9f7f5 #f9f7f5 #f9f7f5 #241523 #eee0d9 #f3eae5 #241523 #eee0d9 #f9f7f5 #f1efee #ece9e9 #241523 #766c74 #ddd2ce #d4c6c1 #ae552a #ae552a #fffaff #f3eae5 #241523 #f4ede9 #74686f #736971 #766c74 #766c74 #fb2c36 #c10007 #f9e7e6 ' +
      '#fe9a00 #b84b00 #f9efe2 #ae552a #71381b #edddd5 #eee0d9 #241523 #ebdad1 #241523 #b23535 #fffaff #9d2f2f #f3f1f0 #241523 #f3eae5 #241523 #71646b #e2d9d6 #ead8cf #e5ccc0 #e2c6b8 #dad0ce #f9f7f5 #241523 #ae552a #ebdad1 #cac5c7 #b5afb2',
    dark: '#291e1a #291e1a #291e1a #fffaff #6e4934 #513728 #fffaff #644330 #291e1a #433835 #4f4543 #fffaff #968e8f #664c3f #7a5d4d #f09a64 #f09a64 #241523 #513728 #fffaff #432e23 #a59996 #aba3a5 #968e8f #968e8f #fb414a #ff6467 #4a2321 ' +
      '#fe9a00 #ffb900 #4b3215 #f09a64 #f5bd9a #684631 #644330 #fffaff #704b34 #fffaff #f78a7a #241523 #f8988a #362b27 #fffaff #39281f #fffaff #a49998 #584943 #4f3528 #5d3f2d #654330 #7e716e #291e1a #fffaff #f09a64 #6e4934 #837a7a #9f9798',
  },
  'iris': {
    light: '#f8f7f9 #f8f7f9 #f8f7f9 #241523 #e5e0f0 #edeaf4 #241523 #e5e0f0 #f8f7f9 #f0eff2 #ebe9ed #241523 #766c76 #d6d1de #ccc5d6 #7253b9 #7253b9 #fffaff #edeaf4 #241523 #f0edf6 #726874 #736973 #766c76 #766c76 #fb2c36 #c10007 #f8e6ea ' +
      '#fe9a00 #b84b00 #f8efe5 #7253b9 #4a3678 #e2ddef #e5e0f0 #241523 #e0d9ee #241523 #a82c87 #fffaff #942777 #f2f1f4 #241523 #edeaf4 #241523 #6f6471 #ddd9e3 #ded8ed #d4cce8 #cfc6e6 #d5d0db #f8f7f9 #241523 #7253b9 #e0d9ee #c9c5ca #b4aeb5',
    dark: '#1d1929 #1d1929 #1d1929 #fffaff #4a3c70 #362d51 #fffaff #433765 #1d1929 #383443 #454250 #fffaff #8e8a95 #4d4366 #5d527b #9d7df2 #9d7df2 #241523 #362d51 #fffaff #2d2643 #9690a1 #a29ea8 #8e8a95 #8e8a95 #fb414a #ff6467 #40202e ' +
      '#fe9a00 #ffb900 #412e23 #9d7df2 #bfabf7 #463969 #433765 #fffaff #4b3d72 #fffaff #f099d8 #241523 #f2a5dd #2a2736 #fffaff #272139 #fffaff #9792a0 #494459 #352c4f #3f345e #433766 #736d7e #1d1929 #fffaff #9d7df2 #4a3c70 #7c7883 #99959f',
  },
};

/** A built-in theme's colours for one appearance (null for T3 Code's stock look and any other id). */
export function builtInThemeColors(id: string, mode: 'light' | 'dark'): Record<string, string> | null {
  const entry = LIBRARY[id];
  if (!entry) return null;
  const values = entry[mode].split(' ');
  return Object.fromEntries(ROLES.map((role, index) => [role, values[index]!]));
}
