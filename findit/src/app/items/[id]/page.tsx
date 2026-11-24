import { notFound } from "next/navigation";
import { z } from "zod";
import { DetailsView } from "@/components/details-view";
export const metadata = { title: "Item details" };
export default async function Item({
  params,
}: {
  params: Promise<{ id: string }>;
}) {
  const { id } = await params;
  if (!z.uuid().safeParse(id).success) notFound();
  return <DetailsView id={id} />;
}
