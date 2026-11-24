import type { Metadata, Viewport } from "next";
import "@fontsource-variable/inter";
import "./globals.css";
import { GuestProvider } from "@/components/guest-provider";
import { AppShell } from "@/components/app-shell";
export const dynamic = "force-dynamic";
export const metadata: Metadata = {
  title: { default: "FindIt — Campus Lost & Found", template: "%s | FindIt" },
  description:
    "Lost something? Let's find it. Report, search, and reunite campus belongings.",
};
export const viewport: Viewport = {
  width: "device-width",
  initialScale: 1,
  themeColor: "#2563EB",
};
export default function RootLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  return (
    <html lang="en" data-scroll-behavior="smooth">
      <body>
        <GuestProvider>
          <AppShell>{children}</AppShell>
        </GuestProvider>
      </body>
    </html>
  );
}
