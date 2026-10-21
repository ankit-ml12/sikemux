import { createContext } from "react";

/** The place in the agent pane's header where the chat shows what holds for the whole chat, such as its worktree and account. */
export const ChatHeaderSlot = createContext<HTMLElement | null>(null);
