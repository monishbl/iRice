#!/usr/bin/env python3
"""Moves your settings and data from iRice's folders to Tessera's, once.

iRice is now Tessera. Its folders were named "irice"; now they're "tessera":
    $XDG_CONFIG_HOME/irice  ->  $XDG_CONFIG_HOME/tessera   (settings)
    $XDG_DATA_HOME/irice    ->  $XDG_DATA_HOME/tessera     (builds, models, backups)
    $XDG_STATE_HOME/irice   ->  $XDG_STATE_HOME/tessera    (to-dos, chats, history)
    $XDG_CACHE_HOME/irice   ->  $XDG_CACHE_HOME/tessera
launch-shell.sh runs this before every shell start (and the installer before
it writes anything); it does nothing once the move is done.

For each one:
  - only the old folder: it's renamed (nothing copied, nothing lost), and the
    old name becomes a link to the new folder, so anything that still uses
    it (a shell or session started before the update, Hyprland's config until
    the next login, an older script) finds the same files;
  - both (something wrote the new one first): what's only in the old folder
    moves over; what's in both stays in the new one, and the old folder, with
    whatever is left in it, is kept as irice.before-tessera beside it. Then
    the old name becomes the link, as above;
  - the old name is already a link, or there's no old folder: nothing.
A note (MOVED-FROM-IRICE.txt) in the new folder says what happened.

    migrate-names.py [--dry-run]
Never fails the caller: problems are printed and it exits 0.
"""
import os
import sys
import time

OLD, NEW = "irice", "tessera"
NOTE = "MOVED-FROM-IRICE.txt"


def bases(env=None):
    """The four XDG folders, as (what, folder)."""
    env = os.environ if env is None else env
    home = env.get("HOME") or os.path.expanduser("~")
    return [
        ("config", env.get("XDG_CONFIG_HOME") or os.path.join(home, ".config")),
        ("data", env.get("XDG_DATA_HOME") or os.path.join(home, ".local", "share")),
        ("state", env.get("XDG_STATE_HOME") or os.path.join(home, ".local", "state")),
        ("cache", env.get("XDG_CACHE_HOME") or os.path.join(home, ".cache")),
    ]


def resolve(base):
    """The folder to use under base: Tessera's, unless only iRice's exists
    (not moved yet: the shell starting now moves it)."""
    new, old = os.path.join(base, NEW), os.path.join(base, OLD)
    if not os.path.exists(new) and os.path.isdir(old):
        return old
    return new


def _spare(path):
    """path, or path.2, path.3 … : the first name that's free."""
    if not os.path.lexists(path):
        return path
    n = 2
    while os.path.lexists(f"{path}.{n}"):
        n += 1
    return f"{path}.{n}"


def _note(new, old, lines):
    try:
        with open(os.path.join(new, NOTE), "a", encoding="utf-8") as f:
            f.write(f"{time.strftime('%Y-%m-%d %H:%M:%S')}: iRice is now Tessera.\n")
            for line in lines:
                f.write(f"  {line}\n")
            f.write(f"  {old} is now a link to this folder, for anything that still uses the old name.\n")
    except OSError:
        pass


def migrate_one(base, dry=False):
    """Moves base/irice to base/tessera. Returns what happened: "none",
    "linked" (already done), "moved", "merged" or "error: …"."""
    old, new = os.path.join(base, OLD), os.path.join(base, NEW)
    if os.path.islink(old) or not os.path.isdir(old):
        return "linked" if os.path.islink(old) else "none"
    if dry:
        return "would merge" if os.path.isdir(new) else "would move"
    try:
        if not os.path.lexists(new):
            try:
                os.rename(old, new)
            except FileNotFoundError:
                return "linked"  # (another start moved it just now)
            try:
                os.symlink(NEW, old)
            except FileExistsError:
                pass
            _note(new, old, [f"Moved here from {old}."])
            return "moved"
        if not os.path.isdir(new):
            return f"error: {new} isn't a folder"
        moved, kept = [], []
        for name in sorted(os.listdir(old)):
            src, dst = os.path.join(old, name), os.path.join(new, name)
            if os.path.lexists(dst):
                kept.append(name)
                continue
            os.rename(src, dst)
            moved.append(name)
        lines = [f"Both {old} and {new} existed: moved over from the old one: {', '.join(moved) or 'nothing'}."]
        if kept:
            spare = _spare(os.path.join(base, OLD + ".before-tessera"))
            os.rename(old, spare)
            lines.append(f"In both, so this folder's copy was kept and the old one is in {spare}: {', '.join(kept)}.")
        else:
            os.rmdir(old)
        try:
            os.symlink(NEW, old)
        except FileExistsError:
            pass
        _note(new, old, lines)
        return "merged"
    except OSError as e:
        return f"error: {e}"


def migrate(env=None, dry=False):
    return {what: migrate_one(base, dry) for what, base in bases(env)}


def main(argv):
    dry = "--dry-run" in argv
    for what, result in migrate(dry=dry).items():
        if result not in ("none", "linked"):
            print(f"migrate-names: {what}: {result}", file=sys.stderr)
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main(sys.argv[1:]))
    except Exception as e:  # never stop a shell from starting
        print(f"migrate-names: {e}", file=sys.stderr)
        sys.exit(0)
