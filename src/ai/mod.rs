mod gemini;
pub(crate) mod openai;
pub(crate) mod prompts;

use crate::types::{CommitType, L402Config, MessageStyle, ModelType};

pub struct GenerateOptions<'a> {
    pub diff: &'a str,
    pub commit_type: Option<CommitType>,
    pub files: &'a [String],
    pub branch_name: &'a str,
    pub message_style: MessageStyle,
    pub l402_config: L402Config,
}

pub async fn generate_commit_message(
    model: ModelType,
    options: GenerateOptions<'_>,
) -> anyhow::Result<String> {
    if options.l402_config.enabled {
        return self::openai::generate_commit_message(
            options.diff,
            options.commit_type,
            options.files,
            options.branch_name,
            options.message_style,
            &options.l402_config,
        )
        .await;
    }

    match model {
        ModelType::Gemini => {
            self::gemini::generate_commit_message(
                options.diff,
                options.commit_type,
                options.files,
                options.branch_name,
                options.message_style,
            )
            .await
        }
        ModelType::Openai => {
            self::openai::generate_commit_message(
                options.diff,
                options.commit_type,
                options.files,
                options.branch_name,
                options.message_style,
                &options.l402_config,
            )
            .await
        }
    }
}

pub enum DynamicLnBackend {
    Lnd(l402_lnd::LndRestBackend),
    Nwc(Box<l402_nwc::NwcBackend>),
}

use async_trait::async_trait;

#[async_trait]
impl l402_proto::port::LnBackend for DynamicLnBackend {
    async fn pay_invoice(
        &self,
        bolt11: &str,
        max_fee_sats: u64,
    ) -> Result<l402_proto::port::PaymentResult, l402_proto::ClientError> {
        match self {
            Self::Lnd(b) => b.pay_invoice(bolt11, max_fee_sats).await,
            Self::Nwc(b) => b.pay_invoice(bolt11, max_fee_sats).await,
        }
    }

    async fn get_balance(&self) -> Result<u64, l402_proto::ClientError> {
        match self {
            Self::Lnd(b) => b.get_balance().await,
            Self::Nwc(b) => b.get_balance().await,
        }
    }

    async fn get_info(&self) -> Result<l402_proto::port::NodeInfo, l402_proto::ClientError> {
        match self {
            Self::Lnd(b) => b.get_info().await,
            Self::Nwc(b) => b.get_info().await,
        }
    }
}
