"""appstore.py: the App Store's search across the Arch repos' AppStream
catalogue, the AUR and Flathub (a stand-in catalogue and network)."""
import gzip
import importlib.util
import os
import tempfile
import unittest
from pathlib import Path

SCRIPT = Path(__file__).resolve().parent.parent / "shell/assets/scripts/appstore.py"

CATALOGUE = """<?xml version="1.0" encoding="utf-8"?>
<components version="1.0" origin="archlinux-arch-extra">
<component type="desktop-application">
  <id>md.obsidian.Obsidian.desktop</id>
  <name>Obsidian</name>
  <name xml:lang="fr">Obsidien</name>
  <summary>A knowledge base on local Markdown files</summary>
  <description><p>Make and follow connections.</p><p>Plugins.</p></description>
  <pkgname>obsidian</pkgname>
  <launchable type="desktop-id">obsidian.desktop</launchable>
  <icon type="cached" width="64" height="64">obsidian_obsidian.jxl</icon>
  <icon type="stock">obsidian</icon>
  <categories><category>Office</category></categories>
  <url type="homepage">https://obsidian.md</url>
</component>
<component type="desktop-application">
  <id>code-oss.desktop</id>
  <name>Code - OSS</name>
  <summary>Code editing. Redefined.</summary>
  <pkgname>code</pkgname>
  <icon type="stock">com.visualstudio.code.oss</icon>
  <categories><category>Development</category></categories>
  <keywords><keyword>vscode</keyword></keywords>
</component>
<component type="console-application">
  <id>htop</id><name>htop</name><summary>Process viewer</summary><pkgname>htop</pkgname>
</component>
</components>
"""


def load():
    spec = importlib.util.spec_from_file_location("appstore", SCRIPT)
    m = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(m)
    return m


class AppStore(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        root = Path(self.tmp.name)
        (root / "as/xml").mkdir(parents=True)
        with gzip.open(root / "as/xml/extra.xml.gz", "wt") as f:
            f.write(CATALOGUE)
        self.env = {"TESSERA_APPSTREAM": str(root / "as"), "XDG_CACHE_HOME": str(root / "cache")}
        self.old = {k: os.environ.get(k) for k in self.env}
        os.environ.update(self.env)
        self.m = load()

    def tearDown(self):
        for k, v in self.old.items():
            if v is None:
                os.environ.pop(k, None)
            else:
                os.environ[k] = v
        self.tmp.cleanup()

    def test_catalogue_has_the_desktop_apps(self):
        apps = self.m.load_catalogue()
        self.assertEqual([a["pkg"] for a in apps], ["obsidian", "code"])  # (not the console tool)
        o = apps[0]
        self.assertEqual((o["name"], o["repo"], o["iconName"], o["desktop"]), ("Obsidian", "extra", "obsidian", "obsidian.desktop"))
        self.assertEqual(o["description"], "Make and follow connections.\n\nPlugins.")
        # read once: the second time from the cache
        self.assertTrue(list(Path(self.env["XDG_CACHE_HOME"], "tessera/appstore").glob("repo-*.json")))

    def test_ranking(self):
        m = self.m
        apps = [
            {"source": "aur", "pkg": "vscode-bookmark-bin", "name": "Vscode Bookmark", "popularity": 0.01, "votes": 1},
            {"source": "flathub", "pkg": "com.visualstudio.code", "name": "Visual Studio Code", "keywords": ["vscode"], "installs": 90000},
            {"source": "aur", "pkg": "google-chrome", "name": "Google Chrome", "popularity": 9.2, "votes": 900},
            {"source": "aur", "pkg": "chrome-remote-desktop", "name": "Chrome Remote", "popularity": 0.4, "votes": 20},
        ]
        # a keyword beats a package that only starts with the word
        self.assertEqual(m.rank(apps, "vscode")[0]["pkg"], "com.visualstudio.code")
        # a whole word of the name beats a prefix of a little-used package
        self.assertEqual(m.rank(apps, "chrome")[0]["pkg"], "google-chrome")
        # one app in several places: the repos' copy first
        same = [{"source": "flathub", "pkg": "md.obsidian.Obsidian", "name": "Obsidian", "installs": 70000},
                {"source": "repo", "pkg": "obsidian", "name": "Obsidian"}]
        self.assertEqual([a["source"] for a in m.rank(same, "obsidian")], ["repo", "flathub"])

    def test_aur_apps_only_one_per_app_and_no_add_ons(self):
        results = [{"Name": n, "Popularity": p, "NumVotes": 1} for n, p in [
            ("spotify", 4.1), ("spotify-git", 0.1), ("gimp-plugin-foo", 1), ("python-spotipy", 2),
            ("obsidian-theme-x", 1), ("firefox-bin", 3), ("foo-debug", 1)]]
        apps = self.m.aur_apps(results, {"firefox"})
        self.assertEqual(sorted(a["pkg"] for a in apps), ["spotify"])  # (firefox: the repos have it)
        self.assertEqual(apps[0]["name"], "Spotify")

    def test_logos_are_borrowed_only_for_the_same_app(self):
        m = self.m
        flathub = [m.flathub_app({"app_id": "com.spotify.Client", "name": "Spotify", "icon": "https://x/spotify.png"}),
                   m.flathub_app({"app_id": "app.zen_browser.zen", "name": "Zen", "icon": "https://x/zen.png"})]
        aur = [
            {"source": "aur", "pkg": "spotify", "name": "Spotify", "summary": "A music streaming service", "homepage": "https://www.spotify.com", "popularity": 4, "votes": 275, "icon": ""},
            {"source": "aur", "pkg": "zen-browser-bin", "name": "Zen Browser", "summary": "Official package for Zen, a Firefox-based web browser", "homepage": "https://github.com/zen-browser/desktop", "popularity": 8.8, "votes": 342, "icon": ""},
            # a little-known package that only shares the name
            {"source": "aur", "pkg": "zen", "name": "Zen", "summary": "Reduce your stress with the C language", "homepage": "https://zen-lang.org", "popularity": 0, "votes": 1, "icon": ""},
        ]
        out = {a["pkg"]: a for a in m.borrow_icons(aur, flathub)}
        self.assertEqual(out["spotify"]["icon"], "https://x/spotify.png")
        self.assertEqual(out["zen-browser-bin"]["icon"], "https://x/zen.png")
        self.assertEqual(out["zen"]["icon"], "")

    def test_search_merges_the_sources_and_survives_one_offline(self):
        m = self.m

        def fetch(url, data=None, timeout=None):
            if "aur.archlinux.org" in url:
                raise OSError("offline")
            return {"hits": [{"app_id": "md.obsidian.Obsidian", "name": "Obsidian", "summary": "Markdown", "type": "desktop-application",
                              "icon": "https://x/o.png", "installs_last_month": 69000}]}
        store = m.Store(fetch=fetch)
        apps = store.search("obsidian", sources=["repo", "flathub", "aur"])
        self.assertEqual([(a["source"], a["pkg"]) for a in apps], [("repo", "obsidian"), ("flathub", "md.obsidian.Obsidian")])
        self.assertTrue(all("installed" in a for a in apps))
        self.assertNotIn("keywords", apps[0])
        self.assertEqual(store.search("o"), [])  # (too short to look)

    def test_the_aur_only_when_asked(self):
        m = self.m
        asked = []

        def fetch(url, data=None, timeout=None):
            asked.append(url)
            if "aur.archlinux.org" in url:
                return {"results": [{"Name": "spotify", "Description": "A music streaming service", "URL": "https://www.spotify.com", "Popularity": 4, "NumVotes": 275}]}
            return {"hits": [{"app_id": "com.spotify.Client", "name": "Spotify", "type": "desktop-application", "icon": "https://x/s.png"}]}
        store = m.Store(fetch=fetch)
        self.assertEqual([a["source"] for a in store.search("spotify")], ["flathub"])
        self.assertFalse(any("aur.archlinux.org" in u for u in asked))  # (not even asked)
        aur = store.search("aur: spotify")
        self.assertEqual([(a["source"], a["pkg"]) for a in aur], [("aur", "spotify")])
        self.assertEqual(aur[0]["icon"], "https://x/s.png")  # (Flathub's logo, the same app)

    def test_repo_matches_first_then_all_and_a_repeat_from_memory(self):
        m = self.m
        calls = []

        def fetch(url, data=None, timeout=None):
            calls.append(url)
            return {"hits": [{"app_id": "md.obsidian.Obsidian", "name": "Obsidian", "type": "desktop-application", "icon": "https://x/o.png"}]}
        store = m.Store(fetch=fetch)
        early = []
        apps = store.search("obsidian", partial=early.append)
        self.assertEqual([a["source"] for a in early[0]], ["repo"])  # (before the network)
        self.assertEqual([a["source"] for a in apps], ["repo", "flathub"])
        n = len(calls)
        store.search("Obsidian ")  # (the same search)
        self.assertEqual(len(calls), n)

    def test_only_the_newest_search_is_done(self):
        reqs = [{"id": 1, "op": "search", "q": "ob"}, {"id": 2, "op": "status"}, {"id": 3, "op": "search", "q": "obsi"}]
        todo, stale = self.m.latest_only(reqs)
        self.assertEqual([r["id"] for r in todo], [2, 3])
        self.assertEqual([r["id"] for r in stale], [1])


if __name__ == "__main__":
    unittest.main()
