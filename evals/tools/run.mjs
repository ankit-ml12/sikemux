#!/usr/bin/env node
import { parseArgs } from 'node:util';
import { readFileSync, readdirSync, mkdirSync, writeFileSync, realpathSync, statSync, existsSync } from 'node:fs';
import { execFileSync } from 'node:child_process';
import { join } from 'node:path';
import { loadSurface, REPO, SIGNED_IN_PLUGINS } from './lib/surface.mjs';
import { startCore } from './lib/core.mjs';
import { prepareCase } from './lib/sandbox.mjs';
import { MODELS, mcpServers, runClaude } from './lib/runners.mjs';
import { runOmp } from './lib/omp.mjs';
import { grade } from './lib/grade.mjs';
import { judge } from './lib/judge.mjs';
import { report } from './lib/report.mjs';

const EVAL = join(REPO, 'evals/tools');
const HOST = join(REPO, 'src-tauri/target/debug/sikemux-host');

const { values: options } = parseArgs({
  options: {
    models: { type: 'string', default: 'sonnet' },
    search: { type: 'string', default: 'on' },
    cases: { type: 'string' },
    category: { type: 'string' },
    concurrency: { type: 'string', default: '4' },
    turns: { type: 'string', default: '16' },
    timeout: { type: 'string', default: '420' },
    build: { type: 'boolean', default: false },
    record: { type: 'string' },
    list: { type: 'boolean', default: false },
    help: { type: 'boolean', short: 'h', default: false },
  },
});

if (options.help) {
  console.log(`pnpm eval:tools [options]
  --models sonnet,opus,deepseek   models to run (also haiku)
  --search on|off|both            Claude tool search; DeepSeek ignores it
  --cases id,id,prefix*           cases to run (default: every replayable case)
  --category wrong_tool,...       only these categories
  --concurrency 4                 cases at once per model
  --turns 16 --timeout 420        limits per case (turns, seconds)
  --build                         rebuild sikemux-host first
  --record <label>                also save the result under evals/tools/results/
  --list                          list the cases that would run, and stop`);
  process.exit(0);
}

const cases = loadCases();
if (options.list) {
  for (const testCase of cases) console.log(`${testCase.id.padEnd(28)} ${testCase.category.padEnd(15)} ${testCase.prompt.slice(0, 90).replace(/\n/g, ' ')}`);
  console.log(`${cases.length} cases`);
  process.exit(0);
}

ensureHost();
const variants = options.models.split(',').flatMap((alias) => {
  const model = MODELS[alias];
  if (!model) throw new Error(`no model called ${alias}; pick from ${Object.keys(MODELS).join(', ')}`);
  if (model.runner !== 'claude') return [{ label: alias, alias, ...model }];
  const searches = options.search === 'both' ? [true, false] : [options.search !== 'off'];
  return searches.map((search) => ({ label: `${alias}${search ? '' : ' (no tool search)'}`, alias, search, ...model }));
});

const stamp = new Date().toISOString().replace(/[:.]/g, '-');
mkdirSync('/tmp/sikemux-tool-eval', { recursive: true });
const runDir = join(realpathSync('/tmp/sikemux-tool-eval'), stamp);
mkdirSync(runDir, { recursive: true });
console.log(`${cases.length} cases × ${variants.map((variant) => variant.label).join(', ')}; runs in ${runDir}`);

const started = Date.now();
const results = await Promise.all(variants.map((variant) => runVariant(variant)));
const summary = {
  date: new Date().toISOString(),
  commit: git('rev-parse', 'HEAD'),
  dirty: git('status', '--porcelain', '--', 'browser', 'src-tauri/crates/sikemux-host/src/tools_mcp', 'src-tauri/plugins') !== '',
  claude: execFileSync(join(process.env.HOME, '.local/bin/claude'), ['--version'], { encoding: 'utf8' }).trim(),
  seconds: Math.round((Date.now() - started) / 1000),
  variants: results,
};
writeFileSync(join(runDir, 'result.json'), JSON.stringify(summary, null, 1));
report(summary, cases);
console.log(`\nresult: ${join(runDir, 'result.json')}`);
if (options.record) {
  const day = summary.date.slice(0, 10);
  const path = join(EVAL, 'results', `${day}-${summary.commit.slice(0, 9)}-${options.record}.json`);
  writeFileSync(path, `${JSON.stringify(compact(summary), null, 1)}\n`);
  console.log(`recorded: ${path}`);
}

function loadCases() {
  const folder = join(EVAL, 'cases');
  const all = readdirSync(folder)
    .filter((name) => name.endsWith('.json'))
    .flatMap((name) => JSON.parse(readFileSync(join(folder, name), 'utf8')).map((testCase) => ({ ...testCase, file: name })));
  const ids = new Set();
  for (const testCase of all) {
    if (ids.has(testCase.id)) throw new Error(`two cases are called ${testCase.id}`);
    ids.add(testCase.id);
  }
  const wanted = options.cases?.split(',');
  const categories = options.category?.split(',');
  return all.filter(
    (testCase) =>
      !testCase.skip &&
      (!wanted || wanted.some((id) => (id.endsWith('*') ? testCase.id.startsWith(id.slice(0, -1)) : testCase.id === id))) &&
      (!categories || categories.includes(testCase.category)),
  );
}

function ensureHost() {
  const sources = join(REPO, 'src-tauri/crates/sikemux-host/src/tools_mcp');
  const newest = Math.max(...readdirSync(sources).map((name) => statSync(join(sources, name)).mtimeMs));
  if (!options.build && existsSync(HOST) && statSync(HOST).mtimeMs > newest) return;
  console.log('building sikemux-host…');
  execFileSync('cargo', ['build', '--manifest-path', join(REPO, 'src-tauri/Cargo.toml'), '-p', 'sikemux-host', '--bin', 'sikemux-host'], {
    stdio: 'inherit',
    env: { ...process.env, PATH: `${join(process.env.HOME, '.cargo/bin')}:${process.env.PATH}` },
  });
}

async function runVariant(variant) {
  const queue = [...cases];
  const done = [];
  const workers = Array.from({ length: Number(options.concurrency) }, async () => {
    while (queue.length) {
      const testCase = queue.shift();
      const result = await runCase(variant, testCase);
      done.push(result);
      const mark = result.pass ? 'pass' : 'FAIL';
      console.log(`${variant.label.padEnd(26)} ${mark} ${testCase.id}${result.pass ? '' : `  ${result.failures.map((failure) => failure.why).join('; ')}`}`);
    }
  });
  await Promise.all(workers);
  done.sort((left, right) => left.id.localeCompare(right.id));
  return { label: variant.label, model: variant.id, search: variant.search ?? null, cases: done };
}

async function runCase(variant, testCase) {
  const dir = join(runDir, variant.label.replace(/\W+/g, '-'), testCase.id);
  mkdirSync(dir, { recursive: true });
  const sandbox = prepareCase(dir, testCase);
  const fixture = testCase.fixture ?? {};
  const surface = loadSurface(fixture.plugins ?? SIGNED_IN_PLUGINS);
  const endpoint = join(dir, 'endpoint.json');
  const core = await startCore({ surface, fixture, project: sandbox.repo, endpointFile: endpoint, logFile: join(dir, 'core.jsonl') });
  const servers = mcpServers({ host: HOST, endpoint, repo: sandbox.repo, dir, rivals: fixture.rivals });
  const limits = { maxTurns: testCase.maxTurns ?? Number(options.turns), timeoutMs: Number(options.timeout) * 1000 };
  let outcome;
  try {
    outcome =
      variant.runner === 'claude'
        ? await runClaude({ model: variant.id, search: variant.search, dir, sandbox, servers, prompt: testCase.prompt, ...limits })
        : await runOmp({ model: variant.id, dir, sandbox, servers, prompt: testCase.prompt, ...limits });
  } finally {
    core.close();
  }
  const failures = grade(testCase, outcome);
  if (outcome.exit.timedOut) failures.push({ check: 'run', why: 'timed out' });
  else if (outcome.failed) failures.push({ check: 'run', why: `the run failed: ${outcome.stop}` });
  const verdict = testCase.judge && !outcome.failed ? await judge({ testCase, outcome, dir, sandbox }) : null;
  if (verdict && !verdict.pass) failures.push({ check: 'judge', why: verdict.why });
  return {
    id: testCase.id,
    category: testCase.category,
    tools: testCase.tools ?? [],
    pass: failures.length === 0,
    failures,
    judge: verdict,
    cost: outcome.cost + (verdict?.cost ?? 0),
    tokens: outcome.tokens,
    turns: outcome.turns,
    calls: outcome.calls.map((call) => ({ tool: call.tool, args: call.args, error: call.error ?? false, batch: call.batch, sub: call.sub || undefined })),
    reply: outcome.reply?.slice(0, 2000) ?? '',
    dir,
  };
}

function compact(result) {
  return {
    ...result,
    variants: result.variants.map((variant) => ({
      ...variant,
      cases: variant.cases.map(({ dir, calls, reply, ...rest }) => ({ ...rest, calls: calls.map((call) => call.tool) })),
    })),
  };
}

function git(...args) {
  return execFileSync('git', args, { cwd: REPO, encoding: 'utf8' }).trim();
}
