// The module from data code only, in a file `app.ts` imports: the bake finds
// it here (LLP 1133.004 stage 0), and its send that answers nothing resolves
// `undefined`.
import { silence } from '@fixture/bell';

export async function quiet(): Promise<{ done: boolean }> {
  const said = await silence({ tag: 't1' });
  return { done: said === undefined };
}
