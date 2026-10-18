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
    await s.wait(1200);
    await s.zoom(".git-right", { scale: 1.35, at: { x: 0.5, y: 0.2 } });
    await s.wait(1500);
    await s.zoom(".git-right", {
      scale: 1.35,
      at: { x: 0.5, y: 0.8 },
      duration: 4200,
    });
    await s.wait(500);
    await s.zoom(".git-left", {
      scale: 1.7,
      at: { x: 0.5, y: 0.25 },
    });
    await s.swipe(".git-left .swipe-page.is-current", {
      at: { x: 0.5, y: 0.3 },
    });
    await s.wait(1600);
    await s.move({
      selector: ".gg-row",
      text: "the terminals page",
      exact: false,
    });
    await s.wait(1500);
    await s.zoom(".git-right", {
      scale: 1.3,
      at: { x: 0.5, y: 0.2 },
      wait: false,
    });
    await s.click({
      selector: ".merge-review-action",
      text: "Expand all",
      exact: false,
    });
    await s.wait(900);
    await s.move(".git-right", { at: { x: 0.85, y: 0.7 } });
    await s.scroll(".git-right", { by: 1500, duration: 8000 });
    await s.wait(1500);
    await s.zoomOut();
    await s.wait(600);
    await s.endCard();
  },
};
