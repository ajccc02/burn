# User flow

```mermaid
flowchart TD
    A[Open FindIt] --> B{Guest session available?}
    B -->|Yes| C[Home]
    B -->|No| D[Complete CAPTCHA and create guest session]
    D --> C
    C --> E[Browse, search, and filter]
    E --> F[Item details]
    F --> G[Possible Matches]
    C --> H[Choose Lost or Found]
    H --> I[Complete report and optional photo]
    I --> J[Validate, upload, and save]
    J --> F
    C --> K[My Reports]
    K --> L[Edit, resolve, reopen, or confirm deletion]
    L --> K
    C --> M[Complete testing tasks]
    M --> N[Consent and submit feedback]
```

Routes are `/`, `/browse`, `/report`, `/report/lost`, `/report/found`, `/items/[id]`, `/my-reports`, `/my-reports/[id]/edit`, `/feedback`, and `/about`.

Errors keep participants in the current flow with a reason and recovery action. A deleted item has an unavailable-report state. A syntactically invalid UUID or unknown route returns the custom 404. Another guest's report can be viewed but cannot be edited or deleted. The About page remains available before authentication.

Insert the group's actual midterm journey map here — [research artifact and source]. Document points where observed participants departed from this intended flow.
