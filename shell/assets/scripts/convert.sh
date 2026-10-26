#!/usr/bin/env bash
# Spotlight conversions through qalc: "10 km to mi", "100 usd in eur",
# "72 f to c", "3 cups in ml". Prints the answer, or nothing when qalc's
# answer doesn't look like the asked-for unit. Exchange rates are refreshed
# (once a day at most) when the query looks like currencies. Mixed answers
# ("6 mi + 376 yd") come back as one number in the target unit.
q="$1"
[[ "$q" =~ ^(.+)[[:space:]]+(to|in|as|into)[[:space:]]+(.+)$ ]] || exit 0
from="${BASH_REMATCH[1]}" to="${BASH_REMATCH[3]}"
# Bare f / c / k after a number are temperatures; plural unit names singular
temp() { sed -E 's/^([-0-9.,]+)[[:space:]]*(°)?[fF]$/\1 °F/; s/^([-0-9.,]+)[[:space:]]*(°)?[cC]$/\1 °C/; s/^(°)?[fF]$/°F/; s/^(°)?[cC]$/°C/' <<<"$1"; }
sing() { sed -E 's/\b(cup|ounce|pound|gram|liter|litre|meter|metre|mile|foot|yard|gallon|pint|quart|teaspoon|tablespoon|second|minute|hour|day|week|month|year|byte|bit)s\b/\1/g; s/\bfeet\b/foot/g; s/\binches\b/inch/g; s/\b([kKmMgGtTpP])i?[bB]\b/\U\1B/g' <<<"$1"; }
from=$(sing "$(temp "$from")") to=$(sing "$(temp "$to")")
stamp="${XDG_CACHE_HOME:-$HOME/.cache}/irice/exrates-stamp"
args=(-t)
if grep -qiE '^[$€£¥₹]|\b[a-z]{3}$' <<<"$from" && grep -qiE '^[a-z]{3}$|^[$€£¥₹]$' <<<"$to" && [[ -z "$(find "$stamp" -mmin -1440 2>/dev/null)" ]]; then
    args+=(-e)
    mkdir -p "${stamp%/*}" && : >"$stamp"
fi
out=$(timeout 6 qalc "${args[@]}" "$from to $to" 2>/dev/null | tail -1)
if [[ "$out" == *" + "* ]]; then
    n=$(timeout 4 qalc -t "($from)/(1 $to)" 2>/dev/null | tail -1)
    [[ "$n" =~ ^[0-9.,−-]+(E[−-]?[0-9]+)?$ ]] && out="$n $to"
fi
# Nonsense in, compound units out: say nothing
[[ -z "$out" || "$out" == *"·"* || "$out" =~ [²³⁴⁵⁶⁷] && ! "$to" =~ [²³23] ]] && exit 0
printf '%s\n' "$out"
