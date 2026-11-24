# Usability testing guide

## Preparation

Use a working deployment with anonymous authentication, CAPTCHA, migrations, and Storage policies verified. Assign anonymous participant codes through the research team. Obtain voluntary participation and explain what will be recorded. Use fictional belongings and item-only sample photos. Do not request student numbers, names, phone numbers, or private documents.

Conduct development verification before inviting participants. Use independent browser contexts for technical session-isolation checks. The rollback-only SQL assertions are technical fixtures and leave no evaluation responses.

## Initial User Testing

Record the actual date, facilitator, device/browser, prototype version, and participant code. Ask participants to think aloud. Start timing when each task is explained and stop when the participant finishes or the stopping rule is reached. Record assistance without treating an assisted completion as unassisted success. Agree on a consistent stopping rule before the study.

| Task | Participant instruction | Observable completion |
| --- | --- | --- |
| 1 | Find a reported lost item using the search bar. | Correct report is opened |
| 2 | Filter found items by category. | Feed displays the requested type and category |
| 3 | Submit a new lost item report with a sample photograph. | Saved report and persistent photo appear after refresh |
| 4 | Locate a possible matching found item. | Participant opens a relevant Possible Match and understands its uncertainty |
| 5 | Mark your own report as resolved. | Owned report displays Resolved |
| 6 | Submit the usability evaluation questionnaire. | Voluntary, complete submission receives confirmation |

Use OBSERVATION_SHEET.md for time, completion, errors, assistance, difficulties, and comments. Avoid coaching toward the control unless assistance becomes necessary, then record it.

## Classroom Showcase

Gather feedback from at least five actual evaluators who have directly used the prototype. Select Classroom Showcase on the feedback form. Record this stage separately from Initial User Testing, including the deployed version and any changes made after initial testing.

## Technical browser checklist

Run `npm run test:e2e` for public-page responsive, keyboard, and accessibility checks. On Windows the suite uses the installed Microsoft Edge browser. On Linux, install its Chromium runtime with `npx playwright install chromium` first. The live report-management test is opt-in. Against a dedicated development/local Supabase environment with anonymous access enabled, set `FINDIT_E2E_LIVE=1` in your shell and rerun `npm run test:e2e`. `FINDIT_E2E_BASE_URL` optionally selects an already-running application. The live test creates a clearly labeled technical report, exercises two isolated contexts, and deletes that report afterward. It validates an incomplete feedback form without storing a participant response. Public CAPTCHA must remain configured for actual classroom deployment.

- Context A creates a lost report with a photograph. Refresh it and verify the photograph still loads.
- Context B finds that report after refreshing the feed, while My Reports excludes A's report.
- Try B's direct edit URL and direct database mutation. The UI and RLS must reject ownership changes.
- A edits the record, replaces/removes the image, resolves it, reopens it, and confirms deletion.
- Check lost/found forms with missing fields, invalid dates, oversized or mislabeled image files, repeated clicks, failed uploads, and interrupted requests.
- Check literal punctuation searches, combined filters, empty results, and pagination.
- Use 320, 375, 390, and 430-pixel widths plus desktop. Check horizontal overflow, keyboard focus order, modal focus return, live announcements, and reduced motion.
- Submit actual voluntary feedback during participant testing and confirm its receipt in the protected dashboard. Ordinary guests must be unable to list responses.

## Analysis

Report actual counts and measurements. Explain the chosen success definition and treatment of assisted tasks. Keep observation notes and questionnaire responses linked by anonymous code and stage. Add actual findings, iterations, retest evidence, and limitations to the six-section UCD report. Do not replace missing evidence with generated scores or fictional participants.
