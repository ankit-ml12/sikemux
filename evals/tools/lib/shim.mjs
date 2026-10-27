import { readFileSync, appendFileSync } from 'node:fs';
import { valueMatches } from './match.mjs';
import { SHIMMED } from './shims.mjs';

const [name, ...args] = process.argv.slice(2);
const caseDir = process.env.TOOL_EVAL_CASE;
const line = [name, ...args].join(' ');
const fixture = JSON.parse(readFileSync(`${caseDir}/fixture.json`, 'utf8'));
const rule = (fixture.shell ?? []).find((candidate) => valueMatches(candidate.match, line)) ?? SHIMMED[name] ?? {};
appendFileSync(`${caseDir}/shell.jsonl`, `${JSON.stringify({ at: Date.now(), command: line })}\n`);
if (rule.stdout) process.stdout.write(rule.stdout);
if (rule.stderr) process.stderr.write(rule.stderr);
process.exitCode = rule.exit ?? 0;
