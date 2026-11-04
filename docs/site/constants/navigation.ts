export type NavPage = {
  route: string;
  path: string;
  source: string;
  description: string;
  split?: boolean;
  hidden?: boolean;
  landing?: boolean;
};

export type NavSection = {
  title: string;
  card?: { title: string; description: string };
  pages: NavPage[];
};

export const sections: NavSection[] = [
  {
    title: 'Home',
    pages: [
      {
        route: 'home',
        path: '/',
        source: 'site/pages/home.md',
        landing: true,
        description: 'Write an app once. It runs natively on the web, macOS, iOS, and Linux.',
      },
    ],
  },
  {
    title: 'Get started',
    card: { title: 'Start here', description: 'Build an app with Exact: the idea, the workflow, and the one screen every app starts from.' },
    pages: [
      {
        route: 'startHere',
        path: '/start',
        source: 'start-here.md',
        description: 'Build an app with Exact: the idea, the workflow, and the one screen every app starts from.',
      },
      {
        route: 'startHereSite',
        path: '/start-website',
        source: 'start-here-site.md',
        description: 'Build a website with Exact: pages at URLs, served and read in a browser.',
      },
    ],
  },
  {
    title: 'Learn',
    card: { title: 'Learn', description: 'Contract, the language of an Exact app, from the first component to delivery.' },
    pages: [
      {
        route: 'contractForHumans',
        path: '/learn/contract-for-humans',
        source: 'contract-for-humans.md',
        split: true,
        description: 'Contract, the language of an Exact app, from the first component to delivery.',
      },
      {
        route: 'contractGrammar',
        path: '/learn/contract-grammar',
        source: 'contract-grammar.md',
        split: true,
        description: 'The Contract grammar and vocabulary the compiler accepts: every declaration, element and attribute.',
      },
      {
        route: 'reference',
        path: '/learn/reference',
        source: 'reference.md',
        split: true,
        description: 'Toolchains, the Contract CLI, the native and web loops, data modules, delivery and the stress examples.',
      },
    ],
  },
  {
    title: 'Recipes',
    card: { title: 'Recipes', description: 'Short answers to common questions, one page each: forms, lists, timers, sign-in, storage, and more.' },
    pages: [
      {
        route: 'recipeAfterAWrite',
        path: '/recipes/after-a-write',
        source: 'recipes/after-a-write.md',
        description: 'Clear fields and close forms only on ok: true.',
      },
      {
        route: 'recipeAppJson',
        path: '/recipes/app-json',
        source: 'recipes/app-json.md',
        description: 'app.json names the app and what it ships; its $schema gives an editor every key.',
      },
      {
        route: 'recipeBackgroundTransfers',
        path: '/recipes/background-transfers',
        source: 'recipes/background-transfers.md',
        description: '@exact/background-transfer hands an upload or download to the system, which finishes it after the app is suspended or terminated.',
      },
      {
        route: 'recipeBackgroundWork',
        path: '/recipes/background-work',
        source: 'recipes/background-work.md',
        description: '@exact/background (LLP 1133.017) has Background Sync\'s and Periodic Background Sync\'s names.',
      },
      {
        route: 'recipeChatThread',
        path: '/recipes/chat-thread',
        source: 'recipes/chat-thread.md',
        description: 'A transcript open at its newest message, over Messages\' composer.',
      },
      {
        route: 'recipeContentDesign',
        path: '/recipes/content-design',
        source: 'recipes/content-design.md',
        description: 'Chrome is the platform\'s; the content is yours: rows, grouping, where actions sit.',
      },
      {
        route: 'recipeDatesAndDays',
        path: '/recipes/dates-and-days',
        source: 'recipes/dates-and-days.md',
        description: 'The date is time.epochAtZero + performanceNow(), with the reserved exactTime() source.',
      },
      {
        route: 'recipeDevice',
        path: '/recipes/device',
        source: 'recipes/device.md',
        description: '@exact/device (LLP 1133.031) answers three live facts with the web\'s names.',
      },
      {
        route: 'recipeEditSheet',
        path: '/recipes/edit-sheet',
        source: 'recipes/edit-sheet.md',
        description: 'A row that opens its item\'s sheet is a link: a tap, a deep link and Back all arrive at follow.',
      },
      {
        route: 'recipeEditableRows',
        path: '/recipes/editable-rows',
        source: 'recipes/editable-rows.md',
        description: 'Rows added and removed while editing: each row\'s remove is a red minus-circle and a swipe; Add is the section\'s last row.',
      },
      {
        route: 'recipeErrorAlert',
        path: '/recipes/error-alert',
        source: 'recipes/error-alert.md',
        description: 'A failure the person must acknowledge is the platform\'s one-button alert: a dialog role="alertdialog" with its text and one closing button.',
      },
      {
        route: 'recipeFetchLoadingError',
        path: '/recipes/fetch-loading-error',
        source: 'recipes/fetch-loading-error.md',
        description: 'A fetched resource with loading, an error and Retry, whose rows belong to the owner that asked for them.',
      },
      {
        route: 'recipeFilesAndGrants',
        path: '/recipes/files-and-grants',
        source: 'recipes/files-and-grants.md',
        description: 'Where an app keeps what it keeps, and the line of app.ts\'s grants that reaches it:',
      },
      {
        route: 'recipeFirstLaunch',
        path: '/recipes/first-launch',
        source: 'recipes/first-launch.md',
        description: 'A welcome shown until the person taps Continue, then never again.',
      },
      {
        route: 'recipeFoldables',
        path: '/recipes/foldables',
        source: 'recipes/foldables.md',
        description: 'A foldable (the iPhone Duo) splits its inner display at the hinge when partially folded.',
      },
      {
        route: 'recipeFunctions',
        path: '/recipes/functions',
        source: 'recipes/functions.md',
        description: 'Every function an expression may call; bun exact.mjs contract vocab <name> has one\'s rules, or what to write for a name Contract lacks.',
      },
      {
        route: 'recipeGroupedForm',
        path: '/recipes/grouped-form',
        source: 'recipes/grouped-form.md',
        description: 'An entry or settings form: a grouped list with a text field, a quantity, a switch and a date picker.',
      },
      {
        route: 'recipeIosDriving',
        path: '/recipes/ios-driving',
        source: 'recipes/ios-driving.md',
        description: 'Check a screen against a reference with compare, not by eye: bun exact.mjs agent ios captures as a device does and says where they differ.',
      },
      {
        route: 'recipeKeyboardAndFocus',
        path: '/recipes/keyboard-and-focus',
        source: 'recipes/keyboard-and-focus.md',
        description: 'Keep typing or dismiss the keyboard.',
      },
      {
        route: 'recipeListRows',
        path: '/recipes/list-rows',
        source: 'recipes/list-rows.md',
        description: 'A list under a large title and the bar\'s search; its rows swipe to delete and long-press for a menu.',
      },
      {
        route: 'recipeLocalNetwork',
        path: '/recipes/local-network',
        source: 'recipes/local-network.md',
        description: 'A light, a printer or a NAS is found by DNS-SD with @exact/local-network and reached with fetch (LLP 1133.036).',
      },
      {
        route: 'recipeMoney',
        path: '/recipes/money',
        source: 'recipes/money.md',
        description: 'Keep money as a whole number of cents, and format it at the edge with formatNumber(dollars, "currency", "USD").',
      },
      {
        route: 'recipeMotion',
        path: '/recipes/motion',
        source: 'recipes/motion.md',
        description: 'Motion is CSS\'s: transition, keyframes and animation, -exact-exit-animation and -exact-layout-transition.',
      },
      {
        route: 'recipeNativeModule',
        path: '/recipes/native-module',
        source: 'recipes/native-module.md',
        description: 'Sound, MIDI and a transport are built in (guide, apps/metronome): write no module.',
      },
      {
        route: 'recipeNativeViews',
        path: '/recipes/native-views',
        source: 'recipes/native-views.md',
        description: 'A view needing a platform API (a map, the camera) is a Swift view: a hyphenated tag in app.json modules, made by modules/apple/.swift.',
      },
      {
        route: 'recipeNavigationStack',
        path: '/recipes/navigation-stack',
        source: 'recipes/navigation-stack.md',
        description: 'A list pushing a detail, a Cancel/Save sheet, an alert (pattern list-detail).',
      },
      {
        route: 'recipeNfc',
        path: '/recipes/nfc',
        source: 'recipes/nfc.md',
        description: '@exact/nfc is Web NFC\'s NDEF reading and writing (LLP 1133.035).',
      },
      {
        route: 'recipeNumberField',
        path: '/recipes/number-field',
        source: 'recipes/number-field.md',
        description: 'inputmode="decimal" is the iPhone\'s number pad with a decimal point, "numeric" digits only; without one, the full keyboard.',
      },
      {
        route: 'recipeNumericReadout',
        path: '/recipes/numeric-readout',
        source: 'recipes/numeric-readout.md',
        description: 'Use tabular digits for numeric columns or values changing in place, including progress percentages.',
      },
      {
        route: 'recipeOauth',
        path: '/recipes/oauth',
        source: 'recipes/oauth.md',
        description: '@exact/oauth signs a person in and keeps them signed in: PKCE in the system browser, refresh, requests with the token, sign-out.',
      },
      {
        route: 'recipePermissions',
        path: '/recipes/permissions',
        source: 'recipes/permissions.md',
        description: 'A native module that needs the person\'s permission (location here) is asked for in the app\'s own UI, read as a fact, and recovered from in the app\'s own UI.',
      },
      {
        route: 'recipePersistedSetting',
        path: '/recipes/persisted-setting',
        source: 'recipes/persisted-setting.md',
        description: 'A setting the app keeps across launches — a tip percentage, °C or °F — is a root state that ends in persist (LLP 1116 D5).',
      },
      {
        route: 'recipePortingAndRelease',
        path: '/recipes/porting-and-release',
        source: 'recipes/porting-and-release.md',
        description: 'Name the port when you make it (exact new <path> --name and --bundle-id), then a release build and splitting the work.',
      },
      {
        route: 'recipePortsAndBrowsers',
        path: '/recipes/ports-and-browsers',
        source: 'recipes/ports-and-browsers.md',
        description: 'Ports.',
      },
      {
        route: 'recipeProperties',
        path: '/recipes/properties',
        source: 'recipes/properties.md',
        description: 'In app.test.contract, state what should hold across the app\'s interactions:',
      },
      {
        route: 'recipePurchases',
        path: '/recipes/purchases',
        source: 'recipes/purchases.md',
        description: '@exact/purchases sells through the store on iOS, macOS and Android (LLP 1133.015).',
      },
      {
        route: 'recipeRandomAndShuffle',
        path: '/recipes/random-and-shuffle',
        source: 'recipes/random-and-shuffle.md',
        description: 'Rolls in the view: random-rolls.',
      },
      {
        route: 'recipeRandomRolls',
        path: '/recipes/random-rolls',
        source: 'recipes/random-rolls.md',
        description: 'Contract has no ambient randomness (LLP 1027.000 D1): a draw comes from a seed passed to a pure function.',
      },
      {
        route: 'recipeResponsiveColumns',
        path: '/recipes/responsive-columns',
        source: 'recipes/responsive-columns.md',
        description: 'Columns side by side when wide and stacked on a phone: flex items that wrap at a minimum width, as in CSS.',
      },
      {
        route: 'recipeRestBackend',
        path: '/recipes/rest-backend',
        source: 'recipes/rest-backend.md',
        description: 'The REST starter creates a shared-list app and a Bun server:',
      },
      {
        route: 'recipeSequencer',
        path: '/recipes/sequencer',
        source: 'recipes/sequencer.md',
        description: 'Lanes are Tracks in a derive; one sequence plays them on the runner\'s clock, with no action per step.',
      },
      {
        route: 'recipeSignIn',
        path: '/recipes/sign-in',
        source: 'recipes/sign-in.md',
        description: '@exact/sign-in is FedCM\'s identity request on @exact/passkeys\' get (LLP 1133.014).',
      },
      {
        route: 'recipeSnapbackAuth',
        path: '/recipes/snapback-auth',
        source: 'recipes/snapback-auth.md',
        description: 'Use the complete mixed identity app for the schema, Contract, TypeScript and browser tests.',
      },
      {
        route: 'recipeSnapbackTests',
        path: '/recipes/snapback-tests',
        source: 'recipes/snapback-tests.md',
        description: 'A Snapback app needs its backend running before test web.',
      },
      {
        route: 'recipeSnapback',
        path: '/recipes/snapback',
        source: 'recipes/snapback.md',
        description: 'For shared server data, exact new app --snapback writes a guestbook and wires its driver, generated operations and device.',
      },
      {
        route: 'recipeSymbols',
        path: '/recipes/symbols',
        source: 'recipes/symbols.md',
        description: 'image "symbol:<role>" is an icon each host draws from its own set (SF Symbols on Apple, matching outlines elsewhere).',
      },
      {
        route: 'recipeTabs',
        path: '/recipes/tabs',
        source: 'recipes/tabs.md',
        description: 'Use tabs for at least two top-level sections, each with its own route stack.',
      },
      {
        route: 'recipeTamber',
        path: '/recipes/tamber',
        source: 'recipes/tamber.md',
        description: 'An app\'s own colours are its design, app.tamber beside app.contract (LLP 1121).',
      },
      {
        route: 'recipeTests',
        path: '/recipes/tests',
        source: 'recipes/tests.md',
        description: 'app.test.contract drives the app by testId, the same on the web and iOS (bun exact.mjs test web, test ios; verify runs them too).',
      },
      {
        route: 'recipeTheme',
        path: '/recipes/theme',
        source: 'recipes/theme.md',
        description: 'A role (-exact-secondary-label) follows light, dark and Increased Contrast with nothing written.',
      },
      {
        route: 'recipeTimerInAComponent',
        path: '/recipes/timer-in-a-component',
        source: 'recipes/timer-in-a-component.md',
        description: 'Only the root may declare a task (type-child-resource): a timer is the app\'s.',
      },
      {
        route: 'recipeTimerThatSurvivesRelaunch',
        path: '/recipes/timer-that-survives-relaunch',
        source: 'recipes/timer-that-survives-relaunch.md',
        description: 'performanceNow() restarts at 0 on every launch, so a start mark that outlives it is a date, time.epochAtZero + performanceNow().',
      },
      {
        route: 'recipeTimers',
        path: '/recipes/timers',
        source: 'recipes/timers.md',
        description: 'A task is the only timer: every(ms, action) repeats, after(ms, action) fires once, and when cond arms it while it holds.',
      },
      {
        route: 'recipeUsbHidSerial',
        path: '/recipes/usb-hid-serial',
        source: 'recipes/usb-hid-serial.md',
        description: '@exact/serial, @exact/hid and @exact/usb are Web Serial, WebHID and WebUSB (LLP 1133.037).',
      },
      {
        route: 'recipeWebLook',
        path: '/recipes/web-look',
        source: 'recipes/web-look.md',
        description: 'What an app leaves unsaid, the web draws as a well-made web app would; iOS and macOS draw their own controls from the same file.',
      },
      {
        route: 'recipeWebsite',
        path: '/recipes/website',
        source: 'recipes/website.md',
        description: 'A website in Exact: when a page is made, how to serve it, and a setting.',
      },
    ],
  },
  {
    title: 'Site',
    pages: [
      {
        route: 'components',
        path: '/_components',
        source: 'site/fixtures/components.md',
        hidden: true,
        description: 'Every component the site renders, for its tests.',
      },
    ],
  },
  {
    title: 'For AI agents',
    pages: [
      {
        route: 'agents',
        path: '/agents',
        source: 'site/pages/agents.md',
        description: 'Give your coding agent these docs, or let it read any page as Markdown.',
      },
      {
        route: 'contractForAgents',
        path: '/agents/contract-for-agents',
        source: 'contract-for-agents.md',
        split: true,
        description: 'Contract for an agent building an app: lookup by section for what no recipe covers.',
      },
      {
        route: 'agentPitfalls',
        path: '/agents/pitfalls',
        source: 'agent-pitfalls.md',
        split: true,
        description: 'Verified footguns in writing Exact apps, each as symptom, cause and what to do.',
      },
    ],
  },
];

// The home page's For AI agents rows, in its HTML and its Markdown. Each opens a section of pages/agents.md.
export const agentRows: { title: string; detail: string; anchor: string }[] = [
  { title: 'Start prompt', detail: 'The one file to read first', anchor: 'start-prompt' },
  { title: 'Markdown pages', detail: 'Add .md to any page URL', anchor: 'markdown-pages' },
  { title: 'llms.txt', detail: 'Every page in one index', anchor: 'llmstxt' },
  { title: 'Open in your agent', detail: 'Claude, ChatGPT, Cursor, Codex', anchor: 'open-in-your-agent' },
  { title: 'Terminal lookups', detail: "Answers from an app's folder", anchor: 'terminal-lookups' },
  { title: 'Feedback', detail: 'Report an error in a page', anchor: 'feedback' },
];
