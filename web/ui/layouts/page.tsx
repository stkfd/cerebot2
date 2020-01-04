import Meta from '../components/meta'
import Footer from '../components/footer'
import {ReactNode} from "react";

interface Props {
    children?: ReactNode,
    title?: string,
}

const Page = ({ children, title }: Props) => (
    <div>
        <Meta title={title || "cerebot"} />
        { children }
        <Footer />
    </div>
);

export default Page;
