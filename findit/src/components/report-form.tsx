"use client";
import { useEffect, useRef, useState } from "react";
import { useRouter } from "next/navigation";
import Image from "next/image";
import { Camera, ImagePlus, LoaderCircle, ShieldCheck, X } from "lucide-react";
import { saveReport, discardUpload } from "@/app/actions";
import { CATEGORIES, LOCATIONS } from "@/lib/constants";
import { today } from "@/lib/format";
import { reportSchema, fieldErrors } from "@/lib/validation";
import { prepareImage, uploadImage } from "@/lib/images";
import type { DisplayReport, ReportType } from "@/lib/types";
import { GuestGate, useGuest } from "./guest-provider";
import { Alert, Button, Field, PageHeading } from "./ui";
export function NewReportForm({ type }: { type: ReportType }) {
  return (
    <GuestGate>
      <ReportForm type={type} />
    </GuestGate>
  );
}
export function ReportForm({
  type,
  initial,
}: {
  type: ReportType;
  initial?: DisplayReport;
}) {
  const guest = useGuest();
  const router = useRouter();
  const [id] = useState(() => initial?.id ?? crypto.randomUUID());
  const [fields, setFields] = useState<Record<string, string>>({});
  const [error, setError] = useState<string | null>(null);
  const [phase, setPhase] = useState<"idle" | "uploading" | "saving">("idle");
  const [processing, setProcessing] = useState(false);
  const [percent, setPercent] = useState(0);
  const [photo, setPhoto] = useState<File | null>(null);
  const [imagePath, setImagePath] = useState<string | null>(
    initial?.image_path ?? null,
  );
  const [preview, setPreview] = useState<string | null>(
    initial?.image_url ?? null,
  );
  const uploaded = useRef<string | null>(null);
  const objectUrl = useRef<string | null>(null);
  const lock = useRef(false);
  const form = useRef<HTMLFormElement>(null);
  const photoInput = useRef<HTMLInputElement>(null);
  useEffect(
    () => () => {
      if (objectUrl.current) URL.revokeObjectURL(objectUrl.current);
    },
    [],
  );
  const busy = phase !== "idle";
  function cleanupSelected() {
    if (objectUrl.current) {
      URL.revokeObjectURL(objectUrl.current);
      objectUrl.current = null;
    }
    if (uploaded.current) {
      void discardUpload(uploaded.current);
      uploaded.current = null;
    }
  }
  async function selectPhoto(file?: File) {
    if (!file) return;
    setProcessing(true);
    setError(null);
    try {
      const processed = await prepareImage(file);
      cleanupSelected();
      setPhoto(processed);
      const url = URL.createObjectURL(processed);
      objectUrl.current = url;
      setPreview(url);
    } catch (issue) {
      setError(
        issue instanceof Error
          ? issue.message
          : "This photograph could not be prepared.",
      );
    } finally {
      setProcessing(false);
      if (photoInput.current) photoInput.current.value = "";
    }
  }
  function removePhoto() {
    cleanupSelected();
    setPhoto(null);
    setImagePath(null);
    setPreview(null);
  }
  function showFields(errors: Record<string, string>) {
    setFields(errors);
    const first = form.current?.elements.namedItem(Object.keys(errors)[0]);
    if (first instanceof HTMLElement) first.focus();
  }
  async function submit(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (lock.current || processing) return;
    setError(null);
    setFields({});
    const values = new FormData(event.currentTarget);
    const input = {
      id,
      report_type: type,
      item_name: String(values.get("item_name") ?? ""),
      category: String(values.get("category") ?? ""),
      description: String(values.get("description") ?? ""),
      color: String(values.get("color") ?? ""),
      location: String(values.get("location") ?? ""),
      incident_date: String(values.get("incident_date") ?? ""),
      image_path: imagePath,
      additional_info: String(values.get("additional_info") ?? ""),
      turned_over: type === "found" && values.get("turned_over") === "on",
    };
    const parsed = reportSchema.safeParse(input);
    if (!parsed.success) {
      showFields(fieldErrors(parsed.error));
      setError("Check the highlighted fields. Your input has been kept.");
      return;
    }
    lock.current = true;
    try {
      let path = imagePath;
      if (photo) {
        setPhase("uploading");
        path =
          uploaded.current ??
          (await uploadImage(photo, guest.userId!, id, setPercent));
        uploaded.current = path;
      }
      setPhase("saving");
      const result = await saveReport(
        { ...input, image_path: path },
        initial?.updated_at,
      );
      if (!result.ok) {
        if (result.fields) showFields(result.fields);
        throw new Error(result.error);
      }
      uploaded.current = null;
      router.push(`/items/${result.data.id}?saved=1`);
      router.refresh();
    } catch (issue) {
      setError(
        issue instanceof Error
          ? issue.message
          : "Your report could not be saved. Check your connection and retry.",
      );
    } finally {
      lock.current = false;
      setPhase("idle");
    }
  }
  const inputProps = (name: string) => ({
    "aria-invalid": Boolean(fields[name]),
    "aria-describedby": fields[name] ? `${name}-error` : undefined,
  });
  return (
    <div className="form-container">
      <PageHeading
        title={
          initial
            ? "Edit your report"
            : type === "lost"
              ? "Report a lost item"
              : "Report a found item"
        }
        description={
          type === "lost"
            ? "A few details can help someone recognize what you’re missing."
            : "Help a campus belonging find its way back to its owner."
        }
      />
      <form
        ref={form}
        onSubmit={(event) => void submit(event)}
        noValidate
        className="report-form card"
        aria-busy={busy || processing}
      >
        <div className="form-section-heading">
          <Camera size={21} aria-hidden />
          <h2>
            Item photograph <span className="optional-label">Optional</span>
          </h2>
        </div>
        <div className="photo-upload">
          {preview ? (
            <div className="upload-preview">
              <Image
                src={preview}
                alt="Preview of the item photograph"
                fill
                unoptimized
                sizes="240px"
                className="object-cover"
              />
            </div>
          ) : (
            <div className="upload-placeholder">
              <ImagePlus size={33} aria-hidden />
              <p>A clear photograph helps people recognize the item.</p>
            </div>
          )}
          <div className="upload-actions">
            <label className="button button-outline photo-label">
              {processing ? (
                <LoaderCircle size={18} className="animate-spin" aria-hidden />
              ) : (
                <ImagePlus size={18} aria-hidden />
              )}
              {processing
                ? "Processing photograph…"
                : preview
                  ? "Replace photograph"
                  : "Choose photograph"}
              <input
                ref={photoInput}
                type="file"
                accept="image/jpeg,image/png,image/webp"
                disabled={busy || processing}
                onChange={(event) => void selectPhoto(event.target.files?.[0])}
                className="file-input"
                aria-label="Choose an item photograph"
              />
            </label>
            {preview && (
              <Button
                type="button"
                variant="ghost"
                onClick={removePhoto}
                disabled={busy || processing}
              >
                <X size={17} aria-hidden />
                Remove photograph
              </Button>
            )}
            <p className="field-hint">JPEG, PNG, or WebP. Up to 5 MB.</p>
          </div>
        </div>
        <p className="privacy-note">
          <ShieldCheck size={18} aria-hidden />
          Use an item-only photograph. Leave out faces, identification numbers,
          personal documents, and other sensitive details.
        </p>
        <div className="form-divider" />
        <h2>Item details</h2>
        <p className="field-hint">Fields marked * are required.</p>
        {error && <Alert>{error}</Alert>}
        <Field
          label="Item name"
          id="item_name"
          error={fields.item_name}
          required
        >
          <input
            id="item_name"
            name="item_name"
            required
            maxLength={100}
            placeholder="e.g. Blue insulated water bottle"
            defaultValue={initial?.item_name}
            {...inputProps("item_name")}
            disabled={busy}
          />
        </Field>
        <div className="two-column">
          <Field
            label="Category"
            id="category"
            error={fields.category}
            required
          >
            <select
              id="category"
              name="category"
              required
              defaultValue={initial?.category ?? ""}
              {...inputProps("category")}
              disabled={busy}
            >
              <option value="">Choose a category</option>
              {CATEGORIES.map((category) => (
                <option key={category}>{category}</option>
              ))}
            </select>
          </Field>
          <Field label="Color" id="color" error={fields.color}>
            <input
              id="color"
              name="color"
              maxLength={50}
              placeholder="e.g. Blue"
              defaultValue={initial?.color ?? ""}
              {...inputProps("color")}
              disabled={busy}
            />
          </Field>
        </div>
        <Field
          label="Description"
          id="description"
          error={fields.description}
          required
          hint="Describe the item's appearance and condition. Keep personal information out of the report."
        >
          <textarea
            id="description"
            name="description"
            required
            minLength={10}
            maxLength={2000}
            rows={4}
            placeholder="e.g. A blue bottle with a black lid and a small dent near the base."
            defaultValue={initial?.description}
            {...inputProps("description")}
            disabled={busy}
          />
        </Field>
        <div className="two-column">
          <Field
            label={type === "lost" ? "Last known location" : "Location found"}
            id="location"
            error={fields.location}
            required
          >
            <input
              id="location"
              name="location"
              required
              list="campus-locations"
              maxLength={100}
              placeholder="Choose or enter a campus location"
              defaultValue={initial?.location}
              {...inputProps("location")}
              disabled={busy}
            />
            <datalist id="campus-locations">
              {LOCATIONS.map((location) => (
                <option key={location} value={location} />
              ))}
            </datalist>
          </Field>
          <Field
            label={type === "lost" ? "Date lost" : "Date found"}
            id="incident_date"
            error={fields.incident_date}
            required
          >
            <input
              id="incident_date"
              name="incident_date"
              type="date"
              required
              min="1900-01-01"
              max={today()}
              defaultValue={initial?.incident_date ?? today()}
              {...inputProps("incident_date")}
              disabled={busy}
            />
          </Field>
        </div>
        <Field
          label={
            type === "lost"
              ? "Distinguishing characteristics"
              : "Additional information"
          }
          id="additional_info"
          error={fields.additional_info}
        >
          <textarea
            id="additional_info"
            name="additional_info"
            rows={3}
            maxLength={1000}
            placeholder="Any other useful item details?"
            defaultValue={initial?.additional_info ?? ""}
            {...inputProps("additional_info")}
            disabled={busy}
          />
        </Field>
        {type === "found" && (
          <label className="checkbox-field">
            <input
              type="checkbox"
              name="turned_over"
              defaultChecked={initial?.turned_over}
              disabled={busy}
            />
            <span>
              I turned this item over to a designated campus lost-and-found
              office.
              <small>
                You may name the office in Additional information. Ownership
                should be verified there.
              </small>
            </span>
          </label>
        )}
        {phase === "uploading" && (
          <div role="status" className="upload-progress">
            <label htmlFor="upload-progress">
              Uploading photograph · {percent}%
            </label>
            <progress id="upload-progress" max={100} value={percent} />
          </div>
        )}
        <div className="form-actions">
          <Button
            type="button"
            variant="ghost"
            disabled={busy || processing}
            onClick={() => {
              cleanupSelected();
              router.push(initial ? `/items/${id}` : "/report");
            }}
          >
            Cancel
          </Button>
          <Button type="submit" disabled={busy || processing}>
            {busy && (
              <LoaderCircle size={18} className="animate-spin" aria-hidden />
            )}
            {phase === "uploading"
              ? "Uploading photograph…"
              : phase === "saving"
                ? "Saving report…"
                : initial
                  ? "Save changes"
                  : "Submit report"}
          </Button>
        </div>
      </form>
    </div>
  );
}
