// T3 Code (MIT), pinned365aa87982 mobileTheme.ts and nativeComposerTheme.ts helper bodies.
// See LICENSE-T3. Native chip drawing requires opaque colors over its actual surface.
// @ref llp/1109.005-composer-and-transcript.decision.md#composer-command-foundation
function rgbChannels(color: string): readonly [number, number, number] | null {
  const match = /^#([\da-f]{2})([\da-f]{2})([\da-f]{2})(?:[\da-f]{2})?$/i.exec(color);
  if (!match) return null;
  const [, red = "0", green = "0", blue = "0"] = match;
  return [Number.parseInt(red, 16), Number.parseInt(green, 16), Number.parseInt(blue, 16)];
}

/**
 * An opaque form of a theme colour, composited over the surface behind it. Native chip drawing
 * parses only opaque hex — an `rgba()` string falls back to a default that is nothing like the
 * colour asked for — so a translucent role like `--color-border` has to be flattened first.
 */
export function flattenThemeColor(color: string, surface: string): string {
  const alphaHex = /^#([\da-f]{6})([\da-f]{2})$/i.exec(color);
  if (alphaHex) {
    const channels = rgbChannels(color)!;
    return flattenThemeColor(
      `rgba(${channels[0]}, ${channels[1]}, ${channels[2]}, ${Number.parseInt(alphaHex[2]!, 16) / 255})`,
      surface,
    );
  }
  const match = /^rgba?\(\s*(\d+)[,\s]+(\d+)[,\s]+(\d+)(?:[,\s/]+([\d.]+))?\s*\)$/i.exec(
    color.trim(),
  );
  if (!match) return color;
  const alpha = match[4] === undefined ? 1 : Number(match[4]);
  const behind = rgbChannels(surface) ?? [0, 0, 0];
  const channels = [match[1], match[2], match[3]].map((channel, index) =>
    Math.max(0, Math.min(255, Math.round(Number(channel) * alpha + behind[index]! * (1 - alpha)))),
  );
  return `#${channels.map((channel) => channel.toString(16).padStart(2, "0")).join("")}`;
}

/** Native chip parsers need opaque hex instead of CSS rgba or platform-specific alpha order. */
export function createNativeComposerTheme(theme: Record<string, string>) {
  const surface = flattenThemeColor(theme["--color-composer-surface"], theme["--color-screen"]);
  const chipBackground = flattenThemeColor(theme["--color-subtle"], surface);
  const skillBackground = flattenThemeColor(theme["--color-inline-skill-background"], surface);
  return {
    text: flattenThemeColor(theme["--color-foreground"], surface),
    placeholder: flattenThemeColor(theme["--color-placeholder"], surface),
    chipBackground,
    chipBorder: flattenThemeColor(theme["--color-border"], chipBackground),
    chipText: flattenThemeColor(theme["--color-foreground"], chipBackground),
    skillBackground,
    skillBorder: flattenThemeColor(theme["--color-inline-skill-border"], skillBackground),
    skillText: flattenThemeColor(theme["--color-inline-skill-foreground"], skillBackground),
    fileTint: flattenThemeColor(theme["--color-icon-muted"], chipBackground),
  };
}
