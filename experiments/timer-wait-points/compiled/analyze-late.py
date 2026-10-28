# Every frame the traces of one run sampled (the rings merged): how many applied
# two commits (the frame a `poll` beat ran in, with the frame task's commit), and
# of the late frames, how many came right after such a frame, against the share
# of frames that follow one.  python3 -I analyze-late.py <run dir>
import json, os, sys
d = sys.argv[1]
recs, late = {}, {}
for f in sorted(os.listdir(d)):
    if f.startswith('trace-') and f.endswith('.json'):
        t = json.load(open(os.path.join(d, f)))
        for r in t['frames'].get('records', []): recs[r['t']] = r
        for r in t['frames'].get('late', []): late[r['t']] = r
rs = sorted(recs.values(), key=lambda r: r['t'])
poll = lambda r: r.get('seq') and r['seq'][1] > r['seq'][0]
after = [i for i in range(1, len(rs)) if poll(rs[i - 1])]
lates = [i for i, r in enumerate(rs) if r['missed'] > 0 or r.get('overrun', 0) > 0]
hit = [i for i in lates if i > 0 and poll(rs[i - 1])]
inpoll = [i for i in lates if poll(rs[i])]
print(f"{d}: {len(rs)} frames sampled, {sum(1 for r in rs if poll(r))} applied two commits; {len(lates)} late (missed or overrun)")
print(f"  late frames right after a two-commit frame: {len(hit)} of {len(lates)} (frames that follow one: {len(after)}/{len(rs)-1} = {100*len(after)/max(1,len(rs)-1):.1f}%)")
print(f"  late frames that were themselves two-commit frames: {len(inpoll)}")
for i in lates:
    r = rs[i]; prev = rs[i - 1] if i else None
    ph = ' '.join(f"{p['phase']} {p['ms']}" for p in (r.get('phases') or []))
    print(f"  t={r['t']:.0f} interval {r['interval']:.1f} missed {r['missed']} overrun {r.get('overrun',0):.1f} seq {r.get('seq')} prev-seq {prev.get('seq') if prev else None}{' (after a poll frame)' if i in hit else ''} {ph} unattributed {r.get('unattributed')}")
