use std::fmt;
use std::str::FromStr;

use reqwest::header::{HeaderMap, HeaderValue, InvalidHeaderValue};
use serde::{Deserialize, Deserializer};
use thiserror::Error;

use crate::genre_ids::Genre;
use std::convert::TryFrom;

const BASE_URL: &str = "https://unogs-unogs-v1.p.rapidapi.com/api.cgi";

pub struct UnogsClient {
    client: reqwest::Client,
}

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Error)]
pub enum Error {
    #[error("{0}")]
    InvalidRequestHeader(#[from] InvalidHeaderValue),
    #[error("{0}")]
    RequestError(#[from] reqwest::Error),
    #[error("Invalid or missing quota header in response")]
    InvalidQuotaHeader,
}

impl UnogsClient {
    pub fn new(rapidapi_key: &str) -> Result<Self> {
        let mut default_headers = HeaderMap::new();
        default_headers.insert("x-rapidapi-key", HeaderValue::from_str(rapidapi_key)?);
        default_headers.insert(
            "x-rapidapi-host",
            HeaderValue::from_static("unogs-unogs-v1.p.rapidapi.com"),
        );
        Ok(UnogsClient {
            client: reqwest::ClientBuilder::new()
                .default_headers(default_headers)
                .build()?,
        })
    }

    pub async fn genre_ids(&self) -> Result<UnogsResponse<List<Genre>>> {
        let response = self
            .client
            .get(BASE_URL)
            .query(&[("t", "genres")])
            .send()
            .await?;
        let quota = QuotaState::try_from(response.headers())?;
        Ok(UnogsResponse {
            content: response.json::<List<Genre>>().await?,
            quota,
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct UnogsResponse<T> {
    pub content: T,
    pub quota: QuotaState,
}

/// State of the RapidAPI quotas after the request
#[derive(Debug, Clone, PartialEq)]
pub struct QuotaState {
    /// Total allowed requests in the current period
    requests_limit: isize,
    /// Remaining allowed requests before shutoff or overage charges
    requests_remaining: isize,
}

impl TryFrom<&HeaderMap> for QuotaState {
    type Error = Error;

    fn try_from(headers: &HeaderMap<HeaderValue>) -> Result<Self> {
        Ok(QuotaState {
            requests_limit: isize::from_str(
                headers
                    .get("x-ratelimit-requests-limit")
                    .ok_or(Error::InvalidQuotaHeader)?
                    .to_str()
                    .map_err(|_| Error::InvalidQuotaHeader)?,
            )
            .map_err(|_| Error::InvalidQuotaHeader)?,
            requests_remaining: isize::from_str(
                headers
                    .get("x-ratelimit-requests-remaining")
                    .ok_or_else(|| Error::InvalidQuotaHeader)?
                    .to_str()
                    .map_err(|_| Error::InvalidQuotaHeader)?,
            )
            .map_err(|_| Error::InvalidQuotaHeader)?,
        })
    }
}

/// Generic list of items in a response
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub struct List<T> {
    /// Number of items in the response
    #[serde(deserialize_with = "from_str")]
    count: usize,
    /// Response items
    items: Vec<T>,
}

pub mod genre_ids;

fn from_str<'de, T, D>(deserializer: D) -> std::result::Result<T, D::Error>
where
    T: FromStr,
    T::Err: fmt::Display,
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    T::from_str(&s).map_err(serde::de::Error::custom)
}
