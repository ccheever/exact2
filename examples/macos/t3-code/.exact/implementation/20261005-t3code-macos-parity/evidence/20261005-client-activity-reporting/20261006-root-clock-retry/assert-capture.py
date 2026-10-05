import pathlib,json,hashlib
p=pathlib.Path(__file__).parent
r=json.loads((p/'capture.json').read_text());initial,completed,before,due=r['captures']
assert initial['clock']==0 and initial['workspaceDiscovery']['needed']
assert completed['clock']==1000 and completed['slots']['workspaceRetryAt']==11000
assert completed['slots']['workspaceRefreshed']['retry'] is True
assert before['clock']==10000 and before['slots']['workspaceRetryAt']==11000
assert before['requests']==completed['requests'],'retry sent before10s deadline'
assert due['clock']==11000 and due['slots']['workspaceRetryAt']==21000
assert len(due['requests'])==len(before['requests'])+1,'exactly one retry required'
assert due['requests'][:-1]==before['requests']
assert due['requests'][-1]['frame']['payload']==completed['requests'][-1]['frame']['payload']
assert due['requests'][-1]['frame']['payload']['instanceId']=='codex'
assert due['slots']['workspaceRefreshed']['key']==completed['slots']['workspaceRefreshed']['key']
assert not any(x['name']=='workspaceRefreshed' for x in due['pending'])
for f,h in r['source'].items():assert hashlib.sha256((pathlib.Path('examples/macos/t3-code')/f).read_bytes()).hexdigest()==h,f
print('PASS actual app root mutation completion1000 -> no retry+9000 -> exactly one retry+10000; next deadline21000; source unchanged.')
