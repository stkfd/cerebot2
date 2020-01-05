import {useLayoutEffect, useRef} from "react";

export function useInitialRender(): boolean {
    const isFirstRun = useRef(true);
    useLayoutEffect(() => {
        if (isFirstRun) {
            isFirstRun.current = false;
        }
    });

    return isFirstRun.current;
}
