import {useEffect, useRef} from "react";

export function useInitialRender(): boolean {
    const isFirstRun = useRef(true);
    useEffect(() => {
        if (isFirstRun) {
            isFirstRun.current = false;
        }
    });

    return isFirstRun.current;
}
