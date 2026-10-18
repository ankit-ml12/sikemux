import { createContext, useContext } from "react";

/** Opens the picker for which host repository the git pane shows, while the folder is on a host with one to pick. */
export const PickRepoContext = createContext<(() => void) | null>(null);

export const usePickRepo = () => useContext(PickRepoContext);
