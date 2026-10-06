"""Owned fixture: record actual committed PTY bytes; never execute input."""
import json
import os
import termios
import time
import tty
from pathlib import Path

log = Path(__file__).with_name('manual-ime-pty.ndjson')
original = termios.tcgetattr(0)
try:
    tty.setraw(0)
    with log.open('w') as output:
        output.write(json.dumps({'event': 'ready', 'timeNs': time.time_ns()}) + '\n')
        output.flush()
        os.write(1, b'\r\nMANUAL_IME_READY - physical Korean input is recorded, not executed.\r\n')
        while True:
            data = os.read(0, 8192)
            if not data:
                break
            output.write(json.dumps({'event': 'input', 'timeNs': time.time_ns(), 'hex': data.hex()}) + '\n')
            output.flush()
            if b'\x04' in data:
                break
            os.write(1, data)
        output.write(json.dumps({'event': 'closed', 'timeNs': time.time_ns()}) + '\n')
finally:
    termios.tcsetattr(0, termios.TCSADRAIN, original)
    os.write(1, b'\r\nMANUAL_IME_RECORDER_CLOSED\r\n')
