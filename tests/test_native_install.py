"""tools/native-install.py: tessera-ui and tessera-events, ready-made or built
here (docs/releases.md). No network: the "releases" are file:// folders made
here, signed with a throwaway minisign key generated in each test (never the
release key), the "programs" are small scripts that answer --version, and the
builder is a fake. Nothing touches the real data folder or config."""
import base64
import hashlib
import importlib.util
import io
import json
import os
import shutil
import subprocess
import sys
import tarfile
import tempfile
import unittest
from pathlib import Path
from unittest import mock

ROOT = Path(__file__).resolve().parent.parent
HELPER = ROOT / "tools/native-install.py"
HAS_CRYPTO = importlib.util.find_spec("cryptography") is not None


def load():
    spec = importlib.util.spec_from_file_location("tessera_native_install", HELPER)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


ni = load()

CPU_V3 = """processor\t: 0
vendor_id\t: GenuineIntel
flags\t\t: fpu vme de pse tsc msr pae mce cx8 apic sep mtrr sse sse2 ssse3 fma cx16 sse4_1 sse4_2 movbe popcnt xsave avx f16c lahf_lm abm bmi1 avx2 bmi2
processor\t: 1
flags\t\t: fpu vme de pse tsc msr pae mce cx8 apic sep mtrr sse sse2 ssse3 fma cx16 sse4_1 sse4_2 movbe popcnt xsave avx f16c lahf_lm abm bmi1 avx2 bmi2
"""
# Sandy Bridge: AVX, but no AVX2, BMI, FMA or MOVBE
CPU_SANDY = """processor\t: 0
flags\t\t: fpu sse sse2 ssse3 cx16 sse4_1 sse4_2 popcnt xsave avx lahf_lm
"""
# An Atom-class processor that has MOVBE but no AVX
CPU_ATOM = "flags\t\t: sse sse2 ssse3 cx16 sse4_1 sse4_2 popcnt movbe lahf_lm\n"


def cpu_without(flag):
    return CPU_V3.replace(" " + flag + " ", " ").replace(" " + flag + "\n", "\n")


# ── A throwaway minisign key (the format, written here) ────────────────────

class Key:
    def __init__(self):
        from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PrivateKey
        from cryptography.hazmat.primitives import serialization
        self.priv = Ed25519PrivateKey.generate()
        self.keyid = os.urandom(8)
        pk = self.priv.public_key().public_bytes(serialization.Encoding.Raw, serialization.PublicFormat.Raw)
        self.pub = ("untrusted comment: minisign public key " + self.keyid[::-1].hex().upper() + "\n"
                    + base64.b64encode(b"Ed" + self.keyid + pk).decode() + "\n")

    def sign(self, data, trusted, prehash=True):
        msg = hashlib.blake2b(data, digest_size=64).digest() if prehash else data
        sig = self.priv.sign(msg)
        glob = self.priv.sign(sig + trusted.encode())
        return ("untrusted comment: signature from minisign secret key\n"
                + base64.b64encode((b"ED" if prehash else b"Ed") + self.keyid + sig).decode() + "\n"
                + "trusted comment: " + trusted + "\n" + base64.b64encode(glob).decode() + "\n")


def git(repo, *args):
    return subprocess.run(["git", "-C", str(repo), *args], capture_output=True, text=True, check=True).stdout.strip()


def fake_program(path, said):
    path.write_text("#!/bin/sh\n[ \"$1\" = --version ] && echo '" + said + "' && exit 0\nexit 3\n")
    path.chmod(0o755)


class Fixture(unittest.TestCase):
    """A git checkout with native/, a data folder, a release folder."""

    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(prefix="tessera-native-")
        self.d = Path(self.tmp.name)
        self.repo = self.d / "repo"
        (self.repo / "native/tessera-ui").mkdir(parents=True)
        (self.repo / "native/tessera-ui/main.rs").write_text("fn main() {}\n")
        git(self.repo.parent, "init", "-q", "-b", "main", str(self.repo))
        git(self.repo, "-c", "user.name=t", "-c", "user.email=t@t", "add", "-A")
        git(self.repo, "-c", "user.name=t", "-c", "user.email=t@t", "commit", "-q", "-m", "one")
        self.tree, self.commit, _ = ni.native_tree("HEAD", str(self.repo))
        self.dest = self.d / "bin"
        self.rel = self.d / "releases"
        self.base = "file://" + str(self.rel) + "/{tag}"
        self.key = Key() if HAS_CRYPTO else None
        self.env = mock.patch.dict(os.environ, {"TESSERA_NATIVE_DEST": str(self.dest), "XDG_CONFIG_HOME": str(self.d / "config"),
                                                "TESSERA_UI_VARIANT": "x86-64-v3"})
        self.env.start()

    def tearDown(self):
        self.env.stop()
        self.tmp.cleanup()

    def commit_change(self, text):
        (self.repo / "native/tessera-ui/main.rs").write_text(text)
        git(self.repo, "-c", "user.name=t", "-c", "user.email=t@t", "commit", "-q", "-am", "change")
        self.tree, self.commit, _ = ni.native_tree("HEAD", str(self.repo))

    def publish(self, tag=ni.ROLLING, tree=None, commit=None, variants=("x86-64-v3", "generic"), version="0.1.0",
                key=None, manifest_edit=None, trusted=None, said=None, programs=None):
        """A release like the workflow's: tarballs, manifest, signatures."""
        tree, commit, key = tree or self.tree, commit or self.commit, key or self.key
        out = self.rel / tag
        out.mkdir(parents=True, exist_ok=True)
        entries = {}
        for v in variants:
            stage = programs or self.d / ("stage-" + v)
            stage.mkdir(exist_ok=True)
            progs = {}
            for name in ni.PROGRAMS:
                if not programs:
                    fake_program(stage / name, (said or {}).get(name, f"{name} {version}"))
                progs[name] = {"sha256": ni.sha256_of(str(stage / name)), "version": f"{name} {version}"}
            buf = io.BytesIO()
            with tarfile.open(fileobj=buf, mode="w") as tar:
                for name in ni.PROGRAMS:
                    tar.add(str(stage / name), arcname=("bin/" if programs else "") + name)
            plain = self.d / "plain.tar"
            plain.write_bytes(buf.getvalue())
            file = ni.tarball_name(tree, v)
            subprocess.run(["zstd", "-q", "-f", "-o", str(out / file), str(plain)], check=True)
            entries[v] = {"file": file, "sha256": ni.sha256_of(str(out / file)), "size": (out / file).stat().st_size, "programs": progs}
        manifest = {"format": 1, "name": "tessera-ui", "commit": commit, "tree": tree, "version": version,
                    "channel": "latest", "tag": tag, "variants": entries}
        if manifest_edit:
            manifest_edit(manifest)
        data = json.dumps(manifest).encode()
        mname = ni.manifest_name(tree)
        (out / mname).write_bytes(data)
        (out / (mname + ".minisig")).write_text(key.sign(data, trusted or f"{mname} tree={tree} commit={commit}"))
        return out

    def fetch(self, **kw):
        args = dict(base=self.base, tags=[ni.ROLLING], pubkey=self.key.pub, repo=str(self.repo))
        variant = kw.pop("variant", "x86-64-v3")
        args.update(kw)
        return ni.fetch(self.tree, self.commit, variant, str(self.dest / ".incoming-t"), **args)

    def install(self, mode=None, builder=None, fetcher=None, **kw):
        calls = []

        def fake_builder(stage, tuned, ui, say):
            calls.append(("build", tuned, ui))
            files = {}
            for name in ni.PROGRAMS if ui else ("tessera-events",):
                fake_program(Path(stage) / name, f"{name} 0.1.0")
                files[name] = str(Path(stage) / name)
            return files

        def real_fetcher(tree, commit, variant, work, repo=None):
            calls.append(("fetch", variant))
            return ni.fetch(tree, commit, variant, work, base=self.base, tags=ni.release_tags(commit, repo),
                            pubkey=self.key.pub if self.key else "", repo=repo)

        r = ni.install(mode, repo=str(self.repo), builder=builder or fake_builder, fetcher=fetcher or real_fetcher,
                       say=lambda s: None, **kw)
        return r, calls

    def installed(self, name="tessera-ui"):
        p = self.dest / name
        return p.read_text() if p.exists() else None


# ── The processor ─────────────────────────────────────────────────────────

class CpuLevel(unittest.TestCase):
    def test_avx2_machine_is_v3(self):
        self.assertEqual(ni.cpu_level(CPU_V3, "x86_64"), "x86-64-v3")

    def test_older_processors_are_generic(self):
        self.assertEqual(ni.cpu_level(CPU_SANDY, "x86_64"), "generic")
        self.assertEqual(ni.cpu_level(CPU_ATOM, "x86_64"), "generic")

    def test_each_v3_flag_is_needed(self):
        for flag in ("avx2", "bmi1", "bmi2", "fma", "movbe", "f16c", "xsave", "avx"):
            self.assertEqual(ni.cpu_level(cpu_without(flag), "x86_64"), "generic", flag)

    def test_lzcnt_as_abm_or_lzcnt(self):
        self.assertEqual(ni.cpu_level(cpu_without("abm"), "x86_64"), "generic")
        self.assertEqual(ni.cpu_level(cpu_without("abm").replace(" bmi1", " lzcnt bmi1"), "x86_64"), "x86-64-v3")

    def test_every_processor_must_have_them(self):
        # (one core without AVX2: a hybrid or a virtual machine's odd one)
        mixed = CPU_V3 + "processor\t: 2\nflags\t\t: " + cpu_without("avx2").splitlines()[2].split(":", 1)[1] + "\n"
        self.assertEqual(ni.cpu_level(mixed, "x86_64"), "generic")

    def test_other_machines_have_no_ready_made_build(self):
        self.assertIsNone(ni.cpu_level(CPU_V3, "aarch64"))
        self.assertEqual(ni.cpu_level("", "x86_64"), "generic")

    def test_this_machine_answers(self):
        self.assertIn(ni.detect_cpu(), ("x86-64-v3", "generic", None))


# ── minisign ──────────────────────────────────────────────────────────────

@unittest.skipUnless(HAS_CRYPTO, "python-cryptography isn't installed")
class Minisign(unittest.TestCase):
    def test_good_signatures_both_kinds(self):
        k = Key()
        for prehash in (True, False):
            self.assertEqual(ni.verify_minisign(k.pub, b"data", k.sign(b"data", "hello tree=x", prehash)), "hello tree=x")

    def test_other_data_key_or_comment_refused(self):
        k, other = Key(), Key()
        sig = k.sign(b"data", "tree=a")
        with self.assertRaises(ni.Failed):
            ni.verify_minisign(k.pub, b"datA", sig)
        with self.assertRaisesRegex(ni.Failed, "another key"):
            ni.verify_minisign(other.pub, b"data", sig)
        forged = sig.replace("trusted comment: tree=a", "trusted comment: tree=b")
        with self.assertRaises(ni.Failed):
            ni.verify_minisign(k.pub, b"data", forged)
        with self.assertRaises(ni.Failed):
            ni.verify_minisign(k.pub, b"data", "not a signature")

    @unittest.skipUnless(shutil.which("minisign"), "minisign isn't installed")
    def test_same_format_as_minisign(self):
        with tempfile.TemporaryDirectory() as d:
            d = Path(d)
            subprocess.run(["minisign", "-G", "-W", "-p", str(d / "k.pub"), "-s", str(d / "k.key")], check=True, capture_output=True)
            (d / "f").write_bytes(b"payload")
            subprocess.run(["minisign", "-S", "-s", str(d / "k.key"), "-m", str(d / "f"), "-t", "x tree=t"], check=True, capture_output=True)
            self.assertEqual(ni.verify_minisign((d / "k.pub").read_text(), b"payload", (d / "f.minisig").read_text()), "x tree=t")
            k = Key()
            (d / "g").write_bytes(b"other")
            (d / "g.minisig").write_text(k.sign(b"other", "y"))
            (d / "mine.pub").write_text(k.pub)
            r = subprocess.run(["minisign", "-V", "-p", str(d / "mine.pub"), "-m", str(d / "g")], capture_output=True)
            self.assertEqual(r.returncode, 0, r.stderr)


# ── Finding and checking a ready-made build ───────────────────────────────

@unittest.skipUnless(HAS_CRYPTO, "python-cryptography isn't installed")
class Fetch(Fixture):
    def test_the_build_for_this_tree(self):
        self.publish()
        got = self.fetch()
        self.assertEqual(got["info"]["source"], "ready-made")
        self.assertEqual(got["info"]["variant"], "x86-64-v3")
        self.assertEqual(got["info"]["commit"], self.commit)
        self.assertEqual(sorted(got["files"]), sorted(ni.PROGRAMS))

    def test_assets_are_named_by_the_native_tree(self):
        self.publish()
        old = self.tree
        # A commit that changes only QML: the same tree, the same build
        (self.repo / "shell.qml").write_text("x")
        git(self.repo, "-c", "user.name=t", "-c", "user.email=t@t", "add", "-A")
        git(self.repo, "-c", "user.name=t", "-c", "user.email=t@t", "commit", "-q", "-m", "qml")
        tree, commit, _ = ni.native_tree("HEAD", str(self.repo))
        self.assertEqual(tree, old)
        self.assertEqual(ni.fetch(tree, commit, "x86-64-v3", str(self.dest / ".w"), base=self.base, tags=[ni.ROLLING],
                                  pubkey=self.key.pub, repo=str(self.repo))["info"]["tree"], tree)
        # One that changes native/: no build for it yet
        self.commit_change("fn main() { println!(); }\n")
        with self.assertRaises(ni.Failed) as e:
            self.fetch()
        self.assertEqual(e.exception.kind, "missing")

    def test_a_tag_at_the_commit_comes_first(self):
        git(self.repo, "tag", "v1.0")
        self.assertEqual(ni.release_tags(self.commit, str(self.repo)), ["v1.0", ni.ROLLING])
        self.publish(tag="v1.0")
        self.assertEqual(self.fetch(tags=ni.release_tags(self.commit, str(self.repo)))["info"]["tag"], "v1.0")

    def test_generic_when_there_is_no_tuned_one(self):
        self.publish(variants=("generic",))
        self.assertEqual(self.fetch()["info"]["variant"], "generic")
        with self.assertRaises(ni.Failed):
            self.fetch(variant=None)

    def test_manifest_for_another_tree_refused(self):
        self.publish(manifest_edit=lambda m: m.update(tree="0" * 40))
        with self.assertRaisesRegex(ni.Failed, "another version"):
            self.fetch()

    def test_signed_comment_for_another_tree_refused(self):
        self.publish(trusted="tessera-ui manifest tree=" + "1" * 40)
        with self.assertRaisesRegex(ni.Failed, "another version"):
            self.fetch()

    def test_manifest_commit_with_other_sources_refused(self):
        first = self.commit
        self.commit_change("fn main() { loop {} }\n")
        # Signed for this tree, but naming a commit this checkout knows with other sources
        self.publish(commit=first)
        with self.assertRaisesRegex(ni.Failed, "other sources"):
            self.fetch()

    def test_bad_signature_refused(self):
        self.publish(key=Key())
        with self.assertRaisesRegex(ni.Failed, "another key"):
            self.fetch()

    def test_bad_checksum_refused(self):
        out = self.publish()
        t = out / ni.tarball_name(self.tree, "x86-64-v3")
        data = bytearray(t.read_bytes())
        data[-1] ^= 1
        t.write_bytes(bytes(data))
        with self.assertRaisesRegex(ni.Failed, "checksum"):
            self.fetch()

    def test_program_that_says_another_version_refused(self):
        self.publish(said={"tessera-ui": "tessera-ui 9.9.9"})
        with self.assertRaisesRegex(ni.Failed, "not tessera-ui 0.1.0"):
            self.fetch()

    def test_unsigned_or_no_key(self):
        out = self.publish()
        os.remove(out / (ni.manifest_name(self.tree) + ".minisig"))
        with self.assertRaisesRegex(ni.Failed, "isn't signed"):
            self.fetch()
        with self.assertRaisesRegex(ni.Failed, "no release key"):
            self.fetch(pubkey="")

    def test_only_https(self):
        with self.assertRaises(ni.Failed) as e:
            ni.download("http://example.invalid/x", str(self.d / "x"), 10)
        self.assertEqual(e.exception.kind, "network")

    def test_releases_come_from_the_github_remote(self):
        with mock.patch.dict(os.environ, {"TESSERA_RELEASES": ""}):
            git(self.repo, "remote", "add", "origin", "https://github.com/someone/Tessera.git")
            self.assertEqual(ni.releases_base(str(self.repo)), "https://github.com/someone/Tessera/releases/download/{tag}")
            git(self.repo, "remote", "set-url", "origin", "git@github.com:someone/Tessera.git")
            self.assertEqual(ni.releases_base(str(self.repo)), "https://github.com/someone/Tessera/releases/download/{tag}")
            git(self.repo, "remote", "set-url", "origin", "https://example.org/x.git")
            self.assertIsNone(ni.releases_base(str(self.repo)))


# ── Installing: atomic, keeping .prev, undo ───────────────────────────────

@unittest.skipUnless(HAS_CRYPTO, "python-cryptography isn't installed")
class Install(Fixture):
    def test_ready_made_then_a_new_version_keeps_prev(self):
        self.publish()
        r, calls = self.install()
        self.assertTrue(r["ok"], r)
        self.assertEqual(r["label"], "ready-made (v3)")
        self.assertEqual(calls, [("fetch", "x86-64-v3")])
        first = self.installed()
        self.assertEqual(ni.read_info(str(self.dest), "tessera-ui")["tree"], self.tree)
        old_tree = self.tree
        self.commit_change("fn main() { 1; }\n")
        self.publish(version="0.1.1")
        r, _ = self.install()
        self.assertTrue(r["ok"], r)
        self.assertIn("0.1.1", self.installed())
        self.assertEqual((self.dest / "tessera-ui.prev").read_text(), first)
        self.assertEqual(ni.read_info(str(self.dest), "tessera-ui", prev=True)["tree"], old_tree)
        self.assertFalse([p for p in self.dest.iterdir() if p.name.startswith((".incoming", ".built")) or ".tmp-" in p.name or p.name.endswith(".new")])

    def test_already_installed_does_nothing(self):
        self.publish()
        self.install()
        r, calls = self.install()
        self.assertTrue(r["ok"])
        self.assertFalse(r["changed"])
        self.assertEqual(calls, [])

    def test_going_back_reuses_prev_without_download(self):
        self.publish()
        self.install()
        old = self.installed()
        first = self.commit
        self.commit_change("fn main() { 2; }\n")
        self.publish(version="0.2.0")
        self.install()
        # The code goes back (the updater's undo): the earlier build comes back as it was
        git(self.repo, "reset", "-q", "--hard", first)
        shutil.rmtree(self.rel)
        r, calls = self.install(offline=True, only_prev=True)
        self.assertTrue(r["ok"], r)
        self.assertEqual(calls, [])
        self.assertEqual(self.installed(), old)
        self.assertIn("0.2.0", (self.dest / "tessera-ui.prev").read_text())
        # (and nothing else when the .prev one isn't this version's)
        self.commit_change("fn main() { 3; }\n")
        r, calls = self.install(only_prev=True)
        self.assertFalse(r["ok"])
        self.assertEqual(calls, [])

    def test_restore_swaps_both_ways(self):
        for name in ni.PROGRAMS:
            self.dest.mkdir(exist_ok=True)
            fake_program(self.dest / name, name + " new")
            fake_program(self.dest / (name + ".prev"), name + " old")
        (self.dest / "tessera-ui.json").write_text(json.dumps({"source": "ready-made", "tree": "b"}))
        (self.dest / "tessera-ui.prev.json").write_text(json.dumps({"source": "built", "tree": "a"}))
        self.assertEqual(ni.restore(str(self.dest)), list(ni.PROGRAMS))
        self.assertIn("old", self.installed())
        self.assertEqual(ni.read_info(str(self.dest), "tessera-ui")["tree"], "a")
        self.assertEqual(ni.read_info(str(self.dest), "tessera-ui", prev=True)["tree"], "b")
        ni.restore(str(self.dest))
        self.assertIn("new", self.installed())

    def test_bad_signature_keeps_the_installed_build(self):
        self.publish()
        self.install()
        before = self.installed()
        self.commit_change("fn main() { 4; }\n")
        self.publish(key=Key(), version="6.6.6")

        def no_build(stage, tuned, ui, say):
            raise ni.Failed("cargo not found", "build")
        r, _ = self.install(builder=no_build)
        self.assertFalse(r["ok"])
        self.assertIn("another key", r["why"])
        self.assertEqual(self.installed(), before)
        self.assertFalse((self.dest / "tessera-ui.prev").exists())

    def test_bad_checksum_keeps_the_installed_build(self):
        self.publish()
        self.install()
        before = self.installed()
        self.commit_change("fn main() { 5; }\n")
        out = self.publish(version="6.6.6")
        (out / ni.tarball_name(self.tree, "x86-64-v3")).write_bytes(b"\x28\xb5\x2f\xfd garbage")
        r, _ = self.install(builder=lambda *a: (_ for _ in ()).throw(ni.Failed("no", "build")))
        self.assertFalse(r["ok"])
        self.assertIn("checksum", r["why"])
        self.assertEqual(self.installed(), before)

    def test_swap_is_one_rename_and_the_program_is_always_whole(self):
        self.dest.mkdir()
        fake_program(self.dest / "tessera-ui", "tessera-ui old")
        new = self.dest / ".incoming-x"
        new.mkdir()
        fake_program(new / "tessera-ui", "tessera-ui new")
        seen = []
        real = os.replace

        def watch(src, dst):
            seen.append((os.path.basename(src), os.path.basename(dst)))
            # Every moment: tessera-ui is there, and whole (old or new)
            self.assertIn(self.installed(), ("#!/bin/sh\n[ \"$1\" = --version ] && echo 'tessera-ui old' && exit 0\nexit 3\n",
                                             "#!/bin/sh\n[ \"$1\" = --version ] && echo 'tessera-ui new' && exit 0\nexit 3\n"))
            return real(src, dst)
        with mock.patch.object(ni.os, "replace", watch):
            ni.swap_in(str(self.dest), {"tessera-ui": str(new / "tessera-ui")}, {"source": "ready-made", "tree": "t"})
        self.assertIn(("tessera-ui.new", "tessera-ui"), seen)
        self.assertIn("new", self.installed())
        self.assertIn("old", (self.dest / "tessera-ui.prev").read_text())

    def test_a_failed_swap_leaves_the_old_program(self):
        self.dest.mkdir()
        fake_program(self.dest / "tessera-ui", "tessera-ui old")
        new = self.dest / ".incoming-y"
        new.mkdir()
        fake_program(new / "tessera-ui", "tessera-ui new")
        real = os.replace

        def fail_last(src, dst):
            if os.path.basename(dst) == "tessera-ui":
                raise OSError("disk full")
            return real(src, dst)
        with mock.patch.object(ni.os, "replace", fail_last), self.assertRaises(OSError):
            ni.swap_in(str(self.dest), {"tessera-ui": str(new / "tessera-ui")}, {"source": "ready-made"})
        self.assertIn("old", self.installed())

    def test_one_install_at_a_time(self):
        self.dest.mkdir()
        with ni.Lock(str(self.dest)):
            r, calls = self.install()
        self.assertFalse(r["ok"])
        self.assertEqual(r["kind"], "busy")
        self.assertEqual(calls, [])

    @unittest.skipUnless(os.environ.get("TESSERA_UI_TEST_BINARY") or (ROOT / "native/tessera-ui/target/release/tessera-ui").exists(),
                         "no built tessera-ui here")
    def test_a_real_tessera_ui_build(self):
        # A real build (main's), packed and signed as the workflow does:
        # checked by --version (run without any display) and installed
        real = Path(os.environ.get("TESSERA_UI_TEST_BINARY") or ROOT / "native/tessera-ui/target/release/tessera-ui")
        said = ni.program_version(str(real))
        self.assertRegex(said, r"^tessera-ui \d")
        stage = self.d / "real"
        stage.mkdir()
        shutil.copy2(real, stage / "tessera-ui")
        fake_program(stage / "tessera-events", "tessera-events " + said.split()[1])
        self.publish(version=said.split()[1], programs=stage, variants=("x86-64-v3",))
        r, _ = self.install()
        self.assertTrue(r["ok"], r)
        self.assertEqual(ni.sha256_of(str(self.dest / "tessera-ui")), ni.sha256_of(str(real)))
        self.assertEqual(ni.program_version(str(self.dest / "tessera-ui")), said)


# ── Modes, and the fallback ───────────────────────────────────────────────

@unittest.skipUnless(HAS_CRYPTO, "python-cryptography isn't installed")
class Modes(Fixture):
    def config(self, **rust):
        p = self.d / "config/tessera/config.json"
        p.parent.mkdir(parents=True, exist_ok=True)
        p.write_text(json.dumps({"rust": rust}))

    def test_auto_fetches(self):
        self.publish()
        r, calls = self.install()
        self.assertEqual(r["source"], "ready-made")
        self.assertEqual(calls, [("fetch", "x86-64-v3")])

    def test_build_and_build_tuned_never_download(self):
        self.publish()
        r, calls = self.install("build")
        self.assertEqual((r["source"], r["label"]), ("built", "built here"))
        self.assertEqual(calls, [("build", False, False)])
        r, calls = self.install("buildTuned", ui=True)
        self.assertEqual((r["source"], r["label"]), ("built-tuned", "built here, tuned"))
        self.assertEqual(calls, [("build", True, True)])

    def test_the_mode_comes_from_settings(self):
        self.config(uiSource="buildTuned", surfaces=["dock"])
        r, calls = self.install()
        self.assertEqual(calls, [("build", True, True)])  # (tessera-ui is used: built too)
        self.config(uiSource="nonsense")
        self.assertEqual(ni.configured_mode(), "auto")

    def test_a_changed_mode_reinstalls(self):
        self.publish()
        self.install()
        r, calls = self.install("build")
        self.assertEqual(calls, [("build", False, False)])  # (tessera-ui is built here only when used)
        r, calls = self.install()  # auto takes a build made here as it is
        self.assertEqual(calls, [])

    def test_fallback_when_there_is_no_asset(self):
        r, calls = self.install()
        self.assertTrue(r["ok"], r)
        self.assertEqual(r["source"], "built")
        self.assertEqual(calls, [("fetch", "x86-64-v3"), ("build", False, False)])
        self.assertIn("no ready-made build", r["fallback"])

    def test_fallback_offline_or_with_edits(self):
        self.publish()
        r, calls = self.install(offline=True)
        self.assertEqual(calls, [("build", False, False)])
        self.assertEqual(r["fallback"], "offline")
        (self.repo / "native/tessera-ui/main.rs").write_text("// mine\n")
        r, calls = self.install()
        self.assertEqual(calls, [("build", False, False)])
        self.assertIn("edits of its own", r["fallback"])
        self.assertIsNone(ni.read_info(str(self.dest), "tessera-events")["tree"])  # (never taken for a clean build)

    def test_fallback_without_a_release_key(self):
        self.publish()

        def keyless(tree, commit, variant, work, repo=None):
            return ni.fetch(tree, commit, variant, work, base=self.base, tags=[ni.ROLLING], pubkey="", repo=repo)
        r, calls = self.install(fetcher=keyless)
        self.assertEqual(r["source"], "built")
        self.assertIn("no release key", r["fallback"])

    def test_build_failure_keeps_everything(self):
        def broken(stage, tuned, ui, say):
            raise ni.Failed("it didn't build (error[E0308])", "build")
        r, _ = self.install(builder=broken)
        self.assertFalse(r["ok"])
        self.assertIn("E0308", r["why"])
        self.assertIn("no ready-made build", r["why"])
        self.assertFalse(self.dest.joinpath("tessera-events").exists())

    def test_tessera_ui_that_does_not_build_still_installs_events(self):
        def half(stage, tuned, ui, say):
            fake_program(Path(stage) / "tessera-events", "tessera-events 0.1.0")
            return {"tessera-events": str(Path(stage) / "tessera-events")}
        r, _ = self.install("build", builder=half, ui=True)
        self.assertFalse(r["ok"])
        self.assertIn("QML", r["message"])
        self.assertTrue((self.dest / "tessera-events").exists())

    def test_plan_labels(self):
        with mock.patch.dict(os.environ, {"TESSERA_RELEASES": self.base}):
            key = self.d / "k.pub"
            key.write_text(self.key.pub)
            with mock.patch.dict(os.environ, {"TESSERA_UI_PUBKEY": str(key)}):
                self.assertEqual(ni.plan(mode="build", repo=str(self.repo))["label"], "tessera-ui: built here")
                self.assertEqual(ni.plan(mode="buildTuned", repo=str(self.repo))["label"], "tessera-ui: built here, tuned")
                self.assertEqual(ni.plan(mode="auto", repo=str(self.repo))["label"], "tessera-ui: ready-made (v3)")
                p = ni.plan(mode="auto", online=True, repo=str(self.repo))
                self.assertEqual(p["label"], "tessera-ui: built here (no ready-made build for it yet)")
                self.publish()
                self.assertEqual(ni.plan(mode="auto", online=True, repo=str(self.repo))["available"], True)
                with mock.patch.dict(os.environ, {"TESSERA_UI_VARIANT": "generic"}):
                    self.assertEqual(ni.plan(mode="auto", repo=str(self.repo))["label"], "tessera-ui: ready-made (generic)")
            with mock.patch.dict(os.environ, {"TESSERA_UI_PUBKEY": str(self.d / "none.pub")}):
                self.assertEqual(ni.plan(mode="auto", repo=str(self.repo))["label"], "tessera-ui: built here (no release key yet)")

    def test_can_build(self):
        enough = "MemAvailable:   8000000 kB\n"
        self.assertTrue(ni.can_build(meminfo=enough, free_disk=10 * ni.GIB, cargo="/usr/bin/cargo")["build"]["ok"])
        r = ni.can_build(meminfo="MemAvailable:   1000000 kB\n", free_disk=1 * ni.GIB, cargo="")
        self.assertFalse(r["buildTuned"]["ok"])
        for why in ("Rust (cargo)", "free memory", "free disk"):
            self.assertIn(why, r["build"]["why"])
        self.assertNotIn("cargo", ni.can_build(packages_coming=True, meminfo=enough, free_disk=10 * ni.GIB, cargo="")["build"]["why"])


# ── The command line, and build-native.sh going through it ────────────────

@unittest.skipUnless(HAS_CRYPTO, "python-cryptography isn't installed")
class CommandLine(Fixture):
    def setUp(self):
        super().setUp()
        (self.repo / "tools").mkdir()
        shutil.copy2(HELPER, self.repo / "tools/native-install.py")
        shutil.copy2(ROOT / "tools/build-native.sh", self.repo / "tools/build-native.sh")
        (self.repo / "tools/tessera-ui.pub").write_text(self.key.pub)
        (self.repo / "shell/assets/scripts").mkdir(parents=True)
        (self.repo / "shell/assets/scripts/migrate-names.py").write_text("")
        self.builder = self.d / "builder.sh"
        self.builder.write_text("#!/bin/sh\n# --stage DIR [--tuned] [--ui]\necho \"$@\" > " + str(self.d / "built-with") + "\n"
                                "printf '#!/bin/sh\\necho tessera-events 0.1.0\\n' > \"$2/tessera-events\"; chmod +x \"$2/tessera-events\"\n")
        self.builder.chmod(0o755)
        self.run_env = dict(os.environ, TESSERA_RELEASES=self.base, TESSERA_NATIVE_BUILDER=str(self.builder))
        for k in ("WAYLAND_DISPLAY", "WAYLAND_SOCKET", "DISPLAY"):
            self.run_env.pop(k, None)

    def cli(self, *args):
        p = subprocess.run([sys.executable, "-I", str(self.repo / "tools/native-install.py"), *args], env=self.run_env,
                           capture_output=True, text=True, timeout=60)
        return p

    def test_install_json_and_status(self):
        self.publish()
        p = self.cli("install", "--json", "--progress")
        r = json.loads(p.stdout)
        self.assertTrue(r["ok"], p.stderr)
        self.assertEqual(r["label"], "ready-made (v3)")
        self.assertIn("STEP Getting tessera-ui (ready-made, v3)", p.stderr)
        s = json.loads(self.cli("status", "--json").stdout)
        self.assertEqual(s["tessera-ui"]["label"], "ready-made (v3)")
        self.assertEqual(self.cli("restore").returncode, 1)  # (nothing before it)

    def test_build_native_goes_through_it(self):
        p = subprocess.run(["bash", str(self.repo / "tools/build-native.sh"), "--tuned"], env=self.run_env,
                           capture_output=True, text=True, timeout=60)
        self.assertEqual(p.returncode, 0, p.stdout + p.stderr)
        self.assertIn("--tuned", (self.d / "built-with").read_text())
        self.assertEqual(ni.read_info(str(self.dest), "tessera-events")["source"], "built-tuned")
        p = subprocess.run(["bash", str(self.repo / "tools/build-native.sh"), "--nope"], env=self.run_env, capture_output=True, text=True)
        self.assertEqual(p.returncode, 2)

    def test_stage_without_cargo_says_so(self):
        env = dict(self.run_env, PATH=str(self.d / "nothing"))
        p = subprocess.run(["/bin/bash", str(ROOT / "tools/build-native.sh"), "--stage", str(self.d / "s")], env=env,
                           capture_output=True, text=True)
        self.assertNotEqual(p.returncode, 0)
        self.assertIn("cargo not found", p.stdout)


# ── The updater and the installers use it ─────────────────────────────────

class Wiring(unittest.TestCase):
    def test_updater_installs_with_the_pull_and_puts_it_back_on_failure(self):
        src = (ROOT / "tools/tessera-update.py").read_text()
        self.assertIn('report["ui"] = native_plan(new_id)', src)
        apply_part = src[src.index("def apply("):]
        # (after the update's code is in, inside the part that goes back on failure)
        self.assertLess(apply_part.index("apply_after_merge(config_dir"), apply_part.index("ui = native_install(40)"))
        self.assertLess(apply_part.index("ui = native_install(40)"), apply_part.index("except BaseException as e:"))
        self.assertIn('native_install(85, "--offline", "--only-prev")', apply_part)

    def test_software_update_shows_the_mode(self):
        page = (ROOT / "shell/modules/settings/pages/update.qml").read_text()
        self.assertIn('page.report.ui?.label ? " · " + page.report.ui.label', page)

    def test_setting_and_default(self):
        cfg = (ROOT / "shell/common/Config.qml").read_text()
        self.assertIn('property string uiSource: "auto"', cfg)

    def test_install_sh_offers_it(self):
        sh = (ROOT / "install.sh").read_text()
        self.assertIn("--ui-source=auto|--ui-source=build|--ui-source=buildTuned", sh)
        self.assertIn('part("rust")["uiSource"] = ui_source', sh)
        self.assertIn('"$ROOT/tools/build-native.sh" ${UI_SOURCE:+--source=$UI_SOURCE}', sh)

    def test_workflow(self):
        wf = (ROOT / ".github/workflows/tessera-ui.yml").read_text()
        for needle in ("archlinux:base-devel", "-C target-cpu=x86-64-v3", "actions/cache@v4", "TODO(PGO)",
                       "TESSERA_UI_MINISIGN_KEY", "tools/tessera-ui.pub", "tessera-ui-latest", "--prerelease",
                       'tags: ["v*"]', '"native/**"', "minisign -V", "tree=$TREE"):
            self.assertIn(needle, wf)
        # (the names the helper asks for)
        self.assertIn("tessera-ui-$TREE-$VARIANT.tar.zst", wf)
        self.assertIn('"out/tessera-ui-$tree.manifest.json"', wf)

    def test_no_private_key_in_the_repo(self):
        for p in (ROOT / "tools").iterdir():
            if p.is_file() and p.suffix in (".key", ".pub", ""):
                text = p.read_text(errors="replace")[:200]
                self.assertNotIn("minisign encrypted secret key", text, p)


class InstallerAdvanced(unittest.TestCase):
    """The interactive installer's Advanced choice (keys and flags; drawing
    needs python-rich and is tested in test_installer_tui.py)."""

    def tui(self, can=None):
        spec = importlib.util.spec_from_file_location("tessera_tui_adv", ROOT / "tools/installer/tui.py")
        mod = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(mod)
        facts = {"cpu": "intel", "installed": [], "pci": [], "screens": [], "hyprland": "0.56.2", "display_manager": ""}
        return mod.Installer(facts, [], {"apps_missing": False, "walls_ready": True,
                                         "can_build": can or {m: {"ok": True, "why": ""} for m in ("build", "buildTuned")}})

    def to_review(self, ins):
        while ins.name != "review":
            ins.key("enter")

    def test_default_asks_nothing_and_downloads(self):
        ins = self.tui()
        self.to_review(ins)
        self.assertNotIn("advanced", [p[0] for p in ins.pages])
        self.assertFalse(any(a.startswith("--ui-source") for a in ins.args()))

    def test_advanced_from_the_review(self):
        ins = self.tui()
        self.to_review(ins)
        ins.key("a")
        self.assertEqual(ins.name, "advanced")
        ins.key("down")
        self.assertEqual(ins.key("enter"), "review")
        self.assertIn("--ui-source=buildTuned", ins.args())
        ins.key("a")
        ins.key("down")
        ins.key("enter")
        self.assertIn("--ui-source=build", ins.args())

    def test_what_cannot_work_is_greyed_and_skipped(self):
        ins = self.tui({m: {"ok": False, "why": "needs Rust (cargo)"} for m in ("build", "buildTuned")})
        self.to_review(ins)
        ins.key("a")
        ins.key("down")
        ins.key("down")
        ins.key("enter")
        self.assertEqual(ins.uisource.value, "auto")
        self.assertEqual(ins.uisource.disabled["build"], "needs Rust (cargo)")


if __name__ == "__main__":
    unittest.main()
