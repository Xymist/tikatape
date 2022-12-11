use std::collections::HashMap;

use color_eyre::eyre::eyre;
use color_eyre::eyre::Result;
use reqwest::Url;

use tracing::instrument;

use crate::Input;

/// Use a Tika server instance, hosted elsewhere. This avoids shelling out
/// and the startup time for the JVM, at the cost of networking and possible
/// infrastructure complexity increase.
#[derive(Debug)]
pub struct RemoteClient {
    url: Url,
    input: Option<Input>,
    last_output: Option<HashMap<String, serde_json::Value>>,
    client: reqwest::Client,
}

impl RemoteClient {
    pub fn new(url: Url, input: Option<Input>) -> Self {
        RemoteClient {
            url,
            input,
            last_output: None,
            client: reqwest::Client::new(),
        }
    }

    pub fn input(&mut self, input: Input) {
        self.input = Some(input);
        self.last_output = None;
    }

    #[instrument]
    pub async fn html(&self) -> Result<String> {
        self.request("tika/html")
            .await?
            .get("X-TIKA:content")
            .and_then(|content| content.as_str())
            .map(ToOwned::to_owned)
            .ok_or(eyre!("Missing or empty content"))
    }

    #[instrument]
    pub async fn text(&self) -> Result<String> {
        self.request("tika/text")
            .await?
            .get("X-TIKA:content")
            .and_then(|content| content.as_str())
            .map(ToOwned::to_owned)
            .ok_or(eyre!("Missing or empty content"))
    }

    #[instrument]
    pub async fn mimetype(&self) -> Result<String> {
        self.request("meta")
            .await?
            .get("Content-Type")
            .and_then(|content| content.as_str())
            .map(ToOwned::to_owned)
            .ok_or(eyre!("Missing or empty content"))
    }

    #[instrument]
    pub async fn metadata(&self) -> Result<HashMap<String, serde_json::Value>> {
        self.request("meta").await
    }

    #[instrument]
    async fn input_data(&self) -> Result<Vec<u8>> {
        if let Some(ref ipt) = self.input {
            Ok(match ipt {
                Input::FilePath(filepath) => tokio::fs::read(filepath).await?,
                Input::Url(url) => self
                    .client
                    .get(url.clone())
                    .send()
                    .await?
                    .bytes()
                    .await?
                    .into(),
            })
        } else {
            Err(eyre!("Input not set"))
        }
    }

    #[instrument]
    async fn request(&self, path: &str) -> Result<HashMap<String, serde_json::Value>> {
        Ok(serde_json::from_str(
            self.client
                .put(self.url.join(path)?)
                .header("Accept", "application/json")
                .body(self.input_data().await?)
                .send()
                .await?
                .text()
                .await?
                .as_str(),
        )?)
    }
}
