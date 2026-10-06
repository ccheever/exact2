import sys,os,tty,termios,select,json
p=os.path.join(os.path.dirname(__file__),'raw-keys.ndjson')
old=termios.tcgetattr(0)
try:
 tty.setraw(0);print('RAW_READY',flush=True)
 while True:
  data=os.read(0,8192)
  with open(p,'a') as f:f.write(json.dumps({'hex':data.hex(),'text':data.decode('utf-8','replace')})+'\n')
  if b'\x04' in data:break
finally:termios.tcsetattr(0,termios.TCSADRAIN,old)
