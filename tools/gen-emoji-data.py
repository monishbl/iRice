#!/usr/bin/env python3
"""Regenerates shell/assets/data/emoji.json for the emoji picker.

Sources (downloaded, pinned):
  - Unicode emoji-test.txt: the emoji, their order, groups and skin-tone variants
  - Unicode CLDR annotations (+ derived): search keywords
  - emojibase-data GitHub shortcodes (CLDR shortcodes as a fallback): :shortcode:
Kaomoji and text symbols are curated below; symbol names come from Python's
unicodedata.

If uharfbuzz is installed, sequences that Noto Color Emoji can't draw as one
glyph are dropped (pip install uharfbuzz, or run it from a venv that has it).

Output (compact arrays, kept small):
  groups:   [[id, title], ...]
  emoji:    [[char, name, keywords, shortcode, group, tone], ...]
            tone: 0 = none, 1 = one tone (insert the modifier after the first
            code point), or [5 strings] for the light..dark variants
  kaomoji:  [[section title, [[text, keywords], ...]], ...]
  symbols:  [[section title, [[char, name], ...]], ...]

Usage: tools/gen-emoji-data.py [path/to/NotoColorEmoji.ttf]
"""
import json
import os
import re
import sys
import tempfile
import unicodedata
import urllib.request

EMOJI_VER = "latest"  # Unicode's latest emoji-test.txt
CLDR_VER = "49.0.0"
EMOJIBASE_VER = "18.0.0"
ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
OUT = os.path.join(ROOT, "shell", "assets", "data", "emoji.json")
FONT = sys.argv[1] if len(sys.argv) > 1 else "/usr/share/fonts/noto/NotoColorEmoji.ttf"

URLS = {
    "test": f"https://unicode.org/Public/emoji/{EMOJI_VER}/emoji-test.txt",
    "ann": f"https://cdn.jsdelivr.net/npm/cldr-annotations-full@{CLDR_VER}/annotations/en/annotations.json",
    "derived": f"https://cdn.jsdelivr.net/npm/cldr-annotations-derived-full@{CLDR_VER}/annotationsDerived/en/annotations.json",
    "github": f"https://cdn.jsdelivr.net/npm/emojibase-data@{EMOJIBASE_VER}/en/shortcodes/github.json",
    "cldr": f"https://cdn.jsdelivr.net/npm/emojibase-data@{EMOJIBASE_VER}/en/shortcodes/cldr.json",
}

GROUPS = [
    ("smileys", "Smileys & people", ["Smileys & Emotion", "People & Body"]),
    ("animals", "Animals & nature", ["Animals & Nature"]),
    ("food", "Food & drink", ["Food & Drink"]),
    ("activities", "Activities", ["Activities"]),
    ("travel", "Travel & places", ["Travel & Places"]),
    ("objects", "Objects", ["Objects"]),
    ("symbols", "Symbols", ["Symbols"]),
    ("flags", "Flags", ["Flags"]),
]
TONES = [0x1F3FB, 0x1F3FC, 0x1F3FD, 0x1F3FE, 0x1F3FF]

KAOMOJI = [
    ("Happy", [
        ("(＾▽＾)", "happy smile joy"), ("(◕‿◕)", "happy smile cute"), ("(｡◕‿◕｡)", "happy smile cute"),
        ("(˶ᵔ ᵕ ᵔ˶)", "happy blush cute"), ("ヽ(・∀・)ﾉ", "happy cheer yay"), ("(ﾉ◕ヮ◕)ﾉ*:･ﾟ✧", "happy excited sparkle yay"),
        ("(✿◠‿◠)", "happy flower smile"), ("(≧▽≦)", "happy excited laugh"), ("(＾ω＾)", "happy smile"),
        ("( ´ ▽ ` )", "happy smile"), ("(*^‿^*)", "happy blush smile"), ("٩(◕‿◕)۶", "happy cheer yay"),
        ("\\(^o^)/", "happy cheer yay"), ("(´∀｀)", "happy smile"), ("(^_^)", "happy smile"),
        ("(^▽^)", "happy smile"), ("(*≧ω≦*)", "happy excited"), ("☆*:.｡.o(≧▽≦)o.｡.:*☆", "happy excited sparkle"),
        ("(o^▽^o)", "happy smile"), ("(⌒‿⌒)", "happy smile content"), ("(•‿•)", "happy smile"),
        ("ヾ(≧▽≦*)o", "happy excited"), ("(★^O^★)", "happy star"), ("( ˘ ³˘)♥", "happy kiss love"),
        ("(^_−)☆", "wink"), ("(^_~)", "wink"), ("( ͡° ͜ʖ ͡°)", "lenny smirk"), ("( ͡~ ͜ʖ ͡°)", "lenny wink"),
        ("(☞ﾟヮﾟ)☞", "point finger guns"), ("ᕕ( ᐛ )ᕗ", "happy walk strut"),
    ]),
    ("Love", [
        ("(♡˙︶˙♡)", "love heart"), ("(´｡• ᵕ •｡`) ♡", "love heart cute"), ("(◍•ᴗ•◍)❤", "love heart"),
        ("(≧◡≦) ♡", "love heart"), ("(*♡∀♡)", "love heart eyes"), ("♡(˃͈ દ ˂͈ ༶ )", "love heart"),
        ("(っ˘з(˘⌣˘ )", "love kiss"), ("(づ￣ ³￣)づ", "love kiss hug"), ("(´ε｀ )♡", "love kiss"),
        ("(灬º‿º灬)♡", "love blush heart"), ("(˘∀˘)/(μ‿μ) ❤", "love couple"), ("( ˘⌣˘)♡(˘⌣˘ )", "love couple"),
        ("♥‿♥", "love heart eyes"), ("(◕‿◕)♡", "love heart"), ("(ღ˘⌣˘ღ)", "love heart"),
        ("(ﾉ´ з `)ノ", "love kiss"), ("(*˘︶˘*).｡.:*♡", "love heart dreamy"), ("ʕ•ᴥ•ʔ♡", "love bear heart"),
    ]),
    ("Hugs & greetings", [
        ("(っ´▽`)っ", "hug"), ("(つ≧▽≦)つ", "hug"), ("⊂(・▽・⊂)", "hug"), ("(づ｡◕‿‿◕｡)づ", "hug cute"),
        ("(づ ◕‿◕ )づ", "hug"), ("⊂(´・ω・｀⊂)", "hug"), ("(ノ^_^)ノ", "wave hello hi"), ("( ´ ▽ ` )ﾉ", "wave hello hi bye"),
        ("(￣▽￣)ノ", "wave hello hi bye"), ("ヾ(・ω・*)", "wave hello hi bye"), ("(*・ω・)ﾉ", "wave hello hi bye"),
        ("(^-^*)/", "wave hello hi"), ("(￣ー￣)ゞ", "salute"), ("(｀・ω・´)ゞ", "salute"), ("m(_ _)m", "bow sorry thanks"),
        ("<(_ _)>", "bow sorry thanks"), ("(シ_ _)シ", "bow sorry"), ("(o´ω`o)ﾉ", "wave hi bye"),
    ]),
    ("Sad", [
        ("(╥﹏╥)", "sad cry tears"), ("(T_T)", "sad cry tears"), ("(;_;)", "sad cry tears"), ("(ಥ﹏ಥ)", "sad cry tears"),
        ("(ಥ_ಥ)", "sad cry tears"), ("(´；ω；`)", "sad cry tears"), ("(っ˘̩╭╮˘̩)っ", "sad"), ("( ╥ω╥ )", "sad cry"),
        ("(｡•́︿•̀｡)", "sad pout"), ("(︶︹︺)", "sad pout"), ("(´・_・`)", "sad disappointed"), ("(-_-)", "meh unamused"),
        ("(._.)", "sad quiet"), ("(｡ŏ﹏ŏ)", "sad worried"), ("(個_個)", "sad cry"), ("(〒﹏〒)", "sad cry tears"),
        ("(◞‸◟；)", "sad worried"), ("( ´_ゝ`)", "sad disappointed"), ("(´-ω-`)", "sigh tired"),
    ]),
    ("Angry", [
        ("(╬ Ò﹏Ó)", "angry mad"), ("(＃`Д´)", "angry mad"), ("(ノಠ益ಠ)ノ", "angry rage"), ("ಠ_ಠ", "disapproval look stare"),
        ("ಠ益ಠ", "angry rage"), ("(҂ `з´ )", "angry mad"), ("(｀ε´)", "angry pout"), ("(•̀ᴗ•́)و ̑̑", "determined fight"),
        ("(ง •̀_•́)ง", "fight determined"), ("(ง'̀-'́)ง", "fight"), ("ヽ(`Д´)ﾉ", "angry rage"), ("(╯°□°)╯︵ ┻━┻", "table flip rage angry"),
        ("┻━┻ ︵ヽ(`Д´)ﾉ︵ ┻━┻", "double table flip rage"), ("┬─┬ノ( º _ ºノ)", "table put back calm"),
        ("(ノ°益°)ノ", "angry rage"), ("(凸ಠ益ಠ)凸", "angry rude finger"), ("(¬_¬)", "annoyed side eye"),
        ("(¬_¬\")", "annoyed side eye"), ("(눈_눈)", "annoyed stare"), ("(ーー;)", "annoyed"),
    ]),
    ("Surprised", [
        ("(⊙_⊙)", "surprised shock"), ("(°o°)", "surprised shock"), ("(O_O)", "surprised shock"), ("(⊙﹏⊙)", "surprised worried"),
        ("Σ(°△°|||)", "surprised shock"), ("(ﾟДﾟ;)", "surprised shock scared"), ("w(°ｏ°)w", "surprised wow"),
        ("(°ロ°) !", "surprised"), ("(o_O)", "confused surprised"), ("(◎_◎;)", "dizzy shocked"), ("(ʘᗩʘ')", "surprised shock"),
        ("Σ(ﾟロﾟ)", "surprised shock"), ("(*°ｏ°*)", "surprised wow"), ("( ꒪Д꒪)ノ", "scared shock"),
    ]),
    ("Shrug & confused", [
        ("¯\\_(ツ)_/¯", "shrug whatever idk"), ("┐(￣ヘ￣)┌", "shrug whatever"), ("╮(￣ω￣;)╭", "shrug whatever"),
        ("ヽ(ー_ー )ノ", "shrug whatever"), ("┐(´～｀)┌", "shrug whatever"), ("(・_・ヾ", "confused scratch"),
        ("(・・;)ゞ", "confused nervous"), ("(￣～￣;)", "confused thinking"), ("(⊙_☉)", "confused"), ("(・・?)", "confused question"),
        ("(◎ ◎)ゞ", "confused"), ("(゜-゜)", "confused blank"), ("(°ー°〃)", "confused"), ("(¯―¯٥)", "nervous sweat"),
        ("(^_^;)", "nervous sweat awkward"), ("(￣▽￣;)", "awkward sweat"), ("(；一_一)", "awkward"), ("(・_・;)", "nervous"),
    ]),
    ("Sleepy", [
        ("(－_－) zzZ", "sleep tired"), ("(∪｡∪)｡｡｡zzZ", "sleep tired"), ("(_ _*)Z z z", "sleep tired"), ("(=_=)", "tired bored"),
        ("(-.-)Zzz...", "sleep tired"), ("(´〜｀*) zzz", "sleep tired"), ("(｡-ω-)zzz", "sleep tired"), ("(ᴗ˳ᴗ)", "sleep peaceful"),
        ("(￣o￣) zzZZzzZZ", "sleep snore"), ("( ˘ω˘ )ｽﾔｧ…", "sleep peaceful"),
    ]),
    ("Animals", [
        ("ʕ•ᴥ•ʔ", "bear"), ("ʕ ᵔᴥᵔ ʔ", "bear happy"), ("ʕ·͡ᴥ·ʔ", "bear"), ("ʕ•̀ω•́ʔ✧", "bear determined"), ("ʕノ•ᴥ•ʔノ ︵ ┻━┻", "bear table flip"),
        ("(=^･ω･^=)", "cat"), ("(=^･ｪ･^=)", "cat"), ("ฅ^•ﻌ•^ฅ", "cat paws"), ("(^._.^)ﾉ", "cat wave"), ("ᓚᘏᗢ", "cat"),
        ("(=ↀωↀ=)", "cat"), ("U^ェ^U", "dog"), ("▼・ᴥ・▼", "dog"), ("V●ᴥ●V", "dog"), ("∪＾ェ＾∪", "dog"),
        ("／(=´x`=)＼", "bunny rabbit"), ("(・θ・)", "bird"), ("(^◇^)", "bird"),
        ("<コ:彡", "squid"), (">°)))彡", "fish"), ("くコ:彡", "squid"), ("(V) (°,,,,°) (V)", "crab"), ("ᘛ⁐̤ᕐᐷ", "mouse rat"),
        ("~>°)～～～", "snake"), ("(・(ｪ)・)", "bear"), ("(｀(ｴ)´)", "bear angry"), ("Ꮚ˘ꈊ˘Ꮚ", "sheep"),
    ]),
    ("Actions", [
        ("(╯°□°）╯︵ ┻━┻", "table flip"), ("(ﾉ≧∀≦)ﾉ ‥…━━━★", "magic sparkle"), ("(∩ᄑ_ᄑ)⊃━☆ﾟ*･｡*･:≡( ε:)", "magic wizard"),
        ("( ˘▽˘)っ♨", "coffee tea drink"), ("(っ˘ڡ˘ς)", "eat yum food"), ("( ´ ∀ `)ノ～ ♡", "send love"),
        ("(ノ・∀・)ノ", "cheer throw"), ("ᕦ(ò_óˇ)ᕤ", "strong flex muscle"), ("ᕙ(⇀‸↼‶)ᕗ", "strong flex muscle"),
        ("(ง ื▿ ื)ว", "dance"), ("┏(＾0＾)┛┗(＾0＾)┓", "dance party"), ("♪～(´ε｀ )", "whistle music"), ("ヾ(⌐■_■)ノ♪", "dance cool music"),
        ("(⌐■_■)", "cool sunglasses deal with it"), ("( •_•)>⌐■-■", "put on sunglasses cool"), ("(•_•)", "neutral stare"),
        ("(ﾉ´ヮ`)ﾉ*: ･ﾟ", "sparkle throw happy"), ("(>_<)", "ouch frustrated"), ("(>人<;)", "please beg"), ("(人´∀｀)", "please hopeful"),
        ("☜(ﾟヮﾟ☜)", "point"), ("(☞ ͡° ͜ʖ ͡°)☞", "point lenny"), ("(｀・ω・´)b", "thumbs up good"), ("d(^_^o)", "thumbs up good"),
        ("(^_-)-☆", "wink star"), ("ε=ε=ε=┌(;*´Д`)ﾉ", "run escape"), ("ε=┌( ≧▽≦)┘", "run excited"), ("(っ•̀ω•́)っ✎⁾⁾", "write note"),
        ("( ˘･з･)", "pout"), ("( ‾́ ◡ ‾́ )", "smug"), ("( ¬‿¬)", "smug side eye"), ("(￣ω￣)", "smug content"),
        ("(-‸ლ)", "facepalm"), ("(／。＼)", "facepalm embarrassed"), ("(⁄ ⁄•⁄ω⁄•⁄ ⁄)", "embarrassed blush"), ("(〃▽〃)", "blush shy"),
    ]),
]

# Text symbols: each section is a string of characters (named by unicodedata)
SYMBOLS = [
    ("Arrows", "←↑→↓↔↕↖↗↘↙⇐⇑⇒⇓⇔⇕⇦⇧⇨⇩↩↪↺↻⟲⟳⇄⇅⇆⇇⇈⇉⇊⇋⇌↞↟↠↡⇤⇥⤴⤵⟵⟶⟷⟸⟹⟺➔➜➝➞➟➠➡➤➥➦➧➨"),
    ("Math", "+−×÷±∓=≠≈≡≢≤≥≪≫∝∞∑∏∫∬∮∂∆∇√∛∜∈∉∋⊂⊃⊆⊇⊄∪∩∅∧∨¬⊕⊗⊥∥∠∡°′″‰‱∴∵∶∷∀∃∄ℕℤℚℝℂℵ"),
    ("Currency", "$€£¥¢₹₽₩₪₫₱₦₴₸₺₼₾₿¤ƒ฿₡₲₵₭₮₤"),
    ("Punctuation", "–—‒―…·•‣◦⁃‐‑‧¡¿‽⁇⁈⁉«»‹›„“”‚‘’\"'¶§†‡※⁂⸮"),
    ("Legal & marks", "©®™℠℗№℃℉℮℡ℹ℅℆℀℁⌀"),
    ("Keyboard", "⌘⌥⇧⌃⎋⏎↵⌫⌦⇥⇤⇪⏏⌤⎈⎇⌨␣⇞⇟↖↘"),
    ("Shapes", "■□▢▣▤▥▦▧▨▩▪▫▬▭▮▯▰▱▲△▴▵▶▷▸▹►▻▼▽▾▿◀◁◂◃◄◅◆◇◈◉◊○◌◍◎●◐◑◒◓◔◕◖◗◘◙◚◛◜◝◞◟◠◡◢◣◤◥◦◧◨◩◪◫◬◭◮◯"),
    ("Stars & marks", "★☆✦✧✩✪✫✬✭✮✯✰✱✲✳✴✵✶✷✸✹✺✻✼✽✾✿❀❁❂❃❄❅❆❇❈❉❊❋✓✔✕✖✗✘☐☑☒"),
    ("Cards & games", "♠♣♥♦♤♧♡♢⚀⚁⚂⚃⚄⚅♔♕♖♗♘♙♚♛♜♝♞♟"),
    ("Music", "♩♪♫♬♭♮♯𝄞𝄢"),
    ("Weather & misc", "☀☁☂☃☄☼☽☾☇☈☉⚡❤❥❣☎☏✆✉✂✃✄✎✏✐✑✒☮☯☸☘☙⚐⚑⚘⚜⚠☢☣⚛⚕⚖⚗⚙⚒♲♻♼♽⌚⌛⏳⧗"),
    ("Superscripts & subscripts", "⁰¹²³⁴⁵⁶⁷⁸⁹⁺⁻⁼⁽⁾ⁿⁱ₀₁₂₃₄₅₆₇₈₉₊₋₌₍₎ₐₑₒₓₔ"),
    ("Fractions & numbers", "½⅓⅔¼¾⅕⅖⅗⅘⅙⅚⅐⅛⅜⅝⅞⅑⅒↉ⅠⅡⅢⅣⅤⅥⅦⅧⅨⅩⅪⅫⅬⅭⅮⅯⅰⅱⅲⅳⅴⅵⅶⅷⅸⅹ①②③④⑤⑥⑦⑧⑨⑩❶❷❸❹❺❻❼❽❾❿"),
    ("Greek", "ΑΒΓΔΕΖΗΘΙΚΛΜΝΞΟΠΡΣΤΥΦΧΨΩαβγδεζηθικλμνξοπρσςτυφχψω"),
    ("Box drawing", "─│┌┐└┘├┤┬┴┼━┃┏┓┗┛┣┫┳┻╋═║╔╗╚╝╠╣╦╩╬╭╮╯╰╱╲╳░▒▓█▀▄▌▐"),
]


def fetch(url, dest):
    with urllib.request.urlopen(url, timeout=60) as r, open(dest, "wb") as f:
        f.write(r.read())
    return dest


def cps(s):
    return [ord(c) for c in s]


def strip_fe0f(s):
    return s.replace("\uFE0F", "")


def hexkey(s):
    return "-".join("%X" % c for c in cps(strip_fe0f(s)))


def shaper():
    """Returns f(str) -> True when the font draws it as a single glyph."""
    try:
        import uharfbuzz as hb
    except ImportError:
        print("uharfbuzz not installed: keeping every emoji (can't check the font)", file=sys.stderr)
        return lambda s: True
    if not os.path.exists(FONT):
        print("font not found: " + FONT, file=sys.stderr)
        return lambda s: True
    face = hb.Face(open(FONT, "rb").read())
    font = hb.Font(face)

    def one(s):
        buf = hb.Buffer()
        buf.add_str(s)
        buf.guess_segment_properties()
        hb.shape(font, buf)
        infos = buf.glyph_infos
        return len(infos) == 1 and infos[0].codepoint != 0

    return one


def main():
    tmp = tempfile.mkdtemp(prefix="irice-emoji-")
    paths = {k: fetch(u, os.path.join(tmp, k)) for k, u in URLS.items()}

    ann = json.load(open(paths["ann"], encoding="utf-8"))["annotations"]["annotations"]
    ann.update(json.load(open(paths["derived"], encoding="utf-8"))["annotationsDerived"]["annotations"])
    github = json.load(open(paths["github"], encoding="utf-8"))
    cldr_codes = json.load(open(paths["cldr"], encoding="utf-8"))
    drawable = shaper()

    group_of = {}
    for i, (_, _, names) in enumerate(GROUPS):
        for n in names:
            group_of[n] = i

    entries = []  # [char, name, group]
    toned = {}  # base key (no tones, no FE0F) -> {tone index: string}
    version = ""
    group = None
    line_re = re.compile(r"^([0-9A-F ]+?)\s*;\s*fully-qualified\s*#\s*(\S+)\s+E[\d.]+\s+(.+)$")
    for line in open(paths["test"], encoding="utf-8"):
        line = line.rstrip("\n")
        if line.startswith("# Version:"):
            version = line.split(":", 1)[1].strip()
        if line.startswith("# group:"):
            group = group_of.get(line.split(":", 1)[1].strip())
            continue
        m = line_re.match(line)
        if not m or group is None:
            continue
        seq = [int(x, 16) for x in m.group(1).split()]
        char, name = m.group(2), m.group(3)
        tones = [c for c in seq if c in TONES]
        if tones:
            if len(set(tones)) == 1:  # uniform tone only (one choice per emoji)
                base = "-".join("%X" % c for c in seq if c not in TONES and c != 0xFE0F)
                toned.setdefault(base, {})[TONES.index(tones[0])] = char
            continue
        entries.append([char, name, group])

    def tone_rule(base, t):
        c = cps(base)
        if len(c) > 1 and c[1] == 0xFE0F:
            del c[1]
        c.insert(1, TONES[t])
        return "".join(chr(x) for x in c)

    out = []
    dropped = 0
    rule_count = explicit_count = 0
    for char, name, group in entries:
        if not drawable(char):
            dropped += 1
            continue
        a = ann.get(char) or ann.get(strip_fe0f(char)) or {}
        words = set(re.findall(r"[\w']+", name.lower()))
        kw = []
        for k in a.get("default", []):
            k = k.lower()
            if k not in kw and not set(re.findall(r"[\w']+", k)) <= words:
                kw.append(k)
        key = hexkey(char)
        sc = github.get(key) or cldr_codes.get(key) or ""
        if isinstance(sc, list):
            sc = sc[0]
        tone = 0
        variants = toned.get(key)
        if variants and len(variants) == 5:
            vs = [variants[i] for i in range(5)]
            if all(drawable(v) for v in vs):
                if all(vs[i] == tone_rule(char, i) for i in range(5)):
                    tone = 1
                    rule_count += 1
                else:
                    tone = vs
                    explicit_count += 1
        out.append([char, name, " ".join(kw), sc, group, tone])

    symbols = []
    for title, chars in SYMBOLS:
        items, seen = [], set()
        for ch in chars:
            if ch in seen:
                continue
            seen.add(ch)
            try:
                nm = unicodedata.name(ch).lower()
            except ValueError:
                nm = ""
            items.append([ch, nm])
        symbols.append([title, items])

    data = {
        "version": version,
        "groups": [[g[0], g[1]] for g in GROUPS],
        "emoji": out,
        "kaomoji": [[t, [list(k) for k in items]] for t, items in KAOMOJI],
        "symbols": symbols,
    }
    os.makedirs(os.path.dirname(OUT), exist_ok=True)
    with open(OUT, "w", encoding="utf-8") as f:
        json.dump(data, f, ensure_ascii=False, separators=(",", ":"))
    print(f"Wrote {OUT}: {len(out)} emoji (Emoji {version}; {rule_count} one-tone, {explicit_count} multi-person toned, {dropped} not in the font), "
          f"{sum(len(i) for _, i in KAOMOJI)} kaomoji, {sum(len(i) for _, i in symbols)} symbols, {os.path.getsize(OUT) // 1024} KB")


if __name__ == "__main__":
    main()
