import Page from "../../layouts/page";
import {pageTitle} from "../../util";
import TopBar from "../../components/topBar";
import PageContent from "../../layouts/pageContent";
import {NextPage} from "next";
import {CommandList, CommandsApi} from "../../api-client";
import * as React from "react";
import {useRouter} from "next/router";
import {getPaginationParams, PaginationParams} from "../../lib/util";
import CommandsTable from "../../components/commandsTable";
import PageTitle from "../../components/page/title";
import {useLoading} from "../../hooks/loading";

const api = new CommandsApi({
    basePath: process.env.apiBaseUrl
});

interface Props {
    data?: CommandList;
}

const defaultPageSize = 25;

const CommandsIndex: NextPage<Props> = (props) => {
    const router = useRouter();
    const [data, setData] = React.useState<CommandList>(props.data || {
        items: [],
        page: 1,
        pageCount: 0,
        totalCount: 0
    });
    const [loadingState, setLoading] = useLoading(true, false);

    const {page, perPage} = getPaginationParams(router.query, defaultPageSize);

    const fetchIdRef = React.useRef<number>(0);
    const fetchData = React.useCallback(async ({page, perPage}: PaginationParams) => {
        const fetchId = ++fetchIdRef.current;
        setLoading(true);
        const result = await api.getCommands(page, perPage);
        if (result.status == 200 && fetchId === fetchIdRef.current) {
            setData(result.data);
            setLoading(false);
        }
    }, []);

    return <Page title={pageTitle("Commands")}>
        <TopBar/>
        <PageTitle>Commands</PageTitle>
        <PageContent>
            <CommandsTable
                loading={loadingState}
                fetchData={fetchData}
                data={data}
                page={page}
                perPage={perPage}
            />
        </PageContent>
    </Page>;
};

CommandsIndex.getInitialProps = async (context): Promise<Props> => {
    const { page, perPage } = getPaginationParams(context.query, defaultPageSize);
    return {
        data: (await api.getCommands(page, perPage)).data
    }
};

export default CommandsIndex;
