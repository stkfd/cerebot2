import {UsePaginationInstanceProps, UsePaginationState} from "react-table";
import {FunctionComponent} from "react";
import {useRouter} from "next/router";
import Link from "next/link";
import "./pagination.css"

type PaginationProps = UsePaginationState<any> & Pick<UsePaginationInstanceProps<any>, "pageCount" | "canPreviousPage" | "canNextPage" | "setPageSize">;

const Pagination: FunctionComponent<PaginationProps> = (
    {
        pageIndex,
        pageSize,
        pageCount,
        canPreviousPage,
        canNextPage,
        setPageSize
    }
) => {
    const router = useRouter();

    return <div className="pagination">
        {canPreviousPage ?
            <>
                <Link href={{pathname: router.pathname, query: {...router.query, page: pageIndex - 1}}}><a>‹</a></Link>
                { pageIndex >= 2 ? <Link href={{pathname: router.pathname, query: {...router.query, page: pageIndex - 2}}}><a>{pageIndex - 1}</a></Link> : null }
                <Link href={{pathname: router.pathname, query: {...router.query, page: pageIndex - 1}}}><a>{pageIndex}</a></Link>
            </>
            :
            <a>‹</a>
        }
        <a className="current">{pageIndex + 1}</a>
        {canNextPage ?
            <>
                <Link href={{pathname: router.pathname, query: {...router.query, page: pageIndex + 1}}}><a>{pageIndex + 2}</a></Link>
                { pageCount - pageIndex >= 3 ? <Link href={{pathname: router.pathname, query: {...router.query, page: pageIndex + 2}}}><a>{pageIndex + 3}</a></Link> : null }
                <Link href={{pathname: router.pathname, query: {...router.query, page: pageIndex + 1}}}><a>›</a></Link>
            </>
            :
            <a>›</a>
        }
        <label className="page-size-label">Per Page:</label>
        <select className="page-size"
                value={pageSize}
                onChange={(event): void => setPageSize(parseInt(event.currentTarget.value))}>
            <option>10</option>
            <option>25</option>
            <option>50</option>
        </select>
    </div>;
};

export default Pagination;