import Page from '../layouts/page'
import Navigation from "../components/navigation";
import PageContent from "../layouts/pageContent";
import dynamic from "next/dynamic";
import * as React from "react";
import "../styles/swagger/main.css"

const SwaggerUI = dynamic(import("swagger-ui-react"), { ssr: false });

export default class DocPage extends React.Component {
    public render() {
        return <Page>
            <Navigation/>
            <PageContent>
                <SwaggerUI url="/spec.yml" docExpansion="none" />
            </PageContent>
        </Page>;
    }
}
