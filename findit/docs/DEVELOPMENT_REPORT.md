# Development report

FindIt is implemented as an independent Next.js application under `findit/`. The application includes all ten requested routes, guest-owned report management, direct image uploads, explainable matching, the usability questionnaire, migrations, optional demo seed, and the requested HCI documentation. See the README for structure, environment variables, database setup, and deployment steps.

## Verified during development

- Production build succeeded using Next.js's supported Webpack builder.
- Strict TypeScript checking and ESLint passed.
- Fourteen matching and validation unit tests passed.
- Five browser checks passed at 320, 375, 390, 430, and 1280 pixels. They cover public layouts, keyboard entry, horizontal overflow, custom unavailable-page rendering, and automated WCAG A/AA accessibility checks on the About page.
- Hosted PostgreSQL authorization assertions passed for shared authenticated reads, owner updates, rejection of non-owner changes, demo protection, storage path protection, feedback read denial, rating constraints, and required consent.
- The SQL test transaction rolled back. A subsequent check confirmed sixteen demo reports, zero feedback responses, and zero remaining test identities.
- Supabase's security advisor returned no findings. The production dependency audit returned zero known vulnerabilities.

The performance advisor reported informational unused-index notices on this newly created dataset. Review actual query usage after the study before changing indexes. [Supabase unused-index advisory](https://supabase.com/docs/guides/database/database-linter?lint=0005_unused_index)

## Live verification requirements

The connected Supabase tools do not expose Auth provider or CAPTCHA configuration. Anonymous sign-ins must be enabled in the project dashboard. Public testing also needs a real Cloudflare Turnstile site key in the application and its matching secret in Supabase. The browser report/upload/two-session workflow is implemented as an opt-in test and has not passed until those settings are configured and that test is executed.

The database and private storage configuration were applied to the dedicated FindIt project. A deployed URL and final production smoke-test outcome must be recorded after deployment. A successful hosting build alone does not establish guest authentication or two-device acceptance.

## Study and scope limitations

Guest sessions have no cross-device account recovery. Matching uses weighted rules and keywords. Abandoned uploads from closed tabs require researcher maintenance. No participant research, evaluation responses, usability scores, or showcase outcomes have been generated. The group must supply its actual midterm research, testing observations, and at least five real showcase evaluations.
