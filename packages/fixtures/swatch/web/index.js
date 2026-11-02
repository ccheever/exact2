// @fixture/swatch on the web: a box filled with its `tint`, which sends
// `message` "tap" when pressed, made by the session's one instance, which
// keeps what it was told of its views and answers it as `swatchReports`.

const say = (heard, line, changed) => { heard.push(line); changed('swatch.swatchReports'); };

class SwatchBox {
  constructor({ implementation, element, props, events, view }) {
    this.fill = element.ownerDocument.createElement('div');
    this.fill.style.cssText = 'width:100%;height:100%;box-sizing:border-box;border-radius:8px';
    this.fill.addEventListener('click', () => events.message('tap'));
    element.append(this.fill);
    this.setProps(props);
    implementation.made(view);
  }
  setProps(props) { this.fill.style.background = props.tint ?? ''; this.fill.textContent = props.caption ?? ''; }
  destroy() { this.fill.remove(); }
}

export const roster = { 'swatch-box': { view: SwatchBox } };

export function implementation({ changed }) {
  const heard = [];
  return {
    swatchReports: () => ({ text: heard.join('; '), count: heard.length }),
    made: (view) => say(heard, `made ${view}`, changed),
    displayed: (view, shown, origin) => say(heard, `displayed ${view} ${shown} ${origin}`, changed),
    bound: (view, prop, origin) => say(heard, `bound ${view} ${prop} ${origin}`, changed),
  };
}
