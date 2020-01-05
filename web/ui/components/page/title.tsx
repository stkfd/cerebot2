import {FunctionComponent} from "react";

const PageTitle: FunctionComponent<{fullWidth?: boolean}> = ({children, fullWidth}) => {
    if (fullWidth) {
        return <h2 className="text-3xl mx-2 mb-3 border-b border-primary-900 px-2">{children}</h2>
    } else {
        return <h2 className="w-4/6 mx-auto text-3xl mx-2 mb-3 border-b border-primary-900 px-2">{children}</h2>
    }
};

export default PageTitle;
