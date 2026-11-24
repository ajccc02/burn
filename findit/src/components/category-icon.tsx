import {
  Backpack,
  BookOpen,
  Glasses,
  KeyRound,
  Laptop,
  Package,
  Shirt,
  Wallet,
  ContactRound,
  BottleWine,
} from "lucide-react";
import type { Category } from "@/lib/types";
const icons = {
  Electronics: Laptop,
  "School Supplies": BookOpen,
  Bags: Backpack,
  "Identification Cards": ContactRound,
  Keys: KeyRound,
  Wallets: Wallet,
  Clothing: Shirt,
  Accessories: Glasses,
  "Water Bottles": BottleWine,
  Others: Package,
};
export function CategoryIcon({
  category,
  size = 24,
}: {
  category: Category;
  size?: number;
}) {
  const Icon = icons[category] ?? Package;
  return <Icon size={size} aria-hidden strokeWidth={1.6} />;
}
