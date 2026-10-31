# Quick Share checks with a real phone

Drivers for testing Quick Share against an Android phone on USB adb (USB
debugging on). The phone's screen is woken only while the share sheet is
driven, then turned off.

- `phone.py`: opens the phone's share sheet with a file or text and taps
  the target (`send URI MIME NAME`), opens its Receive screen (`receive`),
  turns the screen off (`off`). Tap positions are for a 1080×2400 screen.
- `blerx.py NAME [noupgrade|nmhotspot]`: iRice's service as receiver over
  Bluetooth only (no mDNS), accepting everything into ~/Downloads/qs-test.
- `blesend.py FILE MATCH [lan]`: iRice's service sending over Bluetooth to
  the first phone whose name contains MATCH; without `lan`, as on another
  network (the phone has to host its Wi-Fi link).
- `shield-test.py`: hosts a Wi-Fi Direct group through the root helper next
  to NetworkManager's connection and prints, every 2 s, whether the group,
  the connection and the internet are up. Run it from a terminal (pkexec
  asks for the password there).
