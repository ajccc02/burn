"use server";
import { z } from "zod";
import { authenticated } from "@/lib/supabase/server";
import {
  feedbackSchema,
  fieldErrors,
  filterSchema,
  reportSchema,
} from "@/lib/validation";
import { IMAGE_BUCKET } from "@/lib/constants";
import { rankMatches } from "@/lib/matching";
import type {
  Counts,
  DisplayReport,
  Feed,
  MatchSuggestion,
  Report,
  Result,
} from "@/lib/types";

async function attempt<T>(operation: () => Promise<T>): Promise<Result<T>> {
  try {
    return { ok: true, data: await operation() };
  } catch (error) {
    if (error instanceof z.ZodError)
      return {
        ok: false,
        error: "Check the highlighted fields and try again.",
        fields: fieldErrors(error),
      };
    return {
      ok: false,
      error:
        error instanceof Error
          ? error.message
          : "We couldn't complete that request. Please try again.",
    };
  }
}
function dbError(error: { code?: string; message: string } | null): void {
  if (!error) return;
  if (error.code === "42501")
    throw new Error(
      "Your session does not have permission for this action. Refresh and try again.",
    );
  if (["PGRST205", "PGRST202", "42P01"].includes(error.code ?? ""))
    throw new Error(
      "The FindIt database is not set up yet. Apply the supplied Supabase migration.",
    );
  if (error.code === "23514")
    throw new Error(
      "The report contains invalid data or an unavailable photograph. Check the form and upload the photograph again.",
    );
  throw new Error(
    "The database request failed. Check your connection and try again.",
  );
}
type Client = Awaited<ReturnType<typeof authenticated>>["client"];
async function images(
  client: Client,
  reports: Report[],
): Promise<DisplayReport[]> {
  const paths = [
    ...new Set(
      reports.flatMap((report) =>
        report.image_path ? [report.image_path] : [],
      ),
    ),
  ];
  const signed = paths.length
    ? await client.storage.from(IMAGE_BUCKET).createSignedUrls(paths, 600)
    : null;
  const urls = new Map(
    signed?.data?.map((item) => [item.path, item.signedUrl]) ?? [],
  );
  return reports.map((report) => ({
    ...report,
    image_url: report.image_path ? (urls.get(report.image_path) ?? null) : null,
  }));
}
export async function sessionIdentity(): Promise<Result<string>> {
  return attempt(async () => (await authenticated()).user.id);
}
export async function getFeed(input: unknown): Promise<Result<Feed>> {
  return attempt(async () => {
    const filters = filterSchema.parse(input);
    const { client } = await authenticated();
    let query = client.rpc(
      "search_reports",
      { search_term: filters.q },
      { count: "exact" },
    );
    if (filters.type !== "all") query = query.eq("report_type", filters.type);
    if (filters.category) query = query.eq("category", filters.category);
    if (filters.location) query = query.eq("location", filters.location);
    if (filters.status !== "all") query = query.eq("status", filters.status);
    const start = (filters.page - 1) * 12;
    const [feed, locations] = await Promise.all([
      query
        .order("created_at", { ascending: filters.sort === "oldest" })
        .order("id")
        .range(start, start + 11),
      client.rpc("report_locations"),
    ]);
    dbError(feed.error);
    dbError(locations.error);
    return {
      reports: await images(client, feed.data ?? []),
      total: feed.count ?? 0,
      locations: locations.data ?? [],
    };
  });
}
export async function getHome(): Promise<
  Result<{ counts: Counts; reports: DisplayReport[] }>
> {
  return attempt(async () => {
    const { client } = await authenticated();
    const [stats, reports] = await Promise.all([
      client.rpc("report_counts"),
      client
        .from("reports")
        .select("*")
        .eq("status", "active")
        .order("created_at", { ascending: false })
        .order("id")
        .limit(6),
    ]);
    dbError(stats.error);
    dbError(reports.error);
    return {
      counts: stats.data ?? { lost: 0, found: 0, resolved: 0, demo: 0 },
      reports: await images(client, reports.data ?? []),
    };
  });
}
export async function getReport(
  id: string,
): Promise<Result<DisplayReport | null>> {
  return attempt(async () => {
    z.uuid().parse(id);
    const { client } = await authenticated();
    const result = await client
      .from("reports")
      .select("*")
      .eq("id", id)
      .maybeSingle();
    dbError(result.error);
    return result.data ? (await images(client, [result.data]))[0] : null;
  });
}
export async function getMyReports(): Promise<Result<DisplayReport[]>> {
  return attempt(async () => {
    const { client, user } = await authenticated();
    const result = await client
      .from("reports")
      .select("*")
      .eq("user_id", user.id)
      .order("created_at", { ascending: false })
      .order("id");
    dbError(result.error);
    return images(client, result.data ?? []);
  });
}
export async function getMatches(
  id: string,
): Promise<Result<MatchSuggestion[]>> {
  return attempt(async () => {
    z.uuid().parse(id);
    const { client } = await authenticated();
    const source = await client
      .from("reports")
      .select("*")
      .eq("id", id)
      .maybeSingle();
    dbError(source.error);
    if (!source.data) throw new Error("This report is no longer available.");
    const candidates: Report[] = [];
    for (let offset = 0; ; offset += 500) {
      const batch = await client
        .rpc("match_candidates", { source_id: id })
        .order("id")
        .range(offset, offset + 499);
      dbError(batch.error);
      candidates.push(...(batch.data ?? []));
      if ((batch.data?.length ?? 0) < 500) break;
    }
    const ranked = rankMatches(source.data, candidates);
    const displays = await images(
      client,
      ranked.map((match) => match.report),
    );
    return ranked.map((match, index) => ({
      ...match,
      score: Math.round(match.score),
      report: displays[index],
    }));
  });
}
function ownedPath(path: string | null, userId: string, reportId: string) {
  if (
    path &&
    !new RegExp(`^${userId}/${reportId}/[0-9a-f-]{36}\\.(webp|jpg|png)$`).test(
      path,
    )
  )
    throw new Error(
      "The photograph is not associated with this guest session and report.",
    );
}
async function cleanImage(client: Client, path: string | null) {
  if (!path) return;
  const { error } = await client.storage.from(IMAGE_BUCKET).remove([path]);
  if (error) console.warn("FindIt image cleanup needs a retry.");
}
export async function saveReport(
  input: unknown,
  expectedUpdatedAt?: string,
): Promise<Result<{ id: string }>> {
  return attempt(async () => {
    const fields = reportSchema.parse(input);
    const { client, user } = await authenticated();
    ownedPath(fields.image_path, user.id, fields.id);
    const existing = await client
      .from("reports")
      .select("*")
      .eq("id", fields.id)
      .maybeSingle();
    dbError(existing.error);
    if (existing.data) {
      if (existing.data.user_id !== user.id || existing.data.is_demo)
        throw new Error("You can only change your own reports.");
      if (!expectedUpdatedAt) {
        const identical = Object.entries(fields).every(
          ([key, value]) => existing.data![key as keyof Report] === value,
        );
        if (identical) return { id: fields.id };
        throw new Error(
          "A report was already saved from this form. Open My Reports to edit it.",
        );
      }
      z.iso.datetime({ offset: true }).parse(expectedUpdatedAt);
      const changed = await client
        .from("reports")
        .update(fields)
        .eq("id", fields.id)
        .eq("updated_at", expectedUpdatedAt)
        .select("id");
      dbError(changed.error);
      if (!changed.data?.length)
        throw new Error(
          "This report changed in another tab. Reload it before editing again.",
        );
      if (existing.data.image_path !== fields.image_path)
        await cleanImage(client, existing.data.image_path);
    } else {
      if (expectedUpdatedAt)
        throw new Error("This report was deleted. Return to My Reports.");
      const created = await client
        .from("reports")
        .insert({
          ...fields,
          user_id: user.id,
          is_demo: false,
          status: "active",
        });
      if (created.error?.code === "23505") {
        const retry = await client
          .from("reports")
          .select("*")
          .eq("id", fields.id)
          .maybeSingle();
        dbError(retry.error);
        if (
          retry.data?.user_id === user.id &&
          Object.entries(fields).every(
            ([key, value]) => retry.data![key as keyof Report] === value,
          )
        )
          return { id: fields.id };
      }
      dbError(created.error);
    }
    return { id: fields.id };
  });
}
export async function changeReportStatus(
  id: string,
  status: "active" | "resolved",
  updatedAt: string,
): Promise<Result<null>> {
  return attempt(async () => {
    z.uuid().parse(id);
    z.enum(["active", "resolved"]).parse(status);
    z.iso.datetime({ offset: true }).parse(updatedAt);
    const { client, user } = await authenticated();
    const result = await client
      .from("reports")
      .update({ status })
      .eq("id", id)
      .eq("user_id", user.id)
      .eq("is_demo", false)
      .eq("updated_at", updatedAt)
      .select("id");
    dbError(result.error);
    if (!result.data?.length)
      throw new Error(
        "This report changed or is no longer yours to manage. Refresh My Reports.",
      );
    return null;
  });
}
export async function deleteReport(id: string): Promise<Result<null>> {
  return attempt(async () => {
    z.uuid().parse(id);
    const { client, user } = await authenticated();
    const deleted = await client
      .from("reports")
      .delete()
      .eq("id", id)
      .eq("user_id", user.id)
      .eq("is_demo", false)
      .select("id,image_path");
    dbError(deleted.error);
    if (!deleted.data?.length)
      throw new Error("This report is no longer available to delete.");
    await cleanImage(client, deleted.data[0].image_path);
    return null;
  });
}
export async function discardUpload(path: string): Promise<Result<null>> {
  return attempt(async () => {
    const { client, user } = await authenticated();
    if (path.length > 300 || !path.startsWith(`${user.id}/`))
      throw new Error("This photograph is not yours to remove.");
    const references = await client
      .from("reports")
      .select("id")
      .eq("image_path", path)
      .limit(1);
    dbError(references.error);
    if (!references.data?.length) await cleanImage(client, path);
    return null;
  });
}
export async function submitFeedback(input: unknown): Promise<Result<null>> {
  return attempt(async () => {
    const fields = feedbackSchema.parse(input);
    const { client } = await authenticated();
    const { liked, difficulties, improvements, ...ratings } = fields;
    const comments = [
      ["What did you like?", liked],
      ["Difficulties experienced", difficulties],
      ["Suggested improvements", improvements],
    ]
      .filter(([, answer]) => answer)
      .map(([label, answer]) => `${label}\n${answer}`)
      .join("\n\n");
    const result = await client
      .from("usability_feedback")
      .insert({ ...ratings, comments: comments || null });
    if (result.error?.code !== "23505") dbError(result.error);
    return null;
  });
}
