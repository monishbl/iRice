#!/usr/bin/env python3
"""Drives the phone's Quick Share over adb (screen on only while needed).

  phone.py send URI MIME TARGET [WAIT]   open the share sheet with URI, tap TARGET
  phone.py text TEXT TARGET [WAIT]       the same with plain text
  phone.py receive                       open Quick Share's Receive screen
  phone.py off                           screen off
  phone.py shot FILE                     screenshot (small)
Prints what it did; exits 1 if TARGET never appeared.
"""
import re
import subprocess
import sys
import time

ACT = "com.google.android.gms/.nearby.sharing.main.MainActivity"


def adb(*a, check=False):
    return subprocess.run(["adb", *a], capture_output=True, text=True, timeout=60).stdout


def sh(cmd):
    return adb("shell", cmd)


def wake():
    sh("input keyevent 224")
    time.sleep(0.4)
    sh("wm dismiss-keyguard")
    time.sleep(0.6)


def off():
    sh("input keyevent 223")


def ui():
    sh("uiautomator dump /sdcard/ui.xml")
    return sh("cat /sdcard/ui.xml")


def find(xml, text):
    for m in re.finditer(r'text="([^"]*)"[^>]*bounds="\[(\d+),(\d+)\]\[(\d+),(\d+)\]"', xml):
        if text.lower() in m.group(1).lower():
            x1, y1, x2, y2 = map(int, m.groups()[1:])
            return (x1 + x2) // 2, (y1 + y2) // 2
    return None


def tap_target(target, wait):
    end = time.time() + wait
    swiped = False
    while time.time() < end:
        xml = ui()
        pos = find(xml, target)
        if pos and pos[1] < 2200:
            # (the list settles after a device appears: look again, then tap its icon)
            time.sleep(1.5)
            again = find(ui(), target)
            if again != pos:
                continue
            sh(f"input tap {pos[0]} {pos[1] - 100}")
            print(f"tapped {target} at {pos}", flush=True)
            return True
        if "Send to nearby devices" in xml or not swiped:
            sh("input swipe 540 1800 540 700 250")
            swiped = True
        time.sleep(1.5)
    print(f"{target} never appeared", flush=True)
    return False


def main():
    cmd = sys.argv[1]
    if cmd == "off":
        off()
        return
    if cmd == "shot":
        wake()
        data = subprocess.run(["adb", "exec-out", "screencap", "-p"], capture_output=True, timeout=60).stdout
        open(sys.argv[2], "wb").write(data)
        return
    if cmd == "receive":
        wake()
        sh(f"am start -n {ACT}")
        time.sleep(3)
        xml = ui()
        pos = find(xml, "Receive")
        if pos:
            sh(f"input tap {pos[0]} {pos[1]}")
        print("receive screen", flush=True)
        return
    wake()
    pass
    if cmd == "send":
        uri, mime, target = sys.argv[2:5]
        wait = int(sys.argv[5]) if len(sys.argv) > 5 else 40
        sh(f"am start -a android.intent.action.SEND -t {mime} --eu android.intent.extra.STREAM {uri} "
           f"--grant-read-uri-permission -n {ACT}")
    else:
        text, target = sys.argv[2:4]
        wait = int(sys.argv[4]) if len(sys.argv) > 4 else 40
        sh(f"am start -a android.intent.action.SEND -t text/plain --es android.intent.extra.TEXT '{text}' -n {ACT}")
    time.sleep(2)
    ok = tap_target(target, wait)
    time.sleep(1)
    off()
    sys.exit(0 if ok else 1)


if __name__ == "__main__":
    main()
