import Page from '../layouts/page'
import TopBar from "../components/topBar";
import PageContent from "../layouts/pageContent";
import {NextPage} from "next";

const Home: NextPage = () => (
    <Page>
        <TopBar/>
        <PageContent>
            <p>Home</p>
        </PageContent>
    </Page>
);

export default Home;