import { notFound } from "next/navigation";
import { z } from "zod";
import { EditView } from "@/components/edit-view";
export const metadata = { title: "Edit report" };
export default async function Edit({
  params,
}: {
  params: Promise<{ id: string }>;
}) {
  const { id } = await params;
  if (!z.uuid().safeParse(id).success) notFound();
  return <EditView id={id} />;
}
