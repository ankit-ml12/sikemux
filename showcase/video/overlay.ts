import "./overlay.css";

/** Where a beat points: a selector in the app, an element of the app holding some text, a part of the framed screen, or a rect on it. */
export type Target =
  | string
  | { text: string | RegExp; selector?: string; exact?: boolean }
  | { stage: string }
  | Rect;

export interface Rect {
  x: number;
  y: number;
  width: number;
  height: number;
}

export interface Frame {
  camera: { x: number; y: number; scale: number; blur: number };
  cursor: { x: number; y: number; press: number; opacity: number };
}

type Shape = "arrow" | "hand" | "beam";

const camera = document.querySelector<HTMLElement>(".camera")!;
const app = document.querySelector<HTMLIFrameElement>("iframe")!;

const ARROW = `<svg viewBox="-2 -2 22 30" width="22" height="30"><path d="M0 0v22.6l5.3-5.1 3.6 8.4 3.5-1.5-3.6-8.3H16L0 0z" fill="#000" stroke="#fff" stroke-width="1.6" stroke-linejoin="round"/></svg>`;
const HAND = `<svg viewBox="-1 -1 26 30" width="26" height="30"><path d="M8.6 1.2c-1.2 0-2.1.9-2.1 2.1v10.3l-1.8-1.9c-.9-.9-2.2-1-3.1-.2-.9.8-1 2-.3 3l4.3 6c1.6 2.2 4 3.6 6.8 3.6h2.4c3.9 0 7-3.1 7-7v-5.2c0-1.2-.9-2.1-2.1-2.1s-2.1.9-2.1 2.1v-1c0-1.2-.9-2.1-2.1-2.1s-2.1.9-2.1 2.1v-.9c0-1.2-.9-2.1-2.1-2.1s-2.1.9-2.1 2.1V3.3c0-1.2-.9-2.1-2.1-2.1z" fill="#fff" stroke="#000" stroke-width="1.4" stroke-linejoin="round"/><path d="M12.8 15.8v3M16.9 15.8v3M21 15.8v2.6" stroke="#000" stroke-width="1.2" stroke-linecap="round"/></svg>`;
const BEAM = `<svg viewBox="-3 -2 14 28" width="14" height="28"><path d="M0 0c2.4 0 3.6.8 4 2.2C4.4.8 5.6 0 8 0M4 2.2v19.6M0 24c2.4 0 3.6-.8 4-2.2.4 1.4 1.6 2.2 4 2.2M1.6 12h4.8" fill="none" stroke="#fff" stroke-width="3.4" stroke-linecap="round"/><path d="M0 0c2.4 0 3.6.8 4 2.2C4.4.8 5.6 0 8 0M4 2.2v19.6M0 24c2.4 0 3.6-.8 4-2.2.4 1.4 1.6 2.2 4 2.2M1.6 12h4.8" fill="none" stroke="#000" stroke-width="1.5" stroke-linecap="round"/></svg>`;

const cursor = document.createElement("div");
cursor.className = "video-cursor";
cursor.innerHTML = `<span data-shape="arrow">${ARROW}</span><span data-shape="hand">${HAND}</span><span data-shape="beam">${BEAM}</span>`;
camera.append(cursor);

const INTERACTIVE =
  "button, a[href], [role=button], [role=tab], [role=menuitem], [role=option], [role=treeitem], [role=link], summary, label, select";

function shapeAt(x: number, y: number): Shape {
  const element = app.contentDocument?.elementFromPoint(
    x - app.offsetLeft,
    y - app.offsetTop,
  );
  if (!element) return "arrow";
  const style = element.ownerDocument.defaultView!.getComputedStyle(element);
  if (style.cursor === "text" || style.cursor === "auto" && element.closest("input, textarea, [contenteditable=true], [role=textbox]")) return "beam";
  if (style.cursor === "pointer" || element.closest(INTERACTIVE)) return "hand";
  return "arrow";
}

const visible = (element: Element) => {
  const box = element.getBoundingClientRect();
  return box.width > 0 && box.height > 0 && getComputedStyle(element).visibility !== "hidden";
};

function element(target: Exclude<Target, Rect>): Element | null {
  if (typeof target === "object" && "stage" in target)
    return document.querySelector(target.stage);
  const doc = app.contentDocument!;
  if (typeof target === "string")
    return [...doc.querySelectorAll(target)].find(visible) ?? null;
  const matches = (text: string) => {
    const trimmed = text.trim();
    if (target.text instanceof RegExp) return target.text.test(trimmed);
    return target.exact === false ? trimmed.includes(target.text) : trimmed === target.text;
  };
  const candidates = [...doc.querySelectorAll(target.selector ?? "*")].filter(
    (candidate) => visible(candidate) && matches(candidate.textContent ?? ""),
  );
  // The innermost match is the one the words are drawn in.
  return candidates.find((candidate) => !candidates.some((other) => other !== candidate && candidate.contains(other))) ?? null;
}

/** A target's box on the 1920 by 1080 screen, before the camera moves it. */
function rect(target: Target): Rect | null {
  if (typeof target === "object" && "width" in target) return target;
  const found = element(target);
  if (!found) return null;
  const box = found.getBoundingClientRect();
  const inApp = found.ownerDocument !== document;
  const left = inApp ? app.offsetLeft : 0;
  const top = inApp ? app.offsetTop : 0;
  return { x: box.x + left, y: box.y + top, width: box.width, height: box.height };
}

function documents(): Document[] {
  const found: Document[] = [];
  const walk = (doc: Document | null | undefined) => {
    if (!doc) return;
    found.push(doc);
    for (const frame of doc.querySelectorAll("iframe")) {
      try {
        walk(frame.contentDocument);
      } catch {}
    }
  };
  walk(document);
  return found;
}

const clocks = new WeakMap<Animation, { origin: number; from: number }>();

/**
 * The renderer holds every page's timeline still, so CSS transitions and
 * animations move only here, by the same virtual time the scripts run on.
 */
function syncAnimations(now: number) {
  for (const doc of documents()) {
    for (const animation of doc.getAnimations()) {
      if (animation.playState !== "running") continue;
      let clock = clocks.get(animation);
      if (!clock) {
        clock = { origin: now, from: Number(animation.currentTime ?? 0) };
        clocks.set(animation, clock);
      }
      animation.currentTime = clock.from + (now - clock.origin) * animation.playbackRate;
    }
  }
}

let shape: Shape | null = null;

function show(frame: Frame, now: number) {
  const { x, y, scale, blur } = frame.camera;
  camera.style.transform = `translate(${960 - x * scale}px, ${540 - y * scale}px) scale(${scale})`;
  camera.style.filter = blur > 0.05 ? `blur(${blur}px)` : "";
  const next = shapeAt(frame.cursor.x, frame.cursor.y);
  if (next !== shape) {
    shape = next;
    cursor.dataset.shape = next;
  }
  cursor.style.opacity = String(frame.cursor.opacity);
  cursor.style.transform = `translate(${frame.cursor.x}px, ${frame.cursor.y}px) scale(${1 - 0.18 * frame.cursor.press})`;
  syncAnimations(now);
}

Object.assign(window, {
  video: {
    rect,
    show,
    ready: () => app.contentWindow && "showcase" in app.contentWindow && !!app.contentDocument?.querySelector(".shell"),
  },
});
