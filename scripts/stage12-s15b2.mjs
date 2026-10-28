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

const ADA = 'avatar-0:at://did:plc:fixture0/app.bsky.feed.post/fx0';
const BO = 'avatar-1:at://did:plc:fixture1/app.bsky.feed.post/fx4';
const CYN = 'avatar-2:at://did:plc:fixture2/app.bsky.feed.post/fx6';
const DEVI = 'avatar-3:at://did:plc:fixture3/app.bsky.feed.post/fx1';
const VIDEO = 'video-post-card-at://did:plc:fixture3/app.bsky.feed.post/fx9';

/** The profile, from batch 1's (`s15b`) and its sign-in steps. */
export function s15b2Stage(s15b, signIn) {
  const IN = [...signIn, 'clock +1500 real'];
  const step = t => [t, 'clock +1500 real'];
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
      ['profile-ada', [...IN, ...step(`tap ${ADA}`)]],
      // Cyn's profile scrolled under its header: the stuck header's follow button (her 30-line post scrolls it).
      ['profile-cyn-stuck', [...IN, ...step(`tap ${CYN}`), 'tap profile-list wheel 0 2000', 'clock +1500 real']],
      // People search: every follow state, Follow, Follow back, Following and both.
      ['explore-people', [...IN, ...step('tap bottomBarSearchBtn'), 'tap module-search-people', 'type search fixture', ...step('tap search-for')]],
      // The post's "…" sheet (signed out: its signed-out groups) and Ada's profile menu signed in (Block, Report).
      ['post-menu', [...step(`tap more-0:at://did:plc:fixture0/app.bsky.feed.post/fx0`)]],
      ['profile-menu', [...IN, ...step(`tap ${ADA}`), ...step('tap profile-more')]],
      // The empty states: no lists of yours; Devi's list with nobody in it; Bo's followers, none.
      ['my-lists-empty', [...IN, 'tap open-drawer', ...step('tap drawer-lists')]],
      ['list-empty', [...IN, ...step(`tap ${DEVI}`), ...step('tap profile-tab-lists'), ...step('tap card-Reading'), ...step('tap graph-tab-people')]],
      ['followers-ada', [...IN, ...step(`tap ${ADA}`), ...step('tap profile-followers-ada.fixture.invalid')]],
      ['followers-bo-empty', [...IN, ...step(`tap ${BO}`), ...step('tap profile-followers-bo.fixture.invalid')]],
      // The new-chat sheet: ChatPersonRow stays the app's, so its rows are unchanged.
      ['new-chat', [...IN, ...step('tap bottomBarMessagesBtn'), ...step('tap newChatFAB')]],
      // Devi's clip in the video feed: the overlay FollowButton.
      ['video-devi', [...step(`tap ${DEVI}`), ...step('tap profile-tab-videos'), ...step(`tap ${VIDEO}`)]],
    ],
    // The five write the copy's own testIds (their ids are its props), so the build's source map says each is shown,
    // as for ListRow and the rest (consumerPixels' by-component check), not a package-only testId.
    rendered: [...s15b.rendered],
    // NAMED changes (scripts/stage12-named.mjs): each a screen, the SMALLEST container holding the changed instances
    // and what they move (by its testId), and the change it expects. Filled from the drives (the copy scripts'
    // outputs, tools/s15b2/*.out, say each change).
    named: [],
    restyle: [...s15b.restyle],
    exact: true,
    // The package side as it landed on main: 2a, its LLP note, 2b, 2c.
    release: { label: 'stage 15 B batch 2', runs: [['70b643a47c', '3a63d4c319'], ['c7a0b0602a', 'c7a0b0602a'], ['128269c480', 'cee509ac82'], ['91c61aee77', '91c61aee77']] },
    titles: {
      'consumer-pixels': 'The clone\'s follow buttons, person rows, empty states and menus come from @exact/actions FollowButton, @exact/cards PersonRow and EmptyState and @exact/lists MenuItem and MenuGroup, the duplicates gone, the bindings bare, and every other pixel unchanged (conform clean; zero differing pixels outside the allowlist and the NAMED boxes, each named box\'s crops in the record, web, iOS, macOS, Linux, light and dark)',
    },
  };
}
