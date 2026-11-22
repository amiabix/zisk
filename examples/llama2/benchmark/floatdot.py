#!/usr/bin/env python3
"""Proves the llama2 guest with and without FloatDot on one machine, one run at a time.

Usage: floatdot.py ROOT queue [RUN,...]|status|report

ROOT holds fixture/ (llama2-guest.elf, loops.stdin, floatdot.stdin, expected-output.bin).
`queue` runs every stage below in order, skipping finished ones, so it can be restarted:

  pins          sha256 of the ELF, inputs, backend and every proving key file
  check-<mode>  `verify-constraints` (preflight) in a fresh cache, whose ROM setup derives
                the program key from the ELF (`program-setup` also builds the unused
                assembly emulator)
  prove-<run>   `prove` with the settings below; wall/user/sys/peak RSS from wait4,
                RAM and swap sampled every 15 s, process snapshot before
  verify-<run>  `verify` under the leaf key, then the round-trip check of the program
                key from check-<mode> and all 64 expected public outputs

FLOATDOT_CLI selects another cargo-zisk-dev build and FLOATDOT_GPU=1 adds `--gpu` to `prove`;
FLOATDOT_KEY and FLOATDOT_ROUNDTRIP select a copy of the proving key and the round-trip test
binary. FLOATDOT_SETTLE_TIMEOUT_S raises proofman's backstop for
proofs still in flight after the last witness (default 7200 s), which a slow machine can exceed.
Runs alternate the modes: loops-1, floatdot-1, then floatdot-2, loops-2; `queue` takes a
comma-separated subset, in its order, for example `floatdot-1,loops-1` for one pair.
`report` writes runs/results.json from the logs: resources, stage times, instances, steps.
"""
import hashlib, json, os, re, subprocess, sys, threading, time
from pathlib import Path

BACKEND = Path('/home/abix/fp32-floatdot-20261115')
CLI = Path(os.environ.get('FLOATDOT_CLI', BACKEND / 'target/release/cargo-zisk-dev'))
KEY = Path(os.environ.get('FLOATDOT_KEY', BACKEND / 'artifacts/bounded-setup/provingKey'))
LEAF_VK = KEY / 'zisk/vadcop_final/vadcop_final.verkey.json'
# The backend's compiled round-trip test (common/tests/floatdot_roundtrip.rs).
ROUNDTRIP = Path(os.environ.get('FLOATDOT_ROUNDTRIP', BACKEND / 'target/release/deps/floatdot_roundtrip-bab44ceac3ccffa2'))
MAC = sys.platform == 'darwin'
THREADS = '8'
PROVE = ['--random-seed', '0', '--minimal-memory', '--number-threads-witness', THREADS, '--verify-proofs']
GPU = os.environ.get('FLOATDOT_GPU') == '1'
PROVE += ['--gpu'] if GPU else []
MODES = ('loops', 'floatdot')
RUNS = tuple(sys.argv[3].split(',')) if len(sys.argv) > 3 else ('loops-1', 'floatdot-1', 'floatdot-2', 'loops-2')

root = Path(sys.argv[1]).resolve()
fixture, runs = root / 'fixture', root / 'runs'
elf = fixture / 'llama2-guest.elf'


def sha(p):
    h = hashlib.sha256()
    with open(p, 'rb') as f:
        while chunk := f.read(1 << 24):
            h.update(chunk)
    return h.hexdigest()


def env(mode):
    return dict(os.environ, RAYON_NUM_THREADS=THREADS, OMP_NUM_THREADS=THREADS,
                PROOFMAN_SETTLE_TIMEOUT_S=os.environ.get('FLOATDOT_SETTLE_TIMEOUT_S', '7200'), ZISK_HOME=str(root / 'zisk-home'),
                ZISK_CACHE_DIR=str(root / 'cache' / mode),
                LIBRARY_PATH='/opt/intel/oneapi/compiler/2026.0/lib:' + os.environ.get('LIBRARY_PATH', ''))


def sh(cmd):
    return subprocess.run(cmd, shell=True, capture_output=True, text=True).stdout


def mac_memory(pid):
    """macOS: used = active + wired + compressor pages (vm_stat), swap used (vm.swapusage), process RSS."""
    vm = sh('vm_stat')
    page = int(re.search(r'page size of (\d+)', vm)[1])
    pages = dict(re.findall(r'"?([\w -]+?)"?:\s+(\d+)\.', vm))
    used = sum(int(pages[k]) for k in ('Pages active', 'Pages wired down', 'Pages occupied by compressor')) * page // 1024
    m = re.search(r'used = ([\d.]+)M', sh('sysctl vm.swapusage'))
    return used, int(float(m[1]) * 1024), sh(f'ps -o rss= -p {pid}').strip(), ''


def sample(pid, out, stop):
    """RAM and swap of the machine, and RSS and swap of the process, every 15 s."""
    with out.open('w') as f:
        f.write('unix\tmem_used_kib\tswap_used_kib\tproc_rss_kib\tproc_swap_kib' + ('\tgpu_mem_mib\tgpu_util' if GPU else '') + '\n')
        while True:
            if MAC:
                used, swap, rss, pswap = mac_memory(pid)
            else:
                mem = dict(re.findall(r'(\w+):\s+(\d+)', Path('/proc/meminfo').read_text()))
                try:
                    st = dict(re.findall(r'(\w+):\s+(\d+)', Path(f'/proc/{pid}/status').read_text()))
                except FileNotFoundError:
                    st = {}
                used = int(mem['MemTotal']) - int(mem['MemAvailable'])
                swap = int(mem['SwapTotal']) - int(mem['SwapFree'])
                rss, pswap = st.get('VmRSS', ''), st.get('VmSwap', '')
            gpu = '\t' + sh('nvidia-smi --query-gpu=memory.used,utilization.gpu --format=csv,noheader,nounits').strip().replace(', ', '\t') if GPU else ''
            f.write(f"{time.time():.0f}\t{used}\t{swap}\t{rss}\t{pswap}{gpu}\n")
            f.flush()
            if stop.wait(15):
                return


def run(stage, cmd, e, samples=False):
    d = runs / stage
    d.mkdir(parents=True, exist_ok=True)
    snapshot = ('date; uptime; vm_stat; sysctl vm.swapusage; ps -A -r -o pid,pcpu,pmem,rss,etime,comm | head -15' if MAC
                else 'date -Is; uptime; free -k; ps -eo pid,pcpu,pmem,rss,etime,comm --sort=-pcpu | head -15')
    (d / 'before.txt').write_text(sh(snapshot))
    rec = {'stage': stage, 'status': 'running', 'command': list(map(str, cmd)), 'started_unix': time.time()}
    (d / 'status.json').write_text(json.dumps(rec, indent=2) + '\n')
    with (d / 'stdout.log').open('w') as out, (d / 'stderr.log').open('w') as err:
        p = subprocess.Popen(list(map(str, cmd)), env=e, cwd=d, stdout=out, stderr=err)
        rec['pid'] = p.pid
        (d / 'status.json').write_text(json.dumps(rec, indent=2) + '\n')
        stop = threading.Event()
        t = threading.Thread(target=sample, args=(p.pid, d / 'samples.tsv', stop))
        if samples:
            t.start()
        _, raw, ru = os.wait4(p.pid, 0)
        stop.set()
        if samples:
            t.join()
    rc = os.waitstatus_to_exitcode(raw)
    rec.update(status='finished' if rc == 0 else 'failed', exit_code=rc, wall_seconds=time.time() - rec['started_unix'],
               user_seconds=ru.ru_utime, system_seconds=ru.ru_stime,
               max_rss_kib=ru.ru_maxrss // 1024 if MAC else ru.ru_maxrss,
               resource_method='Python os.wait4 child rusage' + (' (macOS ru_maxrss bytes / 1024)' if MAC else ''))
    (d / 'status.json').write_text(json.dumps(rec, indent=2) + '\n')
    if rc:
        raise SystemExit(f'{stage} failed: exit {rc}, see {d}')
    return rec


def done(stage):
    s = runs / stage / 'status.json'
    return s.exists() and json.loads(s.read_text())['status'] == 'finished'


def pins():
    d = runs / 'pins'
    d.mkdir(parents=True, exist_ok=True)
    files = {str(p): sha(p) for p in [elf, *sorted(fixture.glob('*.stdin')), fixture / 'expected-output.bin', CLI, ROUNDTRIP]}
    files.update({str(p): sha(p) for p in sorted(KEY.rglob('*')) if p.is_file()})
    hw = sh('sysctl hw.model hw.memsize hw.ncpu machdep.cpu.brand_string; sw_vers; df -h /System/Volumes/Data; uname -a'
            if MAC else 'lscpu; free -k; df -h /home; uname -a')
    (d / 'pins.json').write_text(json.dumps({'sha256': files, 'hardware': hw}, indent=2) + '\n')
    (d / 'status.json').write_text(json.dumps({'stage': 'pins', 'status': 'finished'}) + '\n')


def check(mode):
    cache = root / 'cache' / mode
    assert not cache.exists() or not any(cache.iterdir()), f'{cache} must start empty'
    run(f'check-{mode}', [CLI, 'verify-constraints', '--elf', elf, '--inputs', fixture / f'{mode}.stdin',
                          '--proving-key', KEY, '--random-seed', '0'], env(mode))
    keys = list(cache.rglob('*.verkey.bin'))
    assert len(keys) == 1, keys
    limbs = [int.from_bytes(keys[0].read_bytes()[i:i + 8], 'little') for i in range(0, 32, 8)]
    (runs / f'check-{mode}' / 'program-vk.json').write_text(json.dumps(limbs) + '\n')


def verify(name):
    mode = name.split('-')[0]
    proof = runs / f'prove-{name}' / 'proof.bin'
    run(f'verify-{name}', [CLI, 'verify', '--proof', proof, '--leaf-setup-vk', LEAF_VK], env(mode))
    out = fixture / 'expected-output.bin'
    words = [int.from_bytes(out.read_bytes()[i:i + 4], 'little') for i in range(0, 256, 4)]
    expected = runs / f'verify-{name}' / 'expected-publics.json'
    expected.write_text(json.dumps({'expected_public_inputs_u32': words}) + '\n')
    e = env(mode)
    e.update(FP32_PROOF=str(proof), FP32_LEAF_SETUP_VK=str(LEAF_VK), FP32_MEASUREMENT=str(expected),
             FP32_PROGRAM_VK=str(runs / f'check-{mode}' / 'program-vk.json'))
    run(f'roundtrip-{name}', [ROUNDTRIP, '--ignored', '--nocapture'], e)


def queue():
    if not done('pins'):
        pins()
    for name in RUNS:
        mode = name.split('-')[0]
        if not done(f'check-{mode}'):
            check(mode)
        if not done(f'prove-{name}'):
            proof = runs / f'prove-{name}' / 'proof.bin'
            assert not proof.exists(), f'keep the earlier {proof}'
            run(f'prove-{name}', [CLI, 'prove', '--elf', elf, '--inputs', fixture / f'{mode}.stdin', '--proving-key', KEY,
                                  *PROVE, '--output', proof], env(mode), samples=True)
        if not done(f'roundtrip-{name}'):
            verify(name)


def report():
    out = {'pins': json.loads((runs / 'pins/pins.json').read_text())['sha256'], 'stages': {}}
    for s in sorted(runs.glob('*/status.json')):
        d, rec = s.parent, json.loads(s.read_text())
        log = (d / 'stdout.log').read_text() if (d / 'stdout.log').exists() else ''
        rec['phases_ms'] = {k: int(v) for k, v in re.findall(r'<<< (\w+) \((\d+)ms\)', log)}
        if m := re.search(r'Zisk \| (.*) \| Total global instances: (\d+)', log):
            rec['instances'] = {k: int(v) for k, v in re.findall(r'(\w+): (\d+)', m[1])}
            rec['instances_total'] = int(m[2])
        if m := re.search(r'steps: (\d+)', log):
            rec['steps'] = int(m[1])
        if m := re.search(r'Proof Time: ([\d.]+) seconds', log):
            rec['reported_proof_seconds'] = float(m[1])
        if (d / 'proof.bin').exists():
            rec['proof_bytes'], rec['proof_sha256'] = (d / 'proof.bin').stat().st_size, sha(d / 'proof.bin')
        if (d / 'samples.tsv').exists():
            rows = [l.split('\t') for l in (d / 'samples.tsv').read_text().splitlines()[1:]]
            rec['samples'] = len(rows)
            for i, k in enumerate(['mem_used_kib', 'swap_used_kib', 'proc_rss_kib', 'proc_swap_kib', 'gpu_mem_mib'], 1):
                rec['max_' + k] = max((int(r[i]) for r in rows if len(r) > i and r[i].strip().isdigit()), default=None)
            rec['swap_used_kib_at_start'] = int(rows[0][2]) if rows else None
        if (d / 'program-vk.json').exists():
            rec['program_vk'] = json.loads((d / 'program-vk.json').read_text())
        out['stages'][rec['stage']] = rec
    (runs / 'results.json').write_text(json.dumps(out, indent=2) + '\n')


if sys.argv[2] == 'queue':
    queue()
elif sys.argv[2] == 'report':
    report()
else:
    for s in sorted(runs.glob('*/status.json')):
        r = json.loads(s.read_text())
        print(r['stage'], r['status'], f"{r.get('wall_seconds', 0):.0f}s", r.get('max_rss_kib', ''))
