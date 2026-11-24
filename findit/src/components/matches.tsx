"use client";
import { useCallback } from "react";
import Link from "next/link";
import { Link2, MapPin, CalendarDays } from "lucide-react";
import { getMatches } from "@/app/actions";
import { formatDate } from "@/lib/format";
import { useResource } from "./use-resource";
import { ReportImage } from "./report-card";
import { Alert, Button, Loading } from "./ui";
export function Matches({ id }: { id: string }) {
  const loader = useCallback(() => getMatches(id), [id]);
  const { data, error, loading, reload } = useResource(loader);
  return (
    <section
      className="section-gap matches-section"
      id="possible-matches"
      tabIndex={-1}
    >
      <div className="section-heading">
        <div>
          <h2>
            <Link2 size={21} aria-hidden />
            Possible Matches
          </h2>
          <p className="text-muted">
            Rule-based suggestions from active reports of the opposite type.
          </p>
        </div>
      </div>
      <p className="match-disclaimer">
        The score estimates similarity using category, name, location, color,
        and date. Ownership must be verified through the campus lost-and-found
        office.
      </p>
      {loading ? (
        <Loading label="Checking possible matches…" />
      ) : error ? (
        <Alert>
          {error}
          <Button variant="ghost" onClick={() => void reload()}>
            Retry matches
          </Button>
        </Alert>
      ) : data?.length ? (
        <div className="match-grid">
          {data.map((match) => (
            <article className="match-card card" key={match.report.id}>
              <Link
                href={`/items/${match.report.id}`}
                className="match-photo"
                aria-label={`View possible match ${match.report.item_name}`}
              >
                <ReportImage report={match.report} />
              </Link>
              <div>
                <span className="match-score">{match.score}% similarity</span>
                <h3>
                  <Link href={`/items/${match.report.id}`}>
                    {match.report.item_name}
                  </Link>
                </h3>
                <p className="metadata">
                  <MapPin size={15} aria-hidden />
                  {match.report.location}
                </p>
                <p className="metadata">
                  <CalendarDays size={15} aria-hidden />
                  {formatDate(match.report.incident_date)}
                </p>
                <p className="match-reasons">{match.reasons.join(" · ")}</p>
                <Link href={`/items/${match.report.id}`} className="text-link">
                  View Details
                </Link>
              </div>
            </article>
          ))}
        </div>
      ) : (
        <div className="empty-state card">
          <h3>No close matches yet</h3>
          <p>
            Check back as new reports are added. Only active reports can have
            possible matches.
          </p>
          <Button variant="outline" onClick={() => void reload()}>
            Check again
          </Button>
        </div>
      )}
    </section>
  );
}
