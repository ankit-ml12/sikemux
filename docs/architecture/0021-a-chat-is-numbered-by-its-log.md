# ADR 0021: A chat is numbered by its log, not by who listens

- Status: Accepted
- Decision date: 2026-10-29

## Context

Every client keeps one copy of a chat and a mark, `{feed, seq}`, of the last change it took
([ADR 0010](./0010-phone-links-only-the-wire-and-client.md)). Coming back with the mark, it is
sent only the rows changed since. The phone saves its mark with its copy and reopens from it,
even after the app is killed.

The number only moved when changes went out to a listener. A chat nobody watched let go of
its folded rows, went on taking in events, and numbered none of them. Folding it again for the
next client stamped every row with the number clients had already heard, so a phone that came
back with that number was sent no rows, saved the same mark again, and stayed on an old turn
for good. Nothing on the phone could notice: it skipped any delivery numbered at or below what
it held and never looked for one that was missing.

## Decision

- **Every change is numbered when it happens.** Each chat has one counter. A change takes the
  number of the next delivery, the one it will go out in: every agent event, every piece of
  streamed text, the person's prompts, rewinds, and the queue, draft, triggers and running state
  the core passes in. Changes made while nobody listens take the next number too.
- **The history file is the log.** Each event's line carries the number of the last change it
  made; text joined into one event moves its number on. The counter goes with the chat when a
  core hands it over, and a core that recovers a chat after a crash starts from the highest
  number in its file.
- **Rows are a pure function of the log.** Each row, a subagent's too, holds the number of the
  last change that touched it. A finished run's fold and line count as changed by the latest
  change to any of its rows, or to whether the turn is running. Letting go of the fold and
  building it again from the log gives every row the number it had.
- **A number closes when its delivery goes out**, or when the fold is built again over changes
  nobody heard. Live deliveries are numbered one after another, as they always were.
- **Resuming is "everything after N"**: the head, which rows of the window stay, and each row
  changed after N. A client gets the chat whole only when its mark is from another run of the
  agent, is ahead of the core, or the window starts at a row it cannot have held.
- **Clients look for gaps.** A delivery numbered one past the mark is taken; one at or below it
  is a repeat and is skipped. A client caught up after falling behind is sent a delivery whose
  head names the number it follows. Anything else means changes went missing, and the client
  attaches again with its mark. The phone and the Mac app do this; the notch reads only the
  device view.
- **No new infrastructure.** The counter is an integer, the log is the history file each chat
  already writes.

## Consequences

- A phone that reopens a chat that moved on while nobody watched gets exactly the rows that
  changed, and a fold built again sends nothing a client already holds.
- Streaming costs no more per change: the number is the counter plus one, and each history line
  is a few bytes longer.
- The wire keeps its shape. Marks are still `{feed, seq}` and live deliveries are still
  consecutive, so released phones keep working without a store build, and `PROTOCOL_VERSION`
  stays. The number a catch-up follows is an optional field older clients ignore.
- History lines and handed-over events written before this change carry no number. They count
  as changed when the core that reads them took the chat, so a mark from before that resumes
  with all of them rather than none.

## Alternatives considered

- **Numbering the fold again when it is rebuilt.** The stopgap. It made every return after a
  rebuild send the whole window, and still left the phone blind to a gap.
- **A number on every change rather than every delivery, with each delivery naming the one
  before it.** Rejected: deliveries would skip numbers, which the Mac app and released phones
  read as missing changes.
- **A message queue or store such as Kafka or Redis.** Rejected: the history file already holds
  every event in order, and one integer per chat is enough to name a place in it.
