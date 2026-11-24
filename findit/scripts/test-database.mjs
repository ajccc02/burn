import { readFile } from "node:fs/promises";
import pg from "pg";
if (!process.env.FINDIT_TEST_DATABASE_URL) {
  console.error(
    "Set FINDIT_TEST_DATABASE_URL to an authorized PostgreSQL connection string, or run tests/database/authorization.sql in the Supabase SQL Editor. All fixture writes are rolled back.",
  );
  process.exit(1);
}
const client = new pg.Client({
  connectionString: process.env.FINDIT_TEST_DATABASE_URL,
});
try {
  await client.connect();
  await client.query(
    await readFile(
      new URL("../tests/database/authorization.sql", import.meta.url),
      "utf8",
    ),
  );
  console.log(
    "Database authorization assertions passed. All test writes were rolled back.",
  );
} catch (error) {
  await client.query("rollback").catch(() => {});
  console.error(
    "Database authorization assertions failed.",
    error instanceof Error ? error.message : "Unknown database error",
  );
  process.exitCode = 1;
} finally {
  await client.end();
}
