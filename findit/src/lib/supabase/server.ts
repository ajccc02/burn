import "server-only";
import { createServerClient } from "@supabase/ssr";
import { cookies } from "next/headers";
import type { Database } from "../types";
import { supabaseConfig } from "./config";
export async function serverSupabase() {
  const { url, key } = supabaseConfig();
  const jar = await cookies();
  return createServerClient<Database>(url, key, {
    cookies: {
      getAll: () => jar.getAll(),
      setAll: (values) => {
        values.forEach(({ name, value, options }) =>
          jar.set(name, value, options),
        );
      },
    },
  });
}
export async function authenticated() {
  const client = await serverSupabase();
  const { data, error } = await client.auth.getUser();
  if (error || !data.user)
    throw new Error(
      "Your guest session could not be verified. Refresh the page and allow cookies to continue.",
    );
  return { client, user: data.user };
}
