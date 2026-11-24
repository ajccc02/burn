"use client";
import { createBrowserClient } from "@supabase/ssr";
import type { Database } from "../types";
import { supabaseConfig } from "./config";
let client: ReturnType<typeof createBrowserClient<Database>> | undefined;
export function browserSupabase() {
  const { url, key } = supabaseConfig();
  client ??= createBrowserClient<Database>(url, key);
  return client;
}
