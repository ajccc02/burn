import { z } from "zod";
import { CATEGORIES } from "./constants";
import { today } from "./format";
const optionalText = (max: number) =>
  z
    .string()
    .trim()
    .max(max)
    .transform((value) => value || null);
const validDate = (value: string) => {
  if (!/^\d{4}-\d{2}-\d{2}$/.test(value)) return false;
  const date = new Date(`${value}T00:00:00Z`);
  return (
    !Number.isNaN(date.valueOf()) &&
    date.toISOString().slice(0, 10) === value &&
    value >= "1900-01-01" &&
    value <= today()
  );
};
export const reportSchema = z
  .object({
    id: z.uuid(),
    report_type: z.enum(["lost", "found"]),
    item_name: z
      .string()
      .trim()
      .min(2, "Enter an item name with at least 2 characters.")
      .max(100)
      .regex(/[\p{L}\p{N}]/u, "Enter a descriptive item name."),
    category: z.enum(CATEGORIES, { error: "Choose a category." }),
    description: z
      .string()
      .trim()
      .min(10, "Describe the item in at least 10 characters.")
      .max(2000),
    color: optionalText(50),
    location: z.string().trim().min(2, "Enter a campus location.").max(100),
    incident_date: z
      .string()
      .refine(validDate, "Choose a valid date on or before today."),
    image_path: z.string().max(300).nullable(),
    additional_info: optionalText(1000),
    turned_over: z.boolean(),
  })
  .strict()
  .refine((value) => value.report_type === "found" || !value.turned_over, {
    message: "Only found items can be marked as turned over.",
    path: ["turned_over"],
  });
const rating = z.number().int().min(1, "Choose a rating.").max(5);
export const feedbackSchema = z
  .object({
    id: z.uuid(),
    participant_code: z
      .string()
      .trim()
      .regex(
        /^[A-Za-z0-9_-]{2,32}$/,
        "Use a 2–32 character participant code with letters, numbers, - or _.",
      ),
    testing_stage: z.enum(["Initial", "Showcase"]),
    consent_given: z.literal(true, {
      error: "Your consent is required to submit feedback.",
    }),
    ease_of_use: rating,
    navigation: rating,
    clarity: rating,
    system_feedback: rating,
    accessibility: rating,
    task_completion: rating,
    overall_satisfaction: rating,
    liked: z.string().trim().max(1000),
    difficulties: z.string().trim().max(1000),
    improvements: z.string().trim().max(1000),
  })
  .strict();
export const filterSchema = z.object({
  q: z.string().trim().max(100).default(""),
  type: z.enum(["all", "lost", "found"]).default("all"),
  category: z.union([z.literal(""), z.enum(CATEGORIES)]).default(""),
  location: z.string().max(100).default(""),
  status: z.enum(["all", "active", "resolved"]).default("active"),
  sort: z.enum(["recent", "oldest"]).default("recent"),
  page: z.coerce.number().int().min(1).max(10000).default(1),
});
export function fieldErrors(error: z.ZodError): Record<string, string> {
  return Object.fromEntries(
    error.issues.map((issue) => [
      String(issue.path[0] ?? "form"),
      issue.message,
    ]),
  );
}
export type ReportInput = z.input<typeof reportSchema>;
export type FeedbackInput = z.input<typeof feedbackSchema>;
