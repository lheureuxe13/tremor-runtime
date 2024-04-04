// Copyright 2021, The Tremor Team
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

mod sink;

use crate::{
    impls::gbq::writer::sink::{GbqSink, TonicChannelFactory},
    prelude::*,
    utils::google::{GouthTokenProvider, TokenSrc},
};

#[derive(Deserialize, Clone)]
pub(crate) struct Config {
    pub table_id: String,
    pub connect_timeout: u64,
    pub request_timeout: u64,
    /// Token to use for authentication
    pub token: TokenSrc,
    #[serde(default = "default_request_size_limit")]
    pub request_size_limit: usize,
}
impl tremor_config::Impl for Config {}

fn default_request_size_limit() -> usize {
    // 10MB
    10 * 1024 * 1024
}

#[derive(Debug, Default)]
pub(crate) struct Builder {}

struct Gbq {
    config: Config,
}

#[async_trait::async_trait]
impl Connector for Gbq {
    async fn create_sink(
        &mut self,
        ctx: SinkContext,
        builder: SinkManagerBuilder,
    ) -> anyhow::Result<Option<SinkAddr>> {
        let sink = GbqSink::<GouthTokenProvider, _, _>::new(
            self.config.clone(),
            Box::new(TonicChannelFactory),
        );

        Ok(Some(builder.spawn(sink, ctx)))
    }

    fn codec_requirements(&self) -> CodecReq {
        CodecReq::Structured
    }
}

#[async_trait::async_trait]
impl ConnectorBuilder for Builder {
    fn connector_type(&self) -> ConnectorType {
        "gbq".into()
    }

    async fn build_cfg(
        &self,
        _: &alias::Connector,
        _: &ConnectorConfig,
        config: &Value,
        _kill_switch: &KillSwitch,
    ) -> anyhow::Result<Box<dyn Connector>> {
        let config = Config::new(config)?;
        Ok(Box::new(Gbq { config }))
    }
}

#[cfg(test)]
mod tests {
    use tokio::sync::broadcast;
    use tremor_common::ids::SinkId;

    use super::*;
    use crate::reconnect::ConnectionLostNotifier;
    use crate::sink::builder;
    use crate::{metrics::SinkReporter, utils::quiescence::QuiescenceBeacon};

    #[tokio::test(flavor = "multi_thread")]
    pub async fn can_spawn_sink() -> anyhow::Result<()> {
        let mut connector = Gbq {
            config: Config {
                table_id: "test".into(),
                connect_timeout: 1,
                request_timeout: 1,
                request_size_limit: 10 * 1024 * 1024,
                token: TokenSrc::dummy(),
            },
        };
        let alias = alias::Connector::new("a", "b");

        let sink_address = connector
            .create_sink(
                SinkContext::new(
                    SinkId::default(),
                    alias.clone(),
                    ConnectorType::default(),
                    QuiescenceBeacon::default(),
                    ConnectionLostNotifier::new(&alias, crate::channel::bounded(128).0),
                ),
                builder(
                    &ConnectorConfig::default(),
                    CodecReq::Structured,
                    &alias,
                    SinkReporter::new(alias.clone(), broadcast::channel(1).0, None),
                )?,
            )
            .await?;
        assert!(sink_address.is_some());
        Ok(())
    }
}