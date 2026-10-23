// Pages an agent's browser tab shows, real public ones and the demo's own app, snapshotted once and committed
// so captures stay the same offline.
export const PAGES = [
  {
    name: "vercel",
    url: "https://vercel.com/",
    width: 640,
    height: 900,
    prepare: (page) => page.waitForTimeout(2500),
  },
  ...["moodboard-cover", "moodboard-shop"].map((name) => ({
    name,
    url: new URL(`./pages/${name}.html`, import.meta.url).href,
    width: 640,
    height: 900,
  })),
];
