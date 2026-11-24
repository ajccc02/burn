import { test, expect } from "@playwright/test";
import AxeBuilder from "@axe-core/playwright";
const live = process.env.FINDIT_E2E_LIVE === "1";

for (const width of [320, 375, 390, 430, 1280]) {
  test(`public layouts and keyboard navigation at ${width}px`, async ({
    page,
  }, testInfo) => {
    await page.setViewportSize({ width, height: 900 });
    await page.goto("/about");
    await expect(page.getByRole("heading", { level: 1 })).toHaveText(
      "A little help. A lot of relief.",
    );
    expect(
      await page.evaluate(
        () => document.documentElement.scrollWidth <= window.innerWidth,
      ),
    ).toBe(true);
    await page.keyboard.press("Tab");
    await expect(
      page.getByRole("link", { name: "Skip to main content" }),
    ).toBeFocused();
    const accessibility = await new AxeBuilder({ page })
      .withTags(["wcag2a", "wcag2aa", "wcag21aa"])
      .analyze();
    expect(accessibility.violations).toEqual([]);
    await page.goto("/report");
    await expect(
      page.getByRole("link", { name: "Report Lost Item", exact: true }),
    ).toBeVisible();
    if (width === 375 || width === 1280)
      await page.screenshot({
        path: testInfo.outputPath("findit-report-chooser.png"),
        fullPage: true,
      });
    expect(
      await page.evaluate(
        () => document.documentElement.scrollWidth <= window.innerWidth,
      ),
    ).toBe(true);
    await page.goto("/items/not-a-uuid");
    await expect(
      page.getByRole("heading", { name: "We couldn't find that page" }),
    ).toBeVisible();
  });
}

test("real guest reports, upload recovery, two-session visibility, and management", async ({
  browser,
}) => {
  test.skip(
    !live,
    "Requires configured Supabase anonymous access or a local Supabase instance.",
  );
  const owner = await browser.newContext();
  const other = await browser.newContext();
  const a = await owner.newPage();
  const b = await other.newPage();
  const name = `QA ${Date.now()} blue water bottle`;
  let id: string | undefined;
  try {
    await a.goto("/report/lost");
    await expect(
      a.getByRole("button", { name: "Submit report", exact: true }),
    ).toBeVisible();
    await a.getByRole("button", { name: "Submit report", exact: true }).click();
    await expect(a.getByRole("alert")).toContainText("highlighted fields");
    await a.getByLabel("Item name", { exact: false }).fill(name);
    await a
      .getByLabel("Category", { exact: false })
      .selectOption("Water Bottles");
    await a
      .getByLabel("Description", { exact: false })
      .fill(
        "Technical QA item photograph and report. Remove after verification.",
      );
    await a.getByLabel("Color", { exact: true }).fill("Blue");
    await a
      .getByLabel("Last known location", { exact: false })
      .fill("Campus Quadrangle");
    await a.getByLabel("Choose an item photograph").setInputFiles({
      name: "wrong.txt",
      mimeType: "text/plain",
      buffer: Buffer.from("not a photograph"),
    });
    await expect(a.getByRole("alert")).toContainText("JPEG, PNG, or WebP");
    await a.getByLabel("Choose an item photograph").setInputFiles({
      name: "sample.png",
      mimeType: "image/png",
      buffer: Buffer.from(
        "iVBORw0KGgoAAAANSUhEUgAAAAIAAAACCAIAAAD91JpzAAAAEElEQVR4nGNQTX4NRAwQCgAligXNFnxqAgAAAABJRU5ErkJggg==",
        "base64",
      ),
    });
    await expect(
      a.getByAltText("Preview of the item photograph"),
    ).toBeVisible();
    await a.route("**/storage/v1/object/report-images/**", (route) =>
      route.abort(),
    );
    await a.getByRole("button", { name: "Submit report", exact: true }).click();
    await expect(a.getByRole("alert")).toContainText("uploaded");
    await expect(a.getByLabel("Item name", { exact: false })).toHaveValue(name);
    await a.unroute("**/storage/v1/object/report-images/**");
    await a
      .getByRole("button", { name: "Submit report", exact: true })
      .dblclick();
    await expect(a).toHaveURL(/\/items\/[0-9a-f-]+\?saved=1/);
    id = new URL(a.url()).pathname.split("/").at(-1);
    await expect(a.getByRole("heading", { name, exact: true })).toBeVisible();
    await a.reload();
    await expect(a.getByAltText(`Photograph of ${name}`)).toBeVisible();
    expect(
      await a
        .getByAltText(`Photograph of ${name}`)
        .evaluate(
          (image: HTMLImageElement) => image.complete && image.naturalWidth > 0,
        ),
    ).toBe(true);
    await b.goto(`/browse?q=${encodeURIComponent(name)}`);
    await expect(b.getByRole("heading", { name, exact: true })).toBeVisible();
    await b.goto(`/my-reports/${id}/edit`);
    await expect(
      b.getByRole("heading", { name: "This report is unavailable to edit" }),
    ).toBeVisible();
    await a.goto(`/my-reports/${id}/edit`);
    await a.getByLabel("Item name", { exact: false }).fill(`${name} edited`);
    await a.getByRole("button", { name: "Save changes", exact: true }).click();
    await expect(
      a.getByRole("heading", { name: `${name} edited`, exact: true }),
    ).toBeVisible();
    await a.goto("/my-reports");
    await a.getByRole("button", { name: "Resolve", exact: true }).click();
    await expect(
      a.getByRole("button", { name: "Reopen", exact: true }),
    ).toBeVisible();
    await a.getByRole("button", { name: "Reopen", exact: true }).click();
    await expect(
      a.getByRole("button", { name: "Resolve", exact: true }),
    ).toBeVisible();
    await a
      .getByRole("button", { name: `Delete ${name} edited`, exact: true })
      .click();
    await a.getByRole("button", { name: "Keep report", exact: true }).click();
    await expect(
      a.getByRole("heading", { name: `${name} edited`, exact: true }),
    ).toBeVisible();
    await a
      .getByRole("button", { name: `Delete ${name} edited`, exact: true })
      .click();
    await a.getByRole("button", { name: "Delete report", exact: true }).click();
    await expect(
      a.getByRole("status").filter({ hasText: "Your report was deleted." }),
    ).toBeVisible();
    id = undefined;
    await a.goto("/feedback");
    await a
      .getByRole("button", { name: "Submit feedback", exact: true })
      .click();
    await expect(a.getByRole("alert")).toContainText("highlighted questions");
  } finally {
    if (id) {
      await a.goto("/my-reports");
      const remove = a.getByRole("button", { name: /^Delete QA / });
      if (await remove.count()) {
        await remove.click();
        await a
          .getByRole("button", { name: "Delete report", exact: true })
          .click();
      }
    }
    await owner.close();
    await other.close();
  }
});
