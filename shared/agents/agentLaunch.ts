/** Whether a mode lets the agent act without asking. */
export const isYolo = (mode: string | null | undefined): boolean => mode === "bypass";
