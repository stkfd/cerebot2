import * as React from "react";

export interface LoadingState {
    loading: boolean;
    slow: boolean;
}

export function useLoading(loadingInit: boolean, slowInit: boolean): [LoadingState, typeof setLoading] {
    const [loading, setLoadingInitial] = React.useState<boolean>(loadingInit);
    const [loadingSlow, setLoadingSlow] = React.useState<boolean>(slowInit);

    const setIdRef = React.useRef<number>(0);

    function setLoading(isLoading: boolean): void {
        const setId = ++setIdRef.current;
        setLoadingInitial(isLoading);
        if (!isLoading) {
            setLoadingSlow(false);
        } else {
            setTimeout(() => {
                if (setId === setIdRef.current) {
                    setLoadingSlow(isLoading);
                }
            }, 100);
        }
    }

    return [
        {
            loading,
            slow: loadingSlow
        },
        setLoading
    ];
}
