import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import { agentDisplay } from "../state/agentStatus";
import { AgentStateIndicator } from "./AgentStateIndicator";

afterEach(cleanup);

describe("AgentStateIndicator", () => {
    it.each([
        ["done", "Done"],
        ["blocked", "Needs input"],
    ] as const)("renders %s as a dot", (state, label) => {
        const { container } = render(<AgentStateIndicator display={agentDisplay({ group: state })} />);
        expect(screen.getByRole("img", { name: label })).toBeInTheDocument();
        expect(container.querySelector(".agent-state-dot")).toBeInTheDocument();
        expect(container.querySelector(".agent-state-loader")).not.toBeInTheDocument();
    });

    it("renders nothing for idle", () => {
        const { container } = render(<AgentStateIndicator display={agentDisplay({ group: "idle" })} />);
        expect(container).toBeEmptyDOMElement();
    });

    it("prefers needs-input over background work", () => {
        render(<AgentStateIndicator display={agentDisplay({ group: "blocked", tasks: 1 })} />);
        expect(screen.getByRole("img", { name: "Needs input" })).toBeInTheDocument();
    });
});
