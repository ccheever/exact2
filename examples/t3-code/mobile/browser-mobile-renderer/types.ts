// Structural type-only counterparts of pinned contracts; one exact runtime constant.
export const PREVIEW_STREAM_HOST_SETUP_CLOSE_CODE = 4503;
export type DevicePlatform = 'ios' | 'android';
export type PreviewStreamHostSetup = { readonly need: 'sandbox' | 'libraries'; readonly command: string };
export type PreviewViewportSetting = { readonly _tag: 'fill' } | { readonly _tag: 'freeform'; readonly width: number; readonly height: number } | { readonly _tag: 'preset'; readonly presetId: string; readonly width: number; readonly height: number };
