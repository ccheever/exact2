// The icon tables gen-icons.mjs writes, as lists of names. An entry is a Lucide icon's name (drawn
// under that name), [name, Lucide name] (another icon drawn under this name), or own(name): geometry
// no lucide@0.564.0 icon has, kept as the clone's tables drew it (DRAWN). Order is the lookup's.

/** Geometry with no lucide@0.564.0 icon of the same shape, as the clone's tables drew it, by name. */
const DRAWN = {
  'message-circle': 'M7.9 20A9 9 0 1 0 4 16.1L2 22Z',
  bolt: 'M13 2l-9 12h7l-1 8 10-12h-7l1-8Z',
  branch: 'M4 5a2 2 0 1 0 4 0a2 2 0 1 0 -4 0M16 6a2 2 0 1 0 4 0a2 2 0 1 0 -4 0M4 19a2 2 0 1 0 4 0a2 2 0 1 0 -4 0M6 7v10m0-4c8 0 12-1 12-5',
  layers: 'M12 2l10 6-10 6L2 8l10-6Zm-10 11 10 6 10-6M2 18l10 6 10-6',
  plan: 'M9 6h12M9 12h12M9 18h12M3 6h.01M3 12h.01M3 18h.01',
  compacted: 'M3 9h6V3m12 12h-6v6M2 2l7 7m13 13-7-7',
  star: 'M11.48 2.35a.56.56 0 0 1 1.04 0l2.49 5.06a2 2 0 0 0 1.51 1.1l5.58.82a.56.56 0 0 1 .31.96l-4.04 3.94a2 2 0 0 0-.58 1.77l.95 5.57a.56.56 0 0 1-.81.59l-5-2.63a2 2 0 0 0-1.86 0l-5 2.63a.56.56 0 0 1-.81-.59l.95-5.57a2 2 0 0 0-.58-1.77L2.59 10.3a.56.56 0 0 1 .31-.96l5.58-.82a2 2 0 0 0 1.51-1.1Z',
  'file-modified': 'M4 7v10m0-10 4 6 4-6v10m7-10v10m-3-3 3 3 3-3',
  provider: 'M12 2l8.66 5v10L12 22l-8.66-5V7L12 2Zm0 0v10m8.66-5L12 12m0 10V12m-8.66 5L12 12m8.66 5L12 12M3.34 7 12 12',
  cursor: 'M12 2.5l8.5 4.9v9.2L12 21.5l-8.5-4.9V7.4ZM3.5 7.4 12 12l8.5-4.6M12 12v9.5',
  'folder-git-2': 'M9 20H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h3.9a2 2 0 0 1 1.69.9l.81 1.2a2 2 0 0 0 1.67.9H20a2 2 0 0 1 2 2v5M11 12a2 2 0 1 0 4 0a2 2 0 1 0 -4 0M18 19c-2.8 0-5-2.2-5-5v8M18 19a2 2 0 1 0 4 0a2 2 0 1 0 -4 0',
  'git-branch': 'M6 3L6 15M15 6a3 3 0 1 0 6 0a3 3 0 1 0 -6 0M3 18a3 3 0 1 0 6 0a3 3 0 1 0 -6 0M18 9a9 9 0 0 1-9 9',
  'mac-mini': 'M4 8h16a2 2 0 0 1 2 2v4a2 2 0 0 1 -2 2h-16a2 2 0 0 1 -2 -2v-4a2 2 0 0 1 2 -2ZM6 12h.01',
  'mac-studio': 'M5 5h14a2 2 0 0 1 2 2v10a2 2 0 0 1 -2 2h-14a2 2 0 0 1 -2 -2v-10a2 2 0 0 1 2 -2ZM7 15h.01M11 15h.01M15 15h.01',
  // Its circle (an arc from (9, 9) to 0.01 above it) is centred on (11, 9); Lucide's image centres it on (9, 9).
  image: 'M5 3h14a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2ZM9 9a2 2 0 1 0 0-.01M21 15l-3.086-3.086a2 2 0 0 0-2.828 0L6 21',
  trash: 'M3 6h18M19 6v14c0 1-1 2-2 2H7c-1 0-2-1-2-2V6M8 6V4c0-1 1-2 2-2h4c1 0 2 1 2 2v2M10 11v6M14 11v6',
};
const own = (name, drawn = name) => [name, { d: DRAWN[drawn] }];

/** icons.contract Icon: the glyphs most of the client draws. */
export const ICON = [
  'text-wrap', own('message-circle'), ['sidebar', 'panel-left-close'], 'sidebar-open', 'panel-left',
  'circle-alert', ['changes', 'panel-right'], 'panel-bottom', 'square-menu', 'search', ['fork', 'git-fork'],
  'copy', ['undo', 'undo-2'], 'info', 'x', 'folder', 'folder-plus', ['compose', 'square-pen'],
  ['chevron', 'chevron-down'], 'chevron-up', ['right', 'chevron-right'], ['up', 'arrow-up'], 'lock', 'lock-open',
  own('bolt'), ['check', 'circle-check'], ['settle', 'check'], 'clock', ['refresh', 'refresh-cw'], 'settings',
  ['pull-requests', 'git-pull-request-arrow'], ['usage', 'chart-no-axes-column'], own('branch'), own('layers'),
  'terminal', own('plan'), own('compacted'), 'brain', own('star'), own('star-filled', 'star'), 'file-diff',
  own('file-modified'), own('provider'),
];
/** Icon's glyphs drawn filled, and its one stroke width other than 2. */
export const ICON_FILLED = ['star-filled', 'bolt'];
export const ICON_THIN = 'bolt';

/** shell-icons.contract ShellIcon: toasts, panel controls, the surface chooser, the details panel and
 *  the title menu. */
export const SHELL_ICON = [
  'panel-right', 'panel-bottom', 'square-menu', ['maximize', 'maximize-2'], ['minimize', 'minimize-2'], 'earth',
  'square-terminal', 'files', 'file-diff', 'git-pull-request-arrow', 'link-2',
  ['pull-request', 'git-pull-request'], 'link', 'smartphone', 'circle-check', 'circle-alert', 'info',
  'triangle-alert', 'shield-question', 'message-circle-question', 'download', ['loader', 'loader-circle'], 'x',
  'copy', 'check', 'chevron-down', 'chevron-up', 'chevron-right', 'folder', 'git-branch', 'git-commit',
  'git-merge', 'plus', 'corner-up-left', 'corner-down-right', 'pin', 'pin-off', 'archive', 'pencil',
  ['trash', 'trash-2'], 'sparkles', 'mail', 'settings', 'clock', ['settle', 'check'], 'external-link',
  ['refresh', 'refresh-cw'], 'corner-left-up', 'git-fork', 'arrow-right', 'hash', 'mail-open', 'timer',
  'message-square-plus', own('cursor'), 'folder-git', 'folder-git-2', 'cloud-upload', 'cloud-download', 'bot',
  'eye', 'eye-off',
];

/** sidebar-icons.contract SidebarIcon: the sidebar's own glyphs. */
export const SIDEBAR_ICON = [
  'pin', 'pin-off', 'alarm-clock', 'alarm-clock-off', 'circle-dashed', 'circle-dot-dashed', 'message-circle-question',
  'shield-question', 'circle-alert', 'circle-check', 'square-pen', 'plus', 'minus', 'check', 'undo-2', 'clock',
  'x', 'folder', 'settings', 'search', 'chevron-down', 'chevron-left', 'chevron-right', 'chevron-up',
  own('folder-git-2'), own('git-branch'), own('layers'), 'calendar', 'laptop', 'monitor', 'server', 'cloud',
  own('mac-mini'), own('mac-studio'), 'messages-square', 'arrow-right-left', ['loader', 'loader-circle'],
  'download', 'triangle-alert', 'list-filter',
];

/** timeline-icons.contract timelineIconGeometry: the timeline's glyphs (TimelineIcon draws a name it lacks
 *  with Icon). */
export const TIMELINE_ICON = [
  'messages-square', ['skill', 'box'], 'film', 'square-terminal', 'mouse-pointer-click', own('image'),
  'circle-dashed', own('message-circle'), 'hammer', 'eye', 'square-pen', 'globe', 'wrench', 'bot', 'zap',
  ['minimize', 'minimize-2'], ['maximize', 'maximize-2'], 'arrow-right-left', 'chevrons-up-down',
  'chevrons-down-up', 'folder-open', 'folder-closed', ['check-mark', 'check'], ['redo', 'redo-2'], 'ellipsis',
  'download', 'quote', 'message-square', 'triangle-alert', ['list-collapse', 'list-chevrons-down-up'],
  ['device', 'smartphone'], 'loader-circle', 'minus', 'circle', 'chevron-down', 'chevron-right', 'code',
  'workflow', 'external-link', 'alarm-clock', 'circle-check',
];

/** panel-icons.contract PanelIcon: the right panel's surfaces. */
export const PANEL_ICON = [
  'earth', 'chevrons-up-down', 'chevrons-down-up', 'wrap-text', 'folder-tree', ['code', 'code-xml'],
  ['table', 'table-2'], 'eye', 'eye-off', 'ellipsis', 'arrow-up-right', 'link', 'link-2', 'unlink-2', 'plus',
  'refresh-cw', 'loader', 'smartphone', 'check', 'circle-alert', 'layers', 'x', 'arrow-left',
];

/** provider-icons.contract PvIcon: the Providers and Connections settings routes. */
export const PROVIDER_ICON = [
  'plus', 'minus', ['refresh', 'refresh-cw'], ['trash', 'trash-2'], ['undo', 'undo-2'],
  ['warning', 'triangle-alert'], ['update', 'circle-arrow-up'], 'x', 'lock', 'lock-open', 'star',
  ['star-filled', 'star'], 'pencil', ['external', 'external-link'], 'check', 'search', 'info',
  ['link', 'chevrons-left-right-ellipsis'], 'terminal', 'ellipsis', 'laptop', 'server', 'download',
  ['chevron', 'chevron-down'], 'arrow-up', 'arrow-down', 'copy',
];
/** PvIcon's one glyph drawn filled. */
export const PROVIDER_FILLED = 'star-filled';

/** settings-icons.contract: one component per icon (Lucide<Name>), each with Lucide's own elements, so a
 *  use inlines only its own geometry. An entry with markup is drawn as written. */
export const SETTINGS_ICONS = [
  'activity', 'archive', 'archive-x', 'arrow-left', 'battery', 'blocks', 'bot', 'brain', 'calendar-clock', 'check',
  'chevron-down', 'chevron-right', 'chevron-up', 'copy', 'cpu', 'database', 'download', 'ellipsis-vertical',
  'external-link', 'file-braces', 'folder-open', 'gauge', 'git-branch', 'hard-drive', 'info', 'keyboard', 'layers',
  'link-2', 'lock-open', 'memory-stick', 'minus', 'palette', 'panel-left-close', 'pencil',
  ['play', '<polygon points="6 3 20 12 6 21 6 3"/>'], 'plus', 'refresh-cw', 'rotate-ccw', 'search', 'settings-2',
  ['snap-shot', '<path d="M8 3H6a3 3 0 0 0-3 3v2M16 3h2a3 3 0 0 1 3 3v2M21 16v2a3 3 0 0 1-3 3h-2M8 21H6a3 3 0 0 1-3-3v-2"/><rect width=10 height=8 x=7 y=8 rx=2/><circle cx=12 cy=12 r=1.5/>'],
  'trash-2', 'triangle-alert', 'undo-2', 'x', 'ellipsis', 'circle-x', 'laptop',
];

/** diff-glyphs.contract DiffGlyph's 24-grid glyphs: the panel's controls, DiffFileStatus (RotateCw, Info),
 *  the gutter's "+", DiffCommentAnnotation (MessageCircle, Trash2) and Cite (Quote). A glyph's rects keep
 *  their own elements and its other elements join into one path, but for those `apart` names (by index). */
export const DIFF_GLYPHS = [
  ['file-diff', 'file-diff', { apart: [0] }], ['stacked', 'rows-3'], ['split', 'columns-2'], 'panel-right',
  ['fold', 'chevrons-down-up'], ['unfold', 'chevrons-up-down'], ['wrap', 'text-wrap'], 'pilcrow',
  ['tree', 'folder-tree', { apart: [0, 1] }], 'copy', ['refresh', 'refresh-cw'], 'rotate-cw', 'info', 'plus', own('trash'),
  own('message-circle'), 'quote',
];
