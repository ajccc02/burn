import Link from "next/link";
import { SearchX } from "lucide-react";
import { buttonClass } from "@/components/ui";
export default function NotFound() {
  return (
    <section className="empty-state">
      <SearchX size={48} aria-hidden />
      <h1>We couldn&apos;t find that page</h1>
      <p>The link may have changed, or the report may have been removed.</p>
      <Link href="/browse" className={buttonClass()}>
        Browse campus reports
      </Link>
    </section>
  );
}
