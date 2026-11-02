import { createContext, useContext } from "react";

/** Expert mode (or an active search): sections show every option, else only the essentials. */
export const ExpertContext = createContext(true);
export const useExpert = () => useContext(ExpertContext);
