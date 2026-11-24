import { describe, expect, it } from "vitest";
import { normalize } from "@/lib/format";
import { keywordSimilarity, rankMatches, scoreMatch } from "@/lib/matching";
import type { Report } from "@/lib/types";
const report: Report = {
  id: "11111111-1111-4111-8111-111111111111",
  user_id: null,
  report_type: "lost",
  item_name: "Blue Water Bottle",
  category: "Water Bottles",
  description: "A blue insulated bottle.",
  color: "Blue",
  location: "University Library",
  incident_date: "2026-11-20",
  image_path: null,
  additional_info: null,
  turned_over: false,
  status: "active",
  is_demo: true,
  created_at: "2026-11-20T00:00:00Z",
  updated_at: "2026-11-20T00:00:00Z",
};
const candidate = (change: Partial<Report> = {}): Report => ({
  ...report,
  id: "22222222-2222-4222-8222-222222222222",
  report_type: "found",
  ...change,
});
describe("rule-based matching", () => {
  it("normalizes Unicode, punctuation, case, and repeated spaces", () => {
    expect(normalize("  BLÚE—Bottle!  ")).toBe("blue bottle");
    expect(keywordSimilarity("the blue bottle", "Blue bottle")).toBe(1);
  });
  it("assigns 100 to available exact factors and explains them", () => {
    const match = scoreMatch(report, candidate())!;
    expect(match.score).toBe(100);
    expect(match.reasons).toHaveLength(5);
  });
  it("normalizes missing optional color instead of penalizing it", () => {
    expect(scoreMatch(report, candidate({ color: null }))?.score).toBe(100);
  });
  it("excludes resolved and same-type reports", () => {
    expect(scoreMatch(report, candidate({ status: "resolved" }))).toBeNull();
    expect(scoreMatch(report, candidate({ report_type: "lost" }))).toBeNull();
    expect(
      scoreMatch({ ...report, status: "resolved" }, candidate()),
    ).toBeNull();
  });
  it("uses actual date distance and accepts either temporal direction", () => {
    expect(
      scoreMatch(report, candidate({ incident_date: "2026-11-06" }))?.score,
    ).toBe(90);
    expect(
      scoreMatch(report, candidate({ incident_date: "2026-11-27" }))?.score,
    ).toBe(95);
  });
  it("does not treat empty optional colors as matching", () => {
    expect(
      scoreMatch({ ...report, color: "" }, candidate({ color: "" }))?.reasons,
    ).not.toContain("Matching color");
  });
  it("recognizes gray and grey", () => {
    expect(
      scoreMatch({ ...report, color: "Grey" }, candidate({ color: "gray" }))
        ?.score,
    ).toBe(100);
  });
  it("ranks deterministically, limits to three, and rejects weak suggestions", () => {
    const rows = [4, 3, 2, 1].map((n) =>
      candidate({ id: `00000000-0000-4000-8000-00000000000${n}` }),
    );
    expect(
      rankMatches(report, rows).map((item) => item.report.id.at(-1)),
    ).toEqual(["1", "2", "3"]);
    expect(
      rankMatches(report, [
        candidate({
          item_name: "Unrelated jacket",
          category: "Clothing",
          color: "Red",
          location: "Gymnasium",
          incident_date: "2025-01-01",
        }),
      ]),
    ).toEqual([]);
  });
});
