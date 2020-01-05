import {ParsedUrlQuery} from "querystring";

export interface PaginationParams {
    page: number,
    perPage: number
}

/**
 * Extract pagination parameters from URL query parameters
 * @param query
 * @param defaultPageSize
 */
export function getPaginationParams(query: ParsedUrlQuery, defaultPageSize: number): PaginationParams {
    return {
        page: (typeof query.page === 'string') ? parseInt(query.page) || 0 : 0,
        perPage: (typeof query.perPage === 'string') ? parseInt(query.perPage) || defaultPageSize : defaultPageSize,
    }
}

export function visualizeBool(b?: boolean): string {
    return b ? "\u2b24" : "\u25cb";
}