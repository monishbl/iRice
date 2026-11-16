"""Where Tessera's code and ready-made builds come from: tools/sources.txt
(or TESSERA_MIRRORS), in order. Shared by tools/tessera-update.py and
tools/native-install.py (each loads this file by its path: they run with
python -I, which doesn't put tools/ on sys.path).

    sources()            every entry, in order
    git_sources()        the git URLs (no tarballs)
    same_repo(a, b)      the same repository, however written (.git, /, case)
    is_official(url)     one of the list's git URLs
    release_base(url)    where a git URL's ready-made builds are, with {tag}
    reachable(url)       a quick look (5 s to connect) before a long git fetch
"""
import os
import re
import subprocess

HERE = os.path.dirname(os.path.abspath(__file__))
LIST = os.path.join(HERE, "sources.txt")
# (only when sources.txt is missing: a copy cut down by hand)
DEFAULT = ["https://github.com/monishbl/iRice.git", "https://gitlab.com/monishbl/iRice.git",
           "https://codeload.github.com/monishbl/iRice/tar.gz/refs/heads/{branch}"]
GITHUB = re.compile(r"^(?:https://github\.com/|git@github\.com:|ssh://git@github\.com/)([A-Za-z0-9_.-]+)/([A-Za-z0-9_.-]+?)(?:\.git)?/?$")
GITLAB = re.compile(r"^(?:https://(gitlab\.[A-Za-z0-9.-]+)/|git@(gitlab\.[A-Za-z0-9.-]+):)([A-Za-z0-9_.-]+(?:/[A-Za-z0-9_.-]+)+?)(?:\.git)?/?$")
# git never asks for a password here: a mirror that doesn't exist yet (GitLab
# answers 401) fails at once instead of waiting at a prompt
QUIET_GIT = {"GIT_TERMINAL_PROMPT": "0", "GIT_ASKPASS": "true", "SSH_ASKPASS": "true", "GCM_INTERACTIVE": "never"}


def parse(text):
    return [w for line in text.splitlines() for w in line.split("#", 1)[0].replace(",", " ").split()]


def sources(path=None):
    env = os.environ.get("TESSERA_MIRRORS", "").strip()
    if env:
        return parse(env)
    try:
        with open(path or LIST) as f:
            return parse(f.read()) or list(DEFAULT)
    except OSError:
        return list(DEFAULT)


def git_sources(path=None):
    return [u for u in sources(path) if "{branch}" not in u]


def canonical(url):
    u = (url or "").strip().rstrip("/")
    u = u[:-4] if u.endswith(".git") else u
    m = re.match(r"^git@([^:]+):(.*)$", u) or re.match(r"^ssh://git@([^/]+)/(.*)$", u)
    if m:
        u = "https://" + m[1] + "/" + m[2]
    if u.startswith("file://"):
        u = u[len("file://"):]
    return u.lower() if "://" in u else os.path.normpath(u)


def same_repo(a, b):
    return bool(a and b) and canonical(a) == canonical(b)


def is_official(url, path=None):
    return any(same_repo(url, s) for s in git_sources(path))


def release_base(url):
    """The download URL with {tag} in it for a git URL's ready-made builds:
    GitHub's releases, or GitLab's generic package registry (package
    "tessera-ui", the release tag as its version: what CI uploads, see
    .github/workflows/tessera-ui.yml). None for anything else."""
    m = GITHUB.match(url or "")
    if m:
        return f"https://github.com/{m[1]}/{m[2]}/releases/download/{{tag}}"
    u = (url or "").rstrip("/")
    m = GITLAB.match(u[:-4] if u.endswith(".git") else u)
    if m:
        host, project = m[1] or m[2], m[3]
        return f"https://{host}/api/v4/projects/{project.replace('/', '%2F')}/packages/generic/tessera-ui/{{tag}}"
    return None


def reachable(url, timeout=10):
    """False only when an https git URL clearly isn't there: the host doesn't
    answer within 5 s, or the repository doesn't exist (GitHub and GitLab
    answer 401 or 404 for one that doesn't). Anything else (a local path,
    no curl) is left to git."""
    if not url.startswith("https://"):
        return True
    base = url.rstrip("/")
    probe = base + ("" if base.endswith(".git") else ".git") + "/info/refs?service=git-upload-pack"
    try:
        r = subprocess.run(["curl", "--silent", "--location", "--proto", "=https", "--connect-timeout", "5",
                            "--max-time", str(timeout), "--user-agent", "git/2 (Tessera)", "--output", "/dev/null",
                            "--write-out", "%{http_code}", probe], capture_output=True, text=True, timeout=timeout + 5)
    except FileNotFoundError:
        return True
    except (OSError, subprocess.SubprocessError):
        return False
    return r.stdout.strip()[-3:] == "200"
