#!/usr/bin/env python3
"""Joins Wi-Fi networks the Quickshell API can't: enterprise (802.1X: PEAP,
TTLS, TLS, PWD, as on eduroam and office networks) and hidden ones
(services/WifiJoin.qml, the Wi-Fi page's "Other options…" and "Join other
network…").

It goes through NetworkManager's own command line (nmcli), and keeps every
secret off the command line, where any process could read it (ps,
/proc/*/cmdline): the request comes in on stdin as JSON, and the profile is
written through nmcli's editor, which reads its commands from stdin too.
Nothing is written to a file, and secrets are never printed.

  wifi-enterprise.py show <ssid>   the saved profile's settings (no secrets)
                                    as JSON, for filling the form in again
  wifi-enterprise.py known         the saved Wi-Fi networks, in range or not:
                                    {"known": [{"ssid", "name"}, ...]}
  wifi-enterprise.py forget <ssid> deletes every saved profile for it:
                                    {"forgotten": n} or {"state": "error", ...}
  wifi-enterprise.py join          request on stdin, as JSON:
      { ssid, hidden, security: "enterprise" | "psk" | "open",
        eap: "peap" | "ttls" | "tls" | "pwd", phase2: "mschapv2" | "pap" |
        "gtc" | "mschap", identity, anonymous, domain,
        ca: "system" | "none" | <file>, clientCert, privateKey,
        password, keyPassword, psk }
    prints one JSON line per step: {"state": "saving"}, {"state":
    "connecting"}, then {"state": "connected"} or {"state": "error",
    "message": "<what happened and how to fix it>"}
"""
import json
import os
import subprocess
import sys

EAP = ("peap", "ttls", "tls", "pwd")
PHASE2 = ("mschapv2", "pap", "gtc", "mschap")
SECURITY = ("enterprise", "psk", "open")
TIMEOUT = 45  # seconds NetworkManager gets to join before we say so


def emit(**kw):
    print(json.dumps(kw), flush=True)


def nmcli(*args, stdin=None, timeout=20):
    return subprocess.run(["nmcli", *args], input=stdin, capture_output=True, text=True, timeout=timeout)


def clean(value, what):
    """A value for one line of nmcli's editor: no line breaks (they would
    start another command) and no surrounding spaces (the editor drops them)."""
    value = "" if value is None else str(value)
    if "\n" in value or "\r" in value or "\0" in value:
        raise ValueError(f"The {what} can't contain line breaks.")
    if value != value.strip() and what not in ("password", "private key password", "Wi-Fi password"):
        value = value.strip()
    return value


def wifi_profiles():
    """(uuid, connection name, ssid) of every saved Wi-Fi profile."""
    r = nmcli("-t", "-f", "NAME,UUID,TYPE", "connection", "show")
    if r.returncode != 0:
        return []
    out = []
    for line in r.stdout.splitlines():
        # nmcli -t escapes ":" inside fields as "\:"
        parts = [p.replace("\0", ":") for p in line.replace("\\:", "\0").split(":")]
        if len(parts) >= 3 and parts[2] == "802-11-wireless":
            s = nmcli("-g", "802-11-wireless.ssid", "connection", "show", parts[1])
            out.append((parts[1], parts[0], s.stdout.strip() if s.returncode == 0 else parts[0]))
    return out


def known():
    seen = {}
    for _, name, ssid in wifi_profiles():
        seen.setdefault(ssid, name)
    emit(known=[{"ssid": k, "name": v} for k, v in sorted(seen.items(), key=lambda kv: kv[0].lower())])


def forget(ssid):
    uuids = [u for u, _, s in wifi_profiles() if s == ssid]
    for u in uuids:
        r = nmcli("connection", "delete", "uuid", u)
        if r.returncode != 0:
            emit(state="error", message=f"Couldn't forget {ssid}. NetworkManager said: {r.stderr.strip() or 'no reason'}.")
            return 1
    emit(forgotten=len(uuids))
    return 0


def profile_for(ssid):
    """The saved Wi-Fi profile for this network name, if there is one."""
    r = nmcli("-t", "-f", "NAME,UUID,TYPE", "connection", "show")
    if r.returncode != 0:
        return None
    for line in r.stdout.splitlines():
        # nmcli -t escapes ":" inside fields as "\:"
        parts = line.replace("\\:", "\0").split(":")
        parts = [p.replace("\0", ":") for p in parts]
        if len(parts) >= 3 and parts[2] == "802-11-wireless":
            name, uuid = parts[0], parts[1]
            s = nmcli("-g", "802-11-wireless.ssid", "connection", "show", uuid)
            if s.returncode == 0 and s.stdout.strip() == ssid:
                return uuid
    return None


def show(ssid):
    uuid = profile_for(ssid)
    out = {"exists": uuid is not None}
    if uuid:
        fields = {
            "802-1x.eap": "eap", "802-1x.phase2-auth": "phase2", "802-1x.identity": "identity",
            "802-1x.anonymous-identity": "anonymous", "802-1x.domain-suffix-match": "domain",
            "802-1x.ca-cert": "caFile", "802-1x.system-ca-certs": "systemCa",
            "802-1x.client-cert": "clientCert", "802-1x.private-key": "privateKey",
            "802-11-wireless-security.key-mgmt": "keyMgmt", "802-11-wireless.hidden": "hidden",
        }
        r = nmcli("-t", "-f", "802-1x,802-11-wireless-security,802-11-wireless", "connection", "show", uuid)
        for line in r.stdout.splitlines():
            key, _, value = line.partition(":")
            if key in fields:
                value = value.split(",")[0] if key == "802-1x.eap" else value
                if value.startswith("file://"):
                    value = value[len("file://"):]
                out[fields[key]] = "" if value in ("--", "") else value
    emit(**out)


def commands(req):
    """nmcli editor commands for the request, secrets included (they only
    ever go to nmcli's stdin)."""
    security = req.get("security", "enterprise")
    if security not in SECURITY:
        raise ValueError("Choose how the network is secured.")
    ssid = clean(req.get("ssid"), "network name")
    if not ssid or len(ssid.encode()) > 32:
        raise ValueError("Type the network name (up to 32 characters).")
    cmds = [f"set 802-11-wireless.ssid {ssid}", f"set 802-11-wireless.hidden {'yes' if req.get('hidden') else 'no'}"]
    if security == "open":
        cmds.append("remove 802-11-wireless-security")
        return ssid, cmds
    if security == "psk":
        psk = clean(req.get("psk"), "Wi-Fi password")
        if not 8 <= len(psk) <= 63:
            raise ValueError("Wi-Fi passwords are 8 to 63 characters.")
        cmds += ["set 802-11-wireless-security.key-mgmt wpa-psk", f"set 802-11-wireless-security.psk {psk}"]
        return ssid, cmds

    eap = req.get("eap", "peap")
    if eap not in EAP:
        raise ValueError("Choose a sign-in method.")
    identity = clean(req.get("identity"), "username")
    if not identity:
        raise ValueError("Type your username (often your email address).")
    cmds += ["set 802-11-wireless-security.key-mgmt wpa-eap", f"set 802-1x.eap {eap}", f"set 802-1x.identity {identity}"]
    anonymous = clean(req.get("anonymous"), "anonymous identity")
    cmds.append(f"set 802-1x.anonymous-identity {anonymous}" if anonymous else "remove 802-1x.anonymous-identity")
    domain = clean(req.get("domain"), "domain")
    cmds.append(f"set 802-1x.domain-suffix-match {domain}" if domain else "remove 802-1x.domain-suffix-match")

    ca = req.get("ca", "system")
    if ca == "system":
        cmds += ["remove 802-1x.ca-cert", "set 802-1x.system-ca-certs yes"]
    elif ca == "none":
        cmds += ["remove 802-1x.ca-cert", "set 802-1x.system-ca-certs no"]
    else:
        path = clean(ca, "CA certificate path")
        if not os.path.isfile(path) or not os.access(path, os.R_OK):
            raise ValueError("The CA certificate file can't be read. Choose it again.")
        cmds += [f"set 802-1x.ca-cert {path}", "set 802-1x.system-ca-certs no"]

    if eap in ("peap", "ttls"):
        phase2 = req.get("phase2", "mschapv2")
        if phase2 not in PHASE2:
            raise ValueError("Choose the inner authentication.")
        cmds.append(f"set 802-1x.phase2-auth {phase2}")
    else:
        cmds.append("remove 802-1x.phase2-auth")

    if eap == "tls":
        cert = clean(req.get("clientCert"), "certificate path")
        key = clean(req.get("privateKey"), "private key path")
        for path, what in ((cert, "certificate"), (key, "private key")):
            if not path or not os.path.isfile(path) or not os.access(path, os.R_OK):
                raise ValueError(f"Choose your {what} file.")
        cmds += [f"set 802-1x.client-cert {cert}", f"set 802-1x.private-key {key}"]
        key_password = clean(req.get("keyPassword"), "private key password")
        if key_password:
            cmds.append(f"set 802-1x.private-key-password {key_password}")
        cmds.append("set 802-1x.private-key-password-flags 0")
    else:
        password = clean(req.get("password"), "password")
        if not password:
            raise ValueError("Type your password.")
        cmds += [f"set 802-1x.password {password}", "set 802-1x.password-flags 0"]
    return ssid, cmds


# What NetworkManager's error means, in words that say how to fix it
def explain(text, req):
    t = text.lower()
    domain = (req.get("domain") or "").strip()
    if "secrets were required" in t or "no secrets" in t or ("authentication" in t and "fail" in t):
        return "Couldn't sign in. Check the username and password."
    if "certificate" in t or ("tls" in t and "fail" in t):
        if domain:
            return f"The network's certificate didn't match {domain}. Check the domain or the CA certificate."
        return "The network's certificate couldn't be checked. Choose its CA certificate, or check the domain."
    if "supplicant" in t:
        hint = f" and that the domain ({domain}) and CA certificate are right" if domain else ""
        return f"Couldn't sign in. Check the username, password and sign-in method{hint}."
    if "no network with ssid" in t or "not found" in t:
        return "That network isn't in range. Check the name, and turn on Hidden if it doesn't broadcast it."
    if "timeout" in t or "timed out" in t:
        return "The network didn't answer in time. Move closer to it and try again."
    if "not authorized" in t or "insufficient privileges" in t:
        return "You aren't allowed to change network settings. Ask your administrator, or sign in as an admin."
    return "Couldn't join the network. Check the settings and try again."


def join(req):
    try:
        ssid, cmds = commands(req)
    except ValueError as e:
        emit(state="error", message=str(e))
        return 1
    emit(state="saving")
    uuid = profile_for(ssid)
    if uuid:
        edit = ["connection", "edit", "uuid", uuid]
    else:
        edit = ["connection", "edit", "type", "802-11-wireless", "con-name", ssid]
    # nmcli's editor asks to confirm saving when it isn't on a terminal
    # in some versions; the trailing "yes" answers that, and is harmless
    # (an unknown command) when nothing asks
    script = "\n".join(cmds + ["save persistent", "yes", "quit", "yes"]) + "\n"
    try:
        r = nmcli(*edit, stdin=script, timeout=20)
    except subprocess.TimeoutExpired:
        emit(state="error", message="NetworkManager didn't answer. Check that it's running (systemctl status NetworkManager).")
        return 1
    # Saved when the profile is there afterwards (nmcli's editor can exit
    # with 0 after a command it refused, so its status alone isn't enough)
    uuid = profile_for(ssid)
    if r.returncode != 0 or not uuid:
        said = (r.stderr + r.stdout).lower()
        if "privileg" in said or "authoriz" in said:
            emit(state="error", message="You aren't allowed to change network settings. Ask your administrator, or sign in as an admin.")
        else:
            emit(state="error", message="Couldn't save the network's settings. Check the values and try again.")
        return 1
    emit(state="connecting")
    try:
        r = nmcli("--wait", str(TIMEOUT), "connection", "up", "uuid", uuid, timeout=TIMEOUT + 10)
    except subprocess.TimeoutExpired:
        emit(state="error", message="The network didn't answer in time. Move closer to it and try again.")
        return 1
    if r.returncode != 0:
        emit(state="error", message=explain(r.stderr + r.stdout, req))
        return 1
    emit(state="connected")
    return 0


def main(argv):
    if len(argv) >= 2 and argv[0] == "show":
        show(argv[1])
        return 0
    if argv[:1] == ["known"]:
        known()
        return 0
    if len(argv) >= 2 and argv[0] == "forget":
        return forget(argv[1])
    if argv[:1] == ["join"]:
        try:
            req = json.loads(sys.stdin.read() or "{}")
        except json.JSONDecodeError:
            emit(state="error", message="Something went wrong sending the settings. Try again.")
            return 1
        return join(req if isinstance(req, dict) else {})
    print(__doc__, file=sys.stderr)
    return 2


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
