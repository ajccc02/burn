# Deployment guide

## Supabase

1. Create/select the dedicated FindIt project on the free plan.
2. Execute both files in `supabase/migrations/` once in filename order in the SQL Editor, or apply them using authenticated Supabase CLI migrations.
3. Confirm `reports` and `usability_feedback` have RLS enabled. Ordinary authenticated sessions have feedback insertion permission and no feedback read permission.
4. Confirm `report-images` is private, limits files to 5 MB, allows JPEG/PNG/WebP, and has the supplied owner/read policies.
5. Optionally execute `supabase/seed.sql`. Repeating it preserves the fixed demo IDs and creates no duplicate reports.
6. In Authentication's sign-in/provider settings, enable anonymous sign-ins. In protection settings, configure Cloudflare Turnstile CAPTCHA using its secret key. Keep rate limits enabled.
7. Obtain the project API URL, publishable key, and public Turnstile site key. Add localhost only when needed for approved development testing.

## Vercel

1. Import `ajccc02/burn` or the repository containing this source into your authorized Vercel account.
2. Set Root Directory to `findit`, framework to Next.js, Node.js to 24.x, Install Command to `npm ci`, and Build Command to `npm run build`.
3. Set the three variables in `.env.example` in Production and in any intended Preview/Development environments.
4. Deploy and note the returned HTTPS hostname. Add that hostname to the Turnstile widget allowlist and set the Supabase Site URL accordingly.
5. Redeploy after modifying public environment values because Next.js includes them in browser bundles.
6. Confirm the chosen production audience can access the page. Preview deployment protection may require Vercel sign-in and is unsuitable as the classroom link unless all evaluators have access.

## Live verification

- Open the URL in two independent browsers/contexts. Each should obtain its own guest identity.
- Submit a report and item-only photo in the first browser. Refresh and confirm persistence.
- Refresh the second browser's feed. Find the report and confirm that editing it is unavailable and blocked at the database level.
- Search, filter, open details, and inspect Possible Matches against actual rows.
- Edit, resolve, reopen, and delete an owned report using confirmation.
- During actual voluntary testing, submit feedback and verify its presence through the researcher dashboard. Ordinary guests must be unable to query responses.
- Check the required mobile widths and keyboard navigation. Record actual deployed verification results.

## Protected feedback export

An authorized researcher opens the Supabase Table Editor for `public.usability_feedback` and exports the desired columns as CSV. Export Initial and Showcase separately or retain the stage column. Restrict the exported file to the research group. Any analysis or summary must come from actual submitted responses.

## Cleanup and retention

Export approved study records before removal. Remove demo rows with `delete from public.reports where is_demo = true;`. Identify test rows individually. Delete corresponding image bytes through Storage, and review unreferenced owner folders for abandoned uploads. Never delete `storage.objects` metadata as a substitute for Storage's object API. Only the research team decides when participant responses and guest accounts are removed. Deleting Auth accounts cascades to their reports, so review ownership first.

## Optional rollback-only authorization check

Execute `tests/database/authorization.sql` with authorized SQL Editor access. It asserts ownership, feedback privacy, rating/consent constraints, and storage path protection. The final rollback leaves both technical Auth identities and feedback fixtures absent from study data. A failed assertion raises an error; roll back the transaction before further SQL work.
