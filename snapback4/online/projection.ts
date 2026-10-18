/** Project a backend row onto a finite Contract shape. Keys must cover every
 * target field, and the row must supply those fields with the target's types. */
export function pick<T extends object>() {
  return <const K extends readonly (keyof T)[]>(row: T, ...keys: K & ([Exclude<keyof T, K[number]>] extends [never] ? unknown : ['pick: include every target field'])): T => {
    return Object.fromEntries(keys.map(key => [key, row[key]])) as T;
  };
}
