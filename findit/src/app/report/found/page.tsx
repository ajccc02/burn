import { NewReportForm } from "@/components/report-form";
export const metadata = { title: "Report a found item" };
export default function Found() {
  return <NewReportForm type="found" />;
}
