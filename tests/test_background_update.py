"""Background updates (docs/releases.md, "Background updates"): the rules,
each on its own, with no network, no real install and no shell:

  native-install.py  per-part keys (tessera-events its own tree), programs
                     kept when their sources didn't change, patches applied
                     and checked (and the whole archive when one doesn't
                     check out), staging beside the installed programs, the
                     build cache (toolchain wipe, pruning, the same flags
                     every time, the machine's own variant), going back to
                     the previous build and marking the new one bad
  tessera-update.py  stage then finish (exactly the staged commit, even when
                     the branch moved on), a checkout that moved since is
                     refused, new conflicts settle safely, every point an
                     update can be cut off at is put back by recover(), a
                     job cut off is seen, a bad version isn't offered, parts
                     rebuilt only when their sources change
  sysupdate.py       which packages ask for a restart of the computer
"""
import hashlib
import importlib.machinery
import importlib.util
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
import time
import unittest
from pathlib import Path
from unittest import mock

ROOT = Path(__file__).resolve().parent.parent
HAS_CRYPTO = importlib.util.find_spec("cryptography") is not None
HAS_ZSTD = shutil.which("zstd") is not None
ID = ["-c", "user.name=Test", "-c", "user.email=test@example.invalid", "-c", "commit.gpgsign=false"]


def load(name, path):
    loader = importlib.machinery.SourceFileLoader(name, str(path))
    spec = importlib.util.spec_from_loader(name, loader)
    mod = importlib.util.module_from_spec(spec)
    loader.exec_module(mod)
    return mod


ni = load("bg_native_install", ROOT / "tools/native-install.py")
tni = load("bg_test_native_install", ROOT / "tests/test_native_install.py")  # (its fixture, its throwaway key)


def git(repo, *args):
    r = subprocess.run(["git", *ID, "-C", str(repo), *args], capture_output=True, text=True)
    if r.returncode != 0:
        raise RuntimeError(" ".join(args) + ": " + r.stderr)
    return r.stdout.strip()


def fake_program(path, said, pad=""):
    path.write_text("#!/bin/sh\n[ \"$1\" = --version ] && echo '" + said + "' && exit 0\nexit 3\n# " + pad + "\n")
    path.chmod(0o755)


# ── native-install.py ─────────────────────────────────────────────────────

class Parts(tni.Fixture):
    def setUp(self):
        super().setUp()
        (self.repo / "native/tessera-events").mkdir(parents=True)
        (self.repo / "native/tessera-events/main.rs").write_text("fn main() {}\n")
        git(self.repo, "add", "-A")
        git(self.repo, "commit", "-qm", "events")
        self.tree, self.commit, _ = ni.native_tree("HEAD", str(self.repo))

    def commit_file(self, path, text):
        p = self.repo / path
        p.parent.mkdir(parents=True, exist_ok=True)
        p.write_text(text)
        git(self.repo, "add", "-A")
        git(self.repo, "commit", "-qm", "change " + path)
        self.tree, self.commit, _ = ni.native_tree("HEAD", str(self.repo))

    def test_each_program_has_its_own_key(self):
        a = ni.part_keys("HEAD", str(self.repo))
        self.assertEqual(a["tessera-events"], git(self.repo, "rev-parse", "HEAD:native/tessera-events"))
        self.commit_file("native/tessera-events/main.rs", "fn main() { 1; }\n")
        b = ni.part_keys("HEAD", str(self.repo))
        self.assertNotEqual(a["tessera-events"], b["tessera-events"])
        self.assertEqual(a["tessera-ui"], b["tessera-ui"], "tessera-ui's sources didn't change")
        # (a file outside its folder that tessera-ui compiles in is its source too)
        self.commit_file("shell/common/Config.qml", "JsonAdapter {}\n")
        c = ni.part_keys("HEAD", str(self.repo))
        self.assertNotEqual(b["tessera-ui"], c["tessera-ui"])
        self.assertEqual(b["tessera-events"], c["tessera-events"])

    def test_ui_extra_is_every_file_tessera_ui_compiles_in_from_outside(self):
        found = set()
        base = ROOT / "native/tessera-ui"
        for rs in base.rglob("*.rs"):
            if "target" in rs.parts or "vendor" in rs.parts:
                continue
            for m in re.finditer(r'include_(?:str|bytes)!\("([^"]+)"\)', rs.read_text(errors="replace")):
                p = (rs.parent / m.group(1)).resolve()
                try:
                    rel = p.relative_to(ROOT)
                except ValueError:
                    continue
                if not str(rel).startswith("native/tessera-ui/"):
                    found.add(str(rel))
        self.assertEqual(found - set(ni.UI_EXTRA), set(), "add these to native-install.py's UI_EXTRA")

    def builder(self, said_ui="tessera-ui 0.1.0", said_ev="tessera-events 0.1.0"):
        def build(stage, tuned, ui, say):
            files = {}
            for name, said in (("tessera-ui", said_ui), ("tessera-events", said_ev)):
                fake_program(Path(stage) / name, said, pad=os.urandom(4).hex())
                files[name] = str(Path(stage) / name)
            return files
        return build

    def test_a_program_whose_sources_didnt_change_is_kept(self):
        r = ni.install("build", ui=True, repo=str(self.repo), builder=self.builder(), say=lambda s: None)
        self.assertTrue(r["ok"], r)
        ui_before = (self.dest / "tessera-ui").read_bytes()
        self.commit_file("native/tessera-events/main.rs", "fn main() { 2; }\n")
        r = ni.install("build", ui=True, repo=str(self.repo), builder=self.builder(said_ev="tessera-events 0.2.0"), say=lambda s: None)
        self.assertTrue(r["ok"], r)
        self.assertEqual(r["installed"], ["tessera-events"])
        self.assertEqual(r["kept"], ["tessera-ui"])
        self.assertEqual((self.dest / "tessera-ui").read_bytes(), ui_before, "tessera-ui stays as it was")
        self.assertEqual(ni.program_version(str(self.dest / "tessera-events")), "tessera-events 0.2.0")
        info = ni.read_info(str(self.dest), "tessera-ui")
        self.assertEqual((info["tree"], info.get("kept")), (self.tree, True))
        # Going back (restore) leaves the kept program as it is
        ni.restore(str(self.dest))
        self.assertEqual((self.dest / "tessera-ui").read_bytes(), ui_before)
        # Nothing changed at all: nothing is built
        calls = []
        r = ni.install("build", ui=True, repo=str(self.repo), builder=lambda *a: calls.append(a) or {}, say=lambda s: None)
        ni.restore(str(self.dest))  # (forward again)

    def test_the_local_build_is_for_this_processor_and_its_flags_never_change(self):
        with mock.patch.dict(os.environ, {"TESSERA_UI_VARIANT": "x86-64-v3", "RUSTFLAGS": "-C opt-level=1", "CARGO_TARGET_DIR": "/elsewhere",
                                          "CARGO_PROFILE_RELEASE_LTO": "false"}):
            self.assertEqual(ni.local_variant(False), "x86-64-v3")
            self.assertEqual(ni.local_variant(True), "native")
            env = ni.build_flags(os.environ, "x86-64-v3")
            self.assertEqual(env["RUSTFLAGS"], "-C target-cpu=x86-64-v3")
            self.assertNotIn("CARGO_TARGET_DIR", env)
            self.assertNotIn("CARGO_PROFILE_RELEASE_LTO", env)
            self.assertEqual(env["CARGO_INCREMENTAL"], "0")
            self.assertEqual(ni.build_flags(dict(os.environ, RUSTFLAGS=""), "x86-64-v3"), env, "the same flags whatever the caller had")
            self.assertTrue(ni.cache_dir("x86-64-v3").endswith("/tessera/cargo/x86-64-v3"))
            self.assertTrue(ni.cache_dir("native").endswith("/tessera/cargo/tuned"))
            self.assertEqual(ni.build_env(False, variant="x86-64-v3")["TESSERA_BUILD_VARIANT"], "x86-64-v3")
        with mock.patch.dict(os.environ, {"TESSERA_UI_VARIANT": "generic"}):
            self.assertEqual(ni.build_flags({}, "generic")["RUSTFLAGS"], "")

    def test_a_build_that_doesnt_start_goes_back_and_is_marked_bad(self):
        with mock.patch.dict(os.environ, {"XDG_STATE_HOME": str(self.d / "state")}):
            ni.install("build", ui=True, repo=str(self.repo), builder=self.builder(said_ui="tessera-ui 0.1.0"), say=lambda s: None)
            old_tree = self.tree
            self.commit_file("native/tessera-ui/main.rs", "fn main() { 3; }\n")
            ni.install("build", ui=True, repo=str(self.repo), builder=self.builder(said_ui="tessera-ui 0.3.0"), say=lambda s: None)
            r = ni.rollback(str(self.dest))
            self.assertEqual(ni.program_version(str(self.dest / "tessera-ui")), "tessera-ui 0.1.0")
            self.assertEqual(r["bad"], self.tree)
            self.assertIn(self.tree, ni.bad_trees())
            self.assertNotIn(old_tree, ni.bad_trees())
            note = json.loads((self.d / "state/tessera/rolled-back.json").read_text())
            self.assertEqual(note["tree"], self.tree)


@unittest.skipUnless(HAS_CRYPTO and HAS_ZSTD, "needs python-cryptography and zstd")
class Patches(tni.Fixture):
    """A release whose manifest lists patches from the build before"""

    def release(self, tree, commit, progs, patches_from=None, key=None, tamper=None):
        key = key or self.key
        out = self.rel / ni.ROLLING
        out.mkdir(parents=True, exist_ok=True)
        entries = {}
        for v in ("x86-64-v3",):
            stage = self.d / ("rel-" + tree[:6] + v)
            stage.mkdir(exist_ok=True)
            for n, said in progs.items():
                fake_program(stage / n, said, pad=said * 400)
            tar = self.d / "p.tar"
            subprocess.run(["tar", "-C", str(stage), "-cf", str(tar), *ni.PROGRAMS], check=True)
            file = ni.tarball_name(tree, v)
            subprocess.run(["zstd", "-q", "-f", "-o", str(out / file), str(tar)], check=True)
            e = {"file": file, "sha256": ni.sha256_of(str(out / file)), "size": (out / file).stat().st_size,
                 "programs": {n: {"sha256": ni.sha256_of(str(stage / n)), "version": progs[n]} for n in ni.PROGRAMS}}
            if patches_from:
                old_tree, old_stage = patches_from
                pe = {}
                for n in ni.PROGRAMS:
                    a, b = old_stage / n, stage / n
                    if ni.sha256_of(str(a)) == ni.sha256_of(str(b)):
                        continue
                    name = ni.patch_name(tree, v, n, old_tree)
                    ni.make_patch(str(a), str(b), str(out / name), level=3)
                    data = (out / name).read_bytes()
                    if tamper == "bytes":
                        (out / name).write_bytes(data[:-3] + b"xyz")
                    (out / (name + ".minisig")).write_text(key.sign(data + b" other bytes" if tamper == "signed-bad" else (out / name).read_bytes(),
                                                                    f"{name} tree={tree} commit={commit}"))
                    pe[n] = {"file": name, "size": len(data), "sha256": hashlib.sha256(data).hexdigest(), "from_sha256": ni.sha256_of(str(a))}
                e["patches"] = {old_tree: pe}
            entries[v] = e
        m = {"format": 1, "name": "tessera-ui", "commit": commit, "tree": tree, "version": "0", "channel": "latest", "tag": ni.ROLLING, "variants": entries}
        data = json.dumps(m).encode()
        (out / ni.manifest_name(tree)).write_bytes(data)
        (out / (ni.manifest_name(tree) + ".minisig")).write_text(key.sign(data, f"{ni.manifest_name(tree)} tree={tree} commit={commit}"))
        return self.d / ("rel-" + tree[:6] + "x86-64-v3")

    def setUp(self):
        super().setUp()
        self.old_tree, self.old_commit = self.tree, self.commit
        self.old_stage = self.release(self.tree, self.commit, {"tessera-ui": "tessera-ui 0.1.0", "tessera-events": "tessera-events 0.1.0"})
        # (the installed copies: their own files, like ~/.local/share/tessera/bin)
        self.installed = self.d / "installed"
        shutil.copytree(self.old_stage, self.installed)
        self.have = {"tree": self.old_tree, "variant": "x86-64-v3", "programs": {n: str(self.installed / n) for n in ni.PROGRAMS}}
        self.commit_change("fn main() { 9; }\n")

    def fetch_new(self, **kw):
        self.release(self.tree, self.commit, {"tessera-ui": "tessera-ui 0.2.0", "tessera-events": "tessera-events 0.1.0"},
                     patches_from=(self.old_tree, self.old_stage), **kw)
        said = []
        got = ni.fetch(self.tree, self.commit, "x86-64-v3", str(self.dest / ".incoming-p"), base=self.base, tags=[ni.ROLLING],
                       pubkey=self.key.pub, repo=str(self.repo), have=self.have, say=said.append)
        return got, said

    def test_the_patch_is_used_and_checked(self):
        got, said = self.fetch_new()
        self.assertEqual(got["info"]["how"], "patch")
        self.assertEqual(ni.program_version(got["files"]["tessera-ui"]), "tessera-ui 0.2.0")
        # (tessera-events didn't change: no patch, it's the same program)
        self.assertEqual(ni.program_version(got["files"]["tessera-events"]), "tessera-events 0.1.0")
        self.assertTrue(any("patch" in s for s in said))
        self.assertLess(got["info"]["bytes"], 4000)

    def test_a_damaged_patch_falls_back_to_the_whole_archive(self):
        got, said = self.fetch_new(tamper="bytes")
        self.assertEqual(got["info"]["how"], "full")
        self.assertTrue(any("didn't work" in s for s in said))
        self.assertEqual(ni.program_version(got["files"]["tessera-ui"]), "tessera-ui 0.2.0")

    def test_a_patch_signed_for_other_bytes_falls_back(self):
        got, _ = self.fetch_new(tamper="signed-bad")
        self.assertEqual(got["info"]["how"], "full")

    def test_another_installed_program_gets_the_whole_archive(self):
        fake_program(self.installed / "tessera-ui", "tessera-ui 0.1.0", pad="mine")
        got, _ = self.fetch_new()
        self.assertEqual(got["info"]["how"], "full")

    def test_staged_beside_the_installed_then_swapped_in(self):
        self.release(self.tree, self.commit, {"tessera-ui": "tessera-ui 0.2.0", "tessera-events": "tessera-events 0.1.0"})

        def fetcher(tree, commit, variant, work, repo=None, want=None, have=None, say=None):
            return ni.fetch(tree, commit, variant, work, base=self.base, tags=[ni.ROLLING], pubkey=self.key.pub, repo=repo, want=want, have=have, say=say)
        # (the old build installed)
        self.dest.mkdir(parents=True, exist_ok=True)
        for n in ni.PROGRAMS:
            shutil.copy2(self.old_stage / n, self.dest / n)
            ni._write_info(str(self.dest), n, {"source": "ready-made", "variant": "x86-64-v3", "tree": self.old_tree})
        r = ni.stage("HEAD", "auto", ui=True, repo=str(self.repo), fetcher=fetcher, say=lambda s: None, config={})
        self.assertTrue(r["ok"], r)
        self.assertEqual(ni.program_version(str(self.dest / "tessera-ui")), "tessera-ui 0.1.0", "staging installs nothing")
        r = ni.install_staged(str(self.dest), repo=str(self.repo))
        self.assertTrue(r["ok"], r)
        self.assertEqual(ni.program_version(str(self.dest / "tessera-ui")), "tessera-ui 0.2.0")
        self.assertFalse((self.dest / ".staged").exists())
        # Staged for another version: refused, nothing changes
        ni.stage("HEAD", "auto", ui=True, repo=str(self.repo), fetcher=fetcher, say=lambda s: None, config={})
        self.commit_change("fn main() { 10; }\n")
        r = ni.install_staged(str(self.dest), repo=str(self.repo))
        self.assertFalse(r["ok"])


class Cache(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.d = Path(self.tmp.name)
        self.env = mock.patch.dict(os.environ, {"XDG_CACHE_HOME": str(self.d / "cache")})
        self.env.start()

    def tearDown(self):
        self.env.stop()
        self.tmp.cleanup()

    def unit(self, path, size):
        p = Path(path)
        p.mkdir(parents=True, exist_ok=True)
        (p / "blob").write_bytes(b"x" * size)

    def test_a_new_toolchain_wipes_the_cache(self):
        t = ni.cache_dir("x86-64-v3")
        ni.cache_prepare(t, now_toolchain="rustc 1.99.1")
        self.unit(t + "/local/build/serde/0123456789abcdef", 10)
        ni.cache_prepare(t, now_toolchain="rustc 1.99.1")
        self.assertTrue(os.path.exists(t + "/local/build/serde/0123456789abcdef"), "same toolchain: kept")
        ni.cache_prepare(t, now_toolchain="rustc 1.100.0")
        self.assertFalse(os.path.exists(t + "/local/build/serde"), "a new toolchain: everything in it was stale")

    def test_pruning_keeps_what_the_last_build_used(self):
        t = ni.cache_dir("x86-64-v3")
        used, old = "aaaaaaaaaaaaaaaa", "bbbbbbbbbbbbbbbb"
        self.unit(f"{t}/local/build/serde/{used}", 1000)   # (cargo's newer layout)
        self.unit(f"{t}/local/build/serde/{old}", 1000)
        self.unit(f"{t}/local/.fingerprint/slint-{old}", 1000)  # (the older one)
        (Path(t) / "local/deps").mkdir(parents=True)
        (Path(t) / f"local/deps/libslint-{old}.rlib").write_bytes(b"x" * 1000)
        (Path(t) / f"local/deps/libserde-{used}.rlib").write_bytes(b"x" * 1000)
        (Path(t) / "tessera-build-used-tessera-ui.json").write_text(json.dumps([f"{t}/local/deps/libserde-{used}.rlib", f"{t}/local/build/serde/{used}/out"]))
        other = ni.cache_dir("generic")
        self.unit(other + "/local/build/x/cccccccccccccccc", 3000)
        self.assertEqual(ni.cache_prune(t, cap=10 ** 9), 0, "under the cap: nothing goes")
        ni.cache_prune(t, cap=6000)  # (about 8 KB: the 3 KB unused go, about 5 KB left)
        self.assertTrue(os.path.exists(f"{t}/local/build/serde/{used}"))
        self.assertTrue(os.path.exists(f"{t}/local/deps/libserde-{used}.rlib"))
        self.assertFalse(os.path.exists(f"{t}/local/build/serde/{old}"))
        self.assertFalse(os.path.exists(f"{t}/local/.fingerprint/slint-{old}"))
        self.assertFalse(os.path.exists(f"{t}/local/deps/libslint-{old}.rlib"))
        self.assertTrue(os.path.exists(other), "under the cap after the unused units: the other variant stays")
        ni.cache_prune(t, cap=2500)
        self.assertFalse(os.path.exists(other), "still over: the other variant's folder goes")

    def test_size_and_delete(self):
        self.unit(ni.cache_dir("generic") + "/local/build/a/dddddddddddddddd", 5000)
        info = ni.cache_info(repo=str(self.d))
        self.assertGreaterEqual(info["bytes"], 5000)
        with mock.patch.dict(os.environ, {"TESSERA_NATIVE_DEST": str(self.d / "bin")}):
            self.assertGreaterEqual(ni.cache_delete(repo=str(self.d)), 5000)
        self.assertFalse(os.path.exists(ni.cache_root()))


class BuildScript(unittest.TestCase):
    def test_build_native_uses_the_cache_and_the_local_profile(self):
        sh = (ROOT / "tools/build-native.sh").read_text()
        self.assertIn("TESSERA_CARGO_TARGET", sh)
        self.assertIn("TESSERA_CARGO_PROFILE", sh)
        for crate in ("tessera-ui", "tessera-events"):
            self.assertIn("[profile.local]", (ROOT / f"native/{crate}/Cargo.toml").read_text())

    def test_the_new_shell_never_inherits_the_repo_lock(self):
        # (update-run.sh holds .git/tessera-update.lock on fd 8; the shell it
        # started kept it, and the next update in that session, a restart at
        # the lock included, was refused as "Another update is running")
        sh = (ROOT / "shell/assets/scripts/update-run.sh").read_text()
        starts = [l for l in sh.splitlines() if "setsid -f" in l and ("launch-shell.sh" in l or "SCREEN[@]" in l)]
        self.assertGreaterEqual(len(starts), 4)
        for l in starts:
            self.assertIn("8>&-", l, l.strip())
        self.assertIn("exec 9>&- 6>&- 8>&-", sh)

    def test_build_progress_records_the_units_used(self):
        bp = load("bg_build_progress", ROOT / "tools/build-progress.py")
        p = bp.Progress("x", "cargo")
        p.feed_json({"reason": "compiler-artifact", "fresh": True, "filenames": ["/t/local/deps/libserde-aaaaaaaaaaaaaaaa.rlib"], "target": {"name": "serde", "kind": ["lib"]}, "package_id": "registry+x#serde@1"})
        p.feed_json({"reason": "build-script-executed", "out_dir": "/t/local/build/serde/bbbbbbbbbbbbbbbb/out"})
        self.assertEqual(p.used, ["/t/local/deps/libserde-aaaaaaaaaaaaaaaa.rlib", "/t/local/build/serde/bbbbbbbbbbbbbbbb/out"])


# ── tessera-update.py ─────────────────────────────────────────────────────

def load_update():
    return load("bg_tessera_update_" + os.urandom(3).hex(), ROOT / "tools/tessera-update.py")


class Flow(unittest.TestCase):
    """A real remote and clone: main at "two", beta two commits on"""

    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        t = Path(self.tmp.name)
        self.t = t
        self.remote, seed, self.repo, self.config = t / "remote.git", t / "seed", t / "copy", t / "config"
        self.config.mkdir()
        subprocess.run(["git", "init", "-q", "--bare", "-b", "main", str(self.remote)], check=True)
        subprocess.run(["git", "init", "-q", "-b", "main", str(seed)], check=True)
        (seed / "shell/common").mkdir(parents=True)
        (seed / "shell/common/Config.qml").write_text("JsonAdapter {\n}\n")
        self.seed = seed
        for name in ("one", "two"):
            self.commit(name)
        git(seed, "checkout", "-q", "-b", "beta")
        for name in ("three", "four"):
            self.commit(name)
        git(seed, "remote", "add", "origin", str(self.remote))
        git(seed, "push", "-q", "origin", "main", "beta")
        subprocess.run(["git", "clone", "-q", "--branch", "main", "--single-branch", str(self.remote), str(self.repo)], check=True)
        self.m = load_update()
        self.m.REPO = str(self.repo)
        self.calls = []
        self.m.native_install = lambda *a, **k: self.calls.append(a) or {"ok": True, "changed": True}
        self.m.native_plan = lambda *a, **k: None
        stub = t / "ni-stub.py"
        stub.write_text("print('{}')\n")
        self.m.NATIVE = str(stub)
        self.m.native_stage = lambda rev, source, **k: {"ok": True, "staged": ["tessera-ui"], "kept": ["tessera-events"], "how": "patch", "bytes": 8000000}
        self.m.stage_source = lambda rev: str(t)
        self.env = mock.patch.dict(os.environ, {"XDG_STATE_HOME": str(t / "state"), "XDG_DATA_HOME": str(t / "data"), "XDG_CACHE_HOME": str(t / "cache"),
                                                "XDG_RUNTIME_DIR": str(t / "run")})
        self.env.start()
        (self.config / "config.json").write_text(json.dumps({"update": {"channel": "beta"}, "a": 1}))

    def tearDown(self):
        self.env.stop()
        self.tmp.cleanup()

    def commit(self, name, branch=None):
        if branch:
            git(self.seed, "checkout", "-q", branch)
        (self.seed / (name + ".txt")).write_text(name)
        git(self.seed, "add", "-A")
        git(self.seed, "commit", "-qm", name)

    def head(self):
        return git(self.repo, "log", "-1", "--format=%s")

    def stage(self):
        self.m.check(str(self.config))
        return self.m.stage(str(self.config), {})

    def test_stage_changes_nothing_then_finish_takes_exactly_that_version(self):
        out = self.stage()
        self.assertEqual(out["state"], "ready")
        self.assertEqual(self.head(), "two", "the checkout only moves at the restart")
        info = self.m.staged_info()
        self.assertEqual((info["ready"], info["valid"]), (True, True))
        # Beta moves on meanwhile (a check fetched it): the restart still
        # installs what was staged, not the newer one
        self.commit("five", "beta")
        git(self.seed, "push", "-q", "origin", "beta")
        self.m.check(str(self.config))
        r = self.m.finish(str(self.config))
        self.assertTrue(r["applied"], r)
        self.assertEqual(self.head(), "four")
        self.assertIn(("--staged",), [tuple(c[1:]) for c in self.calls], "the staged programs, not a download")
        self.assertFalse(self.m.read_staged(), "used up")
        self.assertTrue(os.path.exists(self.m.fresh_path()), "the new shell is watched: launch-shell.sh's rollback_ui")
        self.assertFalse(os.path.exists(self.m.journal_path()))

    def test_a_checkout_that_moved_since_is_refused(self):
        self.stage()
        (self.repo / "mine.txt").write_text("x")
        git(self.repo, "add", "-A")
        git(self.repo, "commit", "-qm", "my own")
        with self.assertRaises(SystemExit):
            self.m.finish(str(self.config))
        self.assertEqual(self.head(), "my own")
        self.assertFalse(self.m.read_staged())

    def test_new_conflicts_settle_safely_and_files_wait_for_a_decision(self):
        report = {"conflicts": [{"id": "setting:a", "kind": "setting", "title": "a", "choices": ["keep", "new"]},
                                {"id": "shortcut:x", "kind": "shortcut", "title": "Super+X", "choices": ["mine", "new"]}]}
        self.assertEqual(self.m.settled_choices(report, {}), {"setting:a": "keep", "shortcut:x": "mine"})
        report["conflicts"].append({"id": "code:f", "kind": "code", "title": "f", "choices": ["mine", "new"]})
        with self.assertRaises(SystemExit):
            self.m.settled_choices(report, {})
        self.assertEqual(self.m.settled_choices(report, {"code:f": "new"})["code:f"], "new")

    def cut_at(self, point):
        def cp(name):
            if name == point:
                raise self.m.Cut()
        self.m.checkpoint = cp

    def interrupted(self, point, edit=False, staged=False):
        before = (self.head(), git(self.repo, "symbolic-ref", "--short", "HEAD"), (self.config / "config.json").read_text())
        if edit:
            (self.repo / "one.txt").write_text("my edit")
        if staged:
            self.stage()
        self.m.check(str(self.config))
        self.cut_at(point)
        with self.assertRaises(self.m.Cut):
            if staged:
                self.m.apply(str(self.config), {}, (), None, staged=self.m.read_staged())
            else:
                self.m.apply(str(self.config), {})
        self.assertTrue(os.path.exists(self.m.journal_path()), "the journal says it was cut off")
        self.m.checkpoint = lambda n: None
        r = self.m.recover()
        self.assertTrue(r["recovered"], r)
        self.assertIn("went back", r["error"])
        after = (self.head(), git(self.repo, "symbolic-ref", "--short", "HEAD"), (self.config / "config.json").read_text())
        self.assertEqual(after, before, f"cut off at {point}: back exactly as it was")
        if edit:
            self.assertEqual((self.repo / "one.txt").read_text(), "my edit", "your edit is back")
        self.assertFalse(os.path.exists(self.m.journal_path()))
        self.assertEqual(self.m.recover(), {"recovered": False}, "once")

    def test_cut_off_after_the_backup(self):
        self.interrupted("backup")

    def test_cut_off_with_your_edits_stashed(self):
        self.interrupted("stashed", edit=True)

    def test_cut_off_after_the_new_version_was_taken(self):
        self.interrupted("taken", edit=True)

    def test_cut_off_after_the_settings_were_changed(self):
        self.interrupted("settings")

    def test_cut_off_after_the_programs_went_in(self):
        self.interrupted("programs")

    def test_cut_off_during_the_restart_into_a_staged_update(self):
        self.interrupted("taken", staged=True)
        self.assertTrue(self.m.staged_info()["valid"], "the staged update waits, still good")

    def test_another_checkouts_journal_or_staged_update_is_left_alone(self):
        os.makedirs(os.path.dirname(self.m.journal_path()), exist_ok=True)
        Path(self.m.journal_path()).write_text(json.dumps({"repo": "/elsewhere", "old_head": "0" * 40, "step": "taken"}))
        head = self.head()
        self.assertEqual(self.m.recover(), {"recovered": False, "other": "/elsewhere"})
        self.assertTrue(os.path.exists(self.m.journal_path()), "left for its own checkout")
        self.assertEqual(self.head(), head)
        os.remove(self.m.journal_path())
        self.stage()
        staged = self.m.read_staged()
        staged["repo"] = "/elsewhere"
        Path(self.m.staged_dir(), "staged.json").write_text(json.dumps(staged))
        self.assertFalse(self.m.staged_info()["valid"])
        self.assertFalse(self.m.finish(str(self.config))["applied"])
        self.assertEqual(self.head(), head)
        self.assertTrue(self.m.read_staged(), "not ours to discard either")

    def test_a_job_cut_off_is_seen(self):
        self.assertTrue(self.m.job_interrupted({"state": "running", "pid": 999999}, alive=lambda p: False))
        self.assertFalse(self.m.job_interrupted({"state": "running", "pid": 1}, alive=lambda p: True))
        self.assertFalse(self.m.job_interrupted({"state": "ready", "pid": 999999}, alive=lambda p: False))

    def test_a_job_cut_off_is_resumed_with_what_it_was_asked(self):
        d = Path(self.m.job_dir())
        d.mkdir(parents=True)
        (d / "status.json").write_text(json.dumps({"state": "running", "pid": 999999}))
        (d / "request.json").write_text(json.dumps({"config": str(self.config), "choices": {}, "rebuild": "", "channel": "beta"}))
        self.m.job_interrupted = lambda *a, **k: True
        self.m.check(str(self.config))

        class A:
            rebuild = ""
            channel = None
            config = str(self.config)
        out = self.m.resume(A())
        self.assertTrue(out["resumed"])
        self.assertEqual(json.loads((d / "status.json").read_text())["state"], "ready")
        self.assertFalse((d / "request.json").exists())

    def test_a_version_that_didnt_start_isnt_offered_again(self):
        self.m.check(str(self.config))
        tree = git(self.repo, "rev-parse", "origin/beta:shell")  # (no native/ here: any tree stands in)
        os.makedirs(os.path.join(os.environ["XDG_STATE_HOME"], "tessera"))
        self.m.is_bad = lambda rev: True
        r = self.m.check(str(self.config), fetch=False)
        self.assertFalse(r["available"])
        self.assertTrue(r["bad"])
        self.assertIn("didn't start", r["error"])
        self.assertTrue(tree)

    def test_parts_are_rebuilt_only_when_their_sources_change(self):
        m = self.m
        keys = {"plugin": "p1", "shaders": "s1", "native": "n1"}
        m.part_key = lambda part, rev, **k: {"HEAD": keys, "new": dict(keys, plugin="p2")}[rev][part]
        built = lambda p: True  # noqa: E731
        can = lambda p: True  # noqa: E731
        self.assertEqual(m.parts_needed("new", record=keys, built=built, buildable=can), ["plugin"])
        self.assertEqual(m.parts_needed("HEAD", record=keys, built=built, buildable=can), [])
        # (a build that isn't there is made; one that can't be built here is left alone)
        self.assertEqual(m.parts_needed("HEAD", record=keys, built=lambda p: p != "shaders", buildable=can), ["shaders"])
        self.assertEqual(m.parts_needed("new", record=keys, built=built, buildable=lambda p: p != "plugin"), [])
        # (no record yet: what the installed version is made from counts)
        self.assertEqual(m.parts_needed("new", record={}, built=built, buildable=can, keys_now=keys), ["plugin"])


class Cancel(unittest.TestCase):
    """Cancel stops the whole job, a running child (cargo, rustc) included,
    even when it was started with SIGTERM blocked as the shell starts it,
    and its status ends "cancelled" (19 Nov's arena: it didn't)"""

    JOB = r"""
import importlib.util, json, os, subprocess, sys, time
spec = importlib.util.spec_from_file_location("tu", sys.argv[1])
m = importlib.util.module_from_spec(spec); spec.loader.exec_module(m)
if sys.argv[2] == "unblock":
    m.unblock_signals()
else:
    import signal
    signal.signal(signal.SIGTERM, signal.SIG_IGN)  # (a job that won't hear it at all)
child = subprocess.Popen(["sleep", "300"])
d = m.job_dir(); os.makedirs(d, exist_ok=True)
open(os.path.join(d, "pid"), "w").write(str(os.getpid()))
json.dump({"state": "running", "phase": "build", "pid": os.getpid()}, open(os.path.join(d, "status.json"), "w"))
json.dump({"choices": {}}, open(os.path.join(d, "request.json"), "w"))
open(os.path.join(d, "child"), "w").write(str(child.pid))
time.sleep(300)
"""

    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.env = mock.patch.dict(os.environ, {"XDG_STATE_HOME": self.tmp.name})
        self.env.start()
        self.m = load_update()
        stub = Path(self.tmp.name) / "ni-stub.py"
        stub.write_text("print('{}')\n")
        self.m.NATIVE = str(stub)

    def tearDown(self):
        self.env.stop()
        self.tmp.cleanup()

    def start(self, how):
        import signal

        def as_the_shell():  # (tessera-ui's signalfd mask, inherited)
            signal.pthread_sigmask(signal.SIG_BLOCK, {signal.SIGTERM, signal.SIGINT})
        p = subprocess.Popen([sys.executable, "-c", self.JOB, str(ROOT / "tools/tessera-update.py"), how],
                             env=dict(os.environ), preexec_fn=as_the_shell, start_new_session=True)
        child = Path(self.m.job_dir(), "child")
        for _ in range(100):
            if child.exists() and child.read_text():
                break
            time.sleep(0.05)
        return p, int(child.read_text())

    def check_gone(self, p, child):
        p.wait(timeout=10)
        for _ in range(40):
            if not os.path.exists(f"/proc/{child}") or Path(f"/proc/{child}/stat").read_text().split()[2] == "Z":
                break
            time.sleep(0.05)
        self.assertFalse(os.path.exists(f"/proc/{child}") and Path(f"/proc/{child}/stat").read_text().split()[2] != "Z", "the running child is stopped too")
        st = json.loads(Path(self.m.job_dir(), "status.json").read_text())
        self.assertEqual(st["state"], "cancelled")
        self.assertFalse(Path(self.m.job_dir(), "request.json").exists(), "not resumed at the next start")

    def test_a_job_started_by_the_shell_hears_cancel(self):
        p, child = self.start("unblock")
        t0 = time.time()
        self.assertTrue(self.m.cancel_job(grace=8))
        self.assertLess(time.time() - t0, 8, "SIGTERM was enough: no wait for SIGKILL")
        self.check_gone(p, child)

    def test_a_job_that_ignores_sigterm_is_killed(self):
        p, child = self.start("ignore")
        self.assertTrue(self.m.cancel_job(grace=1))
        self.check_gone(p, child)

    def test_no_job_no_cancel(self):
        self.assertFalse(self.m.cancel_job(grace=0.1))


class RepoLock(unittest.TestCase):
    """"Another update is running" (19 Nov): never for a stale lock. Its
    holder gone, or only a process that inherited the descriptor keeping it
    (a shell started by an older update-run.sh): the lock is taken. An
    update really running: said since when, where and what, never a dead end."""

    # (holds the lock as an update would: its cmdline names tessera-update.py)
    HOLDER = """
import fcntl, json, os, sys, time
f = open(sys.argv[1], "a+")
fcntl.flock(f, fcntl.LOCK_EX)
if sys.argv[2] == "record":
    start = int(open(f"/proc/{os.getpid()}/stat").read().rsplit(")", 1)[1].split()[19])
    f.seek(0); f.truncate()
    json.dump({"pid": os.getpid(), "start": start, "what": "stage", "started": 1700000000, "session": "other", "log": ""}, f)
    f.flush()
print("held", flush=True)
time.sleep(60)
"""

    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.repo = Path(self.tmp.name) / "repo"
        (self.repo / ".git").mkdir(parents=True)
        self.env = mock.patch.dict(os.environ, {"XDG_STATE_HOME": self.tmp.name, "HYPRLAND_INSTANCE_SIGNATURE": "mine"})
        self.env.start()
        os.environ.pop("TESSERA_UPDATE_HELD", None)
        self.m = load_update()
        self.m.REPO = str(self.repo)
        self.procs = []

    def tearDown(self):
        for p in self.procs:
            p.kill()
            p.wait()
            if p.stdout:
                p.stdout.close()
        self.env.stop()
        self.tmp.cleanup()

    def held_by_an_update(self, record=True):
        p = subprocess.Popen([sys.executable, "-c", self.HOLDER, self.m.lock_path(), "record" if record else "bare", "tessera-update.py"],
                             stdout=subprocess.PIPE, text=True)
        self.procs.append(p)
        self.assertEqual(p.stdout.readline().strip(), "held")
        return p

    def inherited_by_a_shell(self, record):
        """The lock taken by a process that's gone, its descriptor kept by
        a child that isn't an update (the old bug's shell)"""
        import fcntl
        f = open(self.m.lock_path(), "a+")
        fcntl.flock(f, fcntl.LOCK_EX)
        if record:
            dead = subprocess.Popen(["true"])
            dead.wait()
            f.seek(0)
            f.truncate()
            json.dump({"pid": dead.pid, "start": 1, "what": "restart", "started": 1700000000, "session": "mine", "log": ""}, f)
            f.flush()
        shell = subprocess.Popen(["sleep", "60"], pass_fds=(f.fileno(),))
        self.procs.append(shell)
        f.close()
        return shell

    def test_a_lock_whose_holder_is_gone_is_taken(self):
        shell = self.inherited_by_a_shell(record=True)
        held = self.m.repo_lock()
        self.assertIsNotNone(held)
        rec = json.loads(Path(self.m.lock_path()).read_text())
        self.assertEqual((rec["pid"], rec["what"], rec["session"]), (os.getpid(), "update", "mine"))
        self.assertIsNone(shell.poll(), "the shell that kept the old one is left alone")

    def test_an_inherited_lock_without_a_record_is_taken(self):
        self.inherited_by_a_shell(record=False)
        self.assertIsNotNone(self.m.repo_lock())

    def test_an_update_running_is_named_not_a_dead_end(self):
        self.held_by_an_update()
        with self.assertRaises(self.m.Busy) as e:
            self.m.repo_lock()
        msg = str(e.exception)
        at = time.strftime("%H:%M", time.localtime(1700000000))
        self.assertTrue(msg.startswith(f"Software Update is already running (started {at}) in another session"), msg)
        self.assertIn("getting an update ready", msg)
        self.assertIn("then try again", msg)
        self.assertNotIn("Another update", msg)
        self.assertEqual(e.exception.holder["what"], "stage")
        # (still a SystemExit: every caller that said why still does)
        self.assertIsInstance(e.exception, SystemExit)

    def test_an_update_without_a_record_still_counts(self):
        self.held_by_an_update(record=False)
        self.assertIsNotNone(self.m.lock_holder())
        with self.assertRaises(self.m.Busy):
            self.m.repo_lock()

    def test_lock_holder_reports_and_marks_the_job(self):
        self.assertEqual(self.m.lock_report(), {"free": True})
        self.inherited_by_a_shell(record=True)
        self.assertEqual(self.m.lock_report(), {"stale": True})
        self.assertEqual(self.m.lock_report(), {"free": True}, "taken apart: free again")
        self.held_by_an_update()
        r = self.m.lock_report(mark=True)
        self.assertTrue(r["busy"])
        st = json.loads(Path(self.m.job_dir(), "status.json").read_text())
        self.assertEqual(st["state"], "busy")
        self.assertEqual(st["holder"]["what"], "stage")
        self.assertTrue(st["error"].startswith("Software Update is already running"))

    def test_a_running_job_keeps_its_own_status(self):
        d = Path(self.m.job_dir())
        d.mkdir(parents=True)
        me = subprocess.Popen([sys.executable, "-c", "import time; time.sleep(60)", "tessera-update.py"])
        self.procs.append(me)
        (d / "status.json").write_text(json.dumps({"state": "running", "pid": me.pid}))
        time.sleep(0.2)
        self.assertFalse(self.m.mark_busy({"what": "restart"}))
        self.assertEqual(json.loads((d / "status.json").read_text())["state"], "running")

    def test_a_second_stage_leaves_the_first_ones_files_alone(self):
        self.held_by_an_update()
        a = mock.Mock(rebuild="", channel=None)
        out = self.m.run_stage(a, str(self.repo), {})
        self.assertTrue(out["busy"])
        self.assertFalse(Path(self.m.job_dir(), "pid").exists(), "no second job: the first one's pid stays its own")
        self.assertEqual(json.loads(Path(self.m.job_dir(), "status.json").read_text())["state"], "busy")

    def test_held_for_us_only_by_our_parent(self):
        # (an older update-run.sh exported TESSERA_UPDATE_HELD=1, and the shell
        # it started passed it to every update after, which then never locked)
        with mock.patch.dict(os.environ, {"TESSERA_UPDATE_HELD": "1"}):
            self.assertIsNotNone(self.m.repo_lock())
        self.m._HELD = None
        with mock.patch.dict(os.environ, {"TESSERA_UPDATE_HELD": str(os.getppid())}):
            self.assertIsNone(self.m.repo_lock())

    def test_update_run_sh_takes_a_stale_lock_and_never_leaks_held(self):
        sh = (ROOT / "shell/assets/scripts/update-run.sh").read_text()
        self.assertNotIn("export TESSERA_UPDATE_HELD", sh)
        self.assertIn('TESSERA_UPDATE_HELD=$$ python3 -I "$REPO/tools/tessera-update.py" "${UPDATE[@]}"', sh)
        self.assertIn('exec 8<>"$LOCKF"', sh, "opening the lock never empties its holder's record")
        self.assertIn("lock-holder", sh)
        self.assertNotIn("Another update is running", sh)
        self.assertIn("TESSERA_UPDATE_HELD=*", sh, "stripped from the shell it starts")

    def test_nothing_says_another_update_any_more(self):
        for f in ("tools/tessera-update.py", "shell/assets/scripts/update-run.sh", "shell/assets/scripts/sysupdate.py"):
            self.assertNotIn("Another update is running", (ROOT / f).read_text(), f)


# ── sysupdate.py ──────────────────────────────────────────────────────────

class RestartNeeded(unittest.TestCase):
    def test_kernels_drivers_and_core_libraries(self):
        su = load("bg_sysupdate", ROOT / "shell/assets/scripts/sysupdate.py")
        for p in ("linux", "linux-zen", "linux-cachyos", "linux-cachyos-lts", "linux-firmware", "linux-firmware-intel", "mesa", "lib32-mesa",
                  "nvidia-utils", "nvidia-open-dkms", "vulkan-radeon", "xf86-video-amdgpu", "systemd", "systemd-libs", "glibc", "amd-ucode", "intel-ucode",
                  "dbus", "dbus-broker-units"):
            self.assertTrue(su.needs_restart(p), p)
        for p in ("linux-headers", "linux-cachyos-headers", "linux-api-headers", "linux-zen-docs", "firefox", "hyprland", "mesa-utils", "systemd-ukify-x", "glib2"):
            self.assertFalse(su.needs_restart(p), p)

    def test_an_aur_helpers_stack_trace_never_reaches_the_error(self):
        # (system1, 19 Nov: paru, with the shell's RUST_BACKTRACE, filled the
        # error Settings showed with "8: <unknown> … __libc_start_main")
        with mock.patch.dict(os.environ, {"RUST_BACKTRACE": "1"}):
            su = load("bg_sysupdate_bt", ROOT / "shell/assets/scripts/sysupdate.py")
            self.assertNotIn("RUST_BACKTRACE", os.environ, "the helpers it runs don't get it")
        with tempfile.TemporaryDirectory() as d:
            log = Path(d, "log")
            log.write_text(":: Conflicts found:\nstack backtrace:\n   8: <unknown>\n  11: __libc_start_main\n      at src/main.rs:12\n"
                           "error: can not install conflicting packages with --noconfirm\n")
            su.LOG = str(log)
            self.assertEqual(su.tail(), ":: Conflicts found:\nerror: can not install conflicting packages with --noconfirm")


if __name__ == "__main__":
    unittest.main()
