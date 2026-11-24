"use client";
import { useCallback } from "react";
import Link from "next/link";
import { useRouter, useSearchParams } from "next/navigation";
import { Search, SlidersHorizontal, SearchX, RefreshCw } from "lucide-react";
import { getFeed } from "@/app/actions";
import { CATEGORIES } from "@/lib/constants";
import { filterSchema } from "@/lib/validation";
import { GuestGate } from "./guest-provider";
import { useResource } from "./use-resource";
import { ReportCard } from "./report-card";
import { Alert, Button, Loading, PageHeading, buttonClass } from "./ui";
export function BrowseView() {
  return (
    <GuestGate>
      <BrowseContent />
    </GuestGate>
  );
}
function BrowseContent() {
  const router = useRouter();
  const params = useSearchParams();
  const query = params.toString();
  const parsed = filterSchema.safeParse(Object.fromEntries(params));
  const filters = parsed.success ? parsed.data : filterSchema.parse({});
  const loader = useCallback(
    () => getFeed(Object.fromEntries(new URLSearchParams(query))),
    [query],
  );
  const { data, error, loading, reload } = useResource(loader);
  function navigate(form: HTMLFormElement) {
    const entries = new FormData(form);
    const next = new URLSearchParams();
    for (const [key, value] of entries)
      if (value.toString()) next.set(key, value.toString());
    router.push(`/browse?${next}`);
  }
  function pageUrl(page: number) {
    const next = new URLSearchParams(query);
    next.set("page", String(page));
    return `/browse?${next}`;
  }
  const hasFilters = Boolean(query && query !== "page=1");
  return (
    <>
      <PageHeading
        title="Browse campus reports"
        description="A familiar item could be one search away."
      >
        <Button
          variant="outline"
          onClick={() => void reload()}
          disabled={loading}
        >
          <RefreshCw size={17} aria-hidden />
          Refresh
        </Button>
      </PageHeading>
      <form
        key={query}
        className="browse-controls card"
        onSubmit={(event) => {
          event.preventDefault();
          navigate(event.currentTarget);
        }}
      >
        <div className="search-bar">
          <Search size={20} aria-hidden />
          <label htmlFor="browse-search" className="sr-only">
            Search items, descriptions, categories, or locations
          </label>
          <input
            id="browse-search"
            name="q"
            type="search"
            maxLength={100}
            placeholder="Search items, descriptions, or locations"
            defaultValue={filters.q}
          />
          <Button type="submit">Search</Button>
        </div>
        <div className="filter-heading">
          <SlidersHorizontal size={17} aria-hidden />
          <span>Filter reports</span>
          {hasFilters && (
            <Link href="/browse" className="text-link">
              Clear filters
            </Link>
          )}
        </div>
        <div className="filter-grid">
          {[
            {
              name: "type",
              label: "Report type",
              value: filters.type,
              options: [
                ["all", "All Items"],
                ["lost", "Lost Items"],
                ["found", "Found Items"],
              ],
            },
            {
              name: "category",
              label: "Category",
              value: filters.category,
              options: [
                ["", "All categories"],
                ...CATEGORIES.map((value) => [value, value]),
              ],
            },
            {
              name: "location",
              label: "Location",
              value: filters.location,
              options: [
                ["", "All locations"],
                ...[
                  ...new Set([
                    ...(data?.locations ?? []),
                    ...(filters.location ? [filters.location] : []),
                  ]),
                ].map((value) => [value, value]),
              ],
            },
            {
              name: "status",
              label: "Status",
              value: filters.status,
              options: [
                ["active", "Active"],
                ["resolved", "Resolved"],
                ["all", "All statuses"],
              ],
            },
            {
              name: "sort",
              label: "Sort by",
              value: filters.sort,
              options: [
                ["recent", "Most recent"],
                ["oldest", "Oldest first"],
              ],
            },
          ].map((filter) => (
            <div key={filter.name} className="field">
              <label htmlFor={`filter-${filter.name}`}>{filter.label}</label>
              <select
                id={`filter-${filter.name}`}
                name={filter.name}
                defaultValue={filter.value}
                onChange={(event) => event.currentTarget.form?.requestSubmit()}
              >
                {filter.options.map(([value, label]) => (
                  <option key={value} value={value}>
                    {label}
                  </option>
                ))}
              </select>
            </div>
          ))}
        </div>
      </form>
      {error && (
        <Alert>
          {error}{" "}
          <Button variant="ghost" onClick={() => void reload()}>
            Try again
          </Button>
        </Alert>
      )}
      {loading ? (
        <Loading />
      ) : (
        data && (
          <>
            <p className="results-summary" role="status">
              {data.total} {data.total === 1 ? "report" : "reports"}
              {filters.q ? ` matching “${filters.q}”` : ""}
            </p>
            {data.reports.length ? (
              <div className="report-grid">
                {data.reports.map((report) => (
                  <ReportCard key={report.id} report={report} />
                ))}
              </div>
            ) : (
              <div className="empty-state card">
                <SearchX size={40} aria-hidden />
                <h2>No matching reports</h2>
                <p>
                  Try another search term, choose a different category, or clear
                  your filters.
                </p>
                <Link href="/browse" className={buttonClass("outline")}>
                  Clear filters
                </Link>
                <Link href="/report" className="text-link">
                  Report your item
                </Link>
              </div>
            )}
            {data.total > 12 && (
              <nav className="pagination" aria-label="Report pages">
                {filters.page > 1 ? (
                  <Link
                    href={pageUrl(filters.page - 1)}
                    className={buttonClass("outline")}
                  >
                    Previous
                  </Link>
                ) : (
                  <span />
                )}
                <span>
                  Page {filters.page} of {Math.ceil(data.total / 12)}
                </span>
                {filters.page * 12 < data.total ? (
                  <Link
                    href={pageUrl(filters.page + 1)}
                    className={buttonClass("outline")}
                  >
                    Next
                  </Link>
                ) : (
                  <span />
                )}
              </nav>
            )}
          </>
        )
      )}
    </>
  );
}
