# Web: a history traversal forgoes the shared-element flight

**Found:** 2026-11-03, building an app whose product photo carries a `sharedElement` name on the
shop grid's card and on the product page (each route's content mounted only while shown in a browser,
so the pair leaves and arrives in one commit).

**What happened:** on the JS target, a commit whose flush may hand a name on runs its tree update
inside `document.startViewTransition` (`host/web-js/shared.js`), which the browser calls
asynchronously. The page's history (`host/web/navigation.js` `popped`) dispatches a Back or Forward
to the root's `navigate` handler and judges the result at once: with the update deferred, no router
change had landed, so it logged `Back to "/shop?c=tees" refused: … committed no router change;
restoring the entry` and put the product's entry back, while the deferred update then showed the shop.
The app and the address bar disagreed, and the agent's `back` reported the app kept the route.

**Now:** `navigation.traversing` is true while `popped` dispatches, and `rt.js`'s commit runs the
tree update at once then, forgoing the flight: a Back lands correctly, and only the forward
navigation (a link, a press) flies.

**Open:** a traversal could fly too if `popped` waited for the deferred update before judging it.
That makes `popped` asynchronous: the `echo`/`drain` queue and the agent's `travel` would wait on
the transition's update callback (shared.js `pending`), and `apply` would have to be told apart from
a later commit's. Not done here.
