import Page from '../layouts/page'
import Navigation from "../components/navigation";
import PageContent from "../layouts/pageContent";

export default () => (
    <Page>
        <Navigation/>
        <PageContent>
            <p>Home</p>
        </PageContent>
    </Page>
);
