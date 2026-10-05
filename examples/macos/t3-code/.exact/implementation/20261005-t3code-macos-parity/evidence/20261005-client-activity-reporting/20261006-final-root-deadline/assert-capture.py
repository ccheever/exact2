import json,pathlib,hashlib
p=pathlib.Path(__file__).parent
r=json.loads((p/'capture.json').read_text());result=json.loads((p/'result.json').read_text())
steps=r['steps'];start=next(x for x in steps if x['name']=='initial-completion');early=next(x for x in steps if x['name']=='before-deadline');due=steps[-1]
assert result['passed'] and start['clock']==21000
assert start['workspaceDiscovery']['needed'] and start['slots']['workspaceRefreshed']['retry']
assert start['slots']['workspaceRetryAt']==start['clock']+10000
assert early['clock']==start['clock']+9000 and early['requests']==start['requests']
assert due['clock']==start['clock']+10000 and due['slots']['workspaceRetryAt']==due['clock']+10000
assert due['requests'][:-1]==start['requests']
assert len(due['requests'])==len(start['requests'])+1
assert due['requests'][-1]['frame']['payload']==start['requests'][-1]['frame']['payload']
assert due['requests'][-1]['frame']['payload']['instanceId']=='codex'
assert not any(x['name']=='workspaceRefreshed' for x in due['pending'])
for f,h in json.loads((p/'captured-source.json').read_text()).items():assert hashlib.sha256((pathlib.Path('examples/macos/t3-code')/f).read_bytes()).hexdigest()==h,f
print('PASS final activity sources: completion21000; no retryat30000; exactlyone at31000; nextdeadline41000.')
