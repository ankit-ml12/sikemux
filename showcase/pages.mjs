// Real public pages an agent's browser tab shows, snapshotted once and committed so captures stay the same offline.
export const PAGES = [
  {
    name: "monkeytype",
    url: "https://monkeytype.com/",
    width: 640,
    height: 900,
    prepare: async (page) => {
      await page.getByRole("button", { name: "reject non-essential" }).click();
      await page.locator("div.fixed.top-0 button").last().click();
      await page.focus("#wordsInput");
      await page.waitForTimeout(1200);
    },
  },
];
