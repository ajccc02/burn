# FindIt project overview

## 1. User Research and Problem Discovery

FindIt addresses the difficulty of locating campus belongings when lost-item information is scattered across informal channels. Intended users are students, faculty, and campus personnel.

Complete with actual midterm evidence:

- Research dates and methods — [group input]
- Participants and recruitment — [group input using anonymous codes]
- Observed problems and supporting evidence — [group input]
- Personas and user journeys — [group input]

The statements above describe the project's intended problem area. They do not claim research has been conducted.

## 2. Design and Requirements

Core tasks are searching, filtering, reporting, reviewing Possible Matches, managing owned reports, and submitting usability feedback. See FUNCTIONAL_REQUIREMENTS.md and USER_FLOW.md. The interface uses familiar campus language, mobile navigation, readable type, labeled statuses, and explicit recovery instructions.

## 3. Prototype Development

The application uses Next.js and Supabase PostgreSQL/Auth/Storage. Database records are shared across devices. Anonymous identities own reports under RLS. Supabase Storage retains photographs in a private bucket. Server actions validate inputs and authenticate each request.

## 4. User Testing and Evaluation

Use the six tasks in USABILITY_TESTING_GUIDE.md. Record observations in OBSERVATION_SHEET.md and collect voluntary questionnaire responses through the application. Keep Initial User Testing and Classroom Showcase separate.

## 5. Findings and Iteration

- Observed usability issue — [actual observation]
- Evidence and frequency — [actual data]
- Design change and reason — [group input]
- Retest outcome — [actual retest]

Record technical development changes separately in ITERATION_LOG.md.

## 6. Final Evaluation and Reflection

Collect showcase feedback from at least five actual evaluators following direct interaction. Summarize actual task outcomes, errors, assistance, and questionnaire responses. Add limitations, ethical/privacy reflections, and future improvements after analysis.
