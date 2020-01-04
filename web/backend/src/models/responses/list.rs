use serde::Serialize;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListResponse<T> {
    page: u32,
    page_count: u32,
    total_count: u64,
    items: Vec<T>,
}

impl<T> ListResponse<T>
where
    T: Serialize,
{
    pub fn new(items: Vec<T>, total: u64, page: u32, per_page: u32) -> ListResponse<T> {
        ListResponse {
            page,
            page_count: (total as f64 / per_page as f64).ceil() as u32,
            total_count: total,
            items,
        }
    }
}
