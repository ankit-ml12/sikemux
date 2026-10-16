import { Calendar, isEmptyActivity, Overview, useActivitySummary } from "../shell/ActivityPage";
import { AgentBuddy } from "../buddies/Buddy";
import { basename } from "../lib/paths";
import type { Agent } from "../state/types";
import "../styles/activity.css";

export function ChatWelcome({ cwd, agent }: { cwd: string; agent: Agent }) {
    const { summary, failed } = useActivitySummary(cwd);
    const totals = summary?.totals;
    return (
        <div className="chat-welcome">
            <div className="chat-welcome-head">
                <span className="chat-welcome-mark agent-glyph buddy-mark" aria-hidden="true">
                    <AgentBuddy agent={agent} size={44} />
                </span>
                <span className="chat-welcome-title">{basename(cwd) || cwd}</span>
            </div>
            {!failed && (
                <div className="chat-welcome-activity">
                    {!isEmptyActivity(totals) && <Overview totals={totals} />}
                    <Calendar days={summary?.days ?? []} loaded={!!summary} />
                </div>
            )}
        </div>
    );
}
