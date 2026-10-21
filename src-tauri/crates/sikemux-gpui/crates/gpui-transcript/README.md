# gpui-transcript

A streaming chat view for [GPUI](https://gpui.rs). Rows keep stable ids, a
message's markdown grows by patches to its tree of blocks, and the list draws
only the rows in view, anchored to the bottom.

- `Rows<R: Row>` holds rows in order: `put` (replace by id or append),
  `update`, `drop_from`, `keep`, `prepend` and `reset`. Each says what it
  moved, so a list can follow.
- `Markdown` holds one message's blocks. `write` applies a patch; the blocks
  it does not reach keep their laid-out text.
- `markdown(id, &doc, &style, window, cx)` draws paragraphs, headings, lists,
  task boxes, code fences, tables, quotes, links, inline code, emphasis,
  details folds and footnotes. `Style` sets the colours and fonts, and takes
  callbacks for links, fences and images.
- `Transcript<R>` is a view: the rows in a windowed, bottom-anchored list.
  Changes pushed to it are applied together when it next draws, at most
  once a frame. Rows added above never move what the reader sees. While the
  reader is at the end, the end stays in view. Near the top it calls
  `on_reach_top` for an earlier page.

The crate draws no rows itself. You hand `Transcript::new` a function that
draws the row at an index, and it calls `markdown` for the text it holds.

## The tree and its patches

Blocks arrive as JSON. Each block is an object tagged by `t` (`p`, `h`,
`quote`, `ul`, `ol`, `li`, `pre`, `hr`, `table`, `fndef`, `details`), and
inline content uses `em`, `strong`, `del`, `code`, `a`, `img`, `br` and
`fnref`, with a bare string for text. A patch is `{ "n": units, "o": [op…] }`.
`n` is the text's length in UTF-16 units after the patch. Each op has a path
`p` of child indexes, through `c`, or through a table's `rows`, then cells,
then cell contents:

- `{ "p": […], "at": k, "c": [...] }`: the list at `p` keeps its first `k`
  entries and gains `c`.
- `{ "p": […], "at": k, "s": "..." }`: the text at `p`, or a code element's `v`,
  keeps its first `k` units and gains `s`.

A patch that does not end at the text's length leaves the tree stale until a
whole one arrives, one that starts with `{ "p": [], "at": 0, "c": … }`.
`is_current` says which case you are in. When the tree is stale, draw the text
with `plain`.

## Example

```sh
FONTS=dir/with/ttf/files cargo run --release --example transcript -- --shots out/
```

It draws offscreen on a simulated clock at 120 Hz. It opens 2,000 rows and
scrolls them to the top with wheel events. Then it opens the last 200 and
scrolls up while earlier pages arrive, checking that each page leaves the view
where it was. Last, it streams a 4,000-token answer at 500 tokens a second. For
each phase it prints the CPU each frame's draw took, the CPU that costs at
120 Hz, and memory. `--shots` saves pictures of the window along the way.

## License

MIT OR Apache-2.0
