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
    pages: [
      {
        route: 'startHere',
        path: '/start',
        source: 'start-here.md',
        description: 'Build an app with Exact: the idea, the workflow, and the one screen every app starts from.',
      },
    ],
  },
  {
    title: 'Learn',
    pages: [
      {
        route: 'contractForHumans',
        path: '/learn/contract-for-humans',
        source: 'contract-for-humans.md',
        split: true,
        description: 'Contract, the language of an Exact app, from the first component to delivery.',
      },
    ],
  },
  {
    title: 'Recipes',
    pages: [
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
];
