// Real public pages an agent's browser tab shows, snapshotted once and committed so captures stay the same offline.
export const PAGES = [
  {
    name: "vercel",
    url: "https://vercel.com/",
    width: 640,
    height: 900,
    prepare: (page) => page.waitForTimeout(2500),
  },
];
