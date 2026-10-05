import json,pathlib,datetime,hashlib
p=pathlib.Path(__file__).parent
rows={r['label']:r['value'] for r in (json.loads(l) for l in (p/'wire.log').read_text().splitlines() if l.startswith('{'))}
assert 'FAIL' not in rows
assert rows['window-facts']['active'] and any(w['key'] and w['main'] and w['visible'] and w['occluded'] for w in rows['window-facts']['windows'])
def stamps(prefix,name):return [next(v['checkedAt'] for v in rows[f'{prefix}-{i}'] if v['instanceId']==name) for i in range(4)]
for provider in ['codex','claudeAgent']:
 baseline,active,background=[stamps(k,provider) for k in ['baseline','active','background']]
 assert len(set(baseline))==1,(provider,baseline)
 assert len(set(active))>=3,(provider,active)
 assert len(set(background))==1,(provider,background)
 print(provider,'baseline stable; active advances; background stable',json.dumps({'baseline':baseline,'active':active,'background':background}))
assert all(rows[f'policy-a-{i}']['activeForegroundLeaseCount']==1 for i in range(4))
assert all(rows[f'background-policy-{i}']['activeForegroundLeaseCount']==0 for i in range(4))
assert rows['fleet-off']['value']['stopped'] is True
assert rows['fleet-off-status']['value']['state']=='disconnected'
b_on=rows['policy-b-on']['value']['leases'];b_off=rows['policy-b-off']['leases'];b_back=rows['policy-b-reconnected']['value']['leases']
assert [x['updatedAt'] for x in b_on]==[x['updatedAt'] for x in b_off]
assert max(x['updatedAt'] for x in b_back)>max(x['updatedAt'] for x in b_off)
assert rows['active-connect']['descriptor']['environmentId']!=rows['fleet-connect']['value']['descriptor']['environmentId']
reference={k:v for r in (json.loads(l) for l in (p/'reference-payload.log').read_text().splitlines()) for k,v in r.items()}
for key,label in [('active','diagnostics-on'),('background','background-policy-3')]:
 lease=rows[label]['leases'][0];r=reference[key]
 for field in ['clientKind','visible','focused','recentlyInteracted','appState']:assert lease[field]==r[field],(key,field)
 assert sorted(x['type'] for x in lease['scopes'])==sorted(x['type'] for x in r['scopes'])
 dt=lambda s:datetime.datetime.fromisoformat(s.replace('Z','+00:00'))
 assert (dt(lease['expiresAt'])-dt(lease['updatedAt'])).total_seconds()*1000==r['ttlMs']
assert rows['diagnostics-off']['activeScopeKeys']==['provider-status']
for file,digest in json.loads((p/'captured-source.json').read_text()).items():
 assert hashlib.sha256((pathlib.Path('examples/macos/t3-code')/file).read_bytes()).hexdigest()==digest,file
print('PASS: actual native foreground/background gate, two-server fleet lifecycle, diagnostics scope and reference payload semantics. Root Contract retry and main GUI effects remain separate checks.')
