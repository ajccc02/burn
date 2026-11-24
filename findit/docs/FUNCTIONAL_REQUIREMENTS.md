# Functional requirements

| Capability | Application behavior | Verification |
| --- | --- | --- |
| Guest access | Restores or creates an anonymous Supabase session with public-testing CAPTCHA | Fresh and returning independent sessions |
| Home | Search, reporting actions, category shortcuts, recent records, actual database counts | Compare with database records |
| Browse | Search four text fields, type/category/location/status filters, sorting and pagination | Combined filters and empty results |
| Report | Required item details, optional image/color/information, found-item turnover | Database persistence and upload failure recovery |
| Details | Item fields, dates, photo, recovery guidance, Possible Matches | Read saved report by UUID |
| Matching | Weighted, normalized rule-based top-three suggestions | Unit tests and related demo reports |
| My Reports | Edit, delete with confirmation, resolve, and reopen owned records | Two-identity RLS checks |
| Feedback | Code, stage, consent, seven ratings, optional comments | Database constraints and protected researcher export |
| Accessibility | Labels, focus, keyboard support, status text and announcements, reduced motion | Automated and manual checks |
| Deployment | Next.js build, Supabase services, Vercel HTTPS application | Deployed smoke test |

Normal reports have an owner. Demo reports have no participant owner and cannot be modified by guests. The feedback table includes the requested fields plus `consent_given`. All seven rating constraints accept integers from one to five. Application mutations use stable IDs to guard retries.

Photographs support JPEG, PNG, and WebP up to 5 MB. Accepted files are decoded and re-encoded for upload. Images with more than forty million pixels are rejected to limit browser memory usage. Unavailable images show a category placeholder after a failed image load.

Human user research, measured task performance, and showcase evaluation results remain to be collected by the group. Live acceptance checks must be recorded after they are executed against the configured deployment.
