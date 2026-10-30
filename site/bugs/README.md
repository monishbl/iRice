# iRice bug reports

The server behind Settings → Report a Bug. A Cloudflare Worker at
`https://irice-bugs.monishbl.in` stores each report in a D1 database and files
it as a GitHub issue in monishbl/iRice, labelled `bug-report`. Everything it
uses is on Cloudflare's free tier.

- `POST /api/bug-reports`: a report (JSON, at most 2.5 MB). Answers
  `{ ok, id, issue_url }`, or `{ ok: false, error }` with a message that says
  how to fix it.
- `GET /api/bug-reports/<id>/screenshot`: a report's screenshot. The id is 128
  random bits, so only the issue's link finds it.

No IP addresses are stored. A salted hash of the address and the hour allows
five reports an hour from one network, and those rows go after a day.

It's a Worker of its own, not part of the site Worker in `site/`, so GitHub's
site deploys keep working before this one exists.

## Setting it up (once)

Run these from the repository root, logged in with `npx wrangler login`.

1. Create the database, then put the `database_id` it prints (and its name)
   into `site/bugs/wrangler.jsonc`. It exists: `irice`, set up on 30 Oct.

       npx wrangler d1 create irice

2. Create the tables:

       npx wrangler d1 migrations apply irice --remote --config site/bugs/wrangler.jsonc

3. Add the two secrets:

       npx wrangler secret put GITHUB_TOKEN --config site/bugs/wrangler.jsonc
       npx wrangler secret put RATE_SALT --config site/bugs/wrangler.jsonc

   `GITHUB_TOKEN` is a fine-grained personal access token limited to the
   monishbl/iRice repository, with the Issues permission set to Read and
   write and nothing else. `RATE_SALT` is any long random string, for example
   the output of `openssl rand -hex 32`.

4. Create the `bug-report` label on GitHub (Issues → Labels → New label), so
   new issues can use it.

5. Deploy:

       npx wrangler deploy --config site/bugs/wrangler.jsonc

   This also sets up the `irice-bugs.monishbl.in` hostname.

## Checking it

    curl -s https://irice-bugs.monishbl.in/api/bug-reports -X POST \
      -H 'Content-Type: application/json' \
      -d '{"title":"Test report","happened":"Testing the report server"}'

That should answer with `"ok": true` and an `issue_url`. Close the test issue
afterwards.

Without `GITHUB_TOKEN` reports are still stored, and `issue_url` is `null`.

## Tests

    node --test tests/bug_report_worker.test.cjs
