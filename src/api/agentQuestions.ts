import type { AgentQuestion, QuestionAnswer } from "../agents/agentQuestions";
import { coreCall } from "./core";

/** Sent by the window whenever the open questions change. */
export const AGENT_QUESTIONS_EVENT = "agent_questions";

export const agentQuestionsApi = {
    list: () => coreCall<AgentQuestion[]>("questions"),
    answer: (id: string, answer: QuestionAnswer) => coreCall<{ answered: boolean }>("answerQuestion", { id, ...answer }),
    /** Answers questions asked together, so each agent reads its answers in one message. */
    answerAll: (answers: readonly { id: string; answer: QuestionAnswer }[]) =>
        coreCall<{ answered: boolean }>("answerQuestions", { answers: answers.map(({ id, answer }) => ({ id, ...answer })) }),
};
