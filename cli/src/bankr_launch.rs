use reqwest::{Client, StatusCode, Url};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fmt;
use std::fs::{self, OpenOptions};
use std::io::Write as _;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tohseno_engine::Ledger;
use tohseno_protocol::canonical;
use tohseno_protocol::digest::ShotId;
use tokio::sync::Mutex;
use zeroize::Zeroizing;

const BANKR_DEPLOY_URL: &str = "https://api.bankr.bot/token-launches/deploy";
/// An Appcoin belongs to one Shot, so its identity is derived from that
/// Shot's name rather than fixed for the whole factory.
const MAX_TOKEN_SYMBOL: usize = 10;
const APPROVAL_LIFETIME: Duration = Duration::from_secs(10 * 60);
const MAX_BANKR_RESPONSE: usize = 1024 * 1024;

#[derive(Clone)]
pub struct BankrLaunchService {
    client: Client,
    configuration: Arc<Configuration>,
    pending: Arc<Mutex<Option<PendingApproval>>>,
}

struct Configuration {
    api_key: Option<Zeroizing<String>>,
    configuration_error: Option<String>,
    deploy_enabled: bool,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum LaunchChain {
    Robinhood,
    Base,
}

impl LaunchChain {
    pub fn chain_id(self) -> u64 {
        match self {
            Self::Robinhood => 4663,
            Self::Base => 8453,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Robinhood => "robinhood",
            Self::Base => "base",
        }
    }

    fn display(self) -> &'static str {
        match self {
            Self::Robinhood => "ROBINHOOD",
            Self::Base => "BASE",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CreatorVesting {
    FifteenPercent,
    None,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CreatorFeeMode {
    Mixed,
    QuoteOnly,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum FeeRecipientKind {
    Wallet,
    Ens,
    X,
    Farcaster,
}

impl FeeRecipientKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Wallet => "wallet",
            Self::Ens => "ens",
            Self::X => "x",
            Self::Farcaster => "farcaster",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FeeRecipient {
    #[serde(rename = "type")]
    pub kind: FeeRecipientKind,
    pub value: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ShotLaunchBinding {
    pub app_name: String,
    pub shot_id: String,
    pub version_ordinal: u64,
}

impl ShotLaunchBinding {
    /// The Appcoin's human-facing name: the Shot's own name, unchanged.
    pub fn token_name(&self) -> String {
        self.app_name.clone()
    }

    /// The Appcoin's ticker: the Shot's name in upper case with separators
    /// removed, bounded to a length exchanges and explorers accept.
    pub fn token_symbol(&self) -> String {
        self.app_name
            .chars()
            .filter(|character| character.is_ascii_alphanumeric())
            .take(MAX_TOKEN_SYMBOL)
            .collect::<String>()
            .to_ascii_uppercase()
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LaunchParameters {
    pub description: String,
    #[serde(default)]
    pub image: Option<String>,
    #[serde(default)]
    pub tweet_url: Option<String>,
    #[serde(default)]
    pub website_url: Option<String>,
    pub chain: LaunchChain,
    pub creator_vesting: CreatorVesting,
    pub creator_fee_mode: CreatorFeeMode,
    pub fee_recipient: FeeRecipient,
    /// Robinhood-issued tokenized stock ticker the new pool is quoted in
    /// (e.g. "AAPL"). Absent means Bankr's standard WETH pairing.
    #[serde(default)]
    pub paired_stock: Option<String>,
    /// The tokenized stock's Robinhood Chain contract address. Bankr's deploy
    /// API identifies the pairing by address; the ticker names the intent and
    /// both must survive the simulation echo together.
    #[serde(default)]
    pub paired_stock_address: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeployApprovalRequest {
    pub approval_id: String,
    pub confirmation: String,
    pub shot: ShotLaunchBinding,
}

#[derive(Debug, Serialize)]
pub struct BankrLaunchStatus {
    pub configured: bool,
    pub configuration_error: Option<String>,
    pub deploy_enabled: bool,
    pub signer: &'static str,
    /// The Appcoin identity is per Shot, so the global status states the
    /// rule rather than a name that would be wrong for every Shot.
    pub token_identity: &'static str,
    pub accepts_session_key: bool,
    pub fee_recipient_rule: &'static str,
    pub supported_chains: [&'static str; 2],
    pub key_setup_url: &'static str,
}

#[derive(Debug, Serialize)]
pub struct SimulationApproval {
    pub approval_id: String,
    pub expires_in_seconds: u64,
    pub configuration_digest: String,
    pub confirmation_phrase: String,
    pub token_name: String,
    pub token_symbol: String,
    pub fee_recipient: FeeRecipient,
    pub fee_recipient_address: String,
    pub signer: &'static str,
    pub shot: ShotLaunchBinding,
    pub parameters: LaunchParameters,
    pub bankr_simulation: Value,
}

#[derive(Debug, Serialize)]
pub struct ShotAssociationEvidence {
    pub action_commitment: String,
    pub lineage_head: String,
    pub availability: &'static str,
    pub outbox_path: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct DeploymentOutcome {
    pub deployed: bool,
    pub token_name: String,
    pub token_symbol: String,
    pub fee_recipient: FeeRecipient,
    pub fee_recipient_address: String,
    pub signer: &'static str,
    pub shot: ShotLaunchBinding,
    pub parameters: LaunchParameters,
    pub simulated_token_address: String,
    pub bankr_deployment: Value,
    pub receipt_path: Option<String>,
    pub shot_association: Option<ShotAssociationEvidence>,
    pub warnings: Vec<String>,
}

struct PendingApproval {
    approval_id: String,
    expires_at: Instant,
    configuration_digest: String,
    confirmation_phrase: String,
    shot: ShotLaunchBinding,
    parameters: LaunchParameters,
    simulated_token_address: String,
    simulated_fee_recipient_address: String,
    api_key: Zeroizing<String>,
}

#[derive(Debug)]
pub struct BankrLaunchError {
    pub message: String,
    pub uncertain_deployment_outcome: bool,
}

impl BankrLaunchError {
    fn definite(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            uncertain_deployment_outcome: false,
        }
    }

    fn uncertain(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            uncertain_deployment_outcome: true,
        }
    }
}

impl fmt::Display for BankrLaunchError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for BankrLaunchError {}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BankrDeployPayload<'a> {
    token_name: &'a str,
    token_symbol: &'a str,
    description: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    image: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tweet_url: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    website_url: Option<&'a str>,
    chain: &'static str,
    fee_recipient: BankrFeeRecipient<'a>,
    #[serde(skip_serializing_if = "Option::is_none")]
    paired_stock_address: Option<&'a str>,
    disable_vesting: bool,
    quote_only_fees: bool,
    simulate_only: bool,
}

#[derive(Serialize)]
struct BankrFeeRecipient<'a> {
    #[serde(rename = "type")]
    kind: &'static str,
    value: &'a str,
}

#[derive(Serialize)]
struct LaunchCommitment<'a> {
    schema: &'static str,
    token_name: &'a str,
    token_symbol: &'a str,
    shot: &'a ShotLaunchBinding,
    parameters: &'a LaunchParameters,
}

#[derive(Serialize)]
struct StoredReceipt<'a> {
    schema: &'static str,
    recorded_at_unix_ms: u128,
    token_name: &'a str,
    token_symbol: &'a str,
    signer: &'static str,
    fee_recipient: &'a FeeRecipient,
    fee_recipient_address: &'a str,
    shot: &'a ShotLaunchBinding,
    configuration_digest: &'a str,
    simulated_token_address: &'a str,
    parameters: &'a LaunchParameters,
    bankr_deployment: &'a Value,
    verification_warnings: &'a [String],
}

impl BankrLaunchService {
    pub fn from_environment() -> Result<Self, BankrLaunchError> {
        let observed = std::env::var("BANKR_API_KEY").ok();
        let (api_key, configuration_error) = match observed {
            None => (None, None),
            Some(value) if value.starts_with("bk_usr_") && value.len() >= 16 => {
                (Some(Zeroizing::new(value)), None)
            }
            Some(_) => (
                None,
                Some(
                    "BANKR_API_KEY must be a Bankr user key beginning with bk_usr_; partner keys are intentionally not accepted by this personal launch surface."
                        .to_owned(),
                ),
            ),
        };
        let client = Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(90))
            .user_agent(format!("tohseno-studio/{}", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|error| {
                BankrLaunchError::definite(format!("Bankr HTTP client could not start: {error}"))
            })?;
        Ok(Self {
            client,
            configuration: Arc::new(Configuration {
                api_key,
                configuration_error,
                deploy_enabled: std::env::var("TOHSENO_ALLOW_BANKR_TOKEN_DEPLOY")
                    .is_ok_and(|value| value == "1"),
            }),
            pending: Arc::new(Mutex::new(None)),
        })
    }

    pub fn status(&self) -> BankrLaunchStatus {
        BankrLaunchStatus {
            configured: self.configuration.api_key.is_some(),
            configuration_error: self.configuration.configuration_error.clone(),
            deploy_enabled: self.configuration.deploy_enabled,
            signer: "Bankr wallet that owns BANKR_API_KEY",
            token_identity: "Each Appcoin takes the name and ticker of its own Shot.",
            accepts_session_key: true,
            fee_recipient_rule: "The person launching chooses an ENS name, wallet, X account, or Farcaster account; Bankr resolves and pins it during simulation.",
            supported_chains: ["robinhood", "base"],
            key_setup_url: "https://bankr.bot/api-keys",
        }
    }

    pub async fn cancel_pending(&self) {
        *self.pending.lock().await = None;
    }

    pub async fn simulate(
        &self,
        shot: ShotLaunchBinding,
        mut parameters: LaunchParameters,
        supplied_api_key: Option<String>,
    ) -> Result<SimulationApproval, BankrLaunchError> {
        normalize_and_validate(&mut parameters)?;
        let api_key = self.resolve_api_key(supplied_api_key)?;
        let bankr_simulation = self
            .call_bankr(&shot, &parameters, true, api_key.as_str())
            .await?;
        let fee_recipient_address = verify_simulation(
            &bankr_simulation,
            parameters.chain,
            &parameters.fee_recipient,
            parameters
                .paired_stock
                .as_deref()
                .zip(parameters.paired_stock_address.as_deref()),
        )?;
        let simulated_token_address = required_string(
            &bankr_simulation,
            &["tokenAddress"],
            "predicted token address",
        )?
        .to_owned();
        let configuration_digest = configuration_digest(&shot, &parameters)?;
        let confirmation_phrase = confirmation_phrase(
            &shot,
            parameters.chain,
            &simulated_token_address,
            &parameters.fee_recipient,
            parameters.paired_stock.as_deref(),
        );
        let approval_id = ShotId::random().to_string();
        *self.pending.lock().await = Some(PendingApproval {
            approval_id: approval_id.clone(),
            expires_at: Instant::now() + APPROVAL_LIFETIME,
            configuration_digest: configuration_digest.clone(),
            confirmation_phrase: confirmation_phrase.clone(),
            shot: shot.clone(),
            parameters: parameters.clone(),
            simulated_token_address,
            simulated_fee_recipient_address: fee_recipient_address.clone(),
            api_key,
        });
        Ok(SimulationApproval {
            approval_id,
            expires_in_seconds: APPROVAL_LIFETIME.as_secs(),
            configuration_digest,
            confirmation_phrase,
            token_name: shot.token_name(),
            token_symbol: shot.token_symbol(),
            fee_recipient: parameters.fee_recipient.clone(),
            fee_recipient_address,
            signer: "Bankr wallet that owns BANKR_API_KEY",
            shot,
            parameters,
            bankr_simulation,
        })
    }

    pub async fn deploy(
        &self,
        request: DeployApprovalRequest,
    ) -> Result<DeploymentOutcome, BankrLaunchError> {
        if !self.configuration.deploy_enabled {
            return Err(BankrLaunchError::definite(
                "deployment is locked; restart Studio with TOHSENO_ALLOW_BANKR_TOKEN_DEPLOY=1 after reviewing the simulation",
            ));
        }
        let approval = {
            let mut pending = self.pending.lock().await;
            let approval = pending.take().ok_or_else(|| {
                BankrLaunchError::definite(
                    "no unused Bankr simulation approval exists; simulate again",
                )
            })?;
            if approval.approval_id != request.approval_id {
                return Err(BankrLaunchError::definite(
                    "the Bankr simulation approval does not match",
                ));
            }
            if Instant::now() > approval.expires_at {
                return Err(BankrLaunchError::definite(
                    "the Bankr simulation approval expired; simulate again",
                ));
            }
            if approval.confirmation_phrase != request.confirmation {
                return Err(BankrLaunchError::definite(
                    "the exact Bankr deployment confirmation phrase was not supplied",
                ));
            }
            if approval.shot != request.shot {
                return Err(BankrLaunchError::definite(
                    "the selected Shot does not match the Bankr simulation approval",
                ));
            }
            if configuration_digest(&approval.shot, &approval.parameters)?
                != approval.configuration_digest
            {
                return Err(BankrLaunchError::definite(
                    "the approved Bankr configuration changed; simulate again",
                ));
            }
            approval
        };

        let bankr_deployment = self
            .call_bankr(
                &approval.shot,
                &approval.parameters,
                false,
                approval.api_key.as_str(),
            )
            .await?;
        let mut warnings = verify_deployment(
            &bankr_deployment,
            approval.parameters.chain,
            &approval.simulated_token_address,
            &approval.simulated_fee_recipient_address,
            approval
                .parameters
                .paired_stock
                .as_deref()
                .zip(approval.parameters.paired_stock_address.as_deref()),
        );
        let receipt_path = match persist_receipt(&approval, &bankr_deployment, &warnings) {
            Ok(path) => Some(path.display().to_string()),
            Err(error) => {
                warnings.push(format!(
                    "The token deployed, but Studio could not persist its local receipt: {error}"
                ));
                None
            }
        };
        Ok(DeploymentOutcome {
            deployed: true,
            token_name: approval.shot.token_name(),
            token_symbol: approval.shot.token_symbol(),
            fee_recipient: approval.parameters.fee_recipient.clone(),
            fee_recipient_address: approval.simulated_fee_recipient_address,
            signer: "Bankr wallet that owns BANKR_API_KEY",
            shot: approval.shot,
            parameters: approval.parameters,
            simulated_token_address: approval.simulated_token_address,
            bankr_deployment,
            receipt_path,
            shot_association: None,
            warnings,
        })
    }

    async fn call_bankr(
        &self,
        shot: &ShotLaunchBinding,
        parameters: &LaunchParameters,
        simulate_only: bool,
        api_key: &str,
    ) -> Result<Value, BankrLaunchError> {
        let token_name = shot.token_name();
        let token_symbol = shot.token_symbol();
        let payload = bankr_payload(&token_name, &token_symbol, parameters, simulate_only);
        let sent = self
            .client
            .post(BANKR_DEPLOY_URL)
            .header("X-API-Key", api_key)
            .header("Accept", "application/json")
            .json(&payload)
            .send()
            .await;
        let response = match sent {
            Ok(response) => response,
            Err(error) if simulate_only => {
                return Err(BankrLaunchError::definite(format!(
                    "Bankr simulation could not be reached: {error}"
                )))
            }
            Err(_) => {
                return Err(BankrLaunchError::uncertain(
                    format!(
                        "Bankr's deployment response was not received. Do not retry automatically: first inspect Bankr's recent launches for ${token_symbol} because the request may have reached Bankr."
                    ),
                ))
            }
        };
        let status = response.status();
        let body = bounded_body(response).await.map_err(|error| {
            if simulate_only {
                BankrLaunchError::definite(error)
            } else {
                BankrLaunchError::uncertain(format!(
                    "{error} Do not retry until Bankr's recent launches have been checked."
                ))
            }
        })?;
        let decoded = serde_json::from_slice::<Value>(&body).map_err(|_| {
            let message = if simulate_only {
                "Bankr returned a non-JSON simulation response"
            } else {
                "Bankr returned a non-JSON deployment response; do not retry until recent launches have been checked"
            };
            if simulate_only {
                BankrLaunchError::definite(message)
            } else {
                BankrLaunchError::uncertain(message)
            }
        })?;
        let expected = if simulate_only {
            StatusCode::OK
        } else {
            StatusCode::CREATED
        };
        if status != expected {
            let detail = bankr_error_message(&decoded);
            if !simulate_only && status.is_server_error() {
                return Err(BankrLaunchError::uncertain(format!(
                    "Bankr returned {status}: {detail}. Do not retry until recent launches have been checked."
                )));
            }
            return Err(BankrLaunchError::definite(format!(
                "Bankr returned {status}: {detail}"
            )));
        }
        Ok(decoded)
    }

    fn resolve_api_key(
        &self,
        supplied_api_key: Option<String>,
    ) -> Result<Zeroizing<String>, BankrLaunchError> {
        if let Some(value) = supplied_api_key {
            let key = Zeroizing::new(value.trim().to_owned());
            validate_api_key(key.as_str())?;
            return Ok(key);
        }
        self.configuration.api_key.as_ref().cloned().ok_or_else(|| {
            BankrLaunchError::definite(
                self.configuration
                    .configuration_error
                    .as_deref()
                    .unwrap_or("enter a Bankr user API key with token-launch access"),
            )
        })
    }
}

fn validate_api_key(value: &str) -> Result<(), BankrLaunchError> {
    if !value.starts_with("bk_usr_") || value.len() < 16 || value.chars().any(char::is_whitespace) {
        return Err(BankrLaunchError::definite(
            "Bankr API key must be a user key beginning with bk_usr_",
        ));
    }
    Ok(())
}

fn normalize_and_validate(parameters: &mut LaunchParameters) -> Result<(), BankrLaunchError> {
    parameters.description = parameters.description.trim().to_owned();
    if parameters.description.is_empty() || parameters.description.chars().count() > 500 {
        return Err(BankrLaunchError::definite(
            "description must contain 1–500 characters",
        ));
    }
    normalize_optional_url("image", &mut parameters.image)?;
    normalize_optional_url("tweet_url", &mut parameters.tweet_url)?;
    normalize_optional_url("website_url", &mut parameters.website_url)?;
    normalize_fee_recipient(&mut parameters.fee_recipient)?;
    normalize_paired_stock(parameters)?;
    Ok(())
}

fn normalize_paired_stock(parameters: &mut LaunchParameters) -> Result<(), BankrLaunchError> {
    let ticker = parameters
        .paired_stock
        .take()
        .map(|observed| observed.trim().trim_start_matches('$').to_ascii_uppercase())
        .filter(|ticker| !ticker.is_empty());
    let address = parameters
        .paired_stock_address
        .take()
        .map(|observed| observed.trim().to_owned())
        .filter(|address| !address.is_empty());
    let (Some(ticker), Some(address)) = (ticker.clone(), address.clone()) else {
        if ticker.is_some() || address.is_some() {
            return Err(BankrLaunchError::definite(
                "a stock pairing needs both the ticker and its Robinhood Chain token address",
            ));
        }
        return Ok(());
    };
    if parameters.chain != LaunchChain::Robinhood {
        return Err(BankrLaunchError::definite(
            "stock pairing is available on Robinhood Chain only; choose Robinhood Chain or clear the pair",
        ));
    }
    if ticker.len() > 10
        || !ticker
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '.')
    {
        return Err(BankrLaunchError::definite(
            "paired stock must be a tokenized-stock ticker of at most 10 letters, digits, or dots (for example AAPL)",
        ));
    }
    if !is_evm_address(&address) {
        return Err(BankrLaunchError::definite(
            "paired stock address must be a 20-byte EVM address",
        ));
    }
    parameters.paired_stock = Some(ticker);
    parameters.paired_stock_address = Some(address);
    Ok(())
}

fn normalize_fee_recipient(recipient: &mut FeeRecipient) -> Result<(), BankrLaunchError> {
    recipient.value = recipient.value.trim().to_owned();
    if matches!(
        recipient.kind,
        FeeRecipientKind::X | FeeRecipientKind::Farcaster
    ) {
        recipient.value = recipient.value.trim_start_matches('@').to_owned();
    }
    if recipient.value.is_empty() || recipient.value.len() > 255 {
        return Err(BankrLaunchError::definite(
            "fee recipient must contain 1–255 characters",
        ));
    }
    match recipient.kind {
        FeeRecipientKind::Wallet if !is_evm_address(&recipient.value) => Err(
            BankrLaunchError::definite("wallet fee recipient must be a 20-byte EVM address"),
        ),
        FeeRecipientKind::Ens
            if !recipient.value.to_ascii_lowercase().ends_with(".eth")
                || recipient.value.chars().any(char::is_whitespace) =>
        {
            Err(BankrLaunchError::definite(
                "ENS fee recipient must be a .eth name without whitespace",
            ))
        }
        FeeRecipientKind::X | FeeRecipientKind::Farcaster
            if !recipient.value.chars().all(|character| {
                character.is_ascii_alphanumeric() || "_.-".contains(character)
            }) =>
        {
            Err(BankrLaunchError::definite(
                "social fee recipient contains unsupported characters",
            ))
        }
        _ => Ok(()),
    }
}

fn normalize_optional_url(
    field: &'static str,
    value: &mut Option<String>,
) -> Result<(), BankrLaunchError> {
    let Some(observed) = value.take() else {
        return Ok(());
    };
    let observed = observed.trim();
    if observed.is_empty() {
        return Ok(());
    }
    if observed.len() > 2048 {
        return Err(BankrLaunchError::definite(format!(
            "{field} must be at most 2048 bytes"
        )));
    }
    let parsed = Url::parse(observed)
        .map_err(|_| BankrLaunchError::definite(format!("{field} must be a valid HTTPS URL")))?;
    if parsed.scheme() != "https"
        || parsed.host_str().is_none()
        || !parsed.username().is_empty()
        || parsed.password().is_some()
    {
        return Err(BankrLaunchError::definite(format!(
            "{field} must be an HTTPS URL without embedded credentials"
        )));
    }
    *value = Some(parsed.to_string());
    Ok(())
}

fn bankr_payload<'a>(
    token_name: &'a str,
    token_symbol: &'a str,
    parameters: &'a LaunchParameters,
    simulate_only: bool,
) -> BankrDeployPayload<'a> {
    BankrDeployPayload {
        token_name,
        token_symbol,
        description: &parameters.description,
        image: parameters.image.as_deref(),
        tweet_url: parameters.tweet_url.as_deref(),
        website_url: parameters.website_url.as_deref(),
        chain: parameters.chain.as_str(),
        fee_recipient: BankrFeeRecipient {
            kind: parameters.fee_recipient.kind.as_str(),
            value: &parameters.fee_recipient.value,
        },
        paired_stock_address: parameters.paired_stock_address.as_deref(),
        disable_vesting: parameters.creator_vesting == CreatorVesting::None,
        quote_only_fees: parameters.creator_fee_mode == CreatorFeeMode::QuoteOnly,
        simulate_only,
    }
}

fn configuration_digest(
    shot: &ShotLaunchBinding,
    parameters: &LaunchParameters,
) -> Result<String, BankrLaunchError> {
    let token_name = shot.token_name();
    let token_symbol = shot.token_symbol();
    canonical::sha256_commitment(&LaunchCommitment {
        schema: "tohseno.bankr-launch-commitment/2",
        token_name: &token_name,
        token_symbol: &token_symbol,
        shot,
        parameters,
    })
    .map(|digest| digest.to_string())
    .map_err(|error| {
        BankrLaunchError::definite(format!(
            "launch configuration could not be committed: {error}"
        ))
    })
}

fn confirmation_phrase(
    shot: &ShotLaunchBinding,
    chain: LaunchChain,
    token_address: &str,
    fee_recipient: &FeeRecipient,
    paired_stock: Option<&str>,
) -> String {
    let pair = paired_stock
        .map(|ticker| format!("/${ticker}"))
        .unwrap_or_default();
    format!(
        "DEPLOY ${}{} FOR SHOT {} ON {} TO {}:{} AT {}",
        shot.token_symbol(),
        pair,
        shot.shot_id,
        chain.display(),
        fee_recipient.kind.as_str().to_ascii_uppercase(),
        fee_recipient.value.to_ascii_uppercase(),
        token_address
    )
}

fn verify_simulation(
    value: &Value,
    chain: LaunchChain,
    fee_recipient: &FeeRecipient,
    paired_stock: Option<(&str, &str)>,
) -> Result<String, BankrLaunchError> {
    if value.get("success").and_then(Value::as_bool) != Some(true) {
        return Err(BankrLaunchError::definite(
            "Bankr simulation did not report success",
        ));
    }
    let address = required_string(value, &["tokenAddress"], "predicted token address")?;
    if !is_evm_address(address) {
        return Err(BankrLaunchError::definite(
            "Bankr simulation returned an invalid predicted token address",
        ));
    }
    verify_chain(value, chain)?;
    verify_paired_stock(value, paired_stock)?;
    let resolved_recipient = verify_creator_recipient(
        value,
        matches!(fee_recipient.kind, FeeRecipientKind::Wallet)
            .then_some(fee_recipient.value.as_str()),
    )?;
    if value.get("txHash").and_then(Value::as_str).is_some() {
        return Err(BankrLaunchError::definite(
            "Bankr simulation unexpectedly returned a transaction hash",
        ));
    }
    Ok(resolved_recipient)
}

/// A requested stock pairing is real only when Bankr's own response names the
/// same ticker and pool quote address; an unechoed request must never reach
/// deployment as if the pairing existed.
fn verify_paired_stock(
    value: &Value,
    requested: Option<(&str, &str)>,
) -> Result<(), BankrLaunchError> {
    let observed = value.get("pairedStock").filter(|value| !value.is_null());
    match (requested, observed) {
        (None, None) => Ok(()),
        (None, Some(_)) => Err(BankrLaunchError::definite(
            "Bankr reported a stock pairing that was not requested; refusing the approval",
        )),
        (Some((ticker, _)), None) => Err(BankrLaunchError::definite(format!(
            "Bankr did not confirm the {ticker} pairing; the response names no pairedStock, so this key or ticker may not support stock-paired launches"
        ))),
        (Some((ticker, requested_address)), Some(observed)) => {
            let symbol = required_string(observed, &["symbol"], "paired stock symbol")?;
            if !symbol.eq_ignore_ascii_case(ticker) {
                return Err(BankrLaunchError::definite(format!(
                    "Bankr paired the launch with {symbol}, but {ticker} was requested"
                )));
            }
            let address = required_string(observed, &["address"], "paired stock address")?;
            if !address.eq_ignore_ascii_case(requested_address) {
                return Err(BankrLaunchError::definite(format!(
                    "Bankr paired the launch with stock token {address}, but {requested_address} was requested"
                )));
            }
            Ok(())
        }
    }
}

fn verify_deployment(
    value: &Value,
    chain: LaunchChain,
    simulated_token_address: &str,
    simulated_fee_recipient_address: &str,
    paired_stock: Option<(&str, &str)>,
) -> Vec<String> {
    let mut warnings = Vec::new();
    if value.get("success").and_then(Value::as_bool) != Some(true) {
        warnings.push("Bankr returned HTTP 201 without success=true.".to_owned());
    }
    match required_string(value, &["tokenAddress"], "deployed token address") {
        Ok(address) if !is_evm_address(address) => {
            warnings.push("Bankr returned an invalid deployed token address.".to_owned())
        }
        Ok(address) if !address.eq_ignore_ascii_case(simulated_token_address) => warnings.push(
            format!(
                "The deployed token address {address} differs from the simulated address {simulated_token_address}."
            ),
        ),
        Err(error) => warnings.push(error.message),
        _ => {}
    }
    match required_string(value, &["txHash"], "deployment transaction hash") {
        Ok(hash) if !is_bytes32(hash) => {
            warnings.push("Bankr returned an invalid deployment transaction hash.".to_owned())
        }
        Err(error) => warnings.push(error.message),
        _ => {}
    }
    if let Err(error) = verify_chain(value, chain) {
        warnings.push(error.message);
    }
    if let Err(error) = verify_paired_stock(value, paired_stock) {
        warnings.push(error.message);
    }
    if let Err(error) = verify_creator_recipient(value, Some(simulated_fee_recipient_address)) {
        warnings.push(error.message);
    }
    warnings
}

fn verify_chain(value: &Value, expected: LaunchChain) -> Result<(), BankrLaunchError> {
    let observed = required_string(value, &["chain"], "chain")?;
    if observed != expected.as_str() {
        return Err(BankrLaunchError::definite(format!(
            "Bankr returned chain {observed}, expected {}",
            expected.as_str()
        )));
    }
    Ok(())
}

fn verify_creator_recipient(
    value: &Value,
    expected_address: Option<&str>,
) -> Result<String, BankrLaunchError> {
    let observed = required_string(
        value,
        &["feeDistribution", "creator", "address"],
        "creator fee recipient",
    )?;
    if !is_evm_address(observed) {
        return Err(BankrLaunchError::definite(
            "Bankr returned an invalid creator fee recipient address",
        ));
    }
    if expected_address.is_some_and(|expected| !observed.eq_ignore_ascii_case(expected)) {
        return Err(BankrLaunchError::definite(format!(
            "Bankr returned creator fee recipient {observed}, but the approved simulation pinned {expected_address}; deployment is blocked",
            expected_address = expected_address.unwrap_or_default()
        )));
    }
    Ok(observed.to_owned())
}

fn required_string<'a>(
    value: &'a Value,
    path: &[&str],
    label: &str,
) -> Result<&'a str, BankrLaunchError> {
    let mut current = value;
    for component in path {
        current = current
            .get(*component)
            .ok_or_else(|| BankrLaunchError::definite(format!("Bankr response omitted {label}")))?;
    }
    current
        .as_str()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| BankrLaunchError::definite(format!("Bankr response has invalid {label}")))
}

fn is_evm_address(value: &str) -> bool {
    value.len() == 42
        && value.starts_with("0x")
        && value[2..].bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn is_bytes32(value: &str) -> bool {
    value.len() == 66
        && value.starts_with("0x")
        && value[2..].bytes().all(|byte| byte.is_ascii_hexdigit())
}

async fn bounded_body(mut response: reqwest::Response) -> Result<Vec<u8>, String> {
    if response
        .content_length()
        .is_some_and(|length| length > MAX_BANKR_RESPONSE as u64)
    {
        return Err("Bankr response exceeded the 1 MiB limit.".to_owned());
    }
    let mut body = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| "Bankr response body could not be read.".to_owned())?
    {
        if body.len() + chunk.len() > MAX_BANKR_RESPONSE {
            return Err("Bankr response exceeded the 1 MiB limit.".to_owned());
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

fn bankr_error_message(value: &Value) -> String {
    for field in ["message", "error", "detail"] {
        if let Some(message) = value.get(field).and_then(Value::as_str) {
            return message.chars().take(500).collect();
        }
    }
    "request was rejected without a message".to_owned()
}

fn persist_receipt(
    approval: &PendingApproval,
    bankr_deployment: &Value,
    warnings: &[String],
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let ledger = Ledger::discover()?;
    let directory = ledger
        .working_tree(&approval.shot.app_name)
        .join(".tohseno")
        .join("token-launches");
    fs::create_dir_all(&directory)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        fs::set_permissions(&directory, fs::Permissions::from_mode(0o700))?;
    }
    let token_address = required_string(
        bankr_deployment,
        &["tokenAddress"],
        "deployed token address",
    )?;
    let path = directory.join(format!(
        "{}-{}-{}.json",
        approval.shot.app_name,
        approval.parameters.chain.as_str(),
        token_address.to_ascii_lowercase()
    ));
    let token_name = approval.shot.token_name();
    let token_symbol = approval.shot.token_symbol();
    let receipt = StoredReceipt {
        schema: "tohseno.bankr-launch-receipt/2",
        recorded_at_unix_ms: SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis(),
        token_name: &token_name,
        token_symbol: &token_symbol,
        signer: "Bankr wallet that owns BANKR_API_KEY",
        fee_recipient: &approval.parameters.fee_recipient,
        fee_recipient_address: &approval.simulated_fee_recipient_address,
        shot: &approval.shot,
        configuration_digest: &approval.configuration_digest,
        simulated_token_address: &approval.simulated_token_address,
        parameters: &approval.parameters,
        bankr_deployment,
        verification_warnings: warnings,
    };
    let encoded = canonical::to_vec(&receipt)?;
    let mut options = OpenOptions::new();
    options.create_new(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o600);
    }
    let mut file = options.open(&path)?;
    file.write_all(&encoded)?;
    file.sync_all()?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shot() -> ShotLaunchBinding {
        ShotLaunchBinding {
            app_name: "anky".to_owned(),
            shot_id: "0x1111111111111111111111111111111111111111111111111111111111111111"
                .to_owned(),
            version_ordinal: 1,
        }
    }

    fn parameters() -> LaunchParameters {
        LaunchParameters {
            description: "Persistent computational identity for coherent human intentions."
                .to_owned(),
            image: Some("https://tohseno.com/token.png".to_owned()),
            tweet_url: None,
            website_url: Some("https://tohseno.com".to_owned()),
            chain: LaunchChain::Robinhood,
            creator_vesting: CreatorVesting::FifteenPercent,
            creator_fee_mode: CreatorFeeMode::QuoteOnly,
            fee_recipient: FeeRecipient {
                kind: FeeRecipientKind::Ens,
                value: "creator.eth".to_owned(),
            },
            paired_stock: None,
            paired_stock_address: None,
        }
    }

    const AAPL_ADDRESS: &str = "0xaF3D76f1834A1d425780943C99Ea8A608f8a93f9";

    #[test]
    fn payload_carries_the_shots_own_appcoin_and_selected_recipient() {
        let shot = shot();
        let payload = serde_json::to_value(bankr_payload(
            &shot.token_name(),
            &shot.token_symbol(),
            &parameters(),
            true,
        ))
        .unwrap();
        assert_eq!(payload["tokenName"], "anky");
        assert_eq!(payload["tokenSymbol"], "ANKY");
        assert_eq!(payload["feeRecipient"]["type"], "ens");
        assert_eq!(payload["feeRecipient"]["value"], "creator.eth");
        assert_eq!(payload["chain"], "robinhood");
        assert_eq!(payload["simulateOnly"], true);
        assert_eq!(payload["disableVesting"], false);
        assert_eq!(payload["quoteOnlyFees"], true);
    }

    #[test]
    fn each_shot_names_its_own_appcoin() {
        assert_eq!(shot().token_name(), "anky");
        assert_eq!(shot().token_symbol(), "ANKY");

        let mut hyphenated = shot();
        hyphenated.app_name = "field-notebook".to_owned();
        assert_eq!(hyphenated.token_name(), "field-notebook");
        assert_eq!(hyphenated.token_symbol(), "FIELDNOTEB");
        assert!(hyphenated.token_symbol().len() <= MAX_TOKEN_SYMBOL);

        // Two Shots never share one Appcoin identity.
        assert_ne!(shot().token_symbol(), hyphenated.token_symbol());
    }

    #[test]
    fn configuration_commitment_is_stable_and_shot_bound() {
        let first = configuration_digest(&shot(), &parameters()).unwrap();
        let second = configuration_digest(&shot(), &parameters()).unwrap();
        assert_eq!(first, second);
        let mut changed = parameters();
        changed.creator_fee_mode = CreatorFeeMode::Mixed;
        assert_ne!(first, configuration_digest(&shot(), &changed).unwrap());
        let mut another_shot = shot();
        another_shot.shot_id =
            "0x2222222222222222222222222222222222222222222222222222222222222222".to_owned();
        assert_ne!(
            first,
            configuration_digest(&another_shot, &parameters()).unwrap()
        );
        // The committed configuration includes the Appcoin identity itself.
        let mut renamed = shot();
        renamed.app_name = "another-shot".to_owned();
        assert_ne!(
            first,
            configuration_digest(&renamed, &parameters()).unwrap()
        );
    }

    #[test]
    fn only_https_metadata_urls_without_credentials_are_accepted() {
        let mut valid = parameters();
        normalize_and_validate(&mut valid).unwrap();
        let mut invalid = parameters();
        invalid.image = Some("http://example.com/token.png".to_owned());
        assert!(normalize_and_validate(&mut invalid).is_err());
        invalid.image = Some("https://user:password@example.com/token.png".to_owned());
        assert!(normalize_and_validate(&mut invalid).is_err());
    }

    #[test]
    fn simulation_must_resolve_the_pinned_personal_wallet() {
        let recipient = FeeRecipient {
            kind: FeeRecipientKind::Ens,
            value: "creator.eth".to_owned(),
        };
        let valid = serde_json::json!({
            "success": true,
            "tokenAddress": "0x1111111111111111111111111111111111111111",
            "chain": "robinhood",
            "feeDistribution": {
                "creator": { "address": "0xed21735DC192dC4eeAFd71b4Dc023bC53fE4DF15", "bps": 9500 }
            }
        });
        verify_simulation(&valid, LaunchChain::Robinhood, &recipient, None).unwrap();
        let mut hostile = valid;
        hostile["feeDistribution"]["creator"]["address"] =
            Value::String("not-an-address".to_owned());
        assert!(verify_simulation(&hostile, LaunchChain::Robinhood, &recipient, None).is_err());
    }

    #[test]
    fn a_requested_stock_pairing_must_be_echoed_exactly_or_the_approval_dies() {
        let unpaired = serde_json::json!({
            "success": true,
            "tokenAddress": "0x1111111111111111111111111111111111111111",
            "chain": "robinhood",
            "feeDistribution": {
                "creator": { "address": "0xed21735DC192dC4eeAFd71b4Dc023bC53fE4DF15", "bps": 9500 }
            }
        });
        let recipient = parameters().fee_recipient;
        let requested = Some(("AAPL", AAPL_ADDRESS));
        // Requested but not echoed: refused.
        assert!(
            verify_simulation(&unpaired, LaunchChain::Robinhood, &recipient, requested).is_err()
        );
        let mut paired = unpaired.clone();
        paired["pairedStock"] = serde_json::json!({
            "address": "0xaf3d76f1834a1d425780943c99ea8a608f8a93f9",
            "symbol": "AAPL"
        });
        // Echoed exactly (case-insensitive address): approved.
        verify_simulation(&paired, LaunchChain::Robinhood, &recipient, requested).unwrap();
        // Echoed with a different stock: refused.
        assert!(
            verify_simulation(
                &paired,
                LaunchChain::Robinhood,
                &recipient,
                Some(("TSLA", AAPL_ADDRESS)),
            )
            .is_err()
        );
        // Echoed with a different quote address: refused.
        assert!(
            verify_simulation(
                &paired,
                LaunchChain::Robinhood,
                &recipient,
                Some(("AAPL", "0x2222222222222222222222222222222222222222")),
            )
            .is_err()
        );
        // Pairing that was never requested: refused.
        assert!(verify_simulation(&paired, LaunchChain::Robinhood, &recipient, None).is_err());
    }

    #[test]
    fn paired_stock_is_normalized_gated_to_robinhood_and_committed() {
        let mut paired = parameters();
        paired.paired_stock = Some(" $aapl ".to_owned());
        paired.paired_stock_address = Some(AAPL_ADDRESS.to_owned());
        normalize_and_validate(&mut paired).unwrap();
        assert_eq!(paired.paired_stock.as_deref(), Some("AAPL"));
        assert_eq!(paired.paired_stock_address.as_deref(), Some(AAPL_ADDRESS));

        // The payload identifies the pairing by the stock token address.
        let payload =
            serde_json::to_value(bankr_payload("anky", "ANKY", &paired, true)).unwrap();
        assert_eq!(payload["pairedStockAddress"], AAPL_ADDRESS);
        assert!(payload.get("pairedStock").is_none());
        let unpaired = serde_json::to_value(bankr_payload("anky", "ANKY", &parameters(), true))
            .unwrap();
        assert!(unpaired.get("pairedStockAddress").is_none());

        // A ticker without its address (or the reverse) is refused.
        let mut half = parameters();
        half.paired_stock = Some("AAPL".to_owned());
        assert!(normalize_and_validate(&mut half).is_err());
        let mut other_half = parameters();
        other_half.paired_stock_address = Some(AAPL_ADDRESS.to_owned());
        assert!(normalize_and_validate(&mut other_half).is_err());

        let mut on_base = parameters();
        on_base.chain = LaunchChain::Base;
        on_base.paired_stock = Some("AAPL".to_owned());
        on_base.paired_stock_address = Some(AAPL_ADDRESS.to_owned());
        assert!(normalize_and_validate(&mut on_base).is_err());

        let mut invalid = parameters();
        invalid.paired_stock = Some("NOT A TICKER!".to_owned());
        invalid.paired_stock_address = Some(AAPL_ADDRESS.to_owned());
        assert!(normalize_and_validate(&mut invalid).is_err());

        let mut bad_address = parameters();
        bad_address.paired_stock = Some("AAPL".to_owned());
        bad_address.paired_stock_address = Some("not-an-address".to_owned());
        assert!(normalize_and_validate(&mut bad_address).is_err());

        let baseline = configuration_digest(&shot(), &parameters()).unwrap();
        assert_ne!(baseline, configuration_digest(&shot(), &paired).unwrap());
    }

    #[test]
    fn confirmation_names_chain_recipient_and_predicted_address() {
        assert_eq!(
            confirmation_phrase(
                &shot(),
                LaunchChain::Base,
                "0x1111111111111111111111111111111111111111",
                &parameters().fee_recipient,
                None,
            ),
            "DEPLOY $ANKY FOR SHOT 0x1111111111111111111111111111111111111111111111111111111111111111 ON BASE TO ENS:CREATOR.ETH AT 0x1111111111111111111111111111111111111111"
        );
        assert_eq!(
            confirmation_phrase(
                &shot(),
                LaunchChain::Robinhood,
                "0x1111111111111111111111111111111111111111",
                &parameters().fee_recipient,
                Some("AAPL"),
            ),
            "DEPLOY $ANKY/$AAPL FOR SHOT 0x1111111111111111111111111111111111111111111111111111111111111111 ON ROBINHOOD TO ENS:CREATOR.ETH AT 0x1111111111111111111111111111111111111111"
        );
    }

    #[test]
    fn recipient_is_normalized_and_committed() {
        let mut social = parameters();
        social.fee_recipient = FeeRecipient {
            kind: FeeRecipientKind::X,
            value: " @creator ".to_owned(),
        };
        normalize_and_validate(&mut social).unwrap();
        assert_eq!(social.fee_recipient.value, "creator");

        let baseline = configuration_digest(&shot(), &parameters()).unwrap();
        assert_ne!(baseline, configuration_digest(&shot(), &social).unwrap());
    }
}
