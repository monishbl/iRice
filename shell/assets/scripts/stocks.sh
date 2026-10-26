#!/usr/bin/env bash
# Today's prices for the Stocks widget (Yahoo Finance's public chart data),
# one JSON line per symbol: {"s","n","p","pc","cur","c":[closes]}.
for s in "$@"; do
    curl -sf --max-time 8 -A "Mozilla/5.0" "https://query1.finance.yahoo.com/v8/finance/chart/$(printf '%s' "$s" | jq -sRr @uri)?range=1d&interval=15m" |
        jq -c '.chart.result[0] | {s: .meta.symbol, n: (.meta.shortName // .meta.longName // .meta.symbol), p: .meta.regularMarketPrice, pc: .meta.chartPreviousClose, cur: .meta.currency, c: [.indicators.quote[0].close[]? | select(. != null)]}' 2>/dev/null
done
