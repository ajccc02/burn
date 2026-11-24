"use client";
import { useCallback } from "react";
import Link from "next/link";
import { getReport } from "@/app/actions";
import { GuestGate, useGuest } from "./guest-provider";
import { ReportForm } from "./report-form";
import { useResource } from "./use-resource";
import { Alert, Button, Loading, buttonClass } from "./ui";
export function EditView({ id }: { id: string }) {
  return (
    <GuestGate>
      <Edit id={id} />
    </GuestGate>
  );
}
function Edit({ id }: { id: string }) {
  const loader = useCallback(() => getReport(id), [id]);
  const { data, error, loading, reload } = useResource(loader);
  const guest = useGuest();
  if (loading) return <Loading label="Loading your report…" />;
  if (error)
    return (
      <Alert>
        {error}
        <Button variant="ghost" onClick={() => void reload()}>
          Retry
        </Button>
      </Alert>
    );
  if (!data || data.user_id !== guest.userId || data.is_demo)
    return (
      <div className="empty-state card">
        <h1>This report is unavailable to edit</h1>
        <p>
          You can manage reports created in this browser&apos;s guest session.
        </p>
        <Link href="/my-reports" className={buttonClass()}>
          My Reports
        </Link>
      </div>
    );
  return (
    <ReportForm key={data.updated_at} type={data.report_type} initial={data} />
  );
}
