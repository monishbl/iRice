"""Where received files land, safely.

A sender names its files; that name is untrusted. Only its last path part
is used, without control characters, never "." or "..", and never longer
than a filesystem allows. A file is written to a hidden partial file next
to its destination (created exclusively, readable only by you), and only
when every byte has arrived does it get its real name: a free one, so
nothing already there is ever replaced ("photo (2).jpg"). A transfer that
fails or is cancelled leaves nothing behind.
"""
import os
import unicodedata

MAX_NAME_BYTES = 200  # leaves room for " (99)" and the partial-file prefix


def safe_name(name, fallback="file"):
    name = (name or "").replace("\\", "/").split("/")[-1]
    name = "".join(ch for ch in name if unicodedata.category(ch)[0] != "C")
    name = name.strip().lstrip(".") or fallback  # (no hidden files, no "." or "..")
    raw = name.encode("utf-8")
    if len(raw) > MAX_NAME_BYTES:
        stem, dot, ext = name.rpartition(".")
        ext = ("." + ext) if dot and 0 < len(ext.encode()) <= 16 else ""
        keep = MAX_NAME_BYTES - len(ext.encode())
        name = (stem if dot and ext else name).encode("utf-8")[:keep].decode("utf-8", "ignore") + ext
    return name


def free_name(folder, name):
    """A name in folder that isn't taken yet: name, then "name (2)"…"""
    stem, dot, ext = name.rpartition(".")
    if not dot or not stem:
        stem, ext = name, ""
    else:
        ext = "." + ext
    candidate = name
    n = 1
    while os.path.lexists(os.path.join(folder, candidate)):
        n += 1
        candidate = f"{stem} ({n}){ext}"
    return candidate


class FileSink:
    """One incoming file: chunks in order, then publish() or fail()."""

    def __init__(self, folder, name, size):
        self.folder = folder
        self.name = safe_name(name)
        self.size = size
        self.written = 0
        self.path = None  # where it landed, once finished
        os.makedirs(folder, exist_ok=True)
        fd = None
        for _ in range(100):
            part = os.path.join(folder, f".{os.urandom(6).hex()}.quickshare-part")
            try:
                fd = os.open(part, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_CLOEXEC, 0o600)
                break
            except FileExistsError:
                continue
        if fd is None:
            raise OSError("couldn't create a file to receive into")
        self._part = part
        self._file = os.fdopen(fd, "wb")

    def write(self, offset, data):
        if offset != self.written:
            raise ValueError("file chunks out of order")
        if self.written + len(data) > self.size:
            raise ValueError("more data than the file's announced size")
        self._file.write(data)
        self.written += len(data)

    def finish(self):
        if self.written != self.size:
            self.fail()
            raise ValueError("the file ended early")
        self._file.close()
        os.chmod(self._part, 0o644)
        # A hard link only succeeds on a free name, so a file that appeared
        # meanwhile is never replaced (then the next free name is tried)
        for _ in range(100):
            final = os.path.join(self.folder, free_name(self.folder, self.name))
            try:
                os.link(self._part, final)
                os.unlink(self._part)
                break
            except FileExistsError:
                continue
            except OSError:
                # No hard links here (a FAT or exFAT drive): rename to the
                # free name just found instead
                os.rename(self._part, final)
                break
        else:
            self.fail()
            raise OSError("no free name to save the file under")
        self.path = final

    def fail(self):
        try:
            self._file.close()
        except OSError:
            pass
        try:
            os.unlink(self._part)
        except FileNotFoundError:
            pass
