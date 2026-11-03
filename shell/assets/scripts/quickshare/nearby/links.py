"""Which links worked with which phone: fastest first, and a link that
keeps failing with one phone is skipped for it for a while.

Each transfer tries the fastest link first and gives each one a second
chance before falling back (service.py). What happened is remembered per
phone (by its name: endpoint ids change every time) and per link:
FAILS_TO_SKIP failures in a row and that link is left out for that phone
for SKIP_FOR seconds; one success forgets its failures. Stored in
$XDG_STATE_HOME/irice/quickshare-links.json (names and counts only).
"""
import json
import os
import time

FAILS_TO_SKIP = 2
SKIP_FOR = 24 * 3600
MAX_PEERS = 64  # (oldest forgotten first)


def default_path():
    state = os.environ.get("XDG_STATE_HOME") or os.path.expanduser("~/.local/state")
    return os.path.join(state, "irice", "quickshare-links.json")


class LinkMemory:
    def __init__(self, path=None, clock=time.time):
        self.path = path if path is not None else default_path()
        self.clock = clock
        self.peers = {}
        self._load()

    def _load(self):
        if not self.path:
            return
        try:
            with open(self.path, encoding="utf-8") as f:
                data = json.load(f)
            if isinstance(data, dict):
                self.peers = {str(k): v for k, v in data.items() if isinstance(v, dict)}
        except (OSError, ValueError):
            self.peers = {}

    def _save(self):
        if not self.path:
            return
        if len(self.peers) > MAX_PEERS:
            newest = sorted(self.peers, key=lambda p: max((m.get("at", 0) for m in self.peers[p].values()), default=0))
            for p in newest[:len(self.peers) - MAX_PEERS]:
                del self.peers[p]
        try:
            os.makedirs(os.path.dirname(self.path), exist_ok=True)
            tmp = self.path + ".tmp"
            with open(os.open(tmp, os.O_WRONLY | os.O_CREAT | os.O_TRUNC, 0o600), "w", encoding="utf-8") as f:
                json.dump(self.peers, f)
            os.replace(tmp, self.path)
        except OSError:
            pass

    def skip(self, peer, link):
        m = self.peers.get(peer or "", {}).get(link)
        if not m:
            return False
        return m.get("fails", 0) >= FAILS_TO_SKIP and self.clock() - m.get("at", 0) < SKIP_FOR

    def failed(self, peer, link):
        if not peer:
            return
        m = self.peers.setdefault(peer, {}).setdefault(link, {"fails": 0})
        m["fails"] = m.get("fails", 0) + 1
        m["at"] = self.clock()
        self._save()

    def worked(self, peer, link):
        if not peer:
            return
        links = self.peers.setdefault(peer, {})
        if links.get(link, {}).get("fails", 0) == 0 and link in links:
            links[link]["at"] = self.clock()
            return  # (nothing new to write)
        links[link] = {"fails": 0, "at": self.clock()}
        self._save()

    def order(self, peer, items, key=lambda item: item):
        """`items` (fastest first) without the links to skip for this phone
        (`key`: an item's link name); when every one would be skipped, all
        of them (better a try than nothing)."""
        kept = [i for i in items if not self.skip(peer, key(i))]
        return kept or list(items)
