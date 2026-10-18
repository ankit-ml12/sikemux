export default {
  setup: async (s) => {
    await s.app(async () => {
      showcase.backend.historyHead = "6ae9dab1";
      const resources = await import("/src/state/resources.ts");
      resources.invalidate((kind) => kind.startsWith("git."));
    });
    await s.open("s-sikemux", "w-sikemux-git");
    await s.place({ x: 1250, y: 860 });
  },
  play: async (s) => {
    await s.wait(700);
    await s.zoom(".git-right", {
      scale: 1.35,
      at: { x: 0.5, y: 0.3 },
      duration: 1600,
      wait: false,
    });
    await s.scroll(".git-right", { by: 760, duration: 5200 });
    await s.wait(500);
    await s.zoom(".git-left", {
      scale: 1.7,
      at: { x: 0.5, y: 0.25 },
      duration: 1300,
      wait: false,
    });
    await s.swipe(".git-left .swipe-page.is-current");
    await s.wait(1600);
    await s.move({
      selector: ".gg-row",
      text: "the terminals page",
      exact: false,
    });
    await s.wait(700);
    await s.zoom(".git-right", {
      scale: 1.3,
      at: { x: 0.5, y: 0.2 },
      duration: 1300,
      wait: false,
    });
    await s.click({
      selector: ".merge-review-action",
      text: "Expand all",
      exact: false,
    });
    await s.wait(900);
    await s.move({ x: 1180, y: 760 });
    await s.scroll(".git-right", { by: 1500, duration: 8000 });
    await s.wait(900);
    await s.zoomOut({ duration: 1400 });
    await s.wait(700);
  },
};
