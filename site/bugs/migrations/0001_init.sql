-- Bug reports from iRice (Settings → Report a Bug), filed as GitHub issues.
-- No IP addresses are kept: the rate limit stores a salted hash of the
-- address and the hour, and those rows are deleted after a day.
CREATE TABLE IF NOT EXISTS reports (
    id TEXT PRIMARY KEY,           -- 32 random hex characters; also the screenshot's unguessable link
    created INTEGER NOT NULL,      -- ms since epoch
    title TEXT NOT NULL,
    happened TEXT NOT NULL,
    steps TEXT NOT NULL DEFAULT '',
    expected TEXT NOT NULL DEFAULT '',
    version TEXT NOT NULL DEFAULT '',
    sysinfo TEXT NOT NULL DEFAULT '{}', -- JSON
    log TEXT NOT NULL DEFAULT '',
    screenshot BLOB,               -- at most ~1.5 MB (D1 rows top out at 2 MB)
    screenshot_type TEXT,
    issue_url TEXT
);
CREATE INDEX IF NOT EXISTS reports_created ON reports (created);

CREATE TABLE IF NOT EXISTS rate (
    bucket TEXT PRIMARY KEY,       -- sha256(salt + ip + hour)
    hour INTEGER NOT NULL,
    count INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS rate_hour ON rate (hour);
