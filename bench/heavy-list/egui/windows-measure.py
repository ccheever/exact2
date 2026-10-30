"""G6: run in the signed-in Windows desktop; build before running this driver."""
import argparse
import ctypes as C
from ctypes import wintypes as W
import hashlib
import json
import math
import os
from pathlib import Path
import re
import subprocess
import sys
import threading
import time

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / 'target/g6/python-libs'))
import psutil
from PIL import ImageGrab

U = C.WinDLL('user32', use_last_error=True)
D = C.WinDLL('dwmapi', use_last_error=True)
K = C.WinDLL('kernel32', use_last_error=True)
U.SetProcessDpiAwarenessContext.argtypes = [W.HANDLE]
U.SetProcessDpiAwarenessContext(W.HANDLE(-4))
U.GetForegroundWindow.restype = W.HWND
U.GetWindowThreadProcessId.argtypes = [W.HWND, C.POINTER(W.DWORD)]
U.SetForegroundWindow.argtypes = [W.HWND]
U.ShowWindow.argtypes = [W.HWND, C.c_int]
U.GetWindowRect.argtypes = [W.HWND, C.POINTER(W.RECT)]
U.GetClientRect.argtypes = [W.HWND, C.POINTER(W.RECT)]
U.ClientToScreen.argtypes = [W.HWND, C.POINTER(W.POINT)]
U.PostMessageW.argtypes = [W.HWND, W.UINT, W.WPARAM, W.LPARAM]
U.GetDpiForWindow.argtypes = [W.HWND]
K.QueryPerformanceCounter.argtypes = [C.POINTER(C.c_longlong)]
K.QueryPerformanceFrequency.argtypes = [C.POINTER(C.c_longlong)]
freq = C.c_longlong()
K.QueryPerformanceFrequency(C.byref(freq))

def qpc():
    value = C.c_longlong()
    K.QueryPerformanceCounter(C.byref(value))
    return value.value


def window(pid):
    found = []
    callback = C.WINFUNCTYPE(W.BOOL, W.HWND, W.LPARAM)
    @callback
    def visit(hwnd, _):
        owner = W.DWORD()
        U.GetWindowThreadProcessId(hwnd, C.byref(owner))
        if owner.value == pid and U.IsWindowVisible(hwnd):
            rect = W.RECT()
            U.GetClientRect(hwnd, C.byref(rect))
            if rect.right >= 300 and rect.bottom >= 200:
                found.append(hwnd)
        return True
    U.EnumWindows(visit, 0)
    return found[0] if found else None


def dimensions(hwnd):
    outer, client = W.RECT(), W.RECT()
    U.GetWindowRect(hwnd, C.byref(outer))
    U.GetClientRect(hwnd, C.byref(client))
    return dict(outer=[outer.left, outer.top, outer.right, outer.bottom],
                client=[client.right, client.bottom], dpi=U.GetDpiForWindow(hwnd))


def snapshot(process, tid):
    cpu = process.cpu_times()
    thread = next(t for t in process.threads() if t.id == tid)
    memory = process.memory_info()
    return dict(qpc=qpc(), processCpuMs=(cpu.user+cpu.system)*1000,
                mainCpuMs=(thread.user_time+thread.system_time)*1000,
                workingSet=memory.rss, peakWorkingSet=memory.peak_wset)


COMPILERS = {'cargo.exe','rustc.exe','swiftc.exe','clang.exe','clang++.exe',
             'xcodebuild.exe','metal.exe','swift-frontend.exe','cl.exe','link.exe','fxc.exe'}

def quiet_sample():
    compilers = []
    for p in psutil.process_iter(['pid','name']):
        if (p.info['name'] or '').lower() in COMPILERS:
            compilers.append(p.info)
    cpu = psutil.cpu_percent()
    return dict(time=time.time(), cpuPercent=cpu, compilers=compilers,
                quiet=not compilers and cpu < 10.)


def record(out, value):
    with out.open('a', encoding='utf8') as f:
        f.write(json.dumps(value)+'\n')


def wait_quiet(out):
    psutil.cpu_percent()
    deadline = time.monotonic()+1200
    while time.monotonic() < deadline:
        time.sleep(5)
        row = quiet_sample()
        record(out/'quiet.jsonl', row)
        if row['quiet']:
            return
    raise RuntimeError('quiet condition did not arrive within 20 minutes')


def run(arm, number, out, full, presentmon, capture=False):
    name = f'{arm}-{number}'
    env = os.environ.copy()
    for key in list(env):
        if key.startswith(('EXACT_', 'G4_', 'G1_', 'BENCH_', 'WGPU_')):
            env.pop(key)
    # Both winit windows request the same client; Windows adds its nonclient frame.
    env.update(BENCH_LIVE='0', BENCH_START_INDEX='0',
               EXACT_SIZE='784x761', EXACT_WINDOW_WIDTH='784', EXACT_WINDOW_HEIGHT='793',
               BENCH_DATA=str(ROOT/'bench/heavy-list/data'),
               EXACT_AGENT_WINDOW_PROBE='on', EXACT_AGENT_STORAGE='g6-'+arm,
               WGPU_BACKEND='dx12')
    binary = ROOT/('target/release/egui-heavy-list.exe' if arm == 'egui' else
                  'bench/heavy-list/exact-heavylist/dist-windows/Heavy list.exe')
    if arm == 'wgpu':
        env['EXACT_INK_DEVICE']='wgpu'
    samples = dict(arm=arm, number=number, launchWallUnix=time.time(), binary=str(binary),
                   sha256=hashlib.sha256(binary.read_bytes()).hexdigest())
    log_path = out/(name+'.log')
    with log_path.open('wb') as log:
        child = subprocess.Popen([str(binary)], env=env, cwd=binary.parent,
                                 stdout=log, stderr=subprocess.STDOUT)
        record(out/'pids.jsonl', dict(name=name, pid=child.pid))
        process = psutil.Process(child.pid)
        pm = None
        try:
            deadline = time.monotonic()+30
            hwnd = None
            while time.monotonic() < deadline:
                if child.poll() is not None:
                    raise RuntimeError(f'{name}: exited {child.returncode} before readiness')
                hwnd = hwnd or window(child.pid)
                if hwnd:
                    U.SetForegroundWindow(hwnd)
                text = log_path.read_text(errors='replace')
                match = re.search(r'first-screen compositor sync ([0-9.]+) ms', text)
                if match and hwnd:
                    samples['dwmMs']=float(match[1])
                    displayed = re.search(r'process to displayed first screen ([0-9.]+) ms', text)
                    samples['dxgiMs']=float(displayed[1]) if displayed else None
                    break
                time.sleep(.02)
            else:
                raise RuntimeError(f'{name}: readiness timeout')
            samples['dimensions']=dimensions(hwnd)
            assert samples['dimensions']['dpi'] == 96, samples['dimensions']
            assert [samples['dimensions']['outer'][2]-samples['dimensions']['outer'][0],
                    samples['dimensions']['outer'][3]-samples['dimensions']['outer'][1]] == [800,800], samples['dimensions']
            tid = U.GetWindowThreadProcessId(hwnd, None)
            samples['pid'], samples['mainThread'] = child.pid, tid
            if capture:
                time.sleep(1)
                ImageGrab.grab(bbox=tuple(samples['dimensions']['outer'])).save(out/(name+'-top.png'))
            if full:
                if presentmon:
                    pm_log = (out/(name+'-presentmon.log')).open('wb')
                    pm = subprocess.Popen([str(ROOT/'target/g6/PresentMon.exe'), '--process_id',str(child.pid),
                        '--output_file',str(out/(name+'-presents.csv')), '--qpc_time','--v1_metrics',
                        '--no_console_stats','--no_track_gpu','--no_track_input',
                        '--set_circular_buffer_size','65536','--timed','23','--terminate_after_timed', '--session_name','ExactG6-'+name+'-'+str(child.pid)],
                        stdout=pm_log,stderr=subprocess.STDOUT)
                    record(out/'pids.jsonl', dict(name=name+'-presentmon', pid=pm.pid))
                # Establish the ETW session and let startup notifications settle before endpoints.
                time.sleep(1)
                if pm and pm.poll() is not None:
                    raise RuntimeError(f'PresentMon exited {pm.returncode}')
                U.SetForegroundWindow(hwnd)
                point = W.POINT(392,400)
                U.ClientToScreen(hwnd,C.byref(point))
                U.SetCursorPos(point.x,point.y)
                samples['flingStart']=snapshot(process,tid)
                begin=time.perf_counter()
                posts=[]
                windows=[]
                peak=process.memory_info().rss
                focus=[]
                for gesture in range(4):
                    while time.perf_counter()-begin < gesture*2.5:
                        time.sleep(.01)
                    start=qpc()
                    # Fractional WM_MOUSEWHEEL notches, one post per DWM refresh.
                    # The integral is 600 logical pixels at Windows' 3 lines × 16 px.
                    sent=0
                    gesture_begin=time.perf_counter()
                    while True:
                        if D.DwmFlush() < 0:
                            raise RuntimeError('display clock DwmFlush failed')
                        elapsed=time.perf_counter()-gesture_begin
                        target=round(1500*(1-math.exp(-6*min(elapsed,1)))/(1-math.exp(-6)))
                        delta=target-sent
                        assert U.PostMessageW(hwnd,0x020A,((-delta)&65535)<<16,
                                              (point.y&65535)<<16 | (point.x&65535))
                        posts.append(dict(qpc=qpc(), delta=-delta,gesture=gesture))
                        sent=target
                        if U.GetForegroundWindow()!=hwnd:
                            focus.append(qpc())
                        peak=max(peak,process.memory_info().rss)
                        if elapsed>=1: break
                    windows.append([start,qpc()])
                while time.perf_counter()-begin<10:
                    peak=max(peak,process.memory_info().rss)
                    time.sleep(.01)
                samples['flingEnd']=snapshot(process,tid)
                samples['sampledPeakFling']=peak
                samples['posts'],samples['gestureWindows']=posts,windows
                rest_end=time.perf_counter()+10
                while time.perf_counter()<rest_end:
                    if U.GetForegroundWindow()!=hwnd: focus.append(qpc())
                    time.sleep(.05)
                samples['restEnd']=snapshot(process,tid)
                samples['focusLosses']=focus
                if capture:
                    ImageGrab.grab(bbox=tuple(samples['dimensions']['outer'])).save(out/(name+'-end.png'))
            samples['qpcFrequency']=freq.value
            samples['status']='ok'
        except Exception as error:
            samples['status']='failed'
            samples['error']=str(error)
            raise
        finally:
            if child.poll() is None:
                if hwnd: U.PostMessageW(hwnd,0x0010,0,0)
                try: child.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    child.terminate(); child.wait()
            samples['exit']=child.returncode
            samples['endWallUnix']=time.time()
            if pm:
                try: pm.wait(timeout=8)
                except subprocess.TimeoutExpired:
                    pm.terminate(); pm.wait()
                samples['presentmonExit']=pm.returncode
                pm_log.close()
                pm_text=(out/(name+'-presentmon.log')).read_text(encoding='utf-16')
                samples['presentmonEventsLost']=sum(int(v) for v in re.findall(r'(\d+) ETW events were lost',pm_text))
            (out/(name+'.json')).write_text(json.dumps(samples,indent=2))
    return samples


def main():
    parser=argparse.ArgumentParser()
    parser.add_argument('--out',required=True)
    parser.add_argument('--diagnostic',action='store_true')
    parser.add_argument('--cold',action='store_true')
    parser.add_argument('--startup-only',action='store_true')
    parser.add_argument('--fling-only',action='store_true')
    parser.add_argument('--arm',choices=['d3d11','wgpu','egui'])
    args=parser.parse_args()
    out=Path(args.out).resolve(); out.mkdir(parents=True,exist_ok=True)
    os.chdir(ROOT)
    if args.diagnostic:
        run(args.arm or 'egui',0,out,True,True,True)
        return
    wait_quiet(out)
    stop=threading.Event()
    def monitor():
        psutil.cpu_percent()
        while not stop.wait(5): record(out/'quiet.jsonl',quiet_sample())
    observer=threading.Thread(target=monitor); observer.start()
    try:
        arms=[args.arm] if args.arm else ['d3d11','wgpu','egui']
        for number in range(1,6 if args.fling_only else 11):
            offset=(number-1)%len(arms)
            order=arms[offset:]+arms[:offset]
            for arm in order:
                if args.cold:
                    with (out/f'{arm}-{number}-reset.log').open('wb') as reset_log:
                        subprocess.run(['powershell','-NoProfile','-ExecutionPolicy','Bypass','-File',
                            str(Path(__file__).with_name('windows-reset-cache.ps1'))],
                            stdout=reset_log,stderr=subprocess.STDOUT,check=True,timeout=30)
                result=run(arm,number,out,number<=5 and not (args.cold or args.startup_only),True)
                print(arm,number,result['dwmMs'],flush=True)
                if result.get('focusLosses'): raise RuntimeError('focus lost')
                if args.cold or args.startup_only or number>5:
                    # Keep startup bursts apart and observe the final launch's quiet interval.
                    time.sleep(5)
    finally:
        stop.set(); observer.join()

if __name__=='__main__':
    main()
