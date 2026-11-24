import Link from "next/link";
import {
  Search,
  PackageCheck,
  ShieldCheck,
  Fingerprint,
  ClipboardCheck,
} from "lucide-react";
import { PageHeading, buttonClass } from "@/components/ui";
export const metadata = { title: "About & privacy" };
export default function About() {
  return (
    <div className="about-container">
      <PageHeading
        title="A little help. A lot of relief."
        description="FindIt — Campus Lost & Found"
      />
      <section className="card about-intro">
        <p>
          FindIt helps university students, faculty, and campus personnel report
          missing belongings, share found items, and discover possible matches.
        </p>
        <p>
          This is a CS212 Human-Computer Interaction prototype for controlled
          user testing and a classroom showcase. Campus location suggestions and
          labeled demonstration reports use generic, fictional examples.
        </p>
      </section>
      <section className="section-gap">
        <h2>How FindIt works</h2>
        <div className="help-grid">
          {[
            {
              Icon: Search,
              title: "Look for your item",
              text: "Search the shared feed by name, description, category, or campus location. Filters can narrow your results.",
            },
            {
              Icon: PackageCheck,
              title: "Share a useful report",
              text: "Describe the item and when and where it was lost or found. You can add an item-only photograph.",
            },
            {
              Icon: ClipboardCheck,
              title: "Check, recover, and resolve",
              text: "Review Possible Matches, verify ownership at the appropriate campus office, and mark your report as resolved when finished.",
            },
          ].map(({ Icon, title, text }) => (
            <article className="card help-card" key={title}>
              <Icon size={26} aria-hidden />
              <h3>{title}</h3>
              <p>{text}</p>
            </article>
          ))}
        </div>
      </section>
      <section className="card about-section">
        <h2>
          <ShieldCheck size={24} aria-hidden />
          Recover belongings through campus
        </h2>
        <p>
          If you find an item, turn it over to the appropriate campus
          lost-and-found office. Add the office name to your report&apos;s
          additional information when useful.
        </p>
        <p>
          Possible Matches use category, keywords, color, location, and dates.
          Their similarity percentages help you compare reports. The campus
          office should verify item ownership before releasing belongings.
        </p>
        <p>
          FindIt does not provide private messaging or publish contact
          information. Keep handovers with the appropriate campus office.
        </p>
      </section>
      <section className="card about-section">
        <h2>
          <Fingerprint size={24} aria-hidden />
          Your guest session and privacy
        </h2>
        <p>
          You can use FindIt without a name, email address, or password.
          Supabase creates a guest identity to protect the reports you submit.
        </p>
        <p>
          Your browser preserves the session when cookies and browser storage
          are available. Clearing browser data, using a private window that
          closes, or changing devices may remove access to manage earlier
          reports. A different device can still browse the shared feed.
        </p>
        <p>
          Reports, their descriptions, and submitted photographs can be viewed
          by other authenticated guests. Please avoid faces, identification
          numbers, personal documents, phone numbers, home addresses, and other
          sensitive information.
        </p>
        <p>
          Photographs are stored in a private bucket and displayed through
          temporary signed links. Authorized project researchers can manage
          study data and export feedback. Feedback responses are protected from
          ordinary participant access.
        </p>
        <p>
          If an upload or submission fails, check the error message and retry.
          FindIt preserves form input while you remain on the page. For changes
          in another tab, reload the report before editing it.
        </p>
      </section>
      <section className="card about-section">
        <h2>Help improve the prototype</h2>
        <p>
          After completing your researcher&apos;s assigned tasks, submit
          feedback using the anonymous participant code they provide. Initial
          User Testing and Classroom Showcase are separate stages.
        </p>
        <p>
          The research team supplies actual findings and evaluation evidence.
          Demonstration item reports are labeled &ldquo;Demo&rdquo; and can be
          removed after the study.
        </p>
        <Link href="/feedback" className={buttonClass()}>
          Share usability feedback
        </Link>
      </section>
    </div>
  );
}
