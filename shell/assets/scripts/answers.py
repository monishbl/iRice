#!/usr/bin/env python3
"""Spotlight's plain-language answers (services/Search.qml), worked out
here so typing stays quick: sums in words and percentages, dates, time
zones, units and currency. Plain arithmetic ("2+2") is Spotlight's own.

  answers.py "18% of 2450"   one answer as JSON, or {}
  answers.py --serve         per line in {"id", "q"}, one line out {"id", ...}

An answer: {"kind": "math" | "date" | "time" | "unit" | "currency",
"title": the answer, "subtitle": what it answers, "copy": what Enter
copies}. Currency rates are the European Central Bank's (frankfurter.app,
no key), fetched at most once a day and kept in ~/.cache/tessera/rates.json.
"""
import ast
import datetime as dt
import json
import operator
import os
import re
import sys
import time
import urllib.request
from pathlib import Path
from zoneinfo import ZoneInfo, available_timezones


def app_dir(base):
    """Tessera's folder under base, or iRice's while it hasn't been moved yet
    (the shell moves it as it starts: migrate-names.py)."""
    new, old = os.path.join(base, "tessera"), os.path.join(base, "irice")
    return old if not os.path.exists(new) and os.path.isdir(old) else new

# ── Numbers in words ─────────────────────────────────────────────────────
SMALL = {w: i for i, w in enumerate("zero one two three four five six seven eight nine ten eleven twelve thirteen "
                                    "fourteen fifteen sixteen seventeen eighteen nineteen".split())}
TENS = {w: 10 * (i + 2) for i, w in enumerate("twenty thirty forty fifty sixty seventy eighty ninety".split())}
SCALES = {"hundred": 100, "thousand": 1000, "million": 10 ** 6, "billion": 10 ** 9, "lakh": 10 ** 5, "crore": 10 ** 7}


def words_to_numbers(text):
    """'twenty-five times four' -> '25 times 4'; 'a dozen' -> 'a dozen' (dozens: math_phrase)."""
    out, run = [], []

    def flush():
        if run:
            total = current = 0
            for w in run:
                if w in SMALL:
                    current += SMALL[w]
                elif w in TENS:
                    current += TENS[w]
                elif w == "hundred":
                    current = max(1, current) * 100
                else:
                    total += max(1, current) * SCALES[w]
                    current = 0
            out.append(str(total + current))
            run.clear()

    for token in text.split():
        parts = token.lower().split("-")
        if all(p in SMALL or p in TENS or p in SCALES for p in parts):
            run.extend(parts)
        elif token.lower() == "and" and run:
            continue
        else:
            flush()
            out.append(token)
    flush()
    return " ".join(out)


OPS = {ast.Add: operator.add, ast.Sub: operator.sub, ast.Mult: operator.mul, ast.Div: operator.truediv,
       ast.Pow: operator.pow, ast.Mod: operator.mod, ast.USub: operator.neg, ast.UAdd: operator.pos}


def safe_eval(expr):
    def ev(node):
        if isinstance(node, ast.Expression):
            return ev(node.body)
        if isinstance(node, ast.Constant) and isinstance(node.value, (int, float)):
            return node.value
        if isinstance(node, ast.BinOp) and type(node.op) in OPS:
            a, b = ev(node.left), ev(node.right)
            if isinstance(node.op, ast.Pow) and abs(b) > 100:
                raise ValueError("too big")
            return OPS[type(node.op)](a, b)
        if isinstance(node, ast.UnaryOp) and type(node.op) in OPS:
            return OPS[type(node.op)](ev(node.operand))
        if isinstance(node, ast.Call) and getattr(node.func, "id", "") == "sqrt" and len(node.args) == 1:
            return ev(node.args[0]) ** 0.5
        raise ValueError("not arithmetic")
    return ev(ast.parse(expr, mode="eval"))


def fmt(v):
    if isinstance(v, float) and v.is_integer() and abs(v) < 1e15:
        v = int(v)
    if isinstance(v, int):
        return f"{v:,}"
    s = f"{v:,.10g}"
    return s if "e" in s else s.rstrip("0").rstrip(".") if "." in s else s


NUM = r"(-?\d[\d,]*(?:\.\d+)?)"


def num(s):
    return float(s.replace(",", ""))


def math_phrase(q):
    """Sums with words in them, and percentages."""
    t = words_to_numbers(q.lower()).strip().rstrip("?").strip()
    t = re.sub(r"^(what is|what's|whats|calculate|how much is)\s+", "", t)
    m = re.fullmatch(NUM + r"\s*(?:%|percent)\s+of\s+" + NUM, t)
    if m:
        return num(m[1]) * num(m[2]) / 100, f"{m[1]}% of {m[2]}"
    m = re.fullmatch(NUM + r"\s*(\+|plus|-|minus|add|less)\s*" + NUM + r"\s*(?:%|percent)", t)
    if m:
        up = m[2] in ("+", "plus", "add")
        return num(m[1]) * (1 + (num(m[3]) if up else -num(m[3])) / 100), f"{m[1]} {'+' if up else '−'} {m[3]}%"
    m = re.fullmatch(NUM + r" is what (?:%|percent|percentage) of " + NUM, t)
    if m and num(m[2]) != 0:
        return num(m[1]) / num(m[2]) * 100, f"{m[1]} as a percentage of {m[2]}", "%"
    m = re.fullmatch(r"(half|a half|a third|third|a quarter|quarter|double|twice|triple|thrice)\s+(?:of\s+)?" + NUM, t)
    if m:
        f = {"half": 0.5, "a half": 0.5, "a third": 1 / 3, "third": 1 / 3, "a quarter": 0.25, "quarter": 0.25,
             "double": 2, "twice": 2, "triple": 3, "thrice": 3}[m[1]]
        return num(m[2]) * f, t
    m = re.fullmatch(r"(?:" + NUM + r"|a)\s+dozens?", t)
    if m:
        return (num(m[1]) if m[1] else 1) * 12, t
    m = re.fullmatch(r"(?:the\s+)?square root of " + NUM, t)
    if m:
        return num(m[1]) ** 0.5, t
    # Operators in words
    e = t
    for words, sym in ((r"multiplied by|times|into|x", "*"), (r"divided by|over", "/"), (r"plus|add", "+"),
                       (r"minus|less", "-"), (r"to the power of|power", "**")):
        e = re.sub(r"(?<=[\d)\s])\s*(?:" + words + r")\s*(?=[\d(\s-])", f" {sym} ", e)
    e = re.sub(r"\s*squared", "**2", e)
    e = re.sub(r"\s*cubed", "**3", e)
    e = e.replace(",", "").replace("^", "**").replace("×", "*").replace("÷", "/")
    if e == t.replace(",", "") or not re.fullmatch(r"[\d.\s+\-*/()%]+", e) or not re.search(r"\d", e):
        return None
    try:
        shown = re.sub(r"\s+", " ", e.replace("**", "^").replace("*", "×").replace("/", "÷")).strip()
        return safe_eval(e), shown
    except Exception:  # noqa: BLE001
        return None


# ── Dates ────────────────────────────────────────────────────────────────
MONTHS = {m: i + 1 for i, m in enumerate("jan feb mar apr may jun jul aug sep oct nov dec".split())}
HOLIDAYS = {"christmas": (12, 25), "xmas": (12, 25), "new year": (1, 1), "new years": (1, 1), "new year's": (1, 1),
            "halloween": (10, 31), "valentine's day": (2, 14), "valentines day": (2, 14), "valentines": (2, 14)}


def parse_day(text, today):
    """'25 dec', 'december 25 2027', 'christmas', '2027-03-01', 'tomorrow' -> a date (the next one)."""
    t = text.strip().lower().rstrip("?").strip()
    t = re.sub(r"^(the\s+)", "", t)
    if t in ("today", "now"):
        return today
    if t == "tomorrow":
        return today + dt.timedelta(days=1)
    if t == "yesterday":
        return today - dt.timedelta(days=1)
    if t in HOLIDAYS:
        m, d = HOLIDAYS[t]
        day = dt.date(today.year, m, d)
        return day if day >= today else dt.date(today.year + 1, m, d)
    m = re.fullmatch(r"(\d{4})-(\d{1,2})-(\d{1,2})", t)
    if m:
        return dt.date(int(m[1]), int(m[2]), int(m[3]))
    m = re.fullmatch(r"(\d{1,2})(?:st|nd|rd|th)?\s+(?:of\s+)?([a-z]+)\.?(?:,?\s+(\d{4}))?", t) or \
        re.fullmatch(r"([a-z]+)\.?\s+(\d{1,2})(?:st|nd|rd|th)?(?:,?\s+(\d{4}))?", t)
    if m:
        a, b, year = m[1], m[2], m[3]
        day, month = (a, b) if a.isdigit() else (b, a)
        mon = MONTHS.get(month[:3])
        if not mon:
            return None
        if year:
            return dt.date(int(year), mon, int(day))
        d = dt.date(today.year, mon, int(day))
        return d if d >= today else dt.date(today.year + 1, mon, int(day))
    return None


def long_date(d):
    return d.strftime("%A ") + str(d.day) + d.strftime(" %B %Y")


def add_months(d, n):
    y, m = divmod(d.month - 1 + n, 12)
    y += d.year
    import calendar
    return dt.date(y, m + 1, min(d.day, calendar.monthrange(y, m + 1)[1]))


def date_phrase(q, today):
    t = words_to_numbers(q.lower()).strip().rstrip("?").strip()
    m = re.fullmatch(r"(?:how many )?days? (?:until|till|til|to|before|left until|left till) (.+)", t) or \
        re.fullmatch(r"how long (?:until|till|to) (.+)", t)
    if m:
        d = parse_day(m[1], today)
        if d:
            n = (d - today).days
            return f"{n:,} day{'s' if n != 1 else ''}", f"Until {long_date(d)}"
    m = re.fullmatch(r"(?:how many )?days? (?:since|after) (.+)", t)
    if m:
        d = parse_day(m[1], today)
        if d:
            if d > today:
                d = d.replace(year=d.year - 1)
            n = (today - d).days
            return f"{n:,} day{'s' if n != 1 else ''}", f"Since {long_date(d)}"
    m = re.fullmatch(r"(?:in\s+)?(\d+) (day|week|month|year)s? (?:from|after) (today|now|tomorrow)", t) or \
        re.fullmatch(r"in (\d+) (day|week|month|year)s?()", t)
    if m:
        base = today + dt.timedelta(days=1) if m[3] == "tomorrow" else today
        n, unit = int(m[1]), m[2]
        d = add_months(base, n) if unit == "month" else add_months(base, 12 * n) if unit == "year" else \
            base + dt.timedelta(days=n * (7 if unit == "week" else 1))
        return long_date(d), t[0].upper() + t[1:]
    m = re.fullmatch(r"(\d+) (day|week|month|year)s? ago", t)
    if m:
        n, unit = int(m[1]), m[2]
        d = add_months(today, -n) if unit == "month" else add_months(today, -12 * n) if unit == "year" else \
            today - dt.timedelta(days=n * (7 if unit == "week" else 1))
        return long_date(d), t[0].upper() + t[1:]
    m = re.fullmatch(r"what day (?:is|was|will be|is it on) (.+)", t)
    if m:
        d = parse_day(m[1], today)
        if d:
            return d.strftime("%A"), long_date(d)
    m = re.fullmatch(r"(?:how old is (?:someone|somebody|a person) born(?: in)?|age (?:of someone |if )?born(?: in)?|born in) (\d{4})", t)
    if m and 1900 <= int(m[1]) <= today.year:
        age = today.year - int(m[1])
        return f"{age - 1} or {age}", f"Born in {m[1]}: {age - 1} before their birthday this year, {age} after"
    return None


# ── Time zones ───────────────────────────────────────────────────────────
ALIASES = {"delhi": "Asia/Kolkata", "new delhi": "Asia/Kolkata", "mumbai": "Asia/Kolkata", "bangalore": "Asia/Kolkata",
           "bengaluru": "Asia/Kolkata", "chennai": "Asia/Kolkata", "hyderabad": "Asia/Kolkata", "pune": "Asia/Kolkata",
           "india": "Asia/Kolkata", "ist": "Asia/Kolkata", "san francisco": "America/Los_Angeles", "sf": "America/Los_Angeles",
           "la": "America/Los_Angeles", "seattle": "America/Los_Angeles", "california": "America/Los_Angeles",
           "nyc": "America/New_York", "new york city": "America/New_York", "boston": "America/New_York",
           "washington": "America/New_York", "miami": "America/New_York", "beijing": "Asia/Shanghai", "china": "Asia/Shanghai",
           "japan": "Asia/Tokyo", "uk": "Europe/London", "england": "Europe/London", "germany": "Europe/Berlin",
           "france": "Europe/Paris", "dubai": "Asia/Dubai", "uae": "Asia/Dubai", "utc": "UTC", "gmt": "UTC",
           "pst": "America/Los_Angeles", "est": "America/New_York", "cet": "Europe/Paris", "jst": "Asia/Tokyo"}
_ZONES = None


def zone_for(place):
    global _ZONES
    p = place.strip().lower().rstrip("?").strip()
    if p in ALIASES:
        return ZoneInfo(ALIASES[p]), place.strip().title() if len(p) > 3 else p.upper()
    if _ZONES is None:
        _ZONES = {}
        for z in available_timezones():
            city = z.split("/")[-1].replace("_", " ").lower()
            if "/" in z and not z.startswith(("Etc/", "SystemV/", "posix/", "right/")):
                _ZONES.setdefault(city, z)
    if p in _ZONES:
        return ZoneInfo(_ZONES[p]), _ZONES[p].split("/")[-1].replace("_", " ")
    return None


def clock(d):
    return d.strftime("%I:%M %p").lstrip("0")


def offset_text(a, b):
    """How far ahead (or behind) b's clock is of a's."""
    diff = int((b.utcoffset() - a.utcoffset()).total_seconds() // 60)
    if diff == 0:
        return "same time"
    h, m = divmod(abs(diff), 60)
    span = f"{h} h" + (f" {m} min" if m else "") if h else f"{m} min"
    return f"{span} {'ahead' if diff > 0 else 'behind'}"


def time_phrase(q, now):
    t = q.lower().strip().rstrip("?").strip()
    m = re.fullmatch(r"(?:what(?:'s| is) the )?(?:current )?time (?:is it )?in (.+)", t) or re.fullmatch(r"(.+?) time", t)
    if m:
        z = zone_for(m[1])
        if z:
            there = now.astimezone(z[0])
            return clock(there), f"{z[1]} · {there.strftime('%A')} · {offset_text(now.astimezone(), there)}"
    m = re.fullmatch(r"(\d{1,2})(?::(\d{2}))?\s*(am|pm)?\s+(?:in\s+)?(.+?)\s+(?:in|to)\s+(.+)", t)
    if m:
        a, b = zone_for(m[4]), zone_for(m[5])
        if a and b:
            hour = int(m[1]) % 12 + (12 if m[3] == "pm" else 0) if m[3] else int(m[1])
            if hour > 23 or int(m[2] or 0) > 59:
                return None
            here = now.astimezone(a[0]).replace(hour=hour, minute=int(m[2] or 0), second=0, microsecond=0)
            there = here.astimezone(b[0])
            day = "" if there.date() == here.date() else (" next day" if there.date() > here.date() else " previous day")
            return clock(there) + day, f"{clock(here)} in {a[1]} is {clock(there)} in {b[1]}"
    return None


# ── Units ────────────────────────────────────────────────────────────────
# unit -> (kind, factor to the kind's base unit)
UNITS = {}
for kind, table in {
    "length": {"mm": 0.001, "millimetre": 0.001, "millimeter": 0.001, "cm": 0.01, "centimetre": 0.01, "centimeter": 0.01,
               "m": 1, "metre": 1, "meter": 1, "km": 1000, "kilometre": 1000, "kilometer": 1000, "in": 0.0254,
               "inch": 0.0254, "inches": 0.0254, "ft": 0.3048, "foot": 0.3048, "feet": 0.3048, "yd": 0.9144,
               "yard": 0.9144, "mi": 1609.344, "mile": 1609.344, "nmi": 1852},
    "mass": {"mg": 1e-6, "g": 0.001, "gram": 0.001, "kg": 1, "kilo": 1, "kilogram": 1, "t": 1000, "tonne": 1000,
             "oz": 0.028349523125, "ounce": 0.028349523125, "lb": 0.45359237, "lbs": 0.45359237, "pound": 0.45359237,
             "st": 6.35029318, "stone": 6.35029318},
    "volume": {"ml": 0.001, "millilitre": 0.001, "milliliter": 0.001, "l": 1, "litre": 1, "liter": 1, "tsp": 0.00492892,
               "teaspoon": 0.00492892, "tbsp": 0.0147868, "tablespoon": 0.0147868, "cup": 0.24, "floz": 0.0295735,
               "pint": 0.473176, "quart": 0.946353, "gallon": 3.78541, "gal": 3.78541},
    "speed": {"kmh": 1 / 3.6, "kph": 1 / 3.6, "km/h": 1 / 3.6, "mph": 0.44704, "m/s": 1, "mps": 1, "knot": 0.514444, "kn": 0.514444},
    "data": {"b": 1, "byte": 1, "kb": 1e3, "mb": 1e6, "gb": 1e9, "tb": 1e12, "kib": 1024, "mib": 1024 ** 2,
             "gib": 1024 ** 3, "tib": 1024 ** 4},
    "time": {"ms": 0.001, "s": 1, "sec": 1, "second": 1, "min": 60, "minute": 60, "h": 3600, "hr": 3600, "hour": 3600,
             "day": 86400, "week": 604800},
    "area": {"sqm": 1, "m2": 1, "sqft": 0.09290304, "ft2": 0.09290304, "acre": 4046.8564224, "hectare": 10000, "ha": 10000},
}.items():
    for u, f in table.items():
        UNITS[u] = (kind, f)
TEMPS = {"c": "°C", "celsius": "°C", "f": "°F", "fahrenheit": "°F", "k": "K", "kelvin": "K"}


def unit_key(u):
    u = u.strip().lower().replace("°", "").replace(" ", "")
    if u in UNITS or u in TEMPS:
        return u
    if u.endswith("es") and u[:-2] in UNITS:
        return u[:-2]
    if u.endswith("s") and u[:-1] in UNITS | TEMPS:
        return u[:-1]
    return None


def unit_phrase(q):
    t = q.lower().strip().rstrip("?").strip()
    m = re.fullmatch(NUM + r"\s*([a-z°/²]+(?:\s[a-z]+)?)\s+(?:in|to|into|as)\s+([a-z°/²]+)", t)
    if not m:
        return None
    v, a, b = num(m[1]), unit_key(m[2]), unit_key(m[3])
    if not a or not b:
        return None
    if a in TEMPS and b in TEMPS:
        c = {"°C": lambda x: x, "°F": lambda x: (x - 32) * 5 / 9, "K": lambda x: x - 273.15}[TEMPS[a]](v)
        out = {"°C": lambda x: x, "°F": lambda x: x * 9 / 5 + 32, "K": lambda x: x + 273.15}[TEMPS[b]](c)
        return f"{fmt(round(out, 2))} {TEMPS[b]}", f"{fmt(v)} {TEMPS[a]}"
    if a in UNITS and b in UNITS and UNITS[a][0] == UNITS[b][0]:
        out = v * UNITS[a][1] / UNITS[b][1]
        return f"{fmt(float(f'{out:.5g}'))} {m[3].strip()}", f"{fmt(v)} {m[2].strip()}"
    return None


# ── Currency ─────────────────────────────────────────────────────────────
CURRENCY_NAMES = {"dollar": "USD", "dollars": "USD", "usd": "USD", "$": "USD", "euro": "EUR", "euros": "EUR", "eur": "EUR",
                  "€": "EUR", "rupee": "INR", "rupees": "INR", "rs": "INR", "inr": "INR", "₹": "INR", "pound": "GBP",
                  "pounds": "GBP", "gbp": "GBP", "£": "GBP", "yen": "JPY", "jpy": "JPY", "¥": "JPY", "yuan": "CNY",
                  "rmb": "CNY", "dirham": "AED", "dirhams": "AED", "won": "KRW"}
RATES_FILE = Path(app_dir(str(Path(os.environ.get("XDG_CACHE_HOME") or Path.home() / ".cache")))) / "rates.json"


def rates(fetch=True):
    """{code: units per euro}, from the cache when it's under a day old."""
    try:
        cached = json.loads(RATES_FILE.read_text())
        if time.time() - cached.get("at", 0) < 86400 or not fetch:
            return cached["rates"]
    except Exception:  # noqa: BLE001
        cached = None
    if not fetch:
        return None
    try:
        with urllib.request.urlopen("https://api.frankfurter.app/latest", timeout=4) as r:
            data = json.loads(r.read())
        table = dict(data["rates"], EUR=1.0)
        RATES_FILE.parent.mkdir(parents=True, exist_ok=True)
        RATES_FILE.write_text(json.dumps({"at": time.time(), "rates": table}))
        return table
    except Exception:  # noqa: BLE001
        return cached["rates"] if cached else None


def code_for(word, table):
    w = word.strip().lower()
    if w in CURRENCY_NAMES:
        return CURRENCY_NAMES[w]
    return w.upper() if len(w) == 3 and table and w.upper() in table else None


def currency_phrase(q, table_fn=rates):
    t = q.lower().strip().rstrip("?").strip()
    m = re.fullmatch(r"([$€£¥₹]?)\s*" + NUM + r"\s*([a-z$€£¥₹]+)?\s+(?:in|to|into)\s+([a-z$€£¥₹]+)", t)
    if not m or not (m[1] or m[3]):
        return None
    src_word = m[1] or m[3]
    if src_word not in CURRENCY_NAMES and not re.fullmatch(r"[a-z]{3}", src_word):
        return None
    if m[4] not in CURRENCY_NAMES and not re.fullmatch(r"[a-z]{3}", m[4]):
        return None
    table = table_fn()
    if not table:
        return None
    a, b = code_for(src_word, table), code_for(m[4], table)
    if not a or not b or a not in table or b not in table or a == b:
        return None
    v = num(m[2])
    out = v / table[a] * table[b]
    return f"{fmt(round(out, 2))} {b}", f"{fmt(v)} {a} · ECB rate, updated daily"


# ── One query ────────────────────────────────────────────────────────────
def answer(q, now=None, table_fn=rates):
    now = now or dt.datetime.now().astimezone()
    q = q.strip()
    if len(q) < 3:
        return {}
    for kind, fn in (("time", lambda: time_phrase(q, now)), ("date", lambda: date_phrase(q, now.date())),
                     ("unit", lambda: unit_phrase(q)), ("currency", lambda: currency_phrase(q, table_fn)),
                     ("math", lambda: math_phrase(q))):
        try:
            r = fn()
        except Exception:  # noqa: BLE001
            r = None
        if r:
            if kind == "math":
                value, what = r[0], r[1]
                suffix = r[2] if len(r) > 2 else ""
                title = fmt(round(value, 10)) + suffix
                return {"kind": kind, "title": title, "subtitle": what, "copy": title.replace(",", "")}
            return {"kind": kind, "title": r[0], "subtitle": r[1], "copy": r[0]}
    return {}


def main(argv):
    if argv[:1] == ["--serve"]:
        for line in sys.stdin:
            try:
                req = json.loads(line)
                out = dict(answer(req.get("q", "")), id=req.get("id"))
            except Exception:  # noqa: BLE001
                out = {"id": None}
            sys.stdout.write(json.dumps(out) + "\n")
            sys.stdout.flush()
        return 0
    print(json.dumps(answer(" ".join(argv))))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
