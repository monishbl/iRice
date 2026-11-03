"""Two rules for Settings, in both looks, read from the page files:

1. One control, not a switch plus an option: a Switch's setting never has a
   companion row that only shows (or only works) while the switch is on.
   That pair is one choice with "Off" in it (Stage Manager: Off / On / On
   with recents; Dock magnification: a slider whose left end is Off).
2. Each setting lives on one page: no config key is written from two page
   files. Another page that mentions it links there (SubpageLink with `page`
   and `target`), as Accessibility does for Shake to find the pointer.

The Material look's pages are shell/'s with material/'s files of the same
name in their place (shell/assets/scripts/material-root.py). Writes are found
through `Config.options.a.b = …`, a page's `property var x: Config.options.a`
(`page.x.b = …`) and a service's `cfg` (`Captions.cfg.b = …`). Dynamic keys
(`Config.options.sounds[key] = …`) can't be named, so they're not counted.
"""
from pathlib import Path
import re
import unittest

ROOT = Path(__file__).resolve().parents[1]
PAGES = 'modules/settings/pages'

# Rule 1 exceptions: a switch for a whole feature, whose options are that
# feature's own set (a subpage or group of them), not one companion choice
FEATURE_SWITCHES = {
    # The Dynamic island page: the switch, then everything the island does
    'island.enabled': 'the island subpage is all its options',
    # StandBy's group (burn-in, waking, clock, displays) dims while it's off
    'lock.standby': "StandBy's own group of options",
    # Keep track: what shows while it's on is the record (apps), not an option
    'power.appEnergy': 'the energy list is a record, not an option',
}

# Rule 2 exceptions: key -> why two pages write it (none today)
SHARED_KEYS = {}

ALIAS = re.compile(r'property var (\w+): *Config\.options((?:\.\w+)+)\s*$', re.M)
COMMENT = re.compile(r'(^|\s)//.*$', re.M)


def pages(look):
    """Page file name -> text, as the look runs them."""
    files = {p.name: p for p in (ROOT/'shell'/PAGES).glob('*.qml')}
    if look == 'material':
        files.update({p.name: p for p in (ROOT/'material'/PAGES).glob('*.qml')})
    return {n: COMMENT.sub(r'\1', p.read_text()) for n, p in sorted(files.items())}


def service_aliases(look):
    """'Captions.cfg' -> 'captions': services' handles on a config object."""
    files = {p.name: p for p in (ROOT/'shell/services').glob('*.qml')}
    if look == 'material':
        files.update({p.name: p for p in (ROOT/'material/services').glob('*.qml')})
    out = {}
    for name, p in files.items():
        for m in ALIAS.finditer(p.read_text()):
            out[Path(name).stem + '.' + m.group(1)] = m.group(2)[1:]
    return out


def prefixes(text, services):
    """Expressions that stand for a config object -> its key path."""
    out = {'Config.options': ''}
    for m in ALIAS.finditer(text):
        out['page.' + m.group(1)] = out[m.group(1)] = m.group(2)[1:]
    out.update(services)
    return out


def refs(text, services):
    """Each `<prefix>.a.b` in the text, as (start, end, 'key.path')."""
    found = []
    for pre, base in prefixes(text, services).items():
        for m in re.finditer(r'(?<![\w.])' + re.escape(pre) + r'((?:\.\w+)+)', text):
            key = (base + m.group(1)).lstrip('.')
            found.append((m.start(), m.end(), key))
    return found


def written(text, services):
    """Config keys the page assigns."""
    keys = set()
    for start, end, key in refs(text, services):
        if re.match(r'\s*(=(?!=)|\+=|-=)', text[end:]):
            keys.add(key)
    return keys


def switched(text, services):
    """Keys a Switch toggles (`onToggled: <key> = !checked`)."""
    keys = set()
    for start, end, key in refs(text, services):
        if re.match(r'\s*=\s*!checked\b', text[end:]):
            keys.add(key)
    return keys


def gated(text, services):
    """Keys that a `visible:` or `enabled:` binding hangs on, as a whole
    condition or one of its `&&` parts (`!key` too: shown only when off)."""
    keys = set()
    known = {(s, e): k for s, e, k in refs(text, services)}
    # (to the end of the binding: the line, or a `;` or `}` on it)
    for m in re.finditer(r'\b(visible|enabled): *([^\n;}]+)', text):
        base = m.start(2)
        expr = m.group(2)
        for part in re.finditer(r'[^&|]+', expr):
            raw = part.group(0)
            core = raw.strip().strip('()').strip().lstrip('!').strip()
            if not core or re.search(r'[=<>?:+\-*/]', core):
                continue
            at = base + part.start() + raw.index(core)
            key = known.get((at, at + len(core)))
            if key:
                keys.add(key)
    return keys


class SettingsRules(unittest.TestCase):
    def test_each_setting_is_written_from_one_page(self):
        for look in ('shell', 'material'):
            services = service_aliases(look)
            writers = {}
            for name, text in pages(look).items():
                for key in written(text, services):
                    writers.setdefault(key, set()).add(name)
            twice = {k: sorted(v) for k, v in writers.items() if len(v) > 1 and k not in SHARED_KEYS}
            self.assertEqual(twice, {}, look + ': keys written from more than one page (give each one home, and link to it from the other page)')
            self.assertTrue(len(writers) > 100, look + ': the scan found too few settings to be reading the pages')

    def test_no_switch_with_a_companion_option(self):
        for look in ('shell', 'material'):
            services = service_aliases(look)
            pairs = {}
            for name, text in pages(look).items():
                for key in switched(text, services) & gated(text, services):
                    pairs.setdefault(key, []).append(name)
            bad = {k: v for k, v in pairs.items() if k not in FEATURE_SWITCHES}
            self.assertEqual(bad, {}, look + ': a Switch whose companion row shows only while it is on (make them one choice with Off in it)')
            # (an exception that no longer happens goes from the list)
            stale = set(FEATURE_SWITCHES) - set(pairs)
            self.assertEqual(stale, set(), look + ': allowed switches that no longer need it')

    def test_links_go_to_pages_that_exist(self):
        files = {p.stem for p in (ROOT/'shell'/PAGES).glob('*.qml')}
        aliases = set(re.findall(r'^\s+"([\w/]+)": \{ page:', (ROOT/'shell/common/SettingsMap.js').read_text(), re.M))
        for look in ('shell', 'material'):
            for name, text in pages(look).items():
                for page in re.findall(r'SubpageLink\s*\{[^}]*?\bpage:\s*"([^"]+)"', text):
                    self.assertTrue(page.replace('/', '-') in files or page in aliases, name + ' links to ' + page)

    def test_the_rules_are_seen(self):
        # (the checks above find what they're meant to: a made-up page with
        # both mistakes)
        page = '''
        Column {
            id: page
            readonly property var cfg: Config.options.dock
            Row { Switch { checked: page.cfg.magnification; onToggled: page.cfg.magnification = !checked } }
            Row { visible: page.cfg.magnification && Theme.ok; Slider { onMoved: v => Config.options.dock.magnifiedScale = v } }
            Row { Switch { onToggled: Captions.cfg.enabled = !checked } }
            Row { enabled: !Captions.cfg.enabled }
            Row { visible: Config.options.dock.autoHide === "off" }
        }'''
        services = {'Captions.cfg': 'captions'}
        self.assertEqual(written(page, services), {'dock.magnification', 'dock.magnifiedScale', 'captions.enabled'})
        self.assertEqual(switched(page, services) & gated(page, services), {'dock.magnification', 'captions.enabled'})


if __name__ == '__main__':
    unittest.main()
