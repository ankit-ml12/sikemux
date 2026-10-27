# Tool-use eval

Real failures from agent transcripts, replayed against sikemux-tools to measure how well each model picks and calls its tools. It is run by hand and stays out of `pnpm check` and CI.

```sh
pnpm eval:tools --cases 'browser-*' --models sonnet
pnpm eval:tools --models sonnet,opus,deepseek --search both --record baseline
pnpm eval:tools --list
```

Each case runs the agent CLI (`claude -p`, or OMP for DeepSeek) in a temporary home and a fresh fixture repo, inside `sandbox-exec`, so it cannot reach the real desk, browser, git or simulator. Its tools come from the real `sikemux-host --tools-mcp` built from this checkout, talking to a fake Sikemux (`lib/core.mjs`) that serves `browser/tools.json`, the guide and the plugin manifests straight from the repo and answers each call from the case's fixture. A description change in the repo shows up in the next run. Rival routes the cases need, such as an Atlassian MCP, come from `rivals/`, and commands like `gh`, `sleep` and `xcrun` from shims in `lib/shims.mjs`.

## Cases

`cases/*.json` hold arrays of cases:

- `prompt`: what the agent is told.
- `fixture`: `project` (`sikemux` or `work`), `files`, `commits` and `dirty` for the repo, `instructions` added to its AGENTS.md, `tab` and `pages` for the desk browser, `agents` beside it, `answers` (rules of `tool`, optional `args`, `reply` or `error`, optional `times`) for any tool, `shell` rules for shimmed commands, `rivals` and `plugins`.
- `grade`: `expect` (calls that must happen, in `order` if set), `forbid`, `limit`, `first`, `reply`, and `allow` to lift one of the checks every case gets: no Atlassian MCP, no Claude browser, no SendMessage, no raw `git commit` or `git add -A`, no `gh`, no sleeping, no two calls at once on one tab, simulator or file.
- `judge`: a rubric for a judge model, for what calls alone cannot show.
- `skip`: why a case cannot be replayed.

Argument matchers compare values, `"/regex/flags"` strings, `{"$present": bool}`, `{"$not": …}` and `{"$any": […]}`.

Runs land in `/tmp/sikemux-tool-eval/<time>/`, one folder per case with its transcript and every call the fake Sikemux answered. `--record <label>` also saves the result under `results/`.
