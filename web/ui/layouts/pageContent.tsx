import {ReactNode} from "react";
import Button from "../components/button";

const PageContent = ({children}: {children: ReactNode}) => (
    <div className="flex justify-center">
        <div className="w-4/6">
            {children}
        </div>
    </div>
);

export default PageContent;