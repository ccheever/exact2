// Stage 13 C's restyle cases (LLP 1139 §5, "One app value restyles the
// library"; theme2's 10:28 and 10:42 rulings): every package token and
// param stage 13 C added, set in apps/library-gate's design a component at a
// time and then removed, each instance read exactly. A read is the row's
// computed value on the node the build publishes it on (`layout <id>`, the
// runner's `node`: kind 1, an exact read of the resolved value), checked
// against its own token's value, a derived token's against the value its
// inputs compute; removal brings each read back to its base value.
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';
import { join, resolve } from 'node:path';

const ROOT = resolve(new URL('..', import.meta.url).pathname);

/** The packages whose designs stage 13 C grew. */
export const PACKAGES = ['lists', 'cards', 'actions'];

/** The packages' tokens before stage 13 C (21a1a836b3^): stage 12's, which
 * its own three cases already restyle; every other token is stage 13 C's. */
export const STAGE12_TOKENS = new Set(`
  cards.at-avatar cards.at-name-line cards.avatar cards.avatar-corner cards.avatar-default cards.avatar-end cards.avatar-end-thread
  cards.avatar-ground cards.avatar-ring cards.avatar-start cards.badge cards.badge-hang cards.badge-mark cards.badge-size cards.column-gap
  cards.context-gap cards.context-hang cards.context-icon cards.context-under cards.divider cards.hairline cards.lead cards.line cards.line-gap
  cards.line-thread cards.line-width cards.link-corner cards.link-frame cards.link-gap cards.link-ground cards.link-name-gap cards.link-pad
  cards.link-row-gap cards.link-top cards.meta cards.meta-gap cards.meta-under cards.name cards.nudge cards.pad-bottom cards.pad-end
  cards.pad-end-thread cards.pad-start cards.pad-start-thread cards.reply-icon cards.reply-rise cards.small-line cards.small-size cards.surface
  cards.text-line cards.text-size cards.thumb-ground cards.tiny-line cards.tiny-size cards.title-line cards.title-size cards.top cards.top-thread
  cards.visit cards.visit-corner cards.visit-icon cards.visit-ink cards.visit-pad-block cards.visit-pad-inline
  actions.count-line actions.count-size actions.count-size-big actions.icon actions.icon-big actions.post-corner actions.post-dim
  actions.post-fill actions.post-height actions.post-ink actions.post-pad actions.post-pressed actions.row-corner actions.row-count
  actions.row-dim actions.row-gap actions.row-hit actions.row-hover actions.row-icon-gap actions.row-lead actions.row-lead-big
  actions.row-liked actions.row-max actions.row-reposted actions.row-saved actions.row-tint actions.row-top actions.row-trail
  actions.row-trail-big`.trim().split(/\s+/));

/** library-gate's FollowButtons by what they hold: a small pill's icon box
 * (0, its image 0/0) and label (1); a small pill with no icon, its label
 * (0); a tiny pill's; a small round one's icon box only; a tiny round
 * one's. */
const FB = {
  pill: ['fb-base', 'fb-following', 'fb-inverted', 'fb-subtle', 'fb-overlay', 'sample-fb-icon-follow', 'sample-fb-inverted-following', 'sample-fb-subtle-following', 'sample-fb-overlay-following', 'sample-fb-icon-following-following'],
  bare: ['sample-fb-icon-following', 'sample-fb-icon-none', 'sample-fb-icon-follow-following'],
  tiny: ['fb-tiny', 'sample-fb-tiny-following'],
  round: ['fb-round', 'sample-fb-round-following'],
  roundTiny: ['sample-fb-round-tiny'],
};

/** Stage 13 C's components: each one's source, its instances in
 * library-gate (by id), and the testId of an instance's first node. */
export const COMPONENTS = {
  ListRow: { source: '@exact/lists/list-row.contract', root: id => id, ids: ['lr-base', 'sample-lr-value', 'sample-lr-check', 'sample-lr-check-off', 'sample-lr-check-symbol', 'sample-lr-none', 'sample-lr-destructive'] },
  SwitchRow: { source: '@exact/lists/switch-row.contract', root: id => `${id}-row`, ids: ['sr-base'] },
  ProfileHeader: { source: '@exact/cards/profile-header.contract', root: id => `profile-header-${id}`, ids: ['ph-base', 'ph-blocked', 'ph-compact', 'sample-ph-verified', 'sample-ph-verifier', 'sample-ph-follows-you', 'sample-ph-alerts', 'sample-ph-note', 'sample-ph-avatar'] },
  ComposeFrame: { source: '@exact/actions/compose-frame.contract', root: id => `compose-bar-${id}`, ids: ['cf-base', 'sample-cf-drafts', 'sample-cf-reply', 'cf-gallery', 'cf-camera', 'cf-gif', 'sample-cf-near', 'sample-cf-over'] },
  BarItem: { source: '@exact/actions/bar-item.contract', root: id => id, ids: ['bi-base', 'sample-bi-selected', 'sample-bi-badge', 'sample-bi-dot', 'sample-bi-avatar'] },
  HeaderButton: { source: '@exact/actions/header-button.contract', root: id => id, ids: ['hb-base', 'hb-small', 'hb-lead'] },
  // Exported in stage 15 B (LLP 1121): no id prop, so each instance is the
  // box library-gate holds it in, and a read's path starts at that box.
  Avatar: { source: '@exact/cards/common.contract', root: id => id, ids: ['av-base', 'sample-av-src'] },
  VerifiedBadge: { source: '@exact/cards/common.contract', root: id => id, ids: ['vb-base', 'sample-vb-verifier'] },
  // Stage 15 B batch 2 (2a): its button is its testId.
  FollowButton: { source: '@exact/actions/follow-button.contract', root: id => id, ids: [...FB.pill, ...FB.bare, ...FB.tiny, ...FB.round, ...FB.roundTiny] },
  // Stage 15 B batch 2 (2b): its row is its testId.
  PersonRow: { source: '@exact/cards/person-row.contract', root: id => id, ids: ['pr-base', 'pr-top', 'pr-none', 'pr-disabled', 'sample-pr-avatar', 'sample-pr-verified', 'sample-pr-verifier', 'sample-pr-pill', 'sample-pr-description', 'sample-pr-description-one', 'sample-pr-all'] },
  // Stage 15 B batch 2's: EmptyState; MenuItem from its `<id>-item`
  // column (the hairline, then its button); MenuGroup, with no id prop,
  // from the box library-gate holds it in.
  EmptyState: { source: '@exact/cards/empty-state.contract', root: id => id, ids: ['es-base', 'sample-es-message', 'sample-es-button'] },
  MenuItem: { source: '@exact/lists/menu.contract', root: id => `${id}-item`, ids: ['sample-mi-first', 'mi-base', 'mi-destructive'] },
  MenuGroup: { source: '@exact/lists/menu.contract', root: id => id, ids: ['mg-base'] },
};

/** library-gate's instances that draw nothing: VerifiedBadge's "none" (no
 * badge), its box empty on every host. */
export const EMPTY = ['sample-vb-none'];

/** The tokens stage 15 B's exported Avatar and VerifiedBadge draw with:
 * stage 12's, read on their own library-gate cells. */
export const EXPORTED_TOKENS = ['cards.avatar', 'cards.avatar-corner', 'cards.hairline', 'cards.avatar-ground', 'cards.avatar-default', 'cards.avatar-ring', 'cards.badge-size', 'cards.badge', 'cards.badge-mark'];

/** The cases' batches (theme2 10:28): each component's tokens by prefix,
 * and the components its params come from. */
export const BATCHES = [
  { name: 'lists', prefix: /^lists\.(?!menu-)/, components: ['ListRow', 'SwitchRow'] },
  { name: 'ProfileHeader', prefix: /^cards\.(?!person-|empty-)/, components: ['ProfileHeader'] },
  { name: 'ComposeFrame', prefix: /^actions\.compose-/, components: ['ComposeFrame'] },
  { name: 'BarItem', prefix: /^actions\.bar-/, components: ['BarItem'] },
  { name: 'HeaderButton', prefix: /^actions\.header-/, components: ['HeaderButton'] },
  // Stage 15 B's exports (LLP 1121): every token Avatar and VerifiedBadge
  // draw with, stage 12's, by name (the sizes and the badge's ink and mark
  // as library-gate passes them, a consumer's token names), so setting one
  // by its full name is shown to restyle the exported use.
  { name: 'Avatar and VerifiedBadge', names: EXPORTED_TOKENS, components: ['Avatar', 'VerifiedBadge'] },
  // Stage 15 B batch 2 (2a): FollowButton's own tokens, new, and its params.
  { name: 'FollowButton', prefix: /^actions\.follow-/, components: ['FollowButton'] },
  // Stage 15 B batch 2 (2b): PersonRow's own tokens, new, and its params.
  { name: 'PersonRow', prefix: /^cards\.person-/, components: ['PersonRow'] },
  // Stage 15 B batch 2's (LLP 1121): EmptyState's tokens and the menu's.
  { name: 'EmptyState', prefix: /^cards\.empty-/, components: ['EmptyState'] },
  { name: 'Menu', prefix: /^lists\.menu-/, components: ['MenuItem', 'MenuGroup'] },
];

/** A token whose value is an expression of others: read, never set; its
 * expected value computed from its inputs' set values. */
export const DERIVED = {
  'cards.avatar-top': { from: ['cards.banner-height', 'cards.avatar-overlap'], value: ([h, o]) => h - o },
  'cards.avatar-top-compact': { from: ['cards.banner-compact', 'cards.avatar-overlap'], value: ([h, o]) => h - o },
  'cards.profile-avatar-box': { from: ['cards.profile-avatar', 'cards.profile-avatar-ring'], value: ([a, r]) => a + 2 * r },
};

/** Every read, by token (a package token's full name) or param
 * (`Component.param`): `<path> <rows> <ids>`, the path from the instance's
 * first node (`n` its nth child, `>n` its nth next sibling, `.` itself),
 * the rows read there, and the instances (`*` every one). Taken from
 * library-gate's nodes whose rows name the token and checked by hand
 * against each component's source; a use moved to another token fails its
 * read, and library-gate.test fails a token or param with none. A symbol
 * image is read by the colour it draws in, its `text_color`: a symbol draws
 * in its `color` on every host (docs/agent-pitfalls.md), its `tint_color`
 * answered `currentcolor` (LLP 1116.019 D6), so a symbol's tint moved to
 * `color` keeps its read. */
export const READS = {
  // Stage 15 B batch 2's PersonRow, from its row (.): its dividers there;
  // the link (0: its padding, gap and a disabled row's opacity) holding the
  // header (0/0: the avatar's box 0/0/0, Avatar's column 0/0/0/0 and picture
  // 0/0/0/0/0; the names 0/0/1: the name 0/0/1/0/0, the badge's column
  // 0/0/1/0/1 and VerifiedBadge 0/0/1/0/1/0 with its ground /0 and check /1;
  // the handle 0/0/1/1), the pill's row (0/1, its text 0/1/0) and the
  // description's column (0/1, or 0/2 after a pill; its text /0); the slot's
  // column (1: its padding; its box 1/0, the avatar's height).
  'cards.person-pad-block': ['0 padding_bottom,padding_top *', '1 padding_top *'],
  'cards.person-pad-inline': ['0 padding_left *', '1 padding_right *'],
  'cards.person-gap': ['0 column_gap,row_gap *'],
  'cards.person-header-gap': ['0 padding_right *', '0/0 column_gap,row_gap *'],
  'cards.person-avatar': ['0/0/0/0 height,width *', '0/0/0/0/0 height,width *', '1/0 height *'],
  'cards.person-name-size': ['0/0/1/0/0 font_size *'],
  'cards.person-name-line': ['0/0/1/0/0 line_height *'],
  'cards.person-badge': ['0/0/1/0/1/0 height,width sample-pr-verified,sample-pr-verifier,sample-pr-all', '0/0/1/0/1/0/0 height,width sample-pr-verified,sample-pr-verifier,sample-pr-all', '0/0/1/0/1/0/1 height,width sample-pr-verified,sample-pr-verifier,sample-pr-all'],
  'cards.person-badge-gap': ['0/0/1/0/1 padding_left sample-pr-verified,sample-pr-verifier,sample-pr-all'],
  'cards.person-handle-size': ['0/0/1/1 font_size *'],
  'cards.person-handle-line': ['0/0/1/1 line_height *'],
  'cards.person-pill-top': ['0/1 padding_top sample-pr-pill,sample-pr-all'],
  'cards.person-pill-size': ['0/1/0 font_size sample-pr-pill,sample-pr-all'],
  'cards.person-pill-line': ['0/1/0 line_height sample-pr-pill,sample-pr-all'],
  'cards.person-pill-pad-block': ['0/1/0 padding_bottom,padding_top sample-pr-pill,sample-pr-all'],
  'cards.person-pill-pad-inline': ['0/1/0 padding_left,padding_right sample-pr-pill,sample-pr-all'],
  'cards.person-pill-corner': ['0/1/0 border_radius_bottom_left,border_radius_bottom_right,border_radius_top_left,border_radius_top_right sample-pr-pill,sample-pr-all'],
  'cards.person-description-top': ['0/1 padding_top sample-pr-description,sample-pr-description-one', '0/2 padding_top sample-pr-all'],
  'cards.person-description-size': ['0/1/0 font_size sample-pr-description,sample-pr-description-one', '0/2/0 font_size sample-pr-all'],
  'cards.person-description-line': ['0/1/0 line_height sample-pr-description,sample-pr-description-one', '0/2/0 line_height sample-pr-all'],
  'cards.person-dim': ['0 opacity pr-disabled'],
  'PersonRow.divider': ['. border_color_bottom,border_color_top *'],
  'PersonRow.meta': ['0/0/1/1 text_color *'],
  'PersonRow.pill': ['0/1/0 background_color sample-pr-pill,sample-pr-all'],
  'PersonRow.badge': ['0/0/1/0/1/0/0 text_color sample-pr-verified,sample-pr-verifier,sample-pr-all'],
  'PersonRow.badgeMark': ['0/0/1/0/1/0/1 text_color sample-pr-verified,sample-pr-verifier,sample-pr-all'],
  // Stage 15 B batch 2's FollowButton, from its button (.): its shape,
  // padding, gap and press scale there; its icon's box (0) and image (0/0,
  // read by the colour it draws in); its label (1, or 0 with no icon). Its
  // colours are its params' reads, each tone's cell and each tone following.
  'actions.follow-corner': ['. border_radius_bottom_left,border_radius_bottom_right,border_radius_top_left,border_radius_top_right *'],
  'actions.follow-pad-block': ['. padding_bottom,padding_top fb-base,fb-following,fb-inverted,fb-subtle,fb-overlay,sample-fb-icon-follow,sample-fb-inverted-following,sample-fb-subtle-following,sample-fb-overlay-following,sample-fb-icon-following-following,sample-fb-icon-following,sample-fb-icon-none,sample-fb-icon-follow-following'],
  'actions.follow-pad-inline': ['. padding_left,padding_right fb-base,fb-following,fb-inverted,fb-subtle,fb-overlay,sample-fb-icon-follow,sample-fb-inverted-following,sample-fb-subtle-following,sample-fb-overlay-following,sample-fb-icon-following-following,sample-fb-icon-following,sample-fb-icon-none,sample-fb-icon-follow-following'],
  'actions.follow-gap': ['. column_gap,row_gap fb-base,fb-following,fb-inverted,fb-subtle,fb-overlay,sample-fb-icon-follow,sample-fb-inverted-following,sample-fb-subtle-following,sample-fb-overlay-following,sample-fb-icon-following-following,sample-fb-icon-following,sample-fb-icon-none,sample-fb-icon-follow-following,fb-round,sample-fb-round-following'],
  'actions.follow-size': ['1 font_size fb-base,fb-following,fb-inverted,fb-subtle,fb-overlay,sample-fb-icon-follow,sample-fb-inverted-following,sample-fb-subtle-following,sample-fb-overlay-following,sample-fb-icon-following-following', '0 font_size sample-fb-icon-following,sample-fb-icon-none,sample-fb-icon-follow-following'],
  'actions.follow-line': ['1 line_height fb-base,fb-following,fb-inverted,fb-subtle,fb-overlay,sample-fb-icon-follow,sample-fb-inverted-following,sample-fb-subtle-following,sample-fb-overlay-following,sample-fb-icon-following-following', '0 line_height sample-fb-icon-following,sample-fb-icon-none,sample-fb-icon-follow-following'],
  'actions.follow-icon': ['0/0 height,width fb-base,fb-following,fb-inverted,fb-subtle,fb-overlay,sample-fb-icon-follow,sample-fb-inverted-following,sample-fb-subtle-following,sample-fb-overlay-following,sample-fb-icon-following-following,fb-round,sample-fb-round-following'],
  'actions.follow-icon-box': ['0 height,width fb-base,fb-following,fb-inverted,fb-subtle,fb-overlay,sample-fb-icon-follow,sample-fb-inverted-following,sample-fb-subtle-following,sample-fb-overlay-following,sample-fb-icon-following-following,fb-round,sample-fb-round-following'],
  'actions.follow-icon-pull': ['0 margin_left,margin_right fb-base,fb-following,fb-inverted,fb-subtle,fb-overlay,sample-fb-icon-follow,sample-fb-inverted-following,sample-fb-subtle-following,sample-fb-overlay-following,sample-fb-icon-following-following'],
  'actions.follow-round': ['. height,width fb-round,sample-fb-round-following'],
  'actions.follow-tiny-pad-block': ['. padding_bottom,padding_top fb-tiny,sample-fb-tiny-following'],
  'actions.follow-tiny-pad-inline': ['. padding_left,padding_right fb-tiny,sample-fb-tiny-following'],
  'actions.follow-tiny-gap': ['. column_gap,row_gap fb-tiny,sample-fb-tiny-following,sample-fb-round-tiny'],
  'actions.follow-tiny-size': ['1 font_size fb-tiny,sample-fb-tiny-following'],
  'actions.follow-tiny-line': ['1 line_height fb-tiny,sample-fb-tiny-following'],
  'actions.follow-tiny-icon': ['0/0 height,width fb-tiny,sample-fb-tiny-following,sample-fb-round-tiny'],
  'actions.follow-tiny-icon-box': ['0 height,width fb-tiny,sample-fb-tiny-following,sample-fb-round-tiny'],
  'actions.follow-tiny-icon-pull': ['0 margin_left,margin_right fb-tiny,sample-fb-tiny-following'],
  'actions.follow-tiny-round': ['. height,width sample-fb-round-tiny'],
  'FollowButton.fill': ['. background_color fb-base,fb-tiny,fb-round,sample-fb-icon-follow,sample-fb-icon-following,sample-fb-icon-none,sample-fb-round-tiny'],
  'FollowButton.ink': ['0/0 text_color fb-base,fb-tiny,fb-round,sample-fb-icon-follow,sample-fb-round-tiny', '1 text_color fb-base,fb-tiny,sample-fb-icon-follow', '0 text_color sample-fb-icon-following,sample-fb-icon-none'],
  'FollowButton.followingFill': ['. background_color fb-following,sample-fb-tiny-following,sample-fb-round-following,sample-fb-icon-follow-following,sample-fb-icon-following-following'],
  'FollowButton.followingInk': ['0/0 text_color fb-following,sample-fb-tiny-following,sample-fb-round-following,sample-fb-icon-following-following', '1 text_color fb-following,sample-fb-tiny-following,sample-fb-icon-following-following', '0 text_color sample-fb-icon-follow-following'],
  'FollowButton.invertedFill': ['. background_color fb-inverted'],
  'FollowButton.invertedInk': ['0/0 text_color fb-inverted', '1 text_color fb-inverted'],
  'FollowButton.invertedFollowingFill': ['. background_color sample-fb-inverted-following'],
  'FollowButton.invertedFollowingInk': ['0/0 text_color sample-fb-inverted-following', '1 text_color sample-fb-inverted-following'],
  'FollowButton.subtleFill': ['. background_color fb-subtle'],
  'FollowButton.subtleInk': ['0/0 text_color fb-subtle', '1 text_color fb-subtle'],
  'FollowButton.subtleFollowingFill': ['. background_color sample-fb-subtle-following'],
  'FollowButton.subtleFollowingInk': ['0/0 text_color sample-fb-subtle-following', '1 text_color sample-fb-subtle-following'],
  'FollowButton.overlayFill': ['. background_color fb-overlay'],
  'FollowButton.overlayInk': ['0/0 text_color fb-overlay', '1 text_color fb-overlay'],
  'FollowButton.overlayFollowingFill': ['. background_color sample-fb-overlay-following'],
  'FollowButton.overlayFollowingInk': ['0/0 text_color sample-fb-overlay-following', '1 text_color sample-fb-overlay-following'],
  'FollowButton.pressScale': ['. press_scale fb-base,fb-following,fb-inverted,fb-subtle,fb-overlay,sample-fb-icon-follow,sample-fb-inverted-following,sample-fb-subtle-following,sample-fb-overlay-following,sample-fb-icon-following-following,sample-fb-icon-following,sample-fb-icon-none,sample-fb-icon-follow-following'],
  'FollowButton.pressScaleTiny': ['. press_scale fb-tiny,sample-fb-tiny-following'],
  'FollowButton.pressScaleRound': ['. press_scale fb-round,sample-fb-round-following,sample-fb-round-tiny'],
  // Stage 15 B's exports, from the box library-gate holds each in: Avatar's
  // column (0), its picture or figure (0/0, the figure's circles and path
  // 0/0/n) and its ring (0/1); VerifiedBadge's column (0) and its two
  // stacked symbols, the ground in ink (0/0) and the check in mark (0/1).
  'cards.avatar': ['0 height,width av-base,sample-av-src', '0/0 height,width av-base,sample-av-src'],
  'cards.avatar-corner': ['0 border_radius_bottom_left,border_radius_bottom_right,border_radius_top_left,border_radius_top_right av-base,sample-av-src', '0/1 border_radius_bottom_left,border_radius_bottom_right,border_radius_top_left,border_radius_top_right av-base,sample-av-src'],
  'cards.avatar-default': ['0/0/0 fill av-base'],
  'cards.avatar-ground': ['0 background_color av-base,sample-av-src'],
  'cards.avatar-ring': ['0/1 border_color_bottom,border_color_left,border_color_right,border_color_top av-base,sample-av-src'],
  'cards.badge': ['0/0 text_color vb-base,sample-vb-verifier'],
  'cards.badge-mark': ['0/0/1 fill av-base', '0/0/2 fill av-base', '0/1 text_color vb-base,sample-vb-verifier'],
  'cards.badge-size': ['0 height,width vb-base,sample-vb-verifier', '0/0 height,width vb-base,sample-vb-verifier', '0/1 height,width vb-base,sample-vb-verifier'],
  'cards.hairline': ['0/1 border_width_bottom,border_width_left,border_width_right,border_width_top av-base,sample-av-src'],
  // Stage 15 B batch 2's. EmptyState from its column: the icon (0), title
  // (1), message (2) and button (3; its icon's box 3/0 holding the icon
  // 3/0/0, and its text 3/1) on es-base;
  // the message alone (0) on sample-es-message; the message (0) and a
  // button with no icon (1, its text 1/0) on sample-es-button.
  'EmptyState.iconInk': ['0 text_color es-base'],
  'EmptyState.titleInk': ['1 text_color es-base'],
  'EmptyState.messageInk': ['2 text_color es-base', '0 text_color sample-es-message,sample-es-button'],
  'EmptyState.buttonFill': ['3 background_color es-base', '1 background_color sample-es-button'],
  'EmptyState.buttonInk': ['3/0/0 text_color es-base', '3/1 text_color es-base', '1/0 text_color sample-es-button'],
  'cards.empty-top': ['. padding_bottom,padding_top *'],
  'cards.empty-pad': ['. padding_left,padding_right *'],
  'cards.empty-gap': ['1 padding_top es-base', '2 padding_top es-base', '0 padding_top sample-es-message,sample-es-button'],
  'cards.empty-icon-size': ['0 height,width es-base'],
  'cards.empty-title-size': ['1 font_size es-base'],
  'cards.empty-message-size': ['2 font_size es-base', '0 font_size sample-es-message,sample-es-button'],
  'cards.empty-message-line': ['2 line_height es-base', '0 line_height sample-es-message,sample-es-button'],
  'cards.empty-button-top': ['3 margin_top es-base', '1 margin_top sample-es-button'],
  'cards.empty-button-gap': ['3 column_gap,row_gap es-base', '1 column_gap,row_gap sample-es-button'],
  'cards.empty-button-pad': ['3 padding_left,padding_right es-base', '1 padding_left,padding_right sample-es-button'],
  'cards.empty-button-pad-block': ['3 padding_bottom,padding_top es-base', '1 padding_bottom,padding_top sample-es-button'],
  'cards.empty-button-line': ['3/1 line_height es-base', '1/0 line_height sample-es-button'],
  'cards.empty-button-icon-pull': ['3/0 margin_left,margin_right es-base'],
  'cards.empty-button-corner': ['3 border_radius_bottom_left,border_radius_bottom_right,border_radius_top_left,border_radius_top_right es-base', '1 border_radius_bottom_left,border_radius_bottom_right,border_radius_top_left,border_radius_top_right sample-es-button'],
  'cards.empty-button-icon': ['3/0/0 height,width es-base'],
  'cards.empty-button-icon-box': ['3/0 height,width es-base'],
  'cards.empty-button-size': ['3/1 font_size es-base', '1/0 font_size sample-es-button'],
  'cards.empty-button-press': ['3 press_scale es-base', '1 press_scale sample-es-button'],
  // MenuItem from its `<id>-item` column: the hairline (0) and the button
  // (1; its label 1/0 and icon 1/1), a group's first item the button (0)
  // alone; MenuGroup's column (0) in its box.
  'MenuItem.surface': ['1 background_color mi-base,mi-destructive', '0 background_color sample-mi-first'],
  'MenuItem.ink': ['1/0 text_color mi-base', '0/0 text_color sample-mi-first'],
  'MenuItem.iconInk': ['1/1 text_color mi-base', '0/1 text_color sample-mi-first'],
  'MenuItem.destructiveInk': ['1/0 text_color mi-destructive', '1/1 text_color mi-destructive'],
  'MenuItem.divider': ['0 background_color mi-base,mi-destructive'],
  'MenuGroup.frame': ['0 border_color_bottom,border_color_left,border_color_right,border_color_top mg-base'],
  'lists.menu-hairline': ['0 height mi-base,mi-destructive', '0 border_width_bottom,border_width_left,border_width_right,border_width_top mg-base'],
  'lists.menu-corner': ['0 border_radius_bottom_left,border_radius_bottom_right,border_radius_top_left,border_radius_top_right mg-base'],
  'lists.menu-gap': ['1 column_gap,row_gap mi-base,mi-destructive', '0 column_gap,row_gap sample-mi-first'],
  'lists.menu-pad-inline': ['1 padding_left,padding_right mi-base,mi-destructive', '0 padding_left,padding_right sample-mi-first'],
  'lists.menu-pad-block': ['1 padding_bottom,padding_top mi-base,mi-destructive', '0 padding_bottom,padding_top sample-mi-first'],
  'lists.menu-height': ['1 min_height mi-base,mi-destructive', '0 min_height sample-mi-first'],
  'lists.menu-label-size': ['1/0 font_size mi-base,mi-destructive', '0/0 font_size sample-mi-first'],
  'lists.menu-icon-size': ['1/1 height,width mi-base,mi-destructive', '0/1 height,width sample-mi-first'],
  'BarItem.badgeFill': ['2 background_color sample-bi-badge,sample-bi-dot'],
  'BarItem.badgeInk': ['2 text_color sample-bi-badge'],
  'BarItem.tint': ['0/0/0 fill bi-base,sample-bi-selected,sample-bi-badge,sample-bi-dot'],
  'ComposeFrame.arc': ['>2/0/1/1/1/1 stroke cf-base,sample-cf-drafts,sample-cf-reply,cf-gallery,cf-camera,cf-gif', '>2/0/1/2/1/1 stroke sample-cf-near'],
  'ComposeFrame.countInk': ['>2/0/1/1/0 text_color cf-base,sample-cf-drafts,sample-cf-reply,cf-gallery,cf-camera,cf-gif', '>2/0/1/2/0 text_color sample-cf-near'],
  'ComposeFrame.divider': ['>2 border_color_top *'],
  'ComposeFrame.label': ['>2/0/1/0/0 text_color cf-base,sample-cf-drafts,sample-cf-reply,cf-gallery,cf-camera,cf-gif', '>2/0/1/1/0 text_color sample-cf-near,sample-cf-over'],
  'ComposeFrame.over': ['>2/0/1/2/0 text_color sample-cf-over', '>2/0/1/2/1/0 stroke sample-cf-over', '>2/0/1/2/1/1 stroke sample-cf-over'],
  'ComposeFrame.surface': ['. background_color *', '>2 background_color *'],
  'ComposeFrame.tint': ['0/0 text_color *', '2/0 text_color sample-cf-drafts,sample-cf-near,sample-cf-over', '>2/0/0/0/0 text_color *', '>2/0/0/1/0 text_color *', '>2/0/0/2/0 text_color *', '>2/0/1/0/0 text_color sample-cf-near,sample-cf-over'],
  'ComposeFrame.track': ['>2/0/1/1/1/0 stroke cf-base,sample-cf-drafts,sample-cf-reply,cf-gallery,cf-camera,cf-gif', '>2/0/1/2/1/0 stroke sample-cf-near'],
  'HeaderButton.tint': ['0/0 fill hb-base,hb-lead'],
  'HeaderButton.tintSmall': ['0/0 fill hb-small'],
  'ListRow.tint': ['0 text_color lr-base,sample-lr-value,sample-lr-check-symbol'],
  'ProfileHeader.alert': ['2/2/0/0 text_color sample-ph-alerts', '2/2/0/1 text_color sample-ph-alerts', '2/2/1/0 text_color sample-ph-alerts', '2/2/1/1 text_color sample-ph-alerts', '2/2/2/0 text_color sample-ph-alerts', '2/2/2/1 text_color sample-ph-alerts', '2/2/3/0 text_color sample-ph-alerts', '2/2/3/1 text_color sample-ph-alerts'],
  'ProfileHeader.alertGround': ['2/2/0 background_color sample-ph-alerts', '2/2/1 background_color sample-ph-alerts', '2/2/2 background_color sample-ph-alerts', '2/2/3 background_color sample-ph-alerts'],
  'ProfileHeader.badge': ['2/1/0/1/0/0 text_color sample-ph-verified,sample-ph-verifier'],
  'ProfileHeader.badgeMark': ['2/1/0/1/0/1 text_color sample-ph-verified,sample-ph-verifier'],
  'ProfileHeader.banner': ['0 background_color *'],
  'ProfileHeader.meta': ['2/1/1/0 text_color ph-base,ph-blocked,ph-compact,sample-ph-verified,sample-ph-verifier,sample-ph-alerts,sample-ph-note,sample-ph-avatar', '2/1/1/1 text_color sample-ph-follows-you', '2/3/0 text_color sample-ph-note', '2/3/0/0/0 text_color ph-base,ph-blocked,ph-compact,sample-ph-verified,sample-ph-verifier,sample-ph-follows-you,sample-ph-alerts,sample-ph-avatar', '2/3/0/0/0/1 text_color ph-base,ph-blocked,ph-compact,sample-ph-verified,sample-ph-verifier,sample-ph-follows-you,sample-ph-alerts,sample-ph-avatar', '2/3/0/1/0 text_color ph-base,ph-blocked,ph-compact,sample-ph-verified,sample-ph-verifier,sample-ph-follows-you,sample-ph-alerts,sample-ph-avatar', '2/3/0/1/0/1 text_color ph-base,ph-blocked,ph-compact,sample-ph-verified,sample-ph-verifier,sample-ph-follows-you,sample-ph-alerts,sample-ph-avatar', '2/3/0/2 text_color ph-base,ph-blocked,ph-compact,sample-ph-verified,sample-ph-verifier,sample-ph-follows-you,sample-ph-alerts,sample-ph-avatar', '2/3/0/2/1 text_color ph-base,ph-blocked,ph-compact,sample-ph-verified,sample-ph-verifier,sample-ph-follows-you,sample-ph-alerts,sample-ph-avatar'],
  'ProfileHeader.surface': ['. background_color *', '1 background_color *'],
  'SwitchRow.on': ['2 accent_color *'],
  'SwitchRow.tint': ['0 text_color *'],
  'actions.bar-badge-corner': ['2 border_radius_bottom_left,border_radius_bottom_right,border_radius_top_left,border_radius_top_right sample-bi-badge'],
  'actions.bar-badge-line': ['2 line_height sample-bi-badge'],
  'actions.bar-badge-pad': ['2 padding_left,padding_right sample-bi-badge'],
  'actions.bar-badge-pad-bottom': ['2 padding_bottom sample-bi-badge'],
  'actions.bar-badge-pad-top': ['2 padding_top sample-bi-badge'],
  'actions.bar-badge-size': ['2 font_size sample-bi-badge'],
  'actions.bar-badge-top': ['2 top sample-bi-badge'],
  'actions.bar-dot': ['2 height,width sample-bi-dot'],
  'actions.bar-dot-corner': ['2 border_radius_bottom_left,border_radius_bottom_right,border_radius_top_left,border_radius_top_right sample-bi-dot'],
  'actions.bar-dot-gap': ['2 margin_left sample-bi-dot'],
  'actions.bar-dot-top': ['2 top sample-bi-dot'],
  'actions.bar-item-bottom': ['. padding_bottom *'],
  'actions.bar-item-press': ['. press_scale *'],
  'actions.bar-item-top': ['. padding_top *'],
  'actions.compose-add-icon': ['>2/0/1/0/0 height,width sample-cf-near,sample-cf-over'],
  'actions.compose-add-pad': ['>2/0/1/0 padding_bottom,padding_left,padding_right,padding_top sample-cf-near,sample-cf-over'],
  'actions.compose-bar-height': ['. min_height *'],
  'actions.compose-bar-lead': ['. padding_left *'],
  'actions.compose-bar-trail': ['. padding_right *'],
  'actions.compose-count-line': ['>2/0/1/1/0 line_height cf-base,sample-cf-drafts,sample-cf-reply,cf-gallery,cf-camera,cf-gif', '>2/0/1/2/0 line_height sample-cf-near,sample-cf-over'],
  'actions.compose-counter-gap': ['>2/0/1/1 column_gap,row_gap cf-base,sample-cf-drafts,sample-cf-reply,cf-gallery,cf-camera,cf-gif', '>2/0/1/2 column_gap,row_gap sample-cf-near,sample-cf-over'],
  'actions.compose-counter-width': ['>2/0/1/1 width cf-base,sample-cf-drafts,sample-cf-reply,cf-gallery,cf-camera,cf-gif', '>2/0/1/2 width sample-cf-near,sample-cf-over'],
  'actions.compose-dim': ['>2/0/0/0 opacity cf-base,sample-cf-drafts,sample-cf-reply,cf-camera,cf-gif', '>2/0/0/1 opacity cf-base,sample-cf-drafts,sample-cf-reply,cf-gallery,cf-gif', '>2/0/0/2 opacity cf-base,sample-cf-drafts,sample-cf-reply,cf-gallery,cf-camera'],
  'actions.compose-drafts-gap': ['2 margin_left,margin_right sample-cf-drafts,sample-cf-near,sample-cf-over'],
  'actions.compose-drafts-pad': ['2 padding_left,padding_right sample-cf-drafts,sample-cf-near,sample-cf-over'],
  'actions.compose-hairline': ['>2 border_width_top *'],
  'actions.compose-icon': ['>2/0/0/0/0 height,width *', '>2/0/0/1/0 height,width *', '>2/0/0/2/0 height,width *'],
  'actions.compose-lang-max': ['>2/0/1/0/0 max_width cf-base,sample-cf-drafts,sample-cf-reply,cf-gallery,cf-camera,cf-gif', '>2/0/1/1/0 max_width sample-cf-near,sample-cf-over'],
  'actions.compose-lang-pad': ['>2/0/1/0 padding_bottom,padding_left,padding_right,padding_top cf-base,sample-cf-drafts,sample-cf-reply,cf-gallery,cf-camera,cf-gif', '>2/0/1/1 padding_bottom,padding_left,padding_right,padding_top sample-cf-near,sample-cf-over'],
  'actions.compose-ring': ['>2/0/1/1/1 height,width cf-base,sample-cf-drafts,sample-cf-reply,cf-gallery,cf-camera,cf-gif', '>2/0/1/2/1 height,width sample-cf-near,sample-cf-over'],
  'actions.compose-small-size': ['>2/0/1/0/0 font_size cf-base,sample-cf-drafts,sample-cf-reply,cf-gallery,cf-camera,cf-gif', '>2/0/1/1/0 font_size *', '>2/0/1/2/0 font_size sample-cf-near,sample-cf-over'],
  'actions.compose-text-height': ['0 height *', '2 height sample-cf-drafts,sample-cf-near,sample-cf-over'],
  'actions.compose-text-pad': ['0 padding_left,padding_right *'],
  'actions.compose-text-size': ['0/0 font_size *', '2/0 font_size sample-cf-drafts,sample-cf-near,sample-cf-over'],
  'actions.compose-tool-corner': ['>2/0/0/0 border_radius_bottom_left,border_radius_bottom_right,border_radius_top_left,border_radius_top_right *', '>2/0/0/1 border_radius_bottom_left,border_radius_bottom_right,border_radius_top_left,border_radius_top_right *', '>2/0/0/2 border_radius_bottom_left,border_radius_bottom_right,border_radius_top_left,border_radius_top_right *', '>2/0/1/0 border_radius_bottom_left,border_radius_bottom_right,border_radius_top_left,border_radius_top_right sample-cf-near,sample-cf-over'],
  'actions.compose-tool-gap': ['>2/0/0 column_gap,row_gap *'],
  'actions.compose-tool-pad': ['>2/0/0/0 padding_bottom,padding_left,padding_right,padding_top *', '>2/0/0/1 padding_bottom,padding_left,padding_right,padding_top *', '>2/0/0/2 padding_bottom,padding_left,padding_right,padding_top *'],
  'actions.compose-toolbar-lead': ['>2/0 padding_left *'],
  'actions.compose-toolbar-pad': ['>2/0 padding_bottom,padding_top *'],
  'actions.compose-toolbar-trail': ['>2/0 padding_right *'],
  'actions.compose-trail-gap': ['>2/0/1 column_gap,row_gap *'],
  'actions.header-corner': ['. border_radius_bottom_left,border_radius_bottom_right,border_radius_top_left,border_radius_top_right *'],
  'actions.header-icon': ['0 height,width hb-base,hb-lead'],
  'actions.header-icon-small': ['0 height,width hb-small'],
  'actions.header-lead': ['. margin_left hb-lead'],
  'actions.header-slot': ['. height,width *'],
  'cards.alert-gap': ['2/2 column_gap,row_gap *'],
  'cards.alert-icon': ['2/2/0/0 height,width sample-ph-alerts', '2/2/1/0 height,width sample-ph-alerts', '2/2/2/0 height,width sample-ph-alerts', '2/2/3/0 height,width sample-ph-alerts'],
  'cards.alert-icon-gap': ['2/2/0 column_gap,row_gap sample-ph-alerts', '2/2/1 column_gap,row_gap sample-ph-alerts', '2/2/2 column_gap,row_gap sample-ph-alerts', '2/2/3 column_gap,row_gap sample-ph-alerts'],
  'cards.alert-pad-block': ['2/2/0 padding_bottom,padding_top sample-ph-alerts', '2/2/1 padding_bottom,padding_top sample-ph-alerts', '2/2/2 padding_bottom,padding_top sample-ph-alerts', '2/2/3 padding_bottom,padding_top sample-ph-alerts'],
  'cards.alert-pad-inline': ['2/2/0 padding_left,padding_right sample-ph-alerts', '2/2/1 padding_left,padding_right sample-ph-alerts', '2/2/2 padding_left,padding_right sample-ph-alerts', '2/2/3 padding_left,padding_right sample-ph-alerts'],
  'cards.alert-under': ['2/2 padding_bottom *'],
  'cards.avatar-top': ['1 top ph-base,ph-blocked,sample-ph-verified,sample-ph-verifier,sample-ph-follows-you,sample-ph-alerts,sample-ph-note,sample-ph-avatar'],
  'cards.avatar-top-compact': ['1 top ph-compact'],
  'cards.banner-compact': ['0 height ph-compact'],
  'cards.banner-height': ['0 height ph-base,ph-blocked,sample-ph-verified,sample-ph-verifier,sample-ph-follows-you,sample-ph-alerts,sample-ph-note,sample-ph-avatar'],
  'cards.banner-shade': ['0/0 background_image *'],
  'cards.banner-shade-reach': ['0/0 padding_bottom *'],
  'cards.pill': ['2/1/1/0 background_color sample-ph-follows-you'],
  'cards.pill-corner': ['2/1/1/0 border_radius_bottom_left,border_radius_bottom_right,border_radius_top_left,border_radius_top_right sample-ph-follows-you', '2/2/0 border_radius_bottom_left,border_radius_bottom_right,border_radius_top_left,border_radius_top_right sample-ph-alerts', '2/2/1 border_radius_bottom_left,border_radius_bottom_right,border_radius_top_left,border_radius_top_right sample-ph-alerts', '2/2/2 border_radius_bottom_left,border_radius_bottom_right,border_radius_top_left,border_radius_top_right sample-ph-alerts', '2/2/3 border_radius_bottom_left,border_radius_bottom_right,border_radius_top_left,border_radius_top_right sample-ph-alerts'],
  'cards.pill-pad-block': ['2/1/1/0 padding_bottom,padding_top sample-ph-follows-you'],
  'cards.pill-pad-inline': ['2/1/1/0 padding_left,padding_right sample-ph-follows-you'],
  'cards.profile-actions-gap': ['2/0 column_gap,row_gap *'],
  'cards.profile-actions-height': ['2/0 min_height *'],
  'cards.profile-actions-lead': ['2/0 padding_left *'],
  'cards.profile-actions-under': ['2/0 padding_bottom *'],
  'cards.profile-avatar': ['1/0 height,width *', '1/0/0 height,width *', '1/0/0/0 height,width sample-ph-avatar'],
  'cards.profile-avatar-box': ['1 height,width *'],
  'cards.profile-avatar-left': ['1 left *'],
  'cards.profile-badge': ['2/1/0/1/0 height,width sample-ph-verified,sample-ph-verifier'],
  'cards.profile-badge-drop': ['2/1/0/1 margin_top sample-ph-verified,sample-ph-verifier'],
  'cards.profile-badge-gap': ['2/1/0/1 padding_left sample-ph-verified,sample-ph-verifier'],
  'cards.profile-bottom': ['2 padding_bottom *'],
  'cards.profile-counts-gap': ['2/3/0 column_gap,row_gap ph-base,ph-blocked,ph-compact,sample-ph-verified,sample-ph-verifier,sample-ph-follows-you,sample-ph-alerts,sample-ph-avatar'],
  'cards.profile-handle-gap': ['2/1/1 column_gap,row_gap *'],
  'cards.profile-meta-line': ['2/1/1/0 line_height ph-base,ph-blocked,ph-compact,sample-ph-verified,sample-ph-verifier,sample-ph-alerts,sample-ph-note,sample-ph-avatar', '2/1/1/1 line_height sample-ph-follows-you', '2/3/0/0/0 line_height ph-base,ph-blocked,ph-compact,sample-ph-verified,sample-ph-verifier,sample-ph-follows-you,sample-ph-alerts,sample-ph-avatar', '2/3/0/0/0/0 line_height ph-base,ph-blocked,ph-compact,sample-ph-verified,sample-ph-verifier,sample-ph-follows-you,sample-ph-alerts,sample-ph-avatar', '2/3/0/0/0/1 line_height ph-base,ph-blocked,ph-compact,sample-ph-verified,sample-ph-verifier,sample-ph-follows-you,sample-ph-alerts,sample-ph-avatar', '2/3/0/1/0 line_height ph-base,ph-blocked,ph-compact,sample-ph-verified,sample-ph-verifier,sample-ph-follows-you,sample-ph-alerts,sample-ph-avatar', '2/3/0/1/0/0 line_height ph-base,ph-blocked,ph-compact,sample-ph-verified,sample-ph-verifier,sample-ph-follows-you,sample-ph-alerts,sample-ph-avatar', '2/3/0/1/0/1 line_height ph-base,ph-blocked,ph-compact,sample-ph-verified,sample-ph-verifier,sample-ph-follows-you,sample-ph-alerts,sample-ph-avatar', '2/3/0/2 line_height ph-base,ph-blocked,ph-compact,sample-ph-verified,sample-ph-verifier,sample-ph-follows-you,sample-ph-alerts,sample-ph-avatar', '2/3/0/2/0 line_height ph-base,ph-blocked,ph-compact,sample-ph-verified,sample-ph-verifier,sample-ph-follows-you,sample-ph-alerts,sample-ph-avatar', '2/3/0/2/1 line_height ph-base,ph-blocked,ph-compact,sample-ph-verified,sample-ph-verifier,sample-ph-follows-you,sample-ph-alerts,sample-ph-avatar'],
  'cards.profile-name-line': ['2/1/0/0 line_height *'],
  'cards.profile-name-size': ['2/1/0/0 font_size *'],
  'cards.profile-names-gap': ['2/1 column_gap,row_gap *'],
  'cards.profile-names-top': ['2/1 padding_top *'],
  'cards.profile-names-under': ['2/1 padding_bottom *'],
  'cards.profile-pad': ['2 padding_left,padding_right *'],
  'cards.profile-top': ['2 padding_top *'],
};

/** Each param's token, pinned here apart from the sources, so a default
 * moved to another token fails library-gate.test rather than moving the
 * reads' expected values with it. */
export const PARAM_TOKENS = {
  'ListRow.tint': 'lists.symbol-tint', 'SwitchRow.tint': 'lists.symbol-tint', 'SwitchRow.on': 'lists.switch-on',
  'ProfileHeader.banner': 'cards.banner', 'ProfileHeader.surface': 'cards.profile-surface', 'ProfileHeader.meta': 'cards.meta',
  'ProfileHeader.badge': 'cards.badge', 'ProfileHeader.badgeMark': 'cards.badge-mark', 'ProfileHeader.alert': 'cards.alert',
  'ProfileHeader.alertGround': 'cards.alert-ground',
  'ComposeFrame.tint': 'actions.compose-tint', 'ComposeFrame.label': 'actions.compose-label', 'ComposeFrame.surface': 'actions.compose-surface',
  'ComposeFrame.divider': 'actions.compose-divider', 'ComposeFrame.countInk': 'actions.compose-count', 'ComposeFrame.over': 'actions.compose-over',
  'ComposeFrame.track': 'actions.compose-track', 'ComposeFrame.arc': 'actions.compose-arc',
  'BarItem.tint': 'actions.bar-tint', 'BarItem.badgeFill': 'actions.bar-badge', 'BarItem.badgeInk': 'actions.bar-badge-ink',
  'HeaderButton.tint': 'actions.header-tint', 'HeaderButton.tintSmall': 'actions.header-tint-small',
  'FollowButton.fill': 'actions.follow-fill', 'FollowButton.ink': 'actions.follow-ink',
  'FollowButton.followingFill': 'actions.follow-following-fill', 'FollowButton.followingInk': 'actions.follow-following-ink',
  'FollowButton.invertedFill': 'actions.follow-inverted-fill', 'FollowButton.invertedInk': 'actions.follow-inverted-ink',
  'FollowButton.invertedFollowingFill': 'actions.follow-following-fill', 'FollowButton.invertedFollowingInk': 'actions.follow-following-ink',
  'FollowButton.subtleFill': 'actions.follow-subtle-fill', 'FollowButton.subtleInk': 'actions.follow-subtle-ink',
  'FollowButton.subtleFollowingFill': 'actions.follow-subtle-following-fill', 'FollowButton.subtleFollowingInk': 'actions.follow-subtle-following-ink',
  'FollowButton.overlayFill': 'actions.follow-overlay-fill', 'FollowButton.overlayInk': 'actions.follow-overlay-ink',
  'FollowButton.overlayFollowingFill': 'actions.follow-overlay-fill', 'FollowButton.overlayFollowingInk': 'actions.follow-overlay-ink',
  'PersonRow.divider': 'cards.divider', 'PersonRow.meta': 'cards.meta', 'PersonRow.pill': 'cards.pill', 'PersonRow.badge': 'cards.badge', 'PersonRow.badgeMark': 'cards.badge-mark',
  'FollowButton.pressScale': 'actions.follow-press', 'FollowButton.pressScaleTiny': 'actions.follow-press-tiny', 'FollowButton.pressScaleRound': 'actions.follow-press-round',
  'EmptyState.iconInk': 'cards.empty-icon', 'EmptyState.titleInk': 'cards.empty-title', 'EmptyState.messageInk': 'cards.empty-message',
  'EmptyState.buttonFill': 'cards.empty-button', 'EmptyState.buttonInk': 'cards.empty-button-ink',
  'MenuItem.surface': 'lists.menu-surface', 'MenuItem.ink': 'lists.menu-label', 'MenuItem.iconInk': 'lists.menu-icon',
  'MenuItem.destructiveInk': 'lists.menu-destructive', 'MenuItem.divider': 'lists.menu-divider', 'MenuGroup.frame': 'lists.menu-frame',
};

/** A read's entries, parsed: [{ component, path, rows, ids }]. */
export function readsOf(key) {
  const component = key.includes('.') && /^[A-Z]/.test(key) ? key.split('.')[0] : null;
  return (READS[key] ?? []).map(entry => {
    const [path, rows, ids] = entry.split(' ');
    // A token two components read (cards.badge-mark: Avatar's figure and
    // VerifiedBadge's check) is owned per read, by its instances.
    const held = ids === '*' ? undefined : Object.keys(COMPONENTS).find(n => COMPONENTS[n].ids.includes(ids.split(',')[0]));
    const owner = component ?? held ?? componentOfPath(key);
    return { component: owner, path, rows: rows.split(','), ids: ids === '*' ? COMPONENTS[owner].ids : ids.split(',') };
  });
}
const componentOfPath = token => {
  const batch = BATCHES.find(b => b.names?.includes(token)) ?? BATCHES.find(b => b.prefix?.test(token));
  if (!batch) throw new Error(`${token}: in no batch`);
  if (batch.components.length === 1) return batch.components[0];
  if (batch.name === 'lists') return token === 'lists.switch-on' ? 'SwitchRow' : 'ListRow';
  if (batch.name === 'Menu' && token !== 'lists.menu-hairline') return token === 'lists.menu-corner' || token === 'lists.menu-frame' ? 'MenuGroup' : 'MenuItem';
  // The exports' batch draws with one token in two components (the menu's
  // hairline too): a read names its instances.
  throw new Error(`${token}: a read in the ${batch.name} batch names no instance of one component (not *)`);
};

/** The packages' tokens from their designs: [{ name, kind }], `kind` a
 * length, a number or (no type) a colour. */
export function packageTokens(root = ROOT) {
  // An icon token (`name = icon(svg "…")`, a space before its `(` or none) is no colour, length or number: no case restyles it.
  return PACKAGES.flatMap(p => [...readFileSync(join(root, 'packages', p, `${p}.tamber`), 'utf8').matchAll(/^\s+([a-z][\w-]*(?:\.[\w-]+)+)(?::\s*([\w-]+))?\s*=(?!\s*icon\s*\()/gm)]
    .map(m => ({ name: m[1], kind: m[2] ?? 'color' })));
}

/** Stage 13 C's components' params from their sources: [{ full, key, token }]. */
export function packageParams(root = ROOT) {
  return Object.entries(COMPONENTS).flatMap(([component, c]) => {
    const [, pkg, file] = /^@exact\/([\w-]+)\/(.+)$/.exec(c.source);
    const text = readFileSync(join(root, 'packages', pkg, file), 'utf8');
    const body = text.slice(text.indexOf(`component ${component}`));
    // Its own block only (common.contract holds two), and no params block, none.
    const next = body.slice(1).search(/\ncomponent /);
    const own = next < 0 ? body : body.slice(0, next + 1);
    if (!/^  params\s*$/m.test(own)) return [];
    const params = own.slice(own.search(/^  params\s*$/m), own.search(/\n  (props|slot|parts|view)\b/));
    // Every param, whatever its default: a token's name, or a literal.
    return [...params.matchAll(/^\s{4}([\w-]+):\s*([\w-]+)\s*=\s*(?:"([^"]*)"|(\S+))/gm)].map(m => ({ full: `${c.source}#${component}.${m[1]}`, key: `${component}.${m[1]}`, kind: m[2], token: /^[a-z][\w-]*(?:\.[\w-]+)+$/.test(m[3] ?? '') ? m[3] : null }));
  });
}

/** Stage 13 C's tokens: the packages' tokens stage 12 didn't have. */
export const newTokens = (root = ROOT) => packageTokens(root).filter(t => !STAGE12_TOKENS.has(t.name));

/** A case's value for its `k`th name of `kind`: lengths from 301 px in steps
 * of 3 (past every default, 2 pt apart at least), numbers from 0.15 in steps
 * of 0.1, colours a step apart by at least 24 in some channel. */
export function valueFor(kind, k) {
  if (kind === 'length') return `${301 + 3 * k}px`;
  if (kind === 'number') return String(Math.round((0.15 + 0.1 * k) * 100) / 100);
  if (kind !== 'color') throw new Error(`no case value for a ${kind} token or param: give the restyle cases one`);
  const ch = (m, o) => (o + ((k * m) % 192)).toString(16).padStart(2, '0');
  return `#${ch(40, 32)}${ch(72, 40)}${ch(104, 48)}`;
}

/** The nineteen cases (stage 13 C's ten, stage 15 B's exports' one, and batch 2's FollowButton, PersonRow, EmptyState and menu two each): each batch's tokens (the derived ones read, not set) and
 * its components' params, each name its own value. */
export function cases(root = ROOT) {
  const tokens = newTokens(root), params = packageParams(root);
  return BATCHES.flatMap(b => {
    const mine = b.names ? b.names.map(n => packageTokens(root).find(t => t.name === n)) : tokens.filter(t => b.prefix.test(t.name) && !DERIVED[t.name]);
    const counts = {};
    const set = mine.map(t => ({ name: t.name, kind: t.kind, value: valueFor(t.kind, (counts[t.kind] = (counts[t.kind] ?? -1) + 1)) }));
    const owned = params.filter(p => b.components.includes(p.key.split('.')[0]));
    return [
      { name: `${b.name} tokens`, batch: b.name, param: false, set, derived: b.prefix ? Object.keys(DERIVED).filter(d => b.prefix.test(d)) : [] },
      // A batch whose components have no params (the exports) has no params case.
      ...(owned.length ? [{ name: `${b.name} params`, batch: b.name, param: true, set: owned.map((p, k) => ({ name: p.full, key: p.key, token: p.token, kind: p.kind, value: valueFor(p.kind, k + 7) })), derived: [] }] : []),
    ];
  });
}

/** The design server's writes for a case: set each name, and remove each. */
export const caseOps = c => ({
  set: c.set.map(s => (c.param ? { op: 'set-param', name: s.name, value: s.value } : { op: 'set-token', name: s.name, value: s.value })),
  unset: c.set.map(s => (c.param ? { op: 'unset-param', name: s.name } : { op: 'remove-token', name: s.name })),
});

/** What each read expects once a case is set: [{ key, component, path,
 * rows, ids, want }], `want` a length or number (points) or a colour
 * (`#rrggbb`), or null where the read keeps its base value. */
export function caseReads(c, params = packageParams()) {
  const value = Object.fromEntries(c.set.map(s => [c.param ? s.key : s.name, s]));
  const num = s => Number.parseFloat(s.value);
  const out = [];
  const add = (key, want, kind) => { for (const r of readsOf(key)) out.push({ key, ...r, want, kind }); };
  if (c.param) {
    for (const s of c.set) add(s.key, s.kind === 'color' ? s.value : num(s), s.kind);
    return out;
  }
  for (const s of c.set) add(s.name, s.kind === 'color' ? s.value : num(s), s.kind);
  for (const d of c.derived) add(d, DERIVED[d].value(DERIVED[d].from.map(n => num(value[n]))), 'length');
  // A param reads its token (as pinned) where nothing sets the param.
  for (const p of params) {
    const token = PARAM_TOKENS[p.key];
    if (value[token]) add(p.key, value[token].value, p.kind);
  }
  return out;
}

/** The node at `path` from `root` (a testId) in a tree reply, or null. */
export function locate(tree, root, path) {
  const by = new Map(tree.nodes.map(n => [n.id, n]));
  // A parent from its children (the JS target's tree gives no parent links).
  const parentOf = new Map(tree.nodes.flatMap(n => (n.children ?? []).map(c => [c, n.id])));
  let node = tree.nodes.find(n => n.props?.testId === root);
  for (const step of path === '.' ? [] : path.split('/')) {
    if (!node) return null;
    if (step.startsWith('>')) {
      const parent = by.get(node.parent ?? parentOf.get(node.id));
      const at = parent ? parent.children.indexOf(node.id) + Number(step.slice(1)) : -1;
      node = parent && by.get(parent.children[at]);
    } else node = by.get(node.children[Number(step)]);
  }
  return node ?? null;
}

/** A computed value as a number of points ("12px", "12" or 12), or NaN. */
export const points = v => (typeof v === 'number' ? v : typeof v === 'string' && /^-?\d+(\.\d+)?(px)?$/.test(v.trim()) ? Number.parseFloat(v) : Number.NaN);

/** A colour as `#rrggbbaa`: the runner's `#rrggbb` (opaque) or `#rrggbbaa`,
 * or the browser's `rgb()` / `rgba()`; null for anything else (a role, a
 * `var()`, a `light-dark()`: no one resolved colour). */
export function hexOf(v) {
  if (typeof v !== 'string') return null;
  const t = v.trim().toLowerCase();
  if (/^#[0-9a-f]{6}$/.test(t)) return `${t}ff`;
  if (/^#[0-9a-f]{8}$/.test(t)) return t;
  const m = /^rgba?\(\s*(\d+)[,\s]+(\d+)[,\s]+(\d+)(?:\s*[,/]\s*([\d.]+%?))?\s*\)$/.exec(t);
  if (!m) return null;
  const a = m[4] === undefined ? 1 : m[4].endsWith('%') ? Number.parseFloat(m[4]) / 100 : Number.parseFloat(m[4]);
  return `#${[m[1], m[2], m[3]].map(x => Number(x).toString(16).padStart(2, '0')).join('')}${Math.round(a * 255).toString(16).padStart(2, '0')}`;
}

/** A gradient's first stop's colour (`linear-gradient(<angle>, <colour> 0%, …)`,
 * the angle optional), as `#rrggbbaa`, or null. */
export function firstStop(v) {
  // The prelude, when there is one, an angle or a side (`180deg`, `to bottom`).
  const m = typeof v === 'string' && /^linear-gradient\((?:\s*(?:-?[\d.]+(?:deg|rad|grad|turn)|to(?:\s+[a-z]+)+)\s*,)?\s*(#[0-9a-f]+|rgba?\([^)]*\))\s+0%/i.exec(v.trim());
  return m ? hexOf(m[1]) : null;
}

/** The colour a read resolves to: the value itself, or a gradient's first
 * stop (the banner's shade starts at its token). */
const resolved = (v, row) => (row === 'background_image' ? firstStop(v) : hexOf(v));

/** Whether computed value `v` of `row` is `want`: a length or number within
 * float rounding, a colour exactly `want`, opaque. */
export function matches(v, want, kind, row = '') {
  if (kind === 'color') return resolved(v, row) === `${want.toLowerCase()}ff`;
  return Math.abs(points(v) - want) <= (kind === 'number' ? 1e-3 : 0.01);
}

/** Whether two computed values are the same (a read back at its base), to
 * the read's own tolerance: a number's as `matches` holds it, 1e-3. */
export const same = (a, b, kind = 'length') => (Number.isFinite(points(a)) && Number.isFinite(points(b)) ? Math.abs(points(a) - points(b)) <= (kind === 'number' ? 1e-3 : 0.01) : JSON.stringify(a) === JSON.stringify(b));

/** Whether set value `want` stands apart from base value `base` by more
 * than the read's tolerance (theme2 10:28 (1)): 2 pt for a length. */
export function apart(base, want, kind, row = '') {
  if (kind === 'color') return resolved(base, row) !== `${want.toLowerCase()}ff`;
  const b = points(base);
  return !Number.isFinite(b) || Math.abs(b - want) >= (kind === 'number' ? 0.05 : 2);
}

/** One drive of `plan` on `host` that reads `ids` (the runner's tree first):
 * { tree, nodes: Map(id, node) }. The terminal is driven itself, as
 * scale-check drives it. */
export function inspectDrive(host, plan, ids, app = null) {
  const steps = ids.map(id => (host === 'terminal' ? ['node', String(id)] : [`layout ${id}`])).flat();
  const launch = host === 'android' ? ['--reader', '--app', app, '--plan', plan] : ['--plan', plan];
  const run = host === 'terminal'
    ? spawnSync('cargo', ['run', '-q', '-p', 'exact-terminal', '--', plan, '--size', '64x64', 'plan', 'runner-tree', ...steps], { cwd: ROOT, encoding: 'utf8', timeout: 900_000, maxBuffer: 1 << 30 })
    : spawnSync(process.execPath, [join(ROOT, 'scripts/agent.mjs'), host, ...launch, '--storage', 's13c-restyle', '--reset-storage', '--json', 'perf', 'tree', ...steps], { cwd: ROOT, encoding: 'utf8', timeout: 900_000, maxBuffer: 1 << 30 });
  const lines = (run.stdout ?? '').split('\n').filter(l => l.startsWith('{'));
  if (run.error || run.status !== 0) throw new Error(`the drive failed${run.error ? ` (${run.error.message})` : ''}\n${(run.stdout ?? '').slice(-1500)}${(run.stderr ?? '').slice(-1500)}`);
  const ran = lines.map(l => /"plan":"([0-9a-f]{64})"/.exec(l)?.[1]).find(Boolean);
  const want = createHash('sha256').update(readFileSync(plan)).digest('hex');
  if (ran !== want) throw new Error(`${host} runs ${ran ? `another plan (${ran.slice(0, 12)}…)` : 'a plan it does not name'}, not ${want.slice(0, 12)}…: not read`);
  const tree = lines.map(l => JSON.parse(l)).find(r => Array.isArray(r.nodes) && 'roots' in r);
  if (!tree) throw new Error(`${host}: no tree in the drive's output`);
  const nodes = new Map();
  for (const l of lines) {
    if (!l.includes('"node"')) continue;
    const r = JSON.parse(l);
    if (r.node?.id != null) nodes.set(r.node.id, r.node);
  }
  return { tree, nodes };
}
