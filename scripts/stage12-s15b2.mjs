// Stage 15 B batch 2's gate profile (LLP 1121, LLP 1139 §5; theme2's 23:16
// approval of ~/x2theme/iosfix-s15b2-gate-plan.md): batch 1's, plus the
// five components batch 2 adopts (FollowButton, PersonRow, EmptyState,
// MenuItem, MenuGroup), the copy's duplicates they retire, the bindings of
// theme2's option B, the screens that show each, and the NAMED pixel
// changes the copy scripts' outputs list (scripts/stage12-named.mjs).

/** The five, by identity. */
export const S15B2_EXPORTS = ['@exact/actions/follow-button.contract#FollowButton', '@exact/cards/person-row.contract#PersonRow',
  '@exact/cards/empty-state.contract#EmptyState', '@exact/lists/menu.contract#MenuItem', '@exact/lists/menu.contract#MenuGroup'];

/** The copy's two bindings (theme2's option B, 14:15 and 18:40): one PersonRow call each, and in its slot at most the
 * `if` whose only branch is the package FollowButton. */
export const S15B2_BINDINGS = ['PersonResultRow', 'FindFollowRow'].map(component => ({ component,
  allowed: ['@exact/cards/person-row.contract#PersonRow', '@exact/actions/follow-button.contract#FollowButton'] }));

const DEVI = 'avatar-3:at://did:plc:fixture3/app.bsky.feed.post/fx1';
const VIDEO = 'video-post-card-at://did:plc:fixture3/app.bsky.feed.post/fx9';

/** The profile, from batch 1's (`s15b`) and its sign-in steps. */
export function s15b2Stage(s15b, signIn) {
  const IN = [...signIn, 'clock +1500 real'];
  const step = t => [t, 'clock +1500 real'];
  // Signed in, home shows the Following feed (empty in the fixture), so a profile opens by its path on the
  // navigation root (the agent's `type <root> <path>`), as a deep link does.
  const open = handle => step(`type bluesky /profile/${handle}.fixture.invalid`);
  return {
    name: 'Stage 15 B batch 2', tag: 's15b2-baseline', out: 's15b2-gate-run.txt',
    adopted: [...s15b.adopted, ...S15B2_EXPORTS],
    before: [...s15b.adopted],
    appPlaced: [...S15B2_EXPORTS],
    // PersonRow: the bindings renamed the copy's PersonResultRow; FindFollowRow is a binding, kept. ChatPersonRow,
    // NoteRow and SearchEmpty stay the app's (theme2's rulings), so none of them is here.
    retired: ['component PersonRow', 'component ChatEmpty', 'component ListEmpty', 'component GraphEmpty', 'component MenuItem', 'style MenuGroup', 'style Group'],
    bindings: S15B2_BINDINGS,
    screens: [
      ...s15b.screens.map(([n, t]) => [n, [...t]]),
      // Ada's header and its FollowButton showing Following (the fixture: you follow her).
      ['profile-ada', [...IN, ...open('ada')]],
      // Cyn's profile scrolled under its header: the stuck header's follow button (her 30-line post scrolls it).
      ['profile-cyn-stuck', [...IN, ...open('cyn'), 'tap profile-list wheel 0 2000', 'clock +1500 real']],
      // People search: every follow state, Follow, Follow back, Following and both.
      ['explore-people', [...IN, ...step('tap bottomBarSearchBtn'), 'tap module-search-people', 'type search fixture', ...step('tap search-for')]],
      // The post's "…" sheet (signed out: its signed-out groups) and Ada's profile menu signed in (Block, Report).
      ['post-menu', [...step(`tap more-0:at://did:plc:fixture0/app.bsky.feed.post/fx0`)]],
      ['profile-menu', [...IN, ...open('ada'), ...step('tap profile-more')]],
      // The empty states: no lists of yours; Devi's list with nobody in it; Bo's followers, none.
      ['my-lists-empty', [...IN, 'tap open-drawer', ...step('tap drawer-lists')]],
      ['list-empty', [...IN, ...open('dev'), ...step('tap profile-tab-lists'), ...step('tap card-Reading'), ...step('tap graph-tab-people')]],
      ['followers-ada', [...IN, ...open('ada'), ...step('tap profile-followers-ada.fixture.invalid')]],
      ['followers-bo-empty', [...IN, ...open('bo'), ...step('tap profile-followers-bo.fixture.invalid')]],
      // The new-chat sheet: ChatPersonRow stays the app's, so its rows are unchanged.
      ['new-chat', [...IN, ...step('tap bottomBarMessagesBtn'), ...step('tap newChatFAB')]],
      // Devi's clip in the video feed: the overlay FollowButton.
      ['video-devi', [...step(`tap ${DEVI}`), ...step('tap profile-tab-videos'), ...step(`tap ${VIDEO}`)]],
    ],
    // The five write the copy's own testIds (their ids are its props), so the build's source map says each is shown,
    // as for ListRow and the rest (consumerPixels' by-component check), not a package-only testId.
    rendered: [...s15b.rendered],
    // NAMED changes (scripts/stage12-named.mjs; theme2 23:16 (a)): each a screen, the SMALLEST container holding the
    // changed instances and what they move (by its testId), and the change it expects, from the copy scripts'
    // outputs (tools/s15b2/*.out) and the web drives that found each box. A screen not listed must be identical.
    named: [
      { screen: 'notifications', target: 'note-follow-at://did:plc:fixture1/app.bsky.graph.follow/fxf1:1', change: 'the follow notification\'s FollowButton (small primary): padding-left 12->14, its text 0.875rem at 600 -> 0.81875rem at 500, the package\'s plus and check (SF on Apple)' },
      { screen: 'video-devi', target: 'vfeed-follow-at://did:plc:fixture3/app.bsky.feed.post/fx9', change: 'the video feed\'s FollowButton (small overlay): ground contrast_900 -> fixed #dce2ea, text and check contrast_25 -> #000, padding-inline 12->14, gap 4->5, check 14->16' },
      { screen: 'video-devi', hosts: ['macos'], target: 'vfeed-panel-at://did:plc:fixture3/app.bsky.feed.post/fx9', change: 'on the Mac (13-point root) the taller button outgrows the author block, so the bottom panel holding it grows about a point and its gradient and text shift; elsewhere the panel keeps its bounds and only the button is named' },
      { screen: 'author-bo', target: 'follow', change: 'Bo\'s header FollowButton (Follow back): the PlusLarge Glyph -> the package\'s plus (SF on Apple), in a 17 box pulled -2' },
      { screen: 'profile-cyn-stuck', target: 'profile-minimal-header', change: 'the stuck header\'s FollowButton: padding 6/12 -> 8/14, gap 4->5, icon 11->16 (the smallest testId\'d container: before\'s button had no testId)' },
      { screen: 'explore-people', target: 'people-results', change: 'PersonResultRow rows (PersonRow): Bluesky\'s ProfileCard spacing (cards.person-*) where the copy had padding 20 and gap 12 and 8; the FollowButton in the slot at the trailing edge; descriptions wrap narrower; rows below shift' },
      { screen: 'followers-ada', target: 'people-list', change: 'PersonResultRow rows (PersonRow): Bluesky\'s ProfileCard spacing where the copy had padding 20 and gap 12 and 8; the FollowButton in the slot at the trailing edge; descriptions wrap narrower; rows below shift' },
      { screen: 'followers-bo-empty', target: 'list-empty', change: 'EmptyState (tone medium) from ListEmpty: padding 48/32 -> 40/space.lg, no gap 8; the message at 500 in contrast_medium where it was 400 in contrast_700' },
      { screen: 'my-lists-empty', target: 'my-lists-empty', change: 'EmptyState: the list-bullet 40 in contrast_700 -> 48 (3xl) in contrast_low; the message 0.875rem at 400 in contrast_700, max-width 200 -> 0.9375rem at 500 in contrast_high' },
      { screen: 'list-empty', target: 'graph-empty', change: 'EmptyState (tone medium) from GraphEmpty: padding 48 top and bottom and gap 16 -> the package\'s; the message at 500 in contrast_medium where it was 400 in contrast_700' },
      { screen: 'post-menu', hosts: ['ios'], target: 'post-menu-sheet', pad: [28, 8, 40, 8], change: 'UIKit\'s sheet frame, painted past the content 8 points each side, 28 above and 40 below (its bottom to the home indicator), padded in on iOS alone; MenuItem and MenuGroup: each item 44 points tall (Bluesky\'s, border-box) where the copy\'s content-box min-height under 10-point padding made 64, so the groups are shorter and the sheet holding them (UIKit\'s on iOS) moves; the items\' colours from lists.menu-* over platform roles where the copy named contrast_25, contrast_900, contrast_700, negative and contrast_100; the hairline -exact-hairline where it was 0.5; MenuGroup\'s frame' },
      { screen: 'post-menu', hosts: ['web', 'web-wasm', 'macos', 'linux'], target: 'post-menu-groups', change: 'MenuItem and MenuGroup: each item 44 points tall (Bluesky\'s, border-box) where the copy\'s content-box min-height under 10-point padding made 64, so the groups are shorter and the sheet holding them (UIKit\'s on iOS) moves; the items\' colours from lists.menu-* over platform roles where the copy named contrast_25, contrast_900, contrast_700, negative and contrast_100; the hairline -exact-hairline where it was 0.5; MenuGroup\'s frame' },
      { screen: 'profile-menu', hosts: ['ios'], target: 'profile-menu-sheet', pad: [28, 8, 40, 8], change: 'UIKit\'s sheet frame, painted past the content 8 points each side, 28 above and 40 below (its bottom to the home indicator), padded in on iOS alone; MenuItem and MenuGroup: each item 44 points tall (Bluesky\'s, border-box) where the copy\'s content-box min-height under 10-point padding made 64, so the groups are shorter and the sheet holding them (UIKit\'s on iOS) moves; the items\' colours from lists.menu-* over platform roles where the copy named contrast_25, contrast_900, contrast_700, negative and contrast_100; the hairline -exact-hairline where it was 0.5; MenuGroup\'s frame' },
      { screen: 'profile-menu', hosts: ['web', 'web-wasm', 'macos', 'linux'], target: 'profile-menu-groups', change: 'MenuItem and MenuGroup: each item 44 points tall (Bluesky\'s, border-box) where the copy\'s content-box min-height under 10-point padding made 64, so the groups are shorter and the sheet holding them (UIKit\'s on iOS) moves; the items\' colours from lists.menu-* over platform roles where the copy named contrast_25, contrast_900, contrast_700, negative and contrast_100; the hairline -exact-hairline where it was 0.5; MenuGroup\'s frame' },
    ],
    restyle: [...s15b.restyle],
    exact: true,
    // The package side as it landed on main: 2a, its LLP note, 2b, 2c, the brand fill's actions half, the EmptyState
    // follow-up.
    release: { label: 'stage 15 B batch 2', runs: [['70b643a47c', '3a63d4c319'], ['c7a0b0602a', 'c7a0b0602a'], ['128269c480', 'cee509ac82'], ['91c61aee77', '91c61aee77'], ['0952139ef4', '0952139ef4'], ['d6d5fc3ec4', 'd6d5fc3ec4']] },
    titles: {
      'consumer-pixels': 'The clone\'s follow buttons, person rows, empty states and menus come from @exact/actions FollowButton, @exact/cards PersonRow and EmptyState and @exact/lists MenuItem and MenuGroup, the duplicates gone, the bindings bare, and every other pixel unchanged (conform clean; zero differing pixels outside the allowlist and the NAMED boxes, each named box\'s crops in the record, web, iOS, macOS, Linux, light and dark)',
    },
  };
}
