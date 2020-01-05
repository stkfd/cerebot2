import Page from '../layouts/page'
import TopBar from "../components/topBar";
import PageContent from "../layouts/pageContent";
import dynamic from "next/dynamic";
import * as React from "react";
import "../styles/swagger/main.css"

const SwaggerUI = dynamic(import("swagger-ui-react"), { ssr: false });

const DocPage = () => {
    return <Page>
        <TopBar/>
        <PageContent>
            <SwaggerUI url="/spec.yml" docExpansion="list" />
        </PageContent>
    </Page>;
};

export default DocPage;