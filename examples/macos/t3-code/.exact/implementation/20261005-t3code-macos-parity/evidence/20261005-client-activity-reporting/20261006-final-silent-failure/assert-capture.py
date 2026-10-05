import json,pathlib,hashlib
p=pathlib.Path(__file__).parent
before=json.loads((p/'silent-before.json').read_text());after=json.loads((p/'silent-after.json').read_text())
assert before['connected'] is True and after['connected'] is False
assert after['elapsedMs']>=25000
ids={t['id'] for t in before['toasts']}
assert not after['newToasts'] and all(t['id'] in ids for t in after['toasts'])
assert not any('reportClientActivity' in str(line) and any(s in str(line).lower() for s in ['refus','fail','error']) for line in after['logs'].get('lines',[])+after['logs'].get('host',[]))
assert json.loads((p/'bundle-receipt-check.json').read_text())['changes']==[]
for f,h in json.loads((p/'captured-source.json').read_text()).items():assert hashlib.sha256((pathlib.Path('examples/macos/t3-code')/f).read_bytes()).hexdigest()==h,f
print(f"PASS actual final app disconnected; no newtoasts or activityreportrefusal logs; {after['elapsedMs']}ms observed; source unchanged.")
