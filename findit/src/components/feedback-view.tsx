"use client";
import { useRef, useState } from "react";
import Link from "next/link";
import {
  CheckCircle2,
  LoaderCircle,
  MessageSquare,
  ShieldCheck,
} from "lucide-react";
import { submitFeedback } from "@/app/actions";
import { QUESTIONS, SCALE } from "@/lib/constants";
import { feedbackSchema, fieldErrors } from "@/lib/validation";
import { GuestGate } from "./guest-provider";
import { Alert, Button, Field, PageHeading, buttonClass } from "./ui";
export function FeedbackView() {
  return (
    <GuestGate>
      <Feedback />
    </GuestGate>
  );
}
function Feedback() {
  const [id] = useState(() => crypto.randomUUID());
  const [fields, setFields] = useState<Record<string, string>>({});
  const [error, setError] = useState<string | null>(null);
  const [pending, setPending] = useState(false);
  const [complete, setComplete] = useState(false);
  const locked = useRef(false);
  const form = useRef<HTMLFormElement>(null);
  async function submit(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (locked.current) return;
    setError(null);
    setFields({});
    const values = new FormData(event.currentTarget);
    const ratings = Object.fromEntries(
      QUESTIONS.map(([name]) => [
        name,
        values.has(name) ? Number(values.get(name)) : 0,
      ]),
    );
    const input = {
      id,
      participant_code: String(values.get("participant_code") ?? ""),
      testing_stage: String(values.get("testing_stage") ?? ""),
      consent_given: values.get("consent_given") === "on",
      ...ratings,
      liked: String(values.get("liked") ?? ""),
      difficulties: String(values.get("difficulties") ?? ""),
      improvements: String(values.get("improvements") ?? ""),
    };
    const parsed = feedbackSchema.safeParse(input);
    if (!parsed.success) {
      const errors = fieldErrors(parsed.error);
      setFields(errors);
      setError("Please complete the highlighted questions before submitting.");
      const first = form.current?.elements.namedItem(Object.keys(errors)[0]);
      if (first instanceof HTMLElement) first.focus();
      else document.getElementById(`${Object.keys(errors)[0]}-group`)?.focus();
      return;
    }
    locked.current = true;
    setPending(true);
    try {
      const result = await submitFeedback(input);
      if (!result.ok) {
        if (result.fields) setFields(result.fields);
        throw new Error(result.error);
      }
      setComplete(true);
    } catch (issue) {
      setError(
        issue instanceof Error
          ? issue.message
          : "Your feedback could not be saved. Please retry.",
      );
    } finally {
      locked.current = false;
      setPending(false);
    }
  }
  if (complete)
    return (
      <section className="empty-state card feedback-success" role="status">
        <CheckCircle2 size={48} aria-hidden />
        <h1>Thank you for helping us improve FindIt.</h1>
        <p>
          Your feedback was saved. The research team can review it through their
          protected Supabase dashboard.
        </p>
        <Link href="/" className={buttonClass()}>
          Back to Home
        </Link>
      </section>
    );
  return (
    <div className="form-container">
      <PageHeading
        title="Your experience matters"
        description="Help us make campus lost and found easier for everyone."
      />
      <aside className="privacy-reminder">
        <MessageSquare size={24} aria-hidden />
        <p>
          Try the assigned tasks before answering. Share your own experience;
          there are no right or wrong answers.
        </p>
      </aside>
      <form
        ref={form}
        onSubmit={(event) => void submit(event)}
        noValidate
        className="feedback-form card"
        aria-busy={pending}
      >
        <h2>About this evaluation</h2>
        <p className="text-muted">
          Use the anonymous code provided by your researcher. Please leave out
          your name and student number.
        </p>
        {error && <Alert>{error}</Alert>}
        <div className="two-column">
          <Field
            id="participant_code"
            label="Participant code"
            error={fields.participant_code}
            required
          >
            <input
              id="participant_code"
              name="participant_code"
              required
              maxLength={32}
              placeholder="Your assigned code"
              autoComplete="off"
              aria-invalid={Boolean(fields.participant_code)}
              aria-describedby={
                fields.participant_code ? "participant_code-error" : undefined
              }
              disabled={pending}
            />
          </Field>
          <Field
            id="testing_stage"
            label="Testing stage"
            error={fields.testing_stage}
            required
          >
            <select
              id="testing_stage"
              name="testing_stage"
              required
              defaultValue=""
              aria-invalid={Boolean(fields.testing_stage)}
              aria-describedby={
                fields.testing_stage ? "testing_stage-error" : undefined
              }
              disabled={pending}
            >
              <option value="">Choose the testing stage</option>
              <option value="Initial">Initial User Testing</option>
              <option value="Showcase">Classroom Showcase</option>
            </select>
          </Field>
        </div>
        <label className="checkbox-field">
          <input
            type="checkbox"
            name="consent_given"
            required
            disabled={pending}
            aria-invalid={Boolean(fields.consent_given)}
            aria-describedby="consent-description"
          />
          <span>
            I voluntarily consent to submit usability feedback.
            <small id="consent-description">
              My responses will be used for the CS212 HCI project. I can leave
              before submitting.
            </small>
          </span>
        </label>
        {fields.consent_given && (
          <p className="field-error">{fields.consent_given}</p>
        )}
        <div className="form-divider" />
        <h2>How was your experience?</h2>
        <p className="field-hint" id="rating-instructions">
          Choose one answer for each statement. 1 = Strongly disagree, 2 =
          Disagree, 3 = Neutral, 4 = Agree, 5 = Strongly agree.
        </p>
        {QUESTIONS.map(([name, statement]) => (
          <fieldset
            key={name}
            id={`${name}-group`}
            className="rating-question"
            tabIndex={-1}
            aria-describedby={
              fields[name] ? `${name}-error` : "rating-instructions"
            }
            disabled={pending}
          >
            <legend>
              {statement}
              <span className="required-mark" aria-label="required">
                {" "}
                *
              </span>
            </legend>
            <div className="rating-options">
              {SCALE.map((label, index) => (
                <label key={label}>
                  <input
                    type="radio"
                    name={name}
                    value={index + 1}
                    required
                    aria-label={`${index + 1} — ${label}`}
                  />
                  <span aria-hidden>{index + 1}</span>
                </label>
              ))}
            </div>
            <div className="rating-endpoints" aria-hidden>
              <span>Strongly disagree</span>
              <span>Strongly agree</span>
            </div>
            {fields[name] && (
              <p className="field-error" id={`${name}-error`}>
                {fields[name]}
              </p>
            )}
          </fieldset>
        ))}
        <div className="form-divider" />
        <h2>Anything else you&apos;d like to share?</h2>
        <p className="field-hint">
          These questions are optional. Please avoid including personal
          information.
        </p>
        {[
          ["liked", "What did you like about the application?"],
          ["difficulties", "What difficulties did you experience?"],
          ["improvements", "What improvements would you suggest?"],
        ].map(([name, label]) => (
          <Field key={name} id={name} label={label} error={fields[name]}>
            <textarea
              id={name}
              name={name}
              rows={3}
              maxLength={1000}
              disabled={pending}
              aria-invalid={Boolean(fields[name])}
              aria-describedby={fields[name] ? `${name}-error` : undefined}
            />
          </Field>
        ))}
        <p className="privacy-note">
          <ShieldCheck size={18} aria-hidden />
          Your responses are available to authorized project researchers. Other
          participants cannot browse them.
        </p>
        <div className="form-actions">
          <Link href="/" className={buttonClass("ghost")}>
            Cancel
          </Link>
          <Button type="submit" disabled={pending}>
            {pending && (
              <LoaderCircle size={18} className="animate-spin" aria-hidden />
            )}
            {pending ? "Saving feedback…" : "Submit feedback"}
          </Button>
        </div>
      </form>
    </div>
  );
}
