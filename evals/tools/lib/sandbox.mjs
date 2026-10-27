import { existsSync, mkdirSync, mkdtempSync, realpathSync, writeFileSync, readFileSync, symlinkSync, cpSync, chmodSync } from 'node:fs';
import { execFileSync } from 'node:child_process';
import { homedir } from 'node:os';
import { join, dirname } from 'node:path';
import { SHIMMED } from './shims.mjs';
import { REPO, readRepo } from './surface.mjs';

const EVAL = join(REPO, 'evals/tools');
const HOME = homedir();
const PRIVATE = ['proj', 'projects', 'Documents', 'Desktop', 'Downloads', 'Pictures', '.config', '.ssh', '.claude', '.omp', '.codex', '.aws', '.pi', 'nodemem', 'work-context', 'Library/Application Support', 'Library/Messages', 'Library/Mail', 'Library/Mobile Documents'];
const NEVER_RUN = [
  ...['/usr/bin/open', '/usr/bin/osascript', '/usr/sbin/screencapture', '/usr/bin/ssh', '/usr/bin/scp', '/usr/bin/curl', '/usr/bin/make'],
  ...['/usr/bin/xcrun', '/usr/bin/xcodebuild', '/usr/sbin/lsof', '/bin/ps', '/usr/bin/pgrep', '/usr/local/bin/docker'],
  ...['gh', 'aws', 'cargo', 'rustc', 'npm', 'npx', 'pnpm', 'psql', 'wget', 'codex'].map((name) => `/opt/homebrew/bin/${name}`),
  ...['.cargo/bin/cargo', '.cargo/bin/rustc', '.cargo/bin/rustup', '.local/bin/npm', '.local/bin/npx', '.local/bin/vercel'].map((path) => join(HOME, path)),
];
const AUTHOR = { GIT_AUTHOR_NAME: 'nodelike', GIT_AUTHOR_EMAIL: 'dev@example.com', GIT_COMMITTER_NAME: 'nodelike', GIT_COMMITTER_EMAIL: 'dev@example.com' };

export const BASE_PATH = [
  join(HOME, '.local/bin'),
  join(HOME, '.bun/bin'),
  '/opt/homebrew/bin',
  '/usr/local/bin',
  '/usr/bin',
  '/bin',
  '/usr/sbin',
  '/sbin',
].join(':');

export function prepareCase(dir, testCase, { at = null, host }) {
  const fixture = testCase.fixture ?? {};
  const root = realpathSync(mkdtempSync('/tmp/u'));
  const home = join(root, 'home');
  const repo = join(home, fixture.project === 'work' ? 'work/app' : 'proj/app');
  const bin = join(home, '.local/bin');
  for (const folder of [home, repo, bin, join(home, 'Library'), join(root, 'tmp')]) mkdirSync(folder, { recursive: true });
  symlinkSync(root, join(dir, 'sandbox'));
  symlinkSync(join(HOME, 'Library/Keychains'), join(home, 'Library/Keychains'));
  writeFileSync(join(home, '.claude.json'), JSON.stringify(claudeAccount()));
  writeFileSync(join(dir, 'fixture.json'), JSON.stringify(fixture, null, 1));

  for (const name of Object.keys(SHIMMED)) {
    const shim = join(bin, name);
    writeFileSync(shim, `#!/bin/sh\nexec "${process.execPath}" "${join(EVAL, 'lib/shim.mjs')}" ${name} "$@"\n`);
    chmodSync(shim, 0o755);
  }

  const work = fixture.project === 'work';
  cpSync(join(EVAL, 'fixtures/project', work ? 'work' : 'sikemux'), repo, { recursive: true });
  if (!work) for (const real of ['AGENTS.md', 'sikemux.json']) writeFileSync(join(repo, real), readRepo(real, at));
  const instructions = [readFileSync(join(repo, 'AGENTS.md'), 'utf8'), fixture.instructions].filter(Boolean).join('\n');
  writeFileSync(join(repo, 'AGENTS.md'), instructions);
  writeFileSync(join(repo, 'CLAUDE.md'), instructions);
  writeFiles(repo, fixture.files);
  const git = (...args) => execFileSync('/usr/bin/git', args, { cwd: repo, env: { ...AUTHOR, HOME: home, PATH: BASE_PATH }, stdio: 'ignore' });
  git('init', '-q', '-b', 'main');
  git('add', '-A');
  git('commit', '-q', '-m', 'Start the project');
  for (const commit of fixture.commits ?? []) {
    writeFiles(repo, commit.files);
    git('add', '-A');
    git('commit', '-q', '-m', commit.message);
  }
  if (fixture.remote) git('remote', 'add', 'origin', fixture.remote);
  writeFiles(repo, fixture.dirty);

  writeFileSync(join(dir, 'sandbox.sb'), profile([dir, root], host));
  return {
    home,
    repo,
    env: {
      ...AUTHOR,
      HOME: home,
      USER: process.env.USER,
      LOGNAME: process.env.USER,
      PATH: `${bin}:${BASE_PATH}`,
      SHELL: '/bin/zsh',
      LANG: 'en_US.UTF-8',
      TERM: 'dumb',
      TMPDIR: join(root, 'tmp/'),
      TOOL_EVAL_CASE: dir,
    },
  };
}

function writeFiles(root, files = {}) {
  for (const [path, content] of Object.entries(files)) {
    const target = join(root, path);
    mkdirSync(dirname(target), { recursive: true });
    writeFileSync(target, content);
  }
}

function claudeAccount() {
  const config = JSON.parse(readFileSync(join(HOME, '.claude.json'), 'utf8'));
  const kept = Object.fromEntries(['oauthAccount', 'userID', 'hasCompletedOnboarding'].filter((key) => key in config).map((key) => [key, config[key]]));
  return { ...kept, bypassPermissionsModeAccepted: true };
}

function profile(writable, host) {
  const quote = (path) => JSON.stringify(path);
  return [
    '(version 1)',
    '(allow default)',
    `(deny file-write* (subpath ${quote(HOME)}))`,
    `(allow file-write* (subpath ${quote(join(HOME, 'Library/Keychains'))}))`,
    `(allow file-write* ${writable.map((path) => `(subpath ${quote(path)})`).join(' ')})`,
    `(deny file-read* ${PRIVATE.map((path) => `(subpath ${quote(join(HOME, path))})`).join(' ')})`,
    '(allow file-read-metadata)',
    `(allow file-read* (subpath ${quote(join(EVAL, 'lib'))}) (subpath ${quote(join(EVAL, 'rivals'))}) (literal ${quote(host)}))`,
    `(deny process-exec ${[...new Set(NEVER_RUN.flatMap((path) => (existsSync(path) ? [path, realpathSync(path)] : [])))].map((path) => `(literal ${quote(path)})`).join(' ')})`,
    '',
  ].join('\n');
}
