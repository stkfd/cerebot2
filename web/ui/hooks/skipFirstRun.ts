import {useLayoutEffect, useRef} from "react";

export function useInitialRender() {
    const isFirstRun = useRef(true);
    useLayoutEffect(() => {
        if (isFirstRun) {
            isFirstRun.current = false;
        }
    });

    return isFirstRun.current;
}