"use client";
import Link from "next/link";
import { usePathname } from "next/navigation";
import {
  Home,
  Search,
  Plus,
  ClipboardList,
  MessageSquare,
  MapPin,
  Info,
  ShieldCheck,
} from "lucide-react";
import type { ReactNode } from "react";
import { cn } from "./ui";
const navigation = [
  { href: "/", label: "Home", icon: Home },
  { href: "/browse", label: "Browse", icon: Search },
  { href: "/report", label: "Report", icon: Plus },
  { href: "/my-reports", label: "My Reports", icon: ClipboardList },
  { href: "/feedback", label: "Feedback", icon: MessageSquare },
];
export function AppShell({ children }: { children: ReactNode }) {
  const path = usePathname();
  const links = (mobile: boolean) =>
    navigation.map(({ href, label, icon: Icon }) => {
      const active = href === "/" ? path === "/" : path.startsWith(href);
      return (
        <Link
          key={href}
          href={href}
          aria-current={active ? "page" : undefined}
          className={cn(
            "nav-link",
            active && "nav-active",
            href === "/report" && "nav-report",
            mobile && "nav-mobile",
          )}
        >
          <span className={href === "/report" ? "report-nav-icon" : undefined}>
            <Icon size={21} aria-hidden />
          </span>
          <span>{label}</span>
        </Link>
      );
    });
  return (
    <>
      <a className="skip-link" href="#main-content">
        Skip to main content
      </a>
      <header className="site-header">
        <div className="header-inner">
          <Link href="/" className="brand" aria-label="FindIt home">
            <span className="brand-mark">
              <MapPin size={25} aria-hidden />
            </span>
            <span>
              Find<span className="brand-accent">It</span>
              <small>CAMPUS LOST &amp; FOUND</small>
            </span>
          </Link>
          <nav className="desktop-nav" aria-label="Main navigation">
            {links(false)}
          </nav>
          <Link href="/about" className="about-link" aria-label="About FindIt">
            <Info size={21} aria-hidden />
          </Link>
        </div>
      </header>
      <main id="main-content" className="main-container" tabIndex={-1}>
        {children}
      </main>
      <footer className="site-footer">
        <span>
          <ShieldCheck size={16} aria-hidden /> A shared space for a more
          helpful campus.
        </span>
        <Link href="/about">Help &amp; privacy</Link>
        <span>CS212 · HCI prototype</span>
      </footer>
      <nav className="bottom-nav" aria-label="Mobile navigation">
        {links(true)}
      </nav>
    </>
  );
}
