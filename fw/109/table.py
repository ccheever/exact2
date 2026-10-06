import json, re, sys
text = open(sys.argv[1]).read().splitlines()
rows, i, tree = [], 0, None
while i < len(text):
    line = text[i]
    m = re.match(r'epoch \d+ · incarnation \d+ · clock (\d+) ms', line)
    if m:
        tree = {'clock': int(m.group(1))}
        j = i + 1
        while j < len(text) and text[j].startswith(' ') or (j < len(text) and text[j].startswith('View#')):
            t = re.search(r'\[(snap|pending)\] "(.*)"$', text[j])
            if t: tree[t.group(1)] = t.group(2)
            j += 1
        i = j; continue
    if line == '{' and tree is not None:
        j = i
        while text[j] != '}': j += 1
        st = json.loads('\n'.join(text[i:j+1]))
        if 'resources' in st:
            p = [x['ticket'] for x in st.get('pending', []) if x['name'] == 'snap']
            snap = st['resources'].get('snap') or {}
            rows.append((tree['clock'], tree.get('snap', '?'), tree.get('pending', '?'), snap.get('n'), p[0] if p else '-'))
            tree = None
        i = j + 1; continue
    i += 1
print('| clock (ms) | `snap` text (tree) | `pending` text | state `snap.n` | state pending ticket |')
print('|---:|---|---|---:|---:|')
for r in rows:
    print(f'| {r[0]} | {r[1] or "(empty)"} | {r[2]} | {r[3]} | {r[4]} |')
