#!/usr/bin/env python3
"""Landlock the fresh CLI and all descendants; fail closed without Linux support.

The candidate and installed toolchains are readable. Parent evaluators, other
attempts, session history and the reference comparison are outside the allowlist.
No mount/user namespace or privileges required. Runner owns the policy JSON.
"""
import ctypes
import json
import os
import sys

libc = ctypes.CDLL(None, use_errno=True)

def checked(result):
    if result < 0:
        raise OSError(ctypes.get_errno(), os.strerror(ctypes.get_errno()))
    return result

class Ruleset(ctypes.Structure):
    _fields_ = [('handled_access_fs', ctypes.c_uint64)]

class PathBeneath(ctypes.Structure):
    _pack_ = 1
    _fields_ = [('allowed_access', ctypes.c_uint64), ('parent_fd', ctypes.c_int32)]

policy = json.load(open(sys.argv[1], encoding='utf8'))
abi = checked(libc.syscall(444, 0, 0, 1))
read = (1 << 0) | (1 << 2) | (1 << 3)
handled = (1 << 13) - 1
if abi >= 2:
    handled |= 1 << 13
if abi >= 3:
    handled |= 1 << 14
rules = Ruleset(handled)
fd = checked(libc.syscall(444, ctypes.byref(rules), ctypes.sizeof(rules), 0))
for mode, paths in policy.items():
    for path in paths:
        if not os.path.exists(path):
            continue
        access = handled if mode == 'write' else (1 << 3) if mode == 'list' else read
        if not os.path.isdir(path):
            access &= (1 << 0) | (1 << 1) | (1 << 2) | ((1 << 14) if abi >= 3 else 0)
        parent = os.open(path, os.O_PATH | os.O_CLOEXEC)
        entry = PathBeneath(access, parent)
        checked(libc.syscall(445, fd, 1, ctypes.byref(entry), 0))
        os.close(parent)
checked(libc.prctl(38, 1, 0, 0, 0))
checked(libc.syscall(446, fd, 0))
os.close(fd)
os.execvpe(sys.argv[2], sys.argv[2:], os.environ)
