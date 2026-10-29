// The sample host's decision on leaving the app (LLP 1133.024 D22 item 7),
// a step of `smoke.mjs host`: a package's ask, posted for one session's view
// as `ExactPackageContext.leave` posts it, answered by the host's delegate
// for that session alone, and refused outside the host's list.
export async function leaveChecks({ s, say, settle, check }) {
  say('leave a https://example.com/page');
  say('leave b https://www.caltrain.com/schedules');
  await settle(500);
  const left = (await s.logs()).host.filter((l) => /leave/.test(l)), all = left.join(' | ');
  check(left.some((l) => /a may not leave for https:\/\/example\.com\/page \(browser\)/.test(l)) && left.some((l) => /leave https:\/\/example\.com\/page from a: refused/.test(l)), `a URL outside the host's list is refused: ${all}`);
  check(left.some((l) => /leave https:\/\/www\.caltrain\.com\/schedules from b: let go/.test(l)) && !left.some((l) => /a may .*caltrain|b may .*example/.test(l)), `one on it is let go, asked of its own session: ${all}`);
}
