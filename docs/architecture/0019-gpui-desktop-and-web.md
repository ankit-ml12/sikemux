# ADR 0019: The desktop and the web are one GPUI app that only draws what the core sends

- Status: Accepted
- Decision date: 2026-10-21

## Context

The desktop app is React in Tauri's WebKit. It grew one feature at a time into many thin
components, state copied between them and logic repeated beside the phone's. WebKit's memory
and main thread are what limit how many agents, panes and projects it keeps up with
(GitHub #115 holds the measurements and the choices behind this ADR). The same app is wanted
as the web client at app.sikemux.com.

## Decision

- **GPUI, on the `sonorahq/gpui` fork with GPUI Kit**, draws the Mac and the web (wasm) from
  one Rust UI. It is written fresh on the `gpui` branch and switched to once everything
  matches, through a pull request.
- **The core owns what the data means** ([ADR 0010](./0010-phone-links-only-the-wire-and-client.md)).
  The app draws the core's views: chats as rows and their changes, agents, questions, the
  catalog, and terminals as cell grids the core emulates.
- **One store.** The app holds one store of the core's view, entities fed by the core's
  changes, and a little state of its own: focus, open menus, scroll, sizes. No view keeps a
  copy of the core's data, no screen has a cache or a second path to the same fact, and what
  two views need lives in the store once.
- **Data flows one way.** The core's changes go into the store, the store notifies views,
  views draw. What the person does goes out as the core's `Call`s. No view writes another
  view's state.
- **A small shared component set comes first**: buttons, rows, list, tabs, menu, popover,
  input, pane chrome, icon and mark box, on GPUI Kit, themed from one token set taken from
  `DESIGN.md`. Screens are put together from these; a screen that needs a one-off widget
  shows the set is missing something, which is added there once.
- **Fewer, bigger pieces.** A few well-named components rather than many thin ones, no layers
  that only pass props through, no per-screen copy of the same row or header.
- **Smaller than what it replaces.** Each area's line count is reported against today's
  TypeScript and CSS for the same area, and is meant to be well below it. Each screen is
  checked against these rules, and simplified, before it lands.

## Consequences

- The behaviour and the look of the Tauri app are what is ported, not its components or its
  state.
- The React app keeps working on the branch until GPUI replaces it, drawing the same core
  views through `src/chat/rows.ts` and the generated wire types.
- Pieces nobody has built for GPUI yet become crates others can use, free of Sikemux types
  and `workspace-hack`, as #115 lists.
