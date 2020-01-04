use std::cmp::max;

use serde::Deserialize;
use validator::Validate;
use validator_derive::Validate;

use persistence::OffsetParameters;

#[derive(Debug, Clone, Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct PaginationParams {
    #[validate(range(min = 1))]
    #[serde(default = "default_page")]
    pub page: u32,
    #[validate(range(min = 1))]
    #[serde(default = "default_per_page")]
    pub per_page: u32,
}

const fn default_page() -> u32 {
    1
}

const fn default_per_page() -> u32 {
    20
}

impl PaginationParams {
    pub fn as_offset(&self) -> OffsetParameters {
        debug_assert!(self.page > 0);
        debug_assert!(self.per_page > 0);
        let offset = (max(self.page, 1) - 1) * self.per_page;
        OffsetParameters::new(offset, self.per_page)
    }
}
