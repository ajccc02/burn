"use client";
import { useCallback } from "react";
import Link from "next/link";
import { useSearchParams } from "next/navigation";
import {
  CalendarDays,
  MapPin,
  ShieldCheck,
  Pencil,
  Search,
} from "lucide-react";
import { getReport } from "@/app/actions";
import { formatDate } from "@/lib/format";
import { GuestGate, useGuest } from "./guest-provider";
import { useResource } from "./use-resource";
import { ReportBadges, ReportImage } from "./report-card";
import { Matches } from "./matches";
import { Alert, Button, Loading, buttonClass } from "./ui";
export function DetailsView({ id }: { id: string }) {
  return (
    <GuestGate>
      <Details id={id} />
    </GuestGate>
  );
}
function Details({ id }: { id: string }) {
  const loader = useCallback(() => getReport(id), [id]);
  const { data, error, loading, reload } = useResource(loader);
  const guest = useGuest();
  const params = useSearchParams();
  if (loading) return <Loading label="Loading item details…" />;
  if (error)
    return (
      <Alert>
        {error}
        <Button variant="ghost" onClick={() => void reload()}>
          Retry
        </Button>
      </Alert>
    );
  if (!data)
    return (
      <section className="empty-state card">
        <h1>This report is no longer available</h1>
        <p>It may have been deleted by its owner.</p>
        <Link href="/browse" className={buttonClass()}>
          Browse reports
        </Link>
      </section>
    );
  return (
    <>
      <Link href="/browse" className="text-link back-link">
        Back to browse
      </Link>
      {params.get("saved") === "1" && (
        <Alert kind="success">
          Your report was saved and is now part of the shared campus feed.
        </Alert>
      )}
      <article className="details-card card">
        <ReportImage report={data} large />
        <div className="details-body">
          <ReportBadges report={data} />
          <p className="category-label">{data.category}</p>
          <h1>{data.item_name}</h1>
          <p className="details-description">{data.description}</p>
          <dl className="details-grid">
            <div>
              <dt>
                <MapPin size={16} aria-hidden />
                {data.report_type === "lost"
                  ? "Last known location"
                  : "Location found"}
              </dt>
              <dd>{data.location}</dd>
            </div>
            <div>
              <dt>
                <CalendarDays size={16} aria-hidden />
                Date {data.report_type}
              </dt>
              <dd>{formatDate(data.incident_date)}</dd>
            </div>
            <div>
              <dt>Color</dt>
              <dd>{data.color || "Not specified"}</dd>
            </div>
            <div>
              <dt>Date posted</dt>
              <dd>{formatDate(data.created_at)}</dd>
            </div>
          </dl>
          {data.additional_info && (
            <div className="additional-info">
              <h2>Additional information</h2>
              <p>{data.additional_info}</p>
            </div>
          )}
          {data.report_type === "found" && (
            <aside className="recovery-note">
              <ShieldCheck size={23} aria-hidden />
              <div>
                <h2>
                  {data.turned_over
                    ? "Marked as turned over to a campus office"
                    : "Recover through a campus office"}
                </h2>
                <p>
                  {data.turned_over
                    ? "Contact the appropriate campus lost-and-found office and describe the item to verify ownership."
                    : "Arrange recovery through the appropriate campus lost-and-found office, where ownership can be verified."}
                </p>
              </div>
            </aside>
          )}
          <div className="detail-actions">
            <a href="#possible-matches" className={buttonClass()}>
              <Search size={18} aria-hidden />
              View Possible Matches
            </a>
            {data.user_id === guest.userId && !data.is_demo && (
              <Link
                href={`/my-reports/${id}/edit`}
                className={buttonClass("outline")}
              >
                <Pencil size={17} aria-hidden />
                Edit my report
              </Link>
            )}
          </div>
        </div>
      </article>
      <Matches id={id} />
    </>
  );
}
