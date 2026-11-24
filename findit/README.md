# FindIt — Campus Lost & Found

**Lost something? Let's find it.**

A mobile-first CS212 Human-Computer Interaction prototype for students, faculty, and campus personnel. Reports, photographs, and evaluation responses use Supabase services. The application is contained in this repository's `findit/` directory.

## Stack and structure

Next.js App Router, React, strict TypeScript, Tailwind CSS, reusable shadcn-style native controls, Radix Alert Dialog, Lucide, Supabase PostgreSQL/Auth/Storage, and Vercel. Inter is bundled with the application.

```text
src/app/                 Routes and authenticated server actions
src/components/          Forms, navigation, cards, guest access, and views
src/lib/                 Validation, matching, image processing, and Supabase clients
supabase/migrations/     Database, functions, RLS, and private storage setup
supabase/seed.sql        Optional repeatable fictional item dataset
tests/unit/             Matching and input-validation checks
tests/database/         Rollback-only PostgreSQL authorization assertions
tests/e2e/              Browser flows and accessibility checks
docs/                   HCI research templates, test guides, and deployment instructions
```

## Install and configure

Use Node.js 24 LTS and npm. Node.js 26 is also accepted for local development. From this repository's root:

```sh
cd findit
npm ci
```

Create a dedicated free Supabase project. In the Supabase SQL Editor, execute both files in `supabase/migrations/` in filename order, once each. The migrations create the two tables, indexes, timestamp triggers, search/statistics/matching candidate functions, database authorization policies, and private `report-images` bucket. For local Supabase with Docker and the Supabase CLI, run `supabase start` and `supabase db reset` inside this directory. Resetting a local database removes its existing data.

In Supabase Authentication, enable anonymous sign-ins. For public testing, create a Cloudflare Turnstile widget for the Vercel production hostname and any approved development hostnames. Enable Turnstile CAPTCHA under Supabase Auth's protection settings and put the widget's **secret** key there. Set the Supabase Site URL to the deployed application's HTTPS URL. Keep Auth rate limits enabled. Many participants sharing one campus Wi-Fi address may reach sign-up rate limits; test the intended classroom setup in advance and stagger first-time guest access if necessary.

Copy `.env.example` to `.env.local` and set:

```dotenv
NEXT_PUBLIC_SUPABASE_URL=https://YOUR_PROJECT.supabase.co
NEXT_PUBLIC_SUPABASE_PUBLISHABLE_KEY=YOUR_PUBLISHABLE_KEY
NEXT_PUBLIC_TURNSTILE_SITE_KEY=YOUR_PUBLIC_SITE_KEY
```

Find the project URL and publishable key in Supabase's project/API settings. Public variables belong in the browser. The Turnstile secret and Supabase service-role/secret keys stay out of this file and out of the frontend. The Git ignore rules exclude local environment files. Production guest creation requires the Turnstile site key. Local development may use a local Supabase instance with CAPTCHA disabled. A CAPTCHA-enabled remote project requires the matching site key during development too.

## Run and verify

```sh
npm run dev
# Open http://127.0.0.1:3000
npm run lint
npm run typecheck
npm test
npm run build
npm start
```

The UI gives setup feedback if configuration is missing. To populate fictional item reports, run `supabase/seed.sql` in the SQL Editor after the migration. Sixteen fixed UUIDs make it safe to rerun. Seeded records display a Demo badge and contribute to database statistics. The seed contains no participant feedback.

Run `tests/database/authorization.sql` in the Supabase SQL Editor to check ownership, shared reads, demo protection, storage paths, consent, rating constraints, and feedback privacy. It rolls back all temporary data. Alternatively:

```sh
# Set FINDIT_TEST_DATABASE_URL securely in your shell to an authorized
# PostgreSQL connection string, then run:
npm run test:database
```

The test database URL is a secret used only by the optional Node test runner. It is never a `NEXT_PUBLIC_` variable. Browser test setup and actual participant procedures are in `docs/USABILITY_TESTING_GUIDE.md`.

## Implemented behavior

- Shared lost/found reporting with validated fields and photographs.
- Database-derived home statistics, recent reports, search, filters, and sorting.
- Item details with explainable rule-based Possible Matches.
- Guest-owned editing, deletion, resolution, and reopening under RLS.
- Anonymous feedback with seven ratings, explicit consent, and optional comments.
- Private image storage, expiring signed links, upload progress, client-side image re-encoding, and cleanup after successful replacement/deletion.
- Accessible navigation, labels, error feedback, keyboard-friendly deletion confirmation, and responsive layouts.

The optional color factor is omitted when either report lacks color. Name and location factors use normalized keyword overlap. Date similarity decreases to zero at fourteen days. Up to three opposite-type active reports with at least 40% similarity are shown. Scores support comparison, and ownership is verified by the campus office.

Guest identity is tied to browser cookies. Clearing browser data or using another device can remove access to manage earlier reports. All authenticated guests can browse the shared feed. Successful report creation opens the saved details and computes current matching suggestions. Refresh or re-fetch the feed on another device to see new reports.

## Deploy to Vercel

Import this GitHub repository into Vercel. Set **Root Directory** to `findit`, framework to Next.js, and Node.js to 24.x. Use `npm ci` and `npm run build`. Add all three public environment variables to the intended deployment environments and redeploy after any value changes. Enable anonymous authentication and configure CAPTCHA against the actual deployment hostname before participant use. The application does not require a local backend server.

The detailed checklist is in `docs/DEPLOYMENT_GUIDE.md`. Verify new guest access, a lost report with a photograph, a second independent guest viewing that report, ownership rejection, report resolution/reopening, and consent-based feedback before calling the deployed application ready for the study. Keep preview deployments protected. Configure the production audience for your approved classroom participants.

## Researchers and study data

The documents under `docs/` contain the six UCD sections and blank spaces for actual research, personas, journeys, findings, and reflection. Record initial testing separately from showcase evaluation. Obtain feedback from at least five actual showcase evaluators after direct interaction. Automated checks are technical verification.

Use authorized researcher access in the Supabase dashboard to open `public.usability_feedback` and export the required columns as CSV. Ordinary application users cannot browse this table. Export only to approved researcher storage and restrict access to the project group. `testing_stage` distinguishes Initial and Showcase. `consent_given` records consent. `comments` preserves the three optional answers under their question labels.

After exporting and following your group's approved retention decision, remove demonstration rows with `delete from public.reports where is_demo = true;`. Identify development/test reports before deleting them. Remove image objects through Supabase Storage or its API; deleting storage metadata directly does not remove file bytes. Export and then delete study feedback only when the research team has decided to do so. Review unused Auth guest accounts and their reports before removal because report ownership has a cascading foreign key. Review owner-scoped storage folders for abandoned uploads from closed tabs or interrupted requests and remove unreferenced objects through Storage. Ordinary cancellation and replacement cleanup checks report references before removing uploads.

## Troubleshooting and limitations

- **Guest access fails** — enable anonymous sign-ins, allow browser cookies, check Auth rate limits, and verify CAPTCHA keys and allowed hostnames.
- **Database setup message** — apply the migration to the same project as the configured URL.
- **Upload rejected** — use JPEG/PNG/WebP up to 5 MB and confirm the bucket, MIME allowlist, and Storage policies. Very large pixel dimensions are rejected before canvas processing.
- **Edit conflict** — reload the report when another tab has changed it.
- **A photograph becomes unavailable** — refresh the report to obtain a fresh signed URL. Storage removal errors require researcher cleanup.
- **Counts are unexpected** — statistics include labeled demo records until researchers remove them.
- **Windows sandbox build fails launching a worker** — run the normal build in a terminal that permits Next.js worker processes and local worker connections.

The prototype targets controlled classroom usage. Browser sessions have no cross-device account recovery. Matching uses rules and keywords. There is no private messaging. Abandoned-upload cleanup after a browser closes is a researcher maintenance task. Human evaluation evidence must be supplied by the project group.
