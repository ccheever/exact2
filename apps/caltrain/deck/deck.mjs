if (!new URLSearchParams(location.search).has('attacker')) {
  const { ready } = await import('./deck-chunk.mjs');
  parent.postMessage(await ready(), '*');
}
