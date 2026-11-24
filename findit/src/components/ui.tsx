import { clsx, type ClassValue } from "clsx";
import { twMerge } from "tailwind-merge";
import { AlertCircle, CheckCircle2, LoaderCircle } from "lucide-react";
import type { ButtonHTMLAttributes, ReactNode } from "react";
export function cn(...values: ClassValue[]) {
  return twMerge(clsx(values));
}
export function buttonClass(
  variant: "primary" | "outline" | "ghost" | "danger" = "primary",
) {
  return cn(
    "button",
    variant === "primary" && "button-primary",
    variant === "outline" && "button-outline",
    variant === "ghost" && "button-ghost",
    variant === "danger" && "button-danger",
  );
}
export function Button({
  variant = "primary",
  className,
  ...props
}: ButtonHTMLAttributes<HTMLButtonElement> & {
  variant?: "primary" | "outline" | "ghost" | "danger";
}) {
  return <button {...props} className={cn(buttonClass(variant), className)} />;
}
export function Alert({
  children,
  kind = "error",
}: {
  children: ReactNode;
  kind?: "error" | "success" | "info";
}) {
  return (
    <div
      className={cn("alert", `alert-${kind}`)}
      role={kind === "error" ? "alert" : "status"}
    >
      {kind === "success" ? (
        <CheckCircle2 size={20} aria-hidden />
      ) : (
        <AlertCircle size={20} aria-hidden />
      )}
      <div>{children}</div>
    </div>
  );
}
export function Loading({ label = "Loading reports…" }: { label?: string }) {
  return (
    <div className="loading-state" role="status">
      <LoaderCircle size={24} className="animate-spin" aria-hidden />
      <span>{label}</span>
    </div>
  );
}
export function PageHeading({
  title,
  description,
  children,
}: {
  title: string;
  description?: string;
  children?: ReactNode;
}) {
  return (
    <div className="page-heading">
      <div>
        <h1>{title}</h1>
        {description && <p className="text-muted">{description}</p>}
      </div>
      {children}
    </div>
  );
}
export function Field({
  label,
  id,
  error,
  hint,
  children,
  required,
}: {
  label: string;
  id: string;
  error?: string;
  hint?: string;
  children: ReactNode;
  required?: boolean;
}) {
  return (
    <div className="field">
      <label htmlFor={id}>
        {label}
        {required && (
          <span className="required-mark" aria-label="required">
            {" "}
            *
          </span>
        )}
      </label>
      {hint && (
        <p id={`${id}-hint`} className="field-hint">
          {hint}
        </p>
      )}
      {children}
      {error && (
        <p className="field-error" id={`${id}-error`}>
          {error}
        </p>
      )}
    </div>
  );
}
