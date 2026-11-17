#!/usr/bin/env python3
"""Where a process's memory is, by kind of mapping (docs/gate.md, "Memory").

    tools/gate/memmap.py <pid> [label]      # one JSON line on stdout

Reads /proc/<pid>/smaps (read-only; any process of this user) and sums
Rss, Pss, Anonymous, AnonHugePages and Swap per kind:

- heap: the main malloc arena ([heap], brk)
- arenas: glibc's other malloc arenas (64 MiB-aligned anonymous heaps,
  at most 64 MiB each; the reserved rest is ---p and holds nothing)
- stacks: thread stacks (8 MiB-ish anonymous rw with a guard page below)
- anon: other anonymous memory (big malloc blocks over the mmap threshold,
  the GPU driver's own mappings, Slint/femtovg buffers)
- gpu: /dev/dri, /dev/nvidia*, memfd/anon_inode/shm (DRM buffers, GL)
- fonts: font files and fontconfig caches
- bin: the binary itself
- libs: every other file (shared libraries: code is shared, data is anon)
plus the totals and /proc/<pid>/status's VmRSS, RssAnon, RssFile, VmSwap.
"""
import json
import re
import sys

ARENA = 64 << 20


def kind(start, end, perms, name, prev_guard):
    size = end - start
    if name == "[heap]":
        return "heap"
    if name.startswith("/dev/dri") or name.startswith("/dev/nvidia") or name.startswith("/memfd:") \
            or name.startswith("anon_inode:") or name.startswith("/dev/shm") or name.startswith("/SYSV"):
        return "gpu"
    if re.search(r"\.(ttf|otf|ttc|pfb|woff2?)$", name, re.I) or "fontconfig" in name:
        return "fonts"
    if name == "[stack]":
        return "stacks"
    if name.startswith("[") or name == "":
        if name not in ("",):
            return "other"
        if start % ARENA == 0 and size <= ARENA and "rw" in perms:
            return "arenas"
        if prev_guard and "rw" in perms and (1 << 20) <= size <= (16 << 20):
            return "stacks"
        return "anon"
    if name.endswith("/tessera-ui") or "/tessera-ui " in name:
        return "bin"
    return "libs"


def read(pid):
    out = {}
    head = re.compile(r"^([0-9a-f]+)-([0-9a-f]+) (\S+) \S+ \S+ \S+\s*(.*)$")
    cur = None
    prev_guard = False
    last_end = None

    def add(k, field, kb):
        d = out.setdefault(k, {"rss": 0, "pss": 0, "anon": 0, "thp": 0, "swap": 0, "n": 0})
        d[field] += kb

    with open(f"/proc/{pid}/smaps") as f:
        for line in f:
            m = head.match(line)
            if m:
                start, end, perms, name = int(m[1], 16), int(m[2], 16), m[3], m[4].strip()
                guard = perms.startswith("---") and end - start <= (64 << 10) * 4
                cur = kind(start, end, perms, name, prev_guard and last_end == start)
                if perms.startswith("---"):
                    cur = None
                else:
                    out.setdefault(cur, {"rss": 0, "pss": 0, "anon": 0, "thp": 0, "swap": 0, "n": 0})["n"] += 1
                prev_guard, last_end = guard, end
                continue
            if cur is None:
                continue
            k, _, v = line.partition(":")
            field = {"Rss": "rss", "Pss": "pss", "Anonymous": "anon", "AnonHugePages": "thp", "Swap": "swap"}.get(k)
            if field:
                add(cur, field, int(v.split()[0]))
    st = {}
    with open(f"/proc/{pid}/status") as f:
        for line in f:
            k, _, v = line.partition(":")
            if k in ("VmRSS", "RssAnon", "RssFile", "VmSwap", "Threads"):
                st[k] = int(v.split()[0])
    mb = lambda kb: round(kb / 1024, 1)
    kinds = {k: {f: (mb(v) if f != "n" else v) for f, v in d.items()} for k, d in sorted(out.items())}
    tot = {f: mb(sum(d[f] for d in out.values())) for f in ("rss", "pss", "anon", "thp", "swap")}
    return {"kinds": kinds, "total": tot, "status": {k: (mb(v) if k != "Threads" else v) for k, v in st.items()}}


if __name__ == "__main__":
    r = read(int(sys.argv[1]))
    if len(sys.argv) > 2:
        r = {"label": sys.argv[2], **r}
    print(json.dumps(r))
