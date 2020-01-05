import Page from '../layouts/page'
import TopBar from "../components/topBar";
import PageContent from "../layouts/pageContent";

export default () => (
    <Page>
        <TopBar/>
        <PageContent>
            <p>Home</p>
        </PageContent>
    </Page>
);
