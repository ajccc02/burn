export function supabaseConfig() {
  const url = process.env.NEXT_PUBLIC_SUPABASE_URL?.trim();
  const key = process.env.NEXT_PUBLIC_SUPABASE_PUBLISHABLE_KEY?.trim();
  if (!url || !key)
    throw new Error(
      "FindIt needs its Supabase project URL and publishable key. Follow the setup instructions in the project README.",
    );
  const parsed = new URL(url);
  if (
    !["https:", "http:"].includes(parsed.protocol) ||
    key.startsWith("sb_secret_")
  )
    throw new Error(
      "The Supabase public configuration is invalid. Use the project's publishable key.",
    );
  if (key.split(".").length === 3) {
    try {
      const claims = JSON.parse(
        atob(key.split(".")[1].replace(/-/g, "+").replace(/_/g, "/")),
      );
      if (claims.role === "service_role")
        throw new Error(
          "A service-role key cannot be used in FindIt's public configuration.",
        );
    } catch (error) {
      if (error instanceof Error && error.message.includes("service-role"))
        throw error;
    }
  }
  return { url: parsed.origin, key };
}
export function hasSupabaseConfig(): boolean {
  return Boolean(
    process.env.NEXT_PUBLIC_SUPABASE_URL &&
    process.env.NEXT_PUBLIC_SUPABASE_PUBLISHABLE_KEY,
  );
}
