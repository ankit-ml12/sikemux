/** Claude's input line. The same marker points at a choice in its menus, so only a line right under a rule counts. */
const PROMPT = /^\s*❯\s?(.*)$/;
const RULE = /^\s*[─━]{8,}\s*$/;
/** How far up from the bottom the input box can sit, below the transcript. */
const BOTTOM_ROWS = 20;

/** What is typed in the input box of an agent's TUI, from its rows on screen; empty when there is none. */
export function promptDraft(rows: readonly string[]): string {
    const from = Math.max(1, rows.length - BOTTOM_ROWS);
    let start = -1;
    for (let row = rows.length - 1; row >= from; row--) {
        if (PROMPT.test(rows[row]) && RULE.test(rows[row - 1])) {
            start = row;
            break;
        }
    }
    if (start < 0) return "";
    const lines = [rows[start].replace(PROMPT, "$1")];
    for (let row = start + 1; row < rows.length && !RULE.test(rows[row]); row++) {
        if (!/^\s{2}/.test(rows[row])) return "";
        lines.push(rows[row].slice(2));
    }
    return lines
        .map((line) => line.trimEnd())
        .join("\n")
        .trim();
}
