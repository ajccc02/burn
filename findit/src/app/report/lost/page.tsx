import { NewReportForm } from "@/components/report-form";
export const metadata = { title: "Report a lost item" };
export default function Lost() {
  return <NewReportForm type="lost" />;
}
