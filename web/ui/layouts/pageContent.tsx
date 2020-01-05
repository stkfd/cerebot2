import {FunctionComponent} from "react";
import {ScaleLoader} from "react-spinners";
import {LoadingState} from "../hooks/loading";

interface PageContentProps {
    loading?: LoadingState;
}

const PageContent: FunctionComponent<PageContentProps> = ({loading: state, children}) => {
    const loading = typeof state !== "undefined" && state.loading;
    const slow = typeof state !== "undefined" && state.slow;

    const spinner = slow ? <div className="justify-center flex">
            <ScaleLoader color="var(--color-primary-500)"/>
        </div> :
        null;
    return <div className="flex justify-center">
        <div className="w-4/6">
            {loading ?
                spinner :
                children
            }
        </div>
    </div>
};

export default PageContent;