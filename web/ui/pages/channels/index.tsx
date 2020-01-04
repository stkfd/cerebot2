import Page from '../../layouts/page'
import {pageTitle} from "../../util";
import Navigation from "../../components/navigation";
import PageContent from "../../layouts/pageContent";

export default () => (
    <Page title={pageTitle("Channels")}>
        <Navigation/>
        <PageContent>
            <h2>Channels</h2>
        </PageContent>
    </Page>
);