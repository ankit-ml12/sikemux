import { IconChevron } from "../ui/Icons";
import { durationLabel } from "./durationLabel";

export function WorkSummary({ took, calls, open, onToggle }: { took: number | null; calls: number; open: boolean; onToggle: () => void }) {
    return (
        <div className="chat-work">
            <button type="button" className="chat-work-sum" aria-expanded={open} onClick={onToggle}>
                <span>{took === null ? "Worked" : `Worked for ${durationLabel(took)}`}</span>
                {took === null && calls > 0 && <span className="chat-work-calls">{`${calls} tool ${calls === 1 ? "call" : "calls"}`}</span>}
                <IconChevron size={10} className="chat-work-chevron" />
            </button>
        </div>
    );
}
