# Tessera bug reports

The server behind Settings → Report a Bug. A Cloudflare Worker at
`https://tessera-bugs.monishbl.in` stores each report in a D1 database and
files it as a GitHub issue in monishbl/tessera, labelled `bug-report`.
Everything it uses is on Cloudflare's free tier.

- `POST /api/bug-reports`: a report (JSON, at most 2.5 MB). Answers
  `{ ok, id, issue_url }`, or `{ ok: false, error }` with a message that says
  how to fix it.
- `GET /api/bug-reports/<id>/screenshot`: a report's screenshot. The id is 128
  random bits, so only the issue's link finds it.

No IP addresses are stored. A salted hash of the address and the hour allows
five reports an hour from one network, and those rows go after a day.

It's a Worker of its own, not part of the site Worker in `site/`, so the
site's deploys keep working without it.

## Setting it up (once)

Run these from the repository root, logged in with `npx wrangler login`.

1. Create the database, then put the `database_id` it prints into
   `site/bugs/wrangler.jsonc`:

       npx wrangler d1 create tessera-bugs

2. Create the tables:

       npx wrangler d1 migrations apply tessera-bugs --remote --config site/bugs/wrangler.jsonc

3. Add the two secrets:

       npx wrangler secret put GITHUB_TOKEN --config site/bugs/wrangler.jsonc
       npx wrangler secret put RATE_SALT --config site/bugs/wrangler.jsonc

   `GITHUB_TOKEN` is a fine-grained personal access token limited to the
   monishbl/tessera repository, with the Issues permission set to Read and
   write and nothing else. `RATE_SALT` is any long random string, for example
   the output of `openssl rand -hex 32`.

4. Create the `bug-report` label on GitHub (Issues → Labels → New label), so
   new issues can use it.

5. Deploy:

       npx wrangler deploy --config site/bugs/wrangler.jsonc

   This also sets up its hostnames.

## Moving from the old name (once, 2026)

Until the switch-over the reports went to the Worker `irice-bugs` at
`irice-bugs.monishbl.in`, with the D1 database `irice`
(id `060067a4-73b9-4ba7-a07b-4e41b425007d`). Tessera's own name takes over
without losing a report or an issue's screenshot link: the old address keeps
answering (installs from before post there until they update, and older
issues link their screenshots there), now from the new Worker and database.
Secrets can't be read back from Cloudflare, so the new Worker gets its own:
`GITHUB_TOKEN` is the token above (the old one, if you still have it, or a new
one), `RATE_SALT` a fresh random string.

    export CLOUDFLARE_ACCOUNT_ID=fc02bfeb77675c77175d0d20804ca5e2
    C=site/bugs/wrangler.jsonc

1. The new database, its tables, and its id in the config (commit that):

       npx wrangler d1 create tessera-bugs
       # → database_id into $C
       npx wrangler d1 migrations apply tessera-bugs --remote --config $C

2. The new Worker's secrets (wrangler offers to create the Worker):

       npx wrangler secret put GITHUB_TOKEN --config $C
       openssl rand -hex 32 | npx wrangler secret put RATE_SALT --config $C

3. Deploy. The old address belongs to the old Worker: answer yes when
   wrangler asks to point it at this one (from then on everything reaches the
   new Worker; the old one gets nothing):

       npx wrangler deploy --config $C

4. The reports so far, into the new database (rows only: the tables exist;
   `OR IGNORE`, so running it again never doubles a report):

       npx wrangler d1 export irice --remote --no-schema --table reports --output /tmp/irice-reports.sql
       sed -i 's/^INSERT INTO /INSERT OR IGNORE INTO /' /tmp/irice-reports.sql
       npx wrangler d1 execute tessera-bugs --remote --file /tmp/irice-reports.sql --config $C

   (The `rate` table's rows last a day and are salted with the old secret:
   nothing to move.)

5. Check: the same number of reports in both, an old issue's screenshot
   from both addresses, and a test report (close its issue afterwards):

       npx wrangler d1 execute irice --remote --command "SELECT COUNT(*) FROM reports"
       npx wrangler d1 execute tessera-bugs --remote --command "SELECT COUNT(*) FROM reports" --config $C
       id=$(npx wrangler d1 execute tessera-bugs --remote --json --command "SELECT id FROM reports WHERE screenshot IS NOT NULL ORDER BY created LIMIT 1" --config $C | python3 -c 'import json,sys; print(json.load(sys.stdin)[0]["results"][0]["id"])')
       curl -sI https://irice-bugs.monishbl.in/api/bug-reports/$id/screenshot | head -1
       curl -sI https://tessera-bugs.monishbl.in/api/bug-reports/$id/screenshot | head -1
       curl -s https://tessera-bugs.monishbl.in/api/bug-reports -X POST -H 'Content-Type: application/json' \
         -d '{"title":"Test report","happened":"Testing the report server after the move"}'

6. A week later, when nothing has gone wrong: the old Worker and database go
   (the export from step 4 is the backup).

       npx wrangler delete irice-bugs
       npx wrangler d1 delete irice

On 1 February 2027 (docs/rename-to-tessera.md) the old address's route goes
from `wrangler.jsonc`, once Software Update has had installs from before send
to the new one for months.

## Checking it

    curl -s https://tessera-bugs.monishbl.in/api/bug-reports -X POST \
      -H 'Content-Type: application/json' \
      -d '{"title":"Test report","happened":"Testing the report server"}'

That should answer with `"ok": true` and an `issue_url`. Close the test issue
afterwards.

Without `GITHUB_TOKEN` reports are still stored, and `issue_url` is `null`.

## Tests

    node --test tests/bug_report_worker.test.cjs
