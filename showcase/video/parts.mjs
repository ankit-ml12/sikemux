const scope = (name) => ({
  text: name,
  selector: ".swipe-page.is-current .scope-track button",
  exact: false,
});
const card = (title) => ({
  selector: ".kanban-card",
  text: title,
  exact: false,
});
  await s.zoom(".browser-viewport", {
    scale: 1.75,
    at: { x: 0.5, y: 0.42 },
    wait: false,
  });
  await s.live({ agent: WREN, steps: 3, pace: pace * 0.5 });
  await s.zoom(
    { selector: ".chat-image", last: true },
    { scale: 1.6, duration: 900 },
  );
  await s.live({ agent: OTTER, steps: 1, pace: pace * 0.25 });
  await s.zoom(
    { selector: ".chat-tools", last: true },
    { scale: 1.8, at: { x: 0.42, y: 0.5 }, duration: 900 },
  );
  await s.zoom(
    { selector: ".agent-letter", last: true },
    { scale: 1.9, duration: 800 },
  );
  await s.live({ agent: MAPLE, steps: 2, pace: pace * 0.3 });
  await s.appear(card("Ledger import onto amounts"));
  await s.zoom(".kanban-columns", {
    scale: 1.45,
    at: { x: 0.5, y: 0.35 },
    duration: 700,
    wait: false,
  });
  await s.zoom(
    { stage: "img.notch" },
    { scale: 3.1, at: { x: 0.5, y: 0.2 }, duration: 1100, wait: false },
  );
  await s.live({ agent: OTTER, steps: 2, pace: 0.6 });
