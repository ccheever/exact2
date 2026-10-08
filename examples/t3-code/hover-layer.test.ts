// fix-hover-cards: the window's hover layer (hover-layer.contract), read from the Contract sources as
// dialog-focus.test.ts reads them. T3 Code portals every TooltipPopup and PopoverPopup to the body, so
// no scroll area or clipping card around a trigger cuts them, and an openOnHover popover stays while
// the pointer crosses into it (safePolygon) and for its closeDelay after it leaves. These checks guard
// the wiring: the triggers hand their frame to the root's `hoverTipAt`, the layer draws at the window
// level, the root times the close delays. The behavior is proven by the macOS drives and the
// real-pointer rows in tasks/20261008-fix-hover-cards.md.
import { describe, expect, test } from 'bun:test';

const source = (file: string) => Bun.file(new URL(`./${file}`, import.meta.url)).text();
/** The lines of `component name` in `file`, up to the next top-level declaration. */
async function component(file: string, name: string): Promise<string> {
  const lines = (await source(file)).split('\n');
  const start = lines.findIndex(line => line === `component ${name}`);
  if (start < 0) throw new Error(`${file}: no component ${name}`);
  const end = lines.findIndex((line, index) => index > start && /^\S/.test(line) && !line.startsWith('//'));
  return lines.slice(start, end < 0 ? undefined : end).join('\n');
}
/** The `hoverTipAt(hoverTipAtFrame(…))` call of a component: its key, kind, side, align, frame id and testId. */
function tipCall(body: string) {
  const call = /hoverTipAt\(hoverTipAtFrame\((`[^`]*`|"[^"]*"|\w+), "(\w+)", .*?, "(top|bottom)", "(start|center|end)", frame\((`[^`]*`|"[^"]*"|\w+)\), .*?, (`[^`]*`|"[^"]*")\), "trigger", (\w+)\)/.exec(body);
  if (!call) throw new Error('no hoverTipAt(hoverTipAtFrame(...)) call');
  const [, key, kind, side, align, frame, testId, inside] = call;
  return { key, kind, side, align, frame, testId, inside };
}

describe('#251: the provider update icon tooltip is drawn by the window layer, above the list and editor scroll areas', () => {
  test('ProviderVersionAdvisory draws no tooltip beside the icon; its hover hands the icon frame to the layer, side top, centred', async () => {
    const body = await component('providers-upkeep.contract', 'ProviderVersionAdvisory');
    expect(body).not.toContain('CnTooltip(tip=advisory.title');
    expect(body).toContain('inject\n    rem: number\n    hoverTipAt: action');
    expect(tipCall(body)).toEqual({ key: '`${advisory.popId}-tip`', kind: 'tip', side: 'top', align: 'center', frame: '`${advisory.popId}-trigger`', testId: '`${advisory.popId}-tip`', inside: 'inside' });
    // The icon's hover shows it and a press closes it (Base UI's closeOnClick).
    expect(body).toContain('action hover(inside: bool)\n    over = inside\n    tip(inside)');
    expect(body).toContain('action pressed\n    over = false\n    tip(false)');
  });
});

describe('#248: "N scopes" and the other Authorized clients tooltips are drawn by the window layer', () => {
  test('NetScopes draws no card itself: hover and focus hand the count frame to the layer as a hover card, side top, align start', async () => {
    const body = await component('connections-network.contract', 'NetScopes');
    expect(body).not.toContain('"Granted scopes" font-size');
    expect(body).not.toContain('when open');
    const call = tipCall(body);
    expect(call).toEqual({ key: 'tipId', kind: 'scopes', side: 'top', align: 'start', frame: 'tipId', testId: '`${tipId}-popup`', inside: 'over' });
    expect(body).toContain('map(scopes, (scope) => scope.text)');
    expect(body).toMatch(/button id=tipId focus=show\(true\) blur=show\(false\)/);
  });

  test('the status dot, the expiry and the share URL in the list hand their frames to the layer too', async () => {
    const dot = await component('connections-network.contract', 'NetDot');
    expect(dot).not.toContain('CnTooltip(');
    expect(tipCall(dot)).toMatchObject({ kind: 'tip', side: 'top', align: 'center', frame: 'tipId', testId: '`${tipId}-tip`' });
    const row = await component('connections-network.contract', 'PairingLinkRow');
    expect(row).not.toContain('CnTooltip(');
    expect(tipCall(row)).toMatchObject({ kind: 'tip', side: 'top', align: 'center', frame: '`pairing-link-${which}-${row.id}`', testId: '`pairing-link-${which}-tip-${row.id}`' });
    expect(row).toContain('hover=hover("expires", row.expiresTip)');
    expect(row).toContain('hover=hover("url", option.url)');
    expect(row).toContain('id=`pairing-link-expires-${row.id}`');
    expect(row).toContain('id=`pairing-link-url-${row.id}`');
    expect(await component('connections-network.contract', 'ClientSessionRow')).not.toContain('CnTooltip(');
  });
});

describe('#262: the base branch hover card is drawn by the window layer and takes the pointer', () => {
  test('PaFreshnessMark draws no hover card beside the mark; its hover hands the mark frame to the layer, side bottom, align start', async () => {
    const body = await component('pages-pr-actions.contract', 'PaFreshnessMark');
    expect(body).not.toContain('when over');
    expect(body).not.toContain('testId="pr-freshness-hover"');
    expect(tipCall(body)).toEqual({ key: '`pr-freshness-${prKey}`', kind: 'freshness', side: 'bottom', align: 'start', frame: '"pr-freshness-mark"', testId: '"pr-freshness-hover"', inside: 'value' });
    expect(body).toContain('button id="pr-freshness-mark" popovertarget="pr-freshness"');
    // The pressed popover keeps its card; its buttons keep nothing in the layer open.
    expect(body).toContain('PaFreshnessCard(actions=actions, scheme=scheme, act=act, keep=stay, testId="pr-freshness-popover")');
  });

  test('the layer draws the open pull request\'s card, and a button in it keeps the card (the host hands the hover to the button)', async () => {
    const window = await source('app-window.contract');
    expect(window).toContain('PaFreshnessCard(actions=prDetail.actions, scheme=viewport.prefersColorScheme, act=prAct, keep=hoverTipAt(hoverTip, "card"), testId=hoverTip.testId)');
    const button = await component('pages-pr-actions.contract', 'PaOutlineButton');
    expect(button).toContain('action hover(value: bool)\n    over = value\n    keep(value)');
    const card = await component('pages-pr-actions.contract', 'PaFreshnessCard');
    expect(card).toContain('keep=keep, testId=`${testId}-${item.key}`');
  });
});

describe('the layer and its timing', () => {
  test('T3Window provides hoverTipAt and draws the layer after every page, panel and overlay, inside the SSH-inert row', async () => {
    const window = await source('app-window.contract');
    expect(window).toMatch(/\n  provide\n    rem = data\.look\.fontSize\n    still = viewport\.prefersReducedMotion\n    hoverTipAt\n/);
    const lines = window.split('\n');
    const overlays = lines.findIndex(line => line.startsWith('        WindowOverlays('));
    const layer = lines.findIndex(line => line.startsWith('          HoverLayer(tip=hoverTip, viewportHeight=viewport.height, hoverAt=hoverTipAt)'));
    const ssh = lines.findIndex(line => line.startsWith('      SshPasswordPrompt('));
    expect(overlays).toBeGreaterThan(0);
    expect(layer).toBeGreaterThan(overlays);
    expect(ssh).toBeGreaterThan(layer);
    expect(lines[layer - 1]).toBe('        when hoverTip.key != ""');
  });

  test('a tooltip takes no pointer; a hover card hears it, its 4 pt sideOffset included, on either side', async () => {
    const body = await component('hover-layer.contract', 'HoverLayer');
    expect(body.match(/column hover=hoverAt\(tip, "card"\) padding-(top|bottom)=4 max-width="20rem" pointer-events="auto"/g)).toHaveLength(2);
    expect(body.match(/column padding-(top|bottom)=4 max-width="20rem" pointer-events="none"\n/g)).toHaveLength(2);
    expect(body).toContain('when card\n');
    // The trigger's side, flipped when the window leaves no room (Base UI's collision flip).
    expect(body).toContain('derive below = tip.side == "bottom" ? tip.y + tip.height + 4 + tip.estimate <= viewportHeight - 5 or tip.y - 4 - tip.estimate < 5 : tip.y - 4 - tip.estimate < 5');
  });

  test('the root closes a tooltip with its trigger, a hover card after the reference closeDelay, and everything on a surface change', async () => {
    const root = await source('app.contract');
    expect(root).toContain('task hoverGrace when hoverTip.key != "" and hoverOn == "" and hoverTip.kind == "scopes" key=hoverLeft // AccessScopeSummary closeDelay={100}\n    after(100, hoverTipClear)');
    expect(root).toContain('task hoverGraceFreshness when hoverTip.key != "" and hoverOn == "" and hoverTip.kind == "freshness" key=hoverLeft // PullRequestBaseFreshnessWarning closeDelay={120}\n    after(120, hoverTipClear)');
    expect(root).toContain('task hoverSurface key=`${settingsOpen}|${settingsRoute}|${utilityPage}|${modal}|${prSelected}`\n    after(1, hoverTipClear)');
    expect(root).toContain(`  action hoverTipAt(tip: HoverTip, part: string, inside: bool)
    if inside
      hoverTip = tip
      hoverOn = part
    else if tip.key == hoverTip.key and hoverOn == part
      hoverOn = ""
      hoverLeft = hoverLeft + 1
      if tip.kind == "tip" // a TooltipPopup closes with its trigger
        hoverTip = noHoverTip()`);
    expect(root).toContain('T3Window(data=data, viewport=viewport, hoverTip=hoverTip, hoverTipAt=hoverTipAt,');
  });
});
