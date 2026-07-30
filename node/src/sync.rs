use crate::model::{
    AuthorityStatus, CandidateContractConfiguration, PeerDescription, PeerSyncResult,
    SegmentStatus, ShotSummary, ShotView, SyncReport, SyncState,
};
use crate::protocol_adapter::parse_public;
use crate::{NodeError, NodeStore, Result, LINEAGE_PROTOCOL, LINEAGE_PROTOCOL_VERSION};
use reqwest::{Client, Url};
use serde::Deserialize;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::{Mutex, RwLock};

pub const MAX_PEERS: usize = 32;
const MAX_PEER_NODE_BYTES: usize = 64 * 1024;
const MAX_PEER_INDEX_BYTES: usize = 4 * 1024 * 1024;
const MAX_REMOTE_SHOTS: usize = 10_000;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Peer {
    base: Url,
}

impl Peer {
    pub fn parse(value: &str) -> Result<Self> {
        let mut base = Url::parse(value).map_err(|_| NodeError::InvalidPeer(value.to_owned()))?;
        if !matches!(base.scheme(), "http" | "https")
            || !base.username().is_empty()
            || base.password().is_some()
            || base.query().is_some()
            || base.fragment().is_some()
            || !(base.path().is_empty() || base.path() == "/")
        {
            return Err(NodeError::InvalidPeer(value.to_owned()));
        }
        base.set_path("/");
        Ok(Self { base })
    }

    pub fn as_str(&self) -> &str {
        self.base.as_str()
    }

    fn endpoint(&self, relative: &str) -> Result<Url> {
        self.base
            .join(relative)
            .map_err(|_| NodeError::InvalidPeer(self.base.to_string()))
    }
}

#[derive(Clone)]
pub struct Node {
    store: Arc<NodeStore>,
    peers: Arc<Vec<Peer>>,
    client: Client,
    sync_status: Arc<RwLock<SyncReport>>,
    sync_lock: Arc<Mutex<()>>,
}

impl Node {
    pub fn new(store: NodeStore, peers: Vec<Peer>) -> Result<Self> {
        if peers.len() > MAX_PEERS {
            return Err(NodeError::PeerLimit { limit: MAX_PEERS });
        }
        let mut normalized = peers;
        normalized.sort_by(|left, right| left.as_str().cmp(right.as_str()));
        normalized.dedup_by(|left, right| left.as_str() == right.as_str());
        let client = Client::builder()
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(20))
            .redirect(reqwest::redirect::Policy::none())
            .user_agent("tohseno-node/0.1")
            .build()
            .map_err(|error| NodeError::PeerRequest(error.to_string()))?;
        Ok(Self {
            store: Arc::new(store),
            peers: Arc::new(normalized),
            client,
            sync_status: Arc::new(RwLock::new(SyncReport::default())),
            sync_lock: Arc::new(Mutex::new(())),
        })
    }

    pub fn store(&self) -> &Arc<NodeStore> {
        &self.store
    }

    pub fn peers(&self) -> Vec<PeerDescription> {
        self.peers
            .iter()
            .map(|peer| PeerDescription {
                url: peer.as_str().to_owned(),
            })
            .collect()
    }

    pub async fn sync_status(&self) -> SyncReport {
        self.sync_status.read().await.clone()
    }

    /// Pulls from the exact configured peers. Ingestion never pushes or
    /// automatically relays, so this cannot turn into implicit gossip.
    pub async fn sync(&self) -> Result<SyncReport> {
        let _guard = self
            .sync_lock
            .try_lock()
            .map_err(|_| NodeError::SyncInProgress)?;
        let started = now_unix();
        {
            let mut status = self.sync_status.write().await;
            *status = SyncReport {
                state: SyncState::Running,
                started_at_unix: Some(started),
                completed_at_unix: None,
                peers: Vec::new(),
            };
        }
        let mut results = Vec::with_capacity(self.peers.len());
        for peer in self.peers.iter() {
            results.push(self.sync_one(peer).await);
        }
        let failed = results.iter().any(|result| result.error.is_some());
        let report = SyncReport {
            state: if failed {
                SyncState::Failed
            } else {
                SyncState::Succeeded
            },
            started_at_unix: Some(started),
            completed_at_unix: Some(now_unix()),
            peers: results,
        };
        *self.sync_status.write().await = report.clone();
        Ok(report)
    }

    async fn sync_one(&self, peer: &Peer) -> PeerSyncResult {
        let mut result = PeerSyncResult {
            peer: peer.as_str().into(),
            fetched: 0,
            already_present: 0,
            rejected: 0,
            error: None,
        };
        if let Err(error) = self.sync_one_inner(peer, &mut result).await {
            result.error = Some(error.to_string());
        }
        result
    }

    async fn sync_one_inner(&self, peer: &Peer, result: &mut PeerSyncResult) -> Result<()> {
        let info_bytes = self
            .get_limited(peer.endpoint("v1/node")?, MAX_PEER_NODE_BYTES)
            .await?;
        let info: RemoteNodeInfo = serde_json::from_slice(&info_bytes)?;
        if info.schema != crate::NODE_PROTOCOL
            || info.node_id == tohseno_protocol::digest::Bytes32::ZERO
            || info.created_at_unix == 0
            || info.lineage_protocol != LINEAGE_PROTOCOL
            || info.lineage_protocol_version != LINEAGE_PROTOCOL_VERSION
            || !info
                .supported_schema_versions
                .contains(&tohseno_protocol::lineage::LINEAGE_SCHEMA_VERSION)
            || info.stored_actions > crate::MAX_ACTIONS_PER_NODE
            || info.indexed_shots > MAX_REMOTE_SHOTS
            || info.contract_configuration != self.store.info()?.contract_configuration
            || info.agreement.is_empty()
            || info.non_agreement.is_empty()
        {
            return Err(NodeError::PeerResponse(
                "peer does not advertise the required lineage protocol".into(),
            ));
        }

        let shots_bytes = self
            .get_limited(peer.endpoint("v1/shots")?, MAX_PEER_INDEX_BYTES)
            .await?;
        let shots: Vec<ShotSummary> = serde_json::from_slice(&shots_bytes)?;
        if shots.len() > MAX_REMOTE_SHOTS {
            return Err(NodeError::PeerResponse(format!(
                "peer advertises more than {MAX_REMOTE_SHOTS} Shots"
            )));
        }
        for summary in shots {
            let view_bytes = self
                .get_limited(
                    peer.endpoint(&format!("v1/shots/{}", summary.shot_id))?,
                    MAX_PEER_INDEX_BYTES,
                )
                .await?;
            let mut view: ShotView = serde_json::from_slice(&view_bytes)?;
            if view.shot.shot_id != summary.shot_id
                || view.shot.action_count != view.actions.len()
                || view.actions.len() > crate::store::MAX_ACTIONS_PER_SHOT
            {
                return Err(NodeError::PeerResponse(
                    "peer Shot inventory is internally inconsistent".into(),
                ));
            }
            view.actions
                .sort_by_key(|reference| (reference.sequence, reference.digest));
            for reference in view.actions {
                if reference.shot_id != summary.shot_id {
                    return Err(NodeError::PeerResponse(
                        "peer action reference changed ShotID".into(),
                    ));
                }
                if reference.validation.segment == SegmentStatus::Rejected
                    || reference.validation.candidate_authority == AuthorityStatus::Rejected
                {
                    result.rejected += 1;
                    continue;
                }
                if self.store.contains(reference.digest)? {
                    result.already_present += 1;
                    continue;
                }
                let bytes = self
                    .get_limited(
                        peer.endpoint(&format!("v1/actions/{}", reference.digest))?,
                        crate::MAX_ACTION_BYTES,
                    )
                    .await?;
                let validated = parse_public(&bytes)?;
                if validated.digest != reference.digest
                    || validated.shot_id != reference.shot_id
                    || validated.sequence != reference.sequence
                    || validated.previous != reference.previous
                {
                    result.rejected += 1;
                    return Err(NodeError::PeerResponse(
                        "peer action bytes do not match the advertised reference".into(),
                    ));
                }
                match self.store.ingest(&bytes) {
                    Ok(outcome) if outcome.digest == reference.digest => {
                        if outcome.stored {
                            result.fetched += 1;
                        } else {
                            result.already_present += 1;
                        }
                    }
                    Ok(_) => {
                        result.rejected += 1;
                        return Err(NodeError::PeerResponse(
                            "stored action commitment changed unexpectedly".into(),
                        ));
                    }
                    Err(error) => {
                        result.rejected += 1;
                        return Err(error);
                    }
                }
            }
        }
        Ok(())
    }

    async fn get_limited(&self, url: Url, limit: usize) -> Result<Vec<u8>> {
        let mut response = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|error| NodeError::PeerRequest(error.to_string()))?;
        if !response.status().is_success() {
            return Err(NodeError::PeerResponse(format!(
                "HTTP status {}",
                response.status()
            )));
        }
        if response
            .content_length()
            .is_some_and(|length| length > limit as u64)
        {
            return Err(NodeError::PeerResponseTooLarge { limit });
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|error| NodeError::PeerRequest(error.to_string()))?
        {
            if bytes.len().saturating_add(chunk.len()) > limit {
                return Err(NodeError::PeerResponseTooLarge { limit });
            }
            bytes.extend_from_slice(&chunk);
        }
        Ok(bytes)
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RemoteNodeInfo {
    schema: String,
    node_id: tohseno_protocol::digest::Bytes32,
    created_at_unix: u64,
    lineage_protocol: String,
    lineage_protocol_version: String,
    supported_schema_versions: Vec<u32>,
    stored_actions: usize,
    indexed_shots: usize,
    contract_configuration: CandidateContractConfiguration,
    agreement: String,
    non_agreement: String,
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn peer_urls_are_static_origins_without_credentials_or_paths() {
        assert!(Peer::parse("http://127.0.0.1:8080").is_ok());
        assert!(Peer::parse("https://node.example").is_ok());
        assert!(Peer::parse("file:///tmp/node").is_err());
        assert!(Peer::parse("http://user@example.test").is_err());
        assert!(Peer::parse("http://example.test/path").is_err());
        assert!(Peer::parse("http://example.test/?target=other").is_err());
    }
}
