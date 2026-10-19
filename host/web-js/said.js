// A pressable button its author filled says so (`data-exact-said`, LLP 1129 D1: its hover is a wash of its own ink), as
// element.rs writes it for a fill known when the page is compiled; a bound fill says it when it lands, unless clear.
const clearFill = t => t == null || /^\s*transparent\s*$/i.test(t) || /^\s*rgba?\([^)]*[,/]\s*0(\.0*)?%?\s*\)\s*$/i.test(t);
export function saidFill(e, t) {
  const words = new Set((e.getAttribute("data-exact-said") ?? "").split(" ").filter(Boolean));
  if (clearFill(t)) words.delete("background-color"); else words.add("background-color");
  const said = ["background-color", ...[...words].filter(w => w !== "background-color")].filter(w => words.has(w)).join(" ");
  if (said) e.setAttribute("data-exact-said", said); else e.removeAttribute("data-exact-said");
}
