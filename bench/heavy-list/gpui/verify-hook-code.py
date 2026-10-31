import argparse,hashlib,json,pathlib,struct
parser=argparse.ArgumentParser(description="Compare signed benchmark executables before their signatures.")
parser.add_argument("base",type=pathlib.Path)
args=parser.parse_args()
base=args.base.resolve()
prov=json.loads((base/'provenance.json').read_text()); rows={}
def code(path):
    data=path.read_bytes(); assert data[:4]==b'\xcf\xfa\xed\xfe'
    n=struct.unpack_from('<I',data,16)[0]; pos=32
    for _ in range(n):
        cmd,size=struct.unpack_from('<II',data,pos)
        if cmd==0x1d:
            offset,length=struct.unpack_from('<II',data,pos+8)
            return dict(codeSha256=hashlib.sha256(data[:offset]).hexdigest(),signatureOffset=offset,signatureBytes=length)
        pos+=size
    raise AssertionError('missing code signature')
for arm in ['native','gpui','egui']:
    new=pathlib.Path(prov[arm]['executable']); old=pathlib.Path(prov[arm]['original'])/'Contents/MacOS'/new.name
    rows[arm]=dict(before=code(old),after=code(new))
    assert rows[arm]['before']['codeSha256']==rows[arm]['after']['codeSha256'],rows[arm]
(base/'code-identity.json').write_text(json.dumps(rows,indent=2)+'\n')
print('native, GPUI and egui are byte-identical before the code signature; only signatures changed.')
