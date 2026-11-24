"use client";
import { Alert, Button } from "@/components/ui";
export default function PageError({
  reset,
}: {
  error: Error & { digest?: string };
  reset: () => void;
}) {
  return (
    <section className="empty-state">
      <h1>Something interrupted this page</h1>
      <Alert>
        We couldn&apos;t load this part of FindIt. Your saved reports are still
        available when the connection returns.
      </Alert>
      <Button onClick={reset}>Try again</Button>
    </section>
  );
}
