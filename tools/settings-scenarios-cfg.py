import json, sys
src, dst, p, look = sys.argv[1:]
c = json.load(open(src))
c.setdefault("rust", {})["standalone"] = p == "rust"
c["rust"]["surfaces"] = []
c["appearance"]["look"] = look
for k in ("ai", "google", "cloud", "phone"):   # (accounts and keys stay out of the arena)
    c.pop(k, None)
json.dump(c, open(dst, "w"), indent=4)
