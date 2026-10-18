      const tabs = await import("/showcase/browserPage.ts");
      tabs.showBrowserPage("agent-hero", {
        url: "about:blank",
        title: "New tab",
        picture: "",
      });
    await s.zoom(composer, {
      scale: 1.22,
      at: { x: 0.5, y: 0.1 },
      duration: 3000,
      wait: false,
    });
    await askOtter(s, { perSecond: 26 });
    await s.zoom(".tab-rail", {
      scale: 2,
      at: { x: 0.5, y: 0.3 },
      duration: 1000,
      wait: false,
    });
    await s.zoom(".tab-rail", {
      scale: 1.8,
      at: { x: 0.5, y: 0.3 },
      duration: 900,
      wait: false,
    });
    await browserCheck(s, { pace: 0.6 });
    await s.wait(300);
    await s.click({
      selector: ".tab-rail .agent-row *",
      text: "Line up the sidebar labels",
      exact: false,
    });
    await messageAndBoard(s, { pace: 0.6, hold: 1000 });
    await approveFromNotch(s, { hold: 900 });
    await s.zoomOut({ duration: 1300, wait: false });
    await s.wait(1900);
