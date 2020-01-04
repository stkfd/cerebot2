#[derive(Debug, Clone)]
pub struct OffsetParameters {
    offset: u32,
    limit: u32,
}

impl OffsetParameters {
    #[inline]
    pub fn new(offset: u32, limit: u32) -> OffsetParameters {
        OffsetParameters { offset, limit }
    }

    #[inline]
    pub fn offset(&self) -> u32 {
        self.offset
    }

    #[inline]
    pub fn limit(&self) -> u32 {
        self.limit
    }
}
