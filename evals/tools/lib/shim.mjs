import { readFileSync, appendFileSync, existsSync } from 'node:fs';
import { valueMatches } from './match.mjs';
import { SHIMMED } from './shims.mjs';

const [name, ...args] = process.argv.slice(2);
const caseDir = process.env.TOOL_EVAL_CASE;
const line = [name, ...args].join(' ');
const fixture = JSON.parse(readFileSync(`${caseDir}/fixture.json`, 'utf8'));
const rule = (fixture.shell ?? []).find((candidate) => valueMatches(candidate.match, line)) ?? SHIMMED[name] ?? {};
appendFileSync(`${caseDir}/shell.jsonl`, `${JSON.stringify({ at: Date.now(), command: line })}\n`);
if (rule.stdout) process.stdout.write(rule.stdout);
else if (!rule.stderr && ['pnpm', 'npm'].includes(name)) process.stdout.write(scriptBanner(args));
if (rule.stderr) process.stderr.write(rule.stderr);
process.exitCode = rule.exit ?? 0;

function scriptBanner(words) {
  const script = words.filter((word) => !word.startsWith('-') && word !== 'run')[0];
  if (!script || !existsSync('package.json')) return '';
  const manifest = JSON.parse(readFileSync('package.json', 'utf8'));
  const command = manifest.scripts?.[script];
  if (!command) return '';
  return `\n> ${manifest.name ?? 'app'}@${manifest.version ?? '0.0.0'} ${script} ${process.cwd()}\n> ${command}\n\n`;
}
