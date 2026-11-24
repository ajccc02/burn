import { normalize } from "./format";
import type { Report } from "./types";
const STOP = new Set(["a", "an", "the", "lost", "found", "item"]);
function tokens(text: string): Set<string> {
  return new Set(
    normalize(text)
      .split(" ")
      .filter((word) => word && !STOP.has(word)),
  );
}
export function keywordSimilarity(a: string, b: string): number {
  const left = tokens(a),
    right = tokens(b);
  if (!left.size || !right.size) return 0;
  const intersection = [...left].filter((word) => right.has(word)).length;
  return intersection / new Set([...left, ...right]).size;
}
function colorValue(value: string): string {
  return normalize(value).replace(/\bgrey\b/g, "gray");
}
export function scoreMatch(
  source: Report,
  candidate: Report,
): { score: number; reasons: string[] } | null {
  if (
    source.id === candidate.id ||
    source.status !== "active" ||
    candidate.status !== "active" ||
    source.report_type === candidate.report_type
  )
    return null;
  const dateDistance =
    Math.abs(
      Date.parse(source.incident_date) - Date.parse(candidate.incident_date),
    ) / 86400000;
  const name = keywordSimilarity(source.item_name, candidate.item_name);
  const location = keywordSimilarity(source.location, candidate.location);
  const sameCategory = source.category === candidate.category;
  const hasColor = Boolean(source.color && candidate.color);
  const sameColor =
    hasColor && colorValue(source.color!) === colorValue(candidate.color!);
  const points =
    (sameCategory ? 30 : 0) +
    name * 30 +
    location * 15 +
    (sameColor ? 15 : 0) +
    Math.max(0, 1 - dateDistance / 14) * 10;
  const reasons = [
    sameCategory && "Same category",
    name > 0 && "Similar item name",
    location > 0 && "Similar campus location",
    sameColor && "Matching color",
    dateDistance <= 7 && "Nearby incident dates",
  ].filter((value): value is string => Boolean(value));
  return { score: (points / (hasColor ? 100 : 85)) * 100, reasons };
}
export function rankMatches(source: Report, candidates: Report[]) {
  return candidates
    .flatMap((report) => {
      const match = scoreMatch(source, report);
      return match && match.score >= 40 ? [{ report, ...match }] : [];
    })
    .sort(
      (a, b) =>
        b.score - a.score ||
        b.report.created_at.localeCompare(a.report.created_at) ||
        a.report.id.localeCompare(b.report.id),
    )
    .slice(0, 3);
}
