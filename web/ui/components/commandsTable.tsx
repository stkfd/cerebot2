/* eslint-disable react/jsx-key */
import {PaginationParams, visualizeBool} from "../lib/util";
import * as React from "react";
import {usePagination, useTable} from "react-table";
import {CommandAttributes, CommandList} from "../api-client";
import Pagination from "./pagination";
import {useRouter} from "next/router";
import {useInitialRender} from "../hooks/skipFirstRun";
import {LoadingState} from "../hooks/loading";

const columns = [
    {
        Header: 'Alias',
        accessor: 'aliases',
    },
    {
        Header: 'Description',
        accessor: 'description',
    },
    {
        Header: 'Cooldown',
        accessor: 'cooldown',
    },
    {
        Header: 'Handler',
        accessor: 'handlerName',
    },
    {
        Header: 'Enabled',
        accessor: 'enabled',
    },
    {
        Header: 'Whisper',
        accessor: 'whisperEnabled',
    },
];

interface CommandsTableProps extends PaginationParams {
    loading: LoadingState;
    data: CommandList;
    fetchData: (pagination: PaginationParams) => Promise<void>;
}

export interface MappedCommandAttributes {
    id?: number;
    description?: string;
    handlerName?: string;
    enabled?: string;
    defaultActive?: string;
    cooldown?: string;
    whisperEnabled?: string;
    aliases?: string;
}

function mapData(data: Array<CommandAttributes>): Array<MappedCommandAttributes> {
    return data.map(command => ({
        id: command.id,
        description: command.description,
        handlerName: command.handlerName,
        enabled: visualizeBool(command.enabled),
        defaultActive: visualizeBool(command.defaultActive),
        cooldown: command.cooldown ? `${command.cooldown / 1000}s` : undefined,
        whisperEnabled: visualizeBool(command.whisperEnabled),
        aliases: command.aliases?.join(", ")
    }));
}

const CommandsTable: React.FunctionComponent<CommandsTableProps> = (props) => {
    const initialRender = useInitialRender();
    const router = useRouter();
    const {fetchData, page: pageIndexQuery, perPage: pageSizeQuery} = props;
    const {
        getTableProps,
        getTableBodyProps,
        headers,
        prepareRow,
        page,
        canPreviousPage,
        canNextPage,
        pageCount,
        setPageSize,
        gotoPage,
        state: { pageIndex, pageSize }
    } = useTable(
        {
            columns,
            data: mapData(props.data.items),
            manualPagination: true,
            pageCount: props.data.pageCount,
            initialState: { pageIndex: pageIndexQuery, pageSize: pageSizeQuery }
        },
        usePagination
    );

    React.useEffect(() => {
        if (pageIndexQuery !== pageIndex || pageSizeQuery !== pageSize) {
            gotoPage(pageIndexQuery);
            setPageSize(pageSizeQuery);
        }
    }, [pageIndexQuery, pageSizeQuery]);

    React.useEffect(() => {
        if (!initialRender) fetchData({page: pageIndex, perPage: pageSize });
    }, [pageIndex, pageSize]);

    const updatePageSize = (newPageSize: number): void => {
        router.push({pathname: router.pathname, query: { ...router.query, page: 0, perPage: newPageSize}})
    };

    return <div>
        <Pagination
            pageIndex={pageIndex}
            pageSize={pageSize}
            pageCount={pageCount}
            canPreviousPage={canPreviousPage}
            canNextPage={canNextPage}
            setPageSize={updatePageSize}
        />
        <table className="table-auto w-full" {...getTableProps()}>
            <thead>
                <tr>
                    {headers.map(column => (
                        <th {...column.getHeaderProps()}>{column.render('Header')}</th>
                    ))}
                </tr>
            </thead>
            <tbody {...getTableBodyProps()}>
                {page.map((row) => {
                    prepareRow(row);
                    return (
                        <tr {...row.getRowProps()}>
                            {row.cells.map(cell => {
                                return <td {...cell.getCellProps()}>{cell.render('Cell')}</td>
                            })}
                        </tr>
                    )
                })}
            </tbody>
        </table>
    </div>;
};

export default CommandsTable;