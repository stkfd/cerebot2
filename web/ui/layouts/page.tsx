import Meta from '../components/meta'
import Footer from '../components/footer'
import {FunctionComponent, ReactNode} from "react";

interface Props {
    children?: ReactNode;
    title?: string;
    loading?: boolean;
}

const Page: FunctionComponent = ({ children, title, loading }: Props) => (
    <>
        <Meta title={title || "cerebot"} />
        { children }
        <Footer />
    </>
);

export default Page;
