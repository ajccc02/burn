"use client";
import { useState } from "react";
import Link from "next/link";
import Image from "next/image";
import { CalendarDays, MapPin } from "lucide-react";
import { formatDate } from "@/lib/format";
import type { DisplayReport } from "@/lib/types";
import { CategoryIcon } from "./category-icon";
export function ReportImage({
  report,
  large = false,
}: {
  report: DisplayReport;
  large?: boolean;
}) {
  return (
    <Photo
      key={report.image_url ?? "placeholder"}
      report={report}
      large={large}
    />
  );
}
function Photo({ report, large }: { report: DisplayReport; large: boolean }) {
  const [failed, setFailed] = useState(false);
  return (
    <div className={large ? "report-image report-image-large" : "report-image"}>
      {report.image_url && !failed ? (
        <Image
          src={report.image_url}
          alt={`Photograph of ${report.item_name}`}
          fill
          unoptimized
          sizes={
            large
              ? "(max-width: 768px) 100vw, 600px"
              : "(max-width: 640px) 100vw, 380px"
          }
          className="object-cover"
          onError={() => setFailed(true)}
        />
      ) : (
        <div className="category-art" data-category={report.category}>
          <CategoryIcon category={report.category} size={large ? 76 : 52} />
          <span>{report.category}</span>
        </div>
      )}
    </div>
  );
}
export function ReportBadges({ report }: { report: DisplayReport }) {
  return (
    <div className="badge-row">
      <span className={`badge badge-${report.report_type}`}>
        {report.report_type === "lost" ? "Lost" : "Found"}
      </span>
      <span
        className={`badge ${report.status === "resolved" ? "badge-neutral" : "badge-active"}`}
      >
        {report.status === "resolved" ? "Resolved" : "Active"}
      </span>
      {report.is_demo && <span className="badge badge-demo">Demo</span>}
    </div>
  );
}
export function ReportCard({ report }: { report: DisplayReport }) {
  return (
    <article className="report-card card">
      <Link
        href={`/items/${report.id}`}
        className="report-image-link"
        aria-label={`View ${report.item_name}`}
      >
        <ReportImage report={report} />
      </Link>
      <div className="report-card-body">
        <ReportBadges report={report} />
        <p className="category-label">{report.category}</p>
        <h3>
          <Link href={`/items/${report.id}`}>{report.item_name}</Link>
        </h3>
        <p className="metadata">
          <MapPin size={16} aria-hidden />
          {report.location}
        </p>
        <p className="metadata">
          <CalendarDays size={16} aria-hidden />
          {formatDate(report.incident_date)}
        </p>
        <Link href={`/items/${report.id}`} className="card-details-link">
          View Details
        </Link>
      </div>
    </article>
  );
}
