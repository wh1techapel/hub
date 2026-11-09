// U4: a Studio project tells the shell when a wide document is open in the canvas beside the
// conversation, so Studio's history folds into the rail meanwhile, the way Gemini folds its own
// sidebar when Canvas opens. The owner can open the history again; nothing is stored.

import { createContext, useContext, useEffect } from "react";

export const CanvasContext = createContext<(wide: boolean) => void>(() => undefined);

/** Says whether a wide document is open now; leaving the page says it no longer is. */
export function useWideCanvas(wide: boolean) {
  const set = useContext(CanvasContext);
  useEffect(() => {
    set(wide);
    return () => set(false);
  }, [set, wide]);
}
