"use client";
import { useState } from "react";
import Link from "next/link";
import * as Dialog from "@radix-ui/react-alert-dialog";
import {
  ClipboardList,
  Plus,
  RefreshCw,
  Pencil,
  Trash2,
  Check,
  RotateCcw,
  Search,
} from "lucide-react";
import { changeReportStatus, deleteReport, getMyReports } from "@/app/actions";
import { GuestGate } from "./guest-provider";
import { useResource } from "./use-resource";
import { ReportCard } from "./report-card";
import { Alert, Button, Loading, PageHeading, buttonClass } from "./ui";
export function MyReportsView() {
  return (
    <GuestGate>
      <MyReports />
    </GuestGate>
  );
}
function MyReports() {
  const { data, error, loading, reload } = useResource(getMyReports);
  const [pending, setPending] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [actionError, setActionError] = useState<string | null>(null);
  async function change(
    id: string,
    updatedAt: string,
    status: "active" | "resolved",
  ) {
    if (pending) return;
    setPending(id);
    setActionError(null);
    try {
      const result = await changeReportStatus(id, status, updatedAt);
      if (!result.ok) throw new Error(result.error);
      setNotice(
        status === "resolved"
          ? "Your report was marked as resolved."
          : "Your report is active again.",
      );
      await reload();
    } catch (issue) {
      setActionError(
        issue instanceof Error
          ? issue.message
          : "The report could not be updated.",
      );
    } finally {
      setPending(null);
    }
  }
  async function remove(id: string) {
    if (pending) return;
    setPending(id);
    setActionError(null);
    try {
      const result = await deleteReport(id);
      if (!result.ok) throw new Error(result.error);
      setNotice("Your report was deleted.");
      await reload();
    } catch (issue) {
      setActionError(
        issue instanceof Error
          ? issue.message
          : "The report could not be deleted.",
      );
    } finally {
      setPending(null);
    }
  }
  return (
    <>
      <PageHeading
        title="My Reports"
        description="Keep track of the items you've shared with campus."
      >
        <Link href="/report" className={buttonClass()}>
          <Plus size={18} aria-hidden />
          New report
        </Link>
      </PageHeading>
      <p className="session-note">
        These reports belong to your current guest session. Clearing browser
        data or switching devices may remove your access to manage them.
      </p>
      <div className="section-heading">
        <h2>
          {data?.length ?? 0} {(data?.length ?? 0) === 1 ? "report" : "reports"}
        </h2>
        <Button
          variant="outline"
          onClick={() => void reload()}
          disabled={loading}
        >
          <RefreshCw size={17} aria-hidden />
          Refresh
        </Button>
      </div>
      {notice && <Alert kind="success">{notice}</Alert>}
      {(error || actionError) && <Alert>{error || actionError}</Alert>}
      {loading ? (
        <Loading />
      ) : data?.length ? (
        <div className="report-grid management-grid">
          {data.map((report) => (
            <div className="management-card" key={report.id}>
              <ReportCard report={report} />
              <div className="management-actions">
                <Link
                  href={`/my-reports/${report.id}/edit`}
                  className={buttonClass("ghost")}
                >
                  <Pencil size={16} aria-hidden />
                  Edit
                </Link>
                <Button
                  variant="ghost"
                  disabled={Boolean(pending)}
                  onClick={() =>
                    void change(
                      report.id,
                      report.updated_at,
                      report.status === "active" ? "resolved" : "active",
                    )
                  }
                >
                  {report.status === "active" ? (
                    <Check size={16} aria-hidden />
                  ) : (
                    <RotateCcw size={16} aria-hidden />
                  )}
                  {pending === report.id
                    ? "Updating…"
                    : report.status === "active"
                      ? "Resolve"
                      : "Reopen"}
                </Button>
                <Dialog.Root>
                  <Dialog.Trigger asChild>
                    <Button
                      variant="ghost"
                      className="delete-link"
                      disabled={Boolean(pending)}
                      aria-label={`Delete ${report.item_name}`}
                    >
                      <Trash2 size={16} aria-hidden />
                      Delete
                    </Button>
                  </Dialog.Trigger>
                  <Dialog.Portal>
                    <Dialog.Overlay className="dialog-overlay" />
                    <Dialog.Content className="dialog-content">
                      <Dialog.Title>Delete this report?</Dialog.Title>
                      <Dialog.Description>
                        Your report for &ldquo;{report.item_name}&rdquo; and its
                        photograph will be removed. This action cannot be
                        undone.
                      </Dialog.Description>
                      <div className="dialog-actions">
                        <Dialog.Cancel asChild>
                          <Button variant="outline">Keep report</Button>
                        </Dialog.Cancel>
                        <Dialog.Action asChild>
                          <Button
                            variant="danger"
                            onClick={() => void remove(report.id)}
                          >
                            Delete report
                          </Button>
                        </Dialog.Action>
                      </div>
                    </Dialog.Content>
                  </Dialog.Portal>
                </Dialog.Root>
              </div>
              <Link
                href={`/items/${report.id}#possible-matches`}
                className="management-matches text-link"
              >
                <Search size={16} aria-hidden />
                View Possible Matches
              </Link>
            </div>
          ))}
        </div>
      ) : (
        <div className="empty-state card">
          <ClipboardList size={42} aria-hidden />
          <h2>Your reports will appear here</h2>
          <p>Report a lost item or help someone recover something you found.</p>
          <Link href="/report" className={buttonClass()}>
            Create your first report
          </Link>
        </div>
      )}
    </>
  );
}
