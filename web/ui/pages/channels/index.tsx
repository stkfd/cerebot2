import Page from '../../layouts/page'
import {pageTitle} from "../../util";
import TopBar from "../../components/topBar";
import PageContent from "../../layouts/pageContent";
import PageTitle from "../../components/page/title";
import {NextPage} from "next";

const ChannelsPage: NextPage = () => (
    <Page title={pageTitle("Channels")}>
        <TopBar/>
        <PageContent>
            <PageTitle fullWidth>Channels</PageTitle>
        </PageContent>
    </Page>
);

export default ChannelsPage;