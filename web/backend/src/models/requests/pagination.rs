use std::cmp::max;

use serde::Deserialize;
use validator::Validate;
use validator_derive::Validate;

use persistence::OffsetParameters;

#[derive(Debug, Clone, Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct PaginationParams {
    #[serde(default = "default_page")]
    pub page: u32,
    #[serde(default = "default_per_page")]
    pub per_page: u32,
}

const fn default_page() -> u32 {
    0
}

const fn default_per_page() -> u32 {
    20
}

impl PaginationParams {
    pub fn as_offset(&self) -> OffsetParameters {
        let offset = self.page * self.per_page;
        OffsetParameters::new(offset, self.per_page)
    }
}
