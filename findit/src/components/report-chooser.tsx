import Link from "next/link";
import { PackageCheck, SearchX, ShieldCheck } from "lucide-react";
import { PageHeading, buttonClass } from "./ui";
export function ReportChooser() {
  return (
    <>
      <PageHeading
        title="How can we help?"
        description="Choose the kind of item you'd like to report."
      />
      <div className="chooser-grid">
        <section className="chooser-card card">
          <span className="choice-icon lost-choice">
            <SearchX size={34} aria-hidden />
          </span>
          <h2>I lost something</h2>
          <p>
            Tell the campus community what you&apos;re missing and where you
            last saw it.
          </p>
          <Link href="/report/lost" className={buttonClass()}>
            Report Lost Item
          </Link>
        </section>
        <section className="chooser-card card">
          <span className="choice-icon found-choice">
            <PackageCheck size={34} aria-hidden />
          </span>
          <h2>I found something</h2>
          <p>
            Help someone find their belongings and share where the item was
            turned over.
          </p>
          <Link href="/report/found" className={buttonClass()}>
            Report Found Item
          </Link>
        </section>
      </div>
      <aside className="privacy-reminder">
        <ShieldCheck size={23} aria-hidden />
        <p>
          Keep reports about the item. Please leave out personal phone numbers,
          student numbers, and sensitive photographs.
        </p>
      </aside>
    </>
  );
}
