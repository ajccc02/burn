"use client";
import Link from "next/link";
import {
  Search,
  SearchX,
  Plus,
  PackageCheck,
  ShieldCheck,
  RefreshCw,
} from "lucide-react";
import { getHome } from "@/app/actions";
import { CATEGORIES } from "@/lib/constants";
import { GuestGate } from "./guest-provider";
import { CategoryIcon } from "./category-icon";
import { ReportCard } from "./report-card";
import { useResource } from "./use-resource";
import { Alert, Button, Loading, buttonClass } from "./ui";
export function HomeView() {
  return (
    <GuestGate>
      <HomeContent />
    </GuestGate>
  );
}
function HomeContent() {
  const { data, error, loading, reload } = useResource(getHome);
  return (
    <>
      <section className="home-hero card">
        <div className="hero-copy">
          <span className="eyebrow">A LITTLE HELP GOES A LONG WAY</span>
          <h1>
            Lost something?
            <br />
            <span>Let&apos;s find it.</span>
          </h1>
          <p>
            From a missing notebook to a found set of keys, help campus
            belongings find their way home.
          </p>
          <form action="/browse" className="search-bar">
            <Search size={21} aria-hidden />
            <label className="sr-only" htmlFor="home-search">
              Search campus reports
            </label>
            <input
              id="home-search"
              name="q"
              maxLength={100}
              placeholder="What are you looking for?"
              type="search"
            />
            <button type="submit" className="button button-primary">
              Search
            </button>
          </form>
          <div className="hero-actions">
            <Link href="/report/lost" className={buttonClass()}>
              <SearchX size={19} aria-hidden />
              Report Lost Item
            </Link>
            <Link href="/report/found" className={buttonClass("outline")}>
              <PackageCheck size={19} aria-hidden />
              Report Found Item
            </Link>
          </div>
        </div>
        <div className="hero-illustration" aria-hidden>
          <div className="illustration-orbit orbit-one" />
          <div className="illustration-orbit orbit-two" />
          <div className="illustration-center">
            <Search size={68} strokeWidth={1.5} />
          </div>
          <div className="floating-item item-keys">
            <CategoryIcon category="Keys" size={35} />
          </div>
          <div className="floating-item item-bag">
            <CategoryIcon category="Bags" size={40} />
          </div>
          <div className="floating-item item-book">
            <CategoryIcon category="School Supplies" size={32} />
          </div>
          <span className="illustration-dot dot-one" />
          <span className="illustration-dot dot-two" />
        </div>
      </section>
      {error && (
        <div className="section-gap">
          <Alert>{error}</Alert>
          <Button variant="outline" onClick={() => void reload()}>
            Retry loading reports
          </Button>
        </div>
      )}
      {loading ? (
        <Loading />
      ) : (
        data && (
          <section className="stats-grid" aria-label="Report statistics">
            <div className="stat-card">
              <span className="stat-icon stat-lost">
                <SearchX size={22} aria-hidden />
              </span>
              <div>
                <strong>{data.counts.lost}</strong>
                <span>Active lost reports</span>
              </div>
            </div>
            <div className="stat-card">
              <span className="stat-icon stat-found">
                <PackageCheck size={22} aria-hidden />
              </span>
              <div>
                <strong>{data.counts.found}</strong>
                <span>Active found reports</span>
              </div>
            </div>
            <div className="stat-card">
              <span className="stat-icon stat-resolved">
                <ShieldCheck size={22} aria-hidden />
              </span>
              <div>
                <strong>{data.counts.resolved}</strong>
                <span>Resolved reports</span>
              </div>
            </div>
          </section>
        )
      )}
      <section className="section-gap">
        <div className="section-heading">
          <h2>Browse by category</h2>
          <Link href="/browse" className="text-link">
            All categories
          </Link>
        </div>
        <div className="category-grid">
          {CATEGORIES.map((category) => (
            <Link
              href={`/browse?category=${encodeURIComponent(category)}`}
              key={category}
              className="category-shortcut"
            >
              <span>
                <CategoryIcon category={category} />
              </span>
              {category}
            </Link>
          ))}
        </div>
      </section>
      <section className="section-gap">
        <div className="section-heading">
          <div>
            <h2>Recently reported</h2>
            <p className="text-muted">
              Someone on campus might have the answer.
            </p>
          </div>
          <Button
            variant="ghost"
            onClick={() => void reload()}
            disabled={loading}
            aria-label="Refresh recent reports"
          >
            <RefreshCw size={18} aria-hidden />
          </Button>
        </div>
        {data && data.counts.demo > 0 && (
          <p className="demo-note">
            Demo records are labeled. Statistics include demonstration data.
          </p>
        )}
        <div className="report-grid">
          {data?.reports.map((report) => (
            <ReportCard key={report.id} report={report} />
          ))}
        </div>
        {data && !data.reports.length && (
          <div className="empty-state card">
            <Search size={32} aria-hidden />
            <h3>No active reports yet</h3>
            <p>Be the first to help an item find its way back.</p>
            <Link href="/report" className={buttonClass()}>
              <Plus size={18} aria-hidden />
              Create a report
            </Link>
          </div>
        )}
        <div className="center-action">
          <Link href="/browse" className={buttonClass("outline")}>
            Browse all reports
          </Link>
        </div>
      </section>
      <aside className="recovery-strip">
        <ShieldCheck size={27} aria-hidden />
        <div>
          <h2>Found something? Keep the handover on campus.</h2>
          <p>
            Turn it over to an appropriate campus office, where ownership can be
            verified.
          </p>
        </div>
        <Link href="/about" className="text-link">
          Recovery guidance
        </Link>
      </aside>
    </>
  );
}
