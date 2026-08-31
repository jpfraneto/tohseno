//! Private product entitlement state retained in TOHSENO 1.2.0.
//!
//! This is deliberately not protocol state. It lives beside the Local
//! Workspace Service command journal and contains no intention, app name,
//! Apple identifier, Companion recovery material, or public lineage bytes.

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use time::format_description::well_known::Rfc3339;
use time::{Date, Duration, Month, OffsetDateTime};
use uuid::Uuid;

pub const ENTITLEMENT_STATE_SCHEMA: &str = "tohseno.private-entitlement-state/1";
const MAXIMUM_STATE_BYTES: u64 = 256 * 1024;
const REQUIRED_SUCCESSFUL_DAYS: usize = 5;
const TRIAL_CALENDAR_DAYS: i64 = 7;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntitlementPhase {
    GenesisIncomplete,
    TrialActive,
    TrialQualified,
    TrialExpired,
    ProMonthly,
    ProYearly,
    ProLapsed,
}

impl EntitlementPhase {
    pub fn factory_mutations_allowed(self) -> bool {
        matches!(self, Self::TrialActive | Self::ProMonthly | Self::ProYearly)
    }

    pub fn purchase_allowed(self) -> bool {
        matches!(self, Self::TrialQualified | Self::ProLapsed)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SubscriptionPlan {
    Monthly,
    Yearly,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SuccessfulDayEvidence {
    pub local_date: String,
    pub command_id: String,
    pub execution_id: String,
    pub accepted_version_id: String,
    pub accepted_at: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VerifiedSubscription {
    pub entitlement_id: String,
    pub plan: SubscriptionPlan,
    pub issued_at: String,
    pub paid_through: String,
    pub cancellation_at_period_end: bool,
    #[serde(default = "initial_provider_revision")]
    pub provider_revision: u64,
    /// SHA-256 of the exact verified receipt bytes. The receipt itself is kept
    /// by the billing boundary and is never copied into public app lineage.
    pub receipt_digest: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EntitlementState {
    pub schema: String,
    pub revision: u64,
    pub phase: EntitlementPhase,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub genesis_completed_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub genesis_local_date: Option<String>,
    pub successful_days: Vec<SuccessfulDayEvidence>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subscription: Option<VerifiedSubscription>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub migration: Option<String>,
}

impl Default for EntitlementState {
    fn default() -> Self {
        Self {
            schema: ENTITLEMENT_STATE_SCHEMA.into(),
            revision: 1,
            phase: EntitlementPhase::GenesisIncomplete,
            genesis_completed_at: None,
            genesis_local_date: None,
            successful_days: Vec::new(),
            subscription: None,
            migration: None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EntitlementStatus {
    pub schema: &'static str,
    pub phase: EntitlementPhase,
    pub genesis_complete: bool,
    pub successful_days: usize,
    pub required_successful_days: usize,
    pub factory_mutations_allowed: bool,
    pub purchase_allowed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plan: Option<SubscriptionPlan>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub paid_through: Option<String>,
    pub cancellation_at_period_end: bool,
}

#[derive(Debug)]
pub enum EntitlementError {
    Io(std::io::Error),
    Json(serde_json::Error),
    Invalid(String),
    Locked(String),
}

impl std::fmt::Display for EntitlementError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "{error}"),
            Self::Json(error) => write!(formatter, "{error}"),
            Self::Invalid(message) | Self::Locked(message) => formatter.write_str(message),
        }
    }
}

impl std::error::Error for EntitlementError {}

impl From<std::io::Error> for EntitlementError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<serde_json::Error> for EntitlementError {
    fn from(value: serde_json::Error) -> Self {
        Self::Json(value)
    }
}

#[derive(Clone, Debug)]
pub struct EntitlementStore {
    path: PathBuf,
    lock: Arc<Mutex<()>>,
}

impl EntitlementStore {
    pub fn open(service_root: impl Into<PathBuf>) -> Result<Self, EntitlementError> {
        let service_root = service_root.into();
        if !service_root.is_absolute() {
            return Err(EntitlementError::Invalid(
                "private entitlement root must be absolute".into(),
            ));
        }
        ensure_private_directory(&service_root)?;
        let directory = service_root.join("entitlement");
        ensure_private_directory(&directory)?;
        let store = Self {
            path: directory.join("state-v1.json"),
            lock: Arc::new(Mutex::new(())),
        };
        if !store.path.exists() {
            store.write_locked(&EntitlementState::default())?;
        } else {
            store.read_locked()?;
        }
        Ok(store)
    }

    pub fn state(&self) -> Result<EntitlementState, EntitlementError> {
        let _guard = self.lock.lock().map_err(|_| {
            EntitlementError::Invalid("private entitlement lock was poisoned".into())
        })?;
        self.read_locked()
    }

    pub fn status_at(
        &self,
        now: OffsetDateTime,
        local_date: Date,
    ) -> Result<EntitlementStatus, EntitlementError> {
        let _guard = self.lock.lock().map_err(|_| {
            EntitlementError::Invalid("private entitlement lock was poisoned".into())
        })?;
        let mut state = self.read_locked()?;
        if refresh_phase(&mut state, now, local_date)? {
            bump(&mut state);
            self.write_locked(&state)?;
        }
        Ok(status_from(&state))
    }

    pub fn status_now(&self) -> Result<EntitlementStatus, EntitlementError> {
        let utc = OffsetDateTime::now_utc();
        self.status_at(utc, local_date_now(utc))
    }

    /// Starts the full-product trial only after physical Companion install,
    /// secure pairing, and durable genesis have all been verified by the
    /// caller. Repeating the same completion is harmless.
    pub fn complete_genesis_at(
        &self,
        completed_at: OffsetDateTime,
        local_date: Date,
    ) -> Result<EntitlementStatus, EntitlementError> {
        let _guard = self.lock.lock().map_err(|_| {
            EntitlementError::Invalid("private entitlement lock was poisoned".into())
        })?;
        let mut state = self.read_locked()?;
        if state.phase == EntitlementPhase::GenesisIncomplete {
            state.phase = EntitlementPhase::TrialActive;
            state.genesis_completed_at = Some(format_timestamp(completed_at)?);
            state.genesis_local_date = Some(local_date.to_string());
            bump(&mut state);
            validate_state(&state)?;
            self.write_locked(&state)?;
        }
        Ok(status_from(&state))
    }

    pub fn complete_genesis_now(&self) -> Result<EntitlementStatus, EntitlementError> {
        let utc = OffsetDateTime::now_utc();
        self.complete_genesis_at(utc, local_date_now(utc))
    }

    /// Migrates an already-paired pre-1.0.0 installation without erasing its
    /// device or workspace. The first 1.0.0 service observation becomes the
    /// deterministic trial anchor; no app count is converted into days.
    pub fn migrate_existing_pairing_at(
        &self,
        migrated_at: OffsetDateTime,
        local_date: Date,
    ) -> Result<EntitlementStatus, EntitlementError> {
        let _guard = self.lock.lock().map_err(|_| {
            EntitlementError::Invalid("private entitlement lock was poisoned".into())
        })?;
        let mut state = self.read_locked()?;
        if state.phase == EntitlementPhase::GenesisIncomplete {
            state.phase = EntitlementPhase::TrialActive;
            state.genesis_completed_at = Some(format_timestamp(migrated_at)?);
            state.genesis_local_date = Some(local_date.to_string());
            state.migration = Some("existing_paired_installation_v1".into());
            bump(&mut state);
            validate_state(&state)?;
            self.write_locked(&state)?;
        }
        Ok(status_from(&state))
    }

    pub fn migrate_existing_pairing_now(&self) -> Result<EntitlementStatus, EntitlementError> {
        let utc = OffsetDateTime::now_utc();
        self.migrate_existing_pairing_at(utc, local_date_now(utc))
    }

    /// Explicit source-checkout escape hatch for local development and
    /// isolated lifecycle fixtures. This function is absent from release
    /// builds; production entitlement always requires a verified receipt.
    #[cfg(debug_assertions)]
    pub fn grant_development_at(
        &self,
        now: OffsetDateTime,
    ) -> Result<EntitlementStatus, EntitlementError> {
        let _guard = self.lock.lock().map_err(|_| {
            EntitlementError::Invalid("private entitlement lock was poisoned".into())
        })?;
        let mut state = self.read_locked()?;
        state.phase = EntitlementPhase::ProYearly;
        state.genesis_completed_at = Some(format_timestamp(now)?);
        state.genesis_local_date = Some(now.date().to_string());
        state.subscription = Some(VerifiedSubscription {
            entitlement_id: "entitlement_development_checkout".into(),
            plan: SubscriptionPlan::Yearly,
            issued_at: format_timestamp(now)?,
            paid_through: format_timestamp(now + Duration::days(3650))?,
            cancellation_at_period_end: false,
            provider_revision: 1,
            receipt_digest: "0".repeat(64),
        });
        state.migration = Some("explicit_debug_development_entitlement_v1".into());
        bump(&mut state);
        validate_state(&state)?;
        self.write_locked(&state)?;
        Ok(status_from(&state))
    }

    /// Records one accepted, installed, and launched Version. The application
    /// service calls this only after its existing completion and lineage gates
    /// pass. The date, command, execution, and Version links are all bounded.
    pub fn record_successful_day_at(
        &self,
        evidence: SuccessfulDayEvidence,
        now: OffsetDateTime,
        local_date: Date,
    ) -> Result<EntitlementStatus, EntitlementError> {
        let _guard = self.lock.lock().map_err(|_| {
            EntitlementError::Invalid("private entitlement lock was poisoned".into())
        })?;
        let mut state = self.read_locked()?;
        let changed = refresh_phase(&mut state, now, local_date)?;
        if state.phase != EntitlementPhase::TrialActive {
            if changed {
                bump(&mut state);
                self.write_locked(&state)?;
            }
            return Ok(status_from(&state));
        }
        validate_evidence(&evidence)?;
        if evidence.local_date != local_date.to_string() {
            return Err(EntitlementError::Invalid(
                "successful-day evidence local date differs from the injected clock".into(),
            ));
        }
        let duplicate = state.successful_days.iter().any(|existing| {
            existing.local_date == evidence.local_date
                || existing.command_id == evidence.command_id
                || existing.execution_id == evidence.execution_id
                || existing.accepted_version_id == evidence.accepted_version_id
        });
        if !duplicate {
            state.successful_days.push(evidence);
            state
                .successful_days
                .sort_by(|left, right| left.local_date.cmp(&right.local_date));
            if state.successful_days.len() >= REQUIRED_SUCCESSFUL_DAYS {
                state.phase = EntitlementPhase::TrialQualified;
            }
            bump(&mut state);
            validate_state(&state)?;
            self.write_locked(&state)?;
        } else if changed {
            bump(&mut state);
            self.write_locked(&state)?;
        }
        Ok(status_from(&state))
    }

    pub fn record_successful_day_now(
        &self,
        mut evidence: SuccessfulDayEvidence,
    ) -> Result<EntitlementStatus, EntitlementError> {
        let utc = OffsetDateTime::now_utc();
        let local_date = local_date_now(utc);
        evidence.local_date = local_date.to_string();
        evidence.accepted_at = format_timestamp(utc)?;
        self.record_successful_day_at(evidence, utc, local_date)
    }

    /// Installs a subscription only after an external receipt verifier has
    /// authenticated the exact server-signed receipt and installation bind.
    pub fn install_verified_subscription(
        &self,
        subscription: VerifiedSubscription,
        now: OffsetDateTime,
        local_date: Date,
    ) -> Result<EntitlementStatus, EntitlementError> {
        validate_subscription(&subscription, now)?;
        let _guard = self.lock.lock().map_err(|_| {
            EntitlementError::Invalid("private entitlement lock was poisoned".into())
        })?;
        let mut state = self.read_locked()?;
        refresh_phase(&mut state, now, local_date)?;
        if !matches!(
            state.phase,
            EntitlementPhase::TrialQualified
                | EntitlementPhase::ProLapsed
                | EntitlementPhase::ProMonthly
                | EntitlementPhase::ProYearly
        ) {
            return Err(EntitlementError::Locked(
                "TOHSENO Pro is available only after five successful days".into(),
            ));
        }
        if let Some(existing) = state.subscription.as_ref() {
            if subscription.provider_revision < existing.provider_revision {
                return Err(EntitlementError::Invalid(
                    "verified entitlement receipt revision moved backwards".into(),
                ));
            }
            if subscription.provider_revision == existing.provider_revision {
                if subscription.receipt_digest == existing.receipt_digest {
                    return Ok(status_from(&state));
                }
                return Err(EntitlementError::Invalid(
                    "verified entitlement receipt revision conflicts with durable state".into(),
                ));
            }
        }
        state.phase = match subscription.plan {
            SubscriptionPlan::Monthly => EntitlementPhase::ProMonthly,
            SubscriptionPlan::Yearly => EntitlementPhase::ProYearly,
        };
        state.subscription = Some(subscription);
        bump(&mut state);
        validate_state(&state)?;
        self.write_locked(&state)?;
        Ok(status_from(&state))
    }

    pub fn require_new_factory_mutation(&self) -> Result<(), EntitlementError> {
        let status = self.status_now()?;
        if status.factory_mutations_allowed {
            return Ok(());
        }
        Err(EntitlementError::Locked(
            match status.phase {
                EntitlementPhase::GenesisIncomplete => {
                    "Finish setting up TOHSENO on your iPhone before creating or evolving an app."
                }
                EntitlementPhase::TrialQualified | EntitlementPhase::ProLapsed => {
                    "Continue with TOHSENO Pro on this Mac before creating or evolving an app."
                }
                EntitlementPhase::TrialExpired => {
                    "Your TOHSENO trial has ended. Everything you made is still here."
                }
                EntitlementPhase::TrialActive
                | EntitlementPhase::ProMonthly
                | EntitlementPhase::ProYearly => unreachable!(),
            }
            .into(),
        ))
    }

    fn read_locked(&self) -> Result<EntitlementState, EntitlementError> {
        let metadata = fs::symlink_metadata(&self.path)?;
        if metadata.file_type().is_symlink()
            || !metadata.is_file()
            || metadata.len() > MAXIMUM_STATE_BYTES
        {
            return Err(EntitlementError::Invalid(
                "private entitlement state is unsafe or oversized".into(),
            ));
        }
        let mut options = OpenOptions::new();
        options.read(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC);
        }
        let mut file = options.open(&self.path)?;
        let mut bytes = Vec::new();
        std::io::Read::by_ref(&mut file)
            .take(MAXIMUM_STATE_BYTES + 1)
            .read_to_end(&mut bytes)?;
        if bytes.len() as u64 > MAXIMUM_STATE_BYTES {
            return Err(EntitlementError::Invalid(
                "private entitlement state is oversized".into(),
            ));
        }
        let state: EntitlementState = serde_json::from_slice(&bytes)?;
        validate_state(&state)?;
        Ok(state)
    }

    fn write_locked(&self, state: &EntitlementState) -> Result<(), EntitlementError> {
        validate_state(state)?;
        let bytes = tohseno_protocol::canonical::to_vec(state)
            .map_err(|error| EntitlementError::Invalid(error.to_string()))?;
        if bytes.len() as u64 > MAXIMUM_STATE_BYTES {
            return Err(EntitlementError::Invalid(
                "private entitlement state is oversized".into(),
            ));
        }
        let parent = self
            .path
            .parent()
            .ok_or_else(|| EntitlementError::Invalid("entitlement path has no parent".into()))?;
        let temporary = parent.join(format!(".state-{}.tmp", Uuid::new_v4().simple()));
        let mut options = OpenOptions::new();
        options.create_new(true).write(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options
                .mode(0o600)
                .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC);
        }
        let mut file = options.open(&temporary)?;
        file.write_all(&bytes)?;
        file.sync_all()?;
        fs::rename(&temporary, &self.path)?;
        File::open(parent)?.sync_all()?;
        Ok(())
    }
}

fn refresh_phase(
    state: &mut EntitlementState,
    now: OffsetDateTime,
    local_date: Date,
) -> Result<bool, EntitlementError> {
    match state.phase {
        EntitlementPhase::TrialActive => {
            let genesis =
                parse_date(state.genesis_local_date.as_deref().ok_or_else(|| {
                    EntitlementError::Invalid("trial has no local anchor".into())
                })?)?;
            if local_date >= genesis + Duration::days(TRIAL_CALENDAR_DAYS) {
                state.phase = EntitlementPhase::TrialExpired;
                return Ok(true);
            }
        }
        EntitlementPhase::ProMonthly | EntitlementPhase::ProYearly => {
            let subscription = state.subscription.as_ref().ok_or_else(|| {
                EntitlementError::Invalid("active Pro state has no verified subscription".into())
            })?;
            if parse_timestamp(&subscription.paid_through)? <= now {
                state.phase = EntitlementPhase::ProLapsed;
                return Ok(true);
            }
        }
        _ => {}
    }
    Ok(false)
}

fn status_from(state: &EntitlementState) -> EntitlementStatus {
    EntitlementStatus {
        schema: "tohseno.private-entitlement-status/1",
        phase: state.phase,
        genesis_complete: state.genesis_completed_at.is_some(),
        successful_days: state.successful_days.len(),
        required_successful_days: REQUIRED_SUCCESSFUL_DAYS,
        factory_mutations_allowed: state.phase.factory_mutations_allowed(),
        purchase_allowed: state.phase.purchase_allowed(),
        plan: state.subscription.as_ref().map(|value| value.plan),
        paid_through: state
            .subscription
            .as_ref()
            .map(|value| value.paid_through.clone()),
        cancellation_at_period_end: state
            .subscription
            .as_ref()
            .is_some_and(|value| value.cancellation_at_period_end),
    }
}

fn validate_state(state: &EntitlementState) -> Result<(), EntitlementError> {
    if state.schema != ENTITLEMENT_STATE_SCHEMA || state.revision == 0 {
        return Err(EntitlementError::Invalid(
            "private entitlement state has an unsupported schema".into(),
        ));
    }
    match (&state.genesis_completed_at, &state.genesis_local_date) {
        (None, None) if state.phase == EntitlementPhase::GenesisIncomplete => {}
        (Some(timestamp), Some(date)) => {
            parse_timestamp(timestamp)?;
            parse_date(date)?;
        }
        _ => {
            return Err(EntitlementError::Invalid(
                "private entitlement genesis anchor is inconsistent".into(),
            ))
        }
    }
    if state.successful_days.len() > REQUIRED_SUCCESSFUL_DAYS {
        return Err(EntitlementError::Invalid(
            "private entitlement state has too many successful days".into(),
        ));
    }
    let mut dates = BTreeSet::new();
    let mut commands = BTreeSet::new();
    let mut executions = BTreeSet::new();
    let mut versions = BTreeSet::new();
    for evidence in &state.successful_days {
        validate_evidence(evidence)?;
        if !dates.insert(&evidence.local_date)
            || !commands.insert(&evidence.command_id)
            || !executions.insert(&evidence.execution_id)
            || !versions.insert(&evidence.accepted_version_id)
        {
            return Err(EntitlementError::Invalid(
                "successful-day evidence is not unique".into(),
            ));
        }
    }
    if matches!(state.phase, EntitlementPhase::TrialQualified)
        && state.successful_days.len() != REQUIRED_SUCCESSFUL_DAYS
    {
        return Err(EntitlementError::Invalid(
            "qualified trial lacks five successful days".into(),
        ));
    }
    if matches!(
        state.phase,
        EntitlementPhase::ProMonthly | EntitlementPhase::ProYearly | EntitlementPhase::ProLapsed
    ) {
        let subscription = state.subscription.as_ref().ok_or_else(|| {
            EntitlementError::Invalid("Pro state lacks a verified subscription".into())
        })?;
        validate_subscription_shape(subscription)?;
    }
    Ok(())
}

fn validate_evidence(evidence: &SuccessfulDayEvidence) -> Result<(), EntitlementError> {
    parse_date(&evidence.local_date)?;
    parse_timestamp(&evidence.accepted_at)?;
    for (label, value) in [
        ("command ID", evidence.command_id.as_str()),
        ("execution ID", evidence.execution_id.as_str()),
        ("accepted Version ID", evidence.accepted_version_id.as_str()),
    ] {
        if value.is_empty()
            || value.len() > 160
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
        {
            return Err(EntitlementError::Invalid(format!(
                "successful-day {label} is invalid"
            )));
        }
    }
    Ok(())
}

fn validate_subscription(
    subscription: &VerifiedSubscription,
    now: OffsetDateTime,
) -> Result<(), EntitlementError> {
    validate_subscription_shape(subscription)?;
    if parse_timestamp(&subscription.paid_through)? <= now {
        return Err(EntitlementError::Invalid(
            "verified entitlement receipt is already expired".into(),
        ));
    }
    Ok(())
}

fn validate_subscription_shape(
    subscription: &VerifiedSubscription,
) -> Result<(), EntitlementError> {
    if subscription.provider_revision == 0
        || subscription.entitlement_id.is_empty()
        || subscription.entitlement_id.len() > 160
        || !subscription
            .entitlement_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
        || subscription.receipt_digest.len() != 64
        || !subscription
            .receipt_digest
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(EntitlementError::Invalid(
            "verified entitlement receipt metadata is invalid".into(),
        ));
    }
    let issued = parse_timestamp(&subscription.issued_at)?;
    let paid_through = parse_timestamp(&subscription.paid_through)?;
    if paid_through <= issued {
        return Err(EntitlementError::Invalid(
            "verified entitlement paid-through date is not after issue".into(),
        ));
    }
    Ok(())
}

const fn initial_provider_revision() -> u64 {
    1
}

fn parse_timestamp(value: &str) -> Result<OffsetDateTime, EntitlementError> {
    OffsetDateTime::parse(value, &Rfc3339)
        .map_err(|_| EntitlementError::Invalid("entitlement timestamp is invalid".into()))
}

fn format_timestamp(value: OffsetDateTime) -> Result<String, EntitlementError> {
    value
        .format(&Rfc3339)
        .map_err(|_| EntitlementError::Invalid("entitlement timestamp is invalid".into()))
}

fn parse_date(value: &str) -> Result<Date, EntitlementError> {
    let format = time::format_description::parse("[year]-[month]-[day]")
        .map_err(|_| EntitlementError::Invalid("local date format is unavailable".into()))?;
    Date::parse(value, &format)
        .map_err(|_| EntitlementError::Invalid("entitlement local date is invalid".into()))
}

fn bump(state: &mut EntitlementState) {
    state.revision = state.revision.saturating_add(1);
}

#[cfg(unix)]
fn local_date_now(utc: OffsetDateTime) -> Date {
    let seconds = utc.unix_timestamp();
    let value = seconds as libc::time_t;
    // SAFETY: `value` and `local` are valid initialized storage for the
    // duration of the call, and localtime_r writes only to the supplied tm.
    let mut local: libc::tm = unsafe { std::mem::zeroed() };
    // SAFETY: both pointers remain valid and non-aliasing for this call.
    let observed = unsafe { libc::localtime_r(&value, &mut local) };
    if observed.is_null() {
        return utc.date();
    }
    let year = local.tm_year.saturating_add(1900);
    let month = u8::try_from(local.tm_mon.saturating_add(1))
        .ok()
        .and_then(|value| Month::try_from(value).ok());
    let day = u8::try_from(local.tm_mday).ok();
    match (month, day) {
        (Some(month), Some(day)) => {
            Date::from_calendar_date(year, month, day).unwrap_or(utc.date())
        }
        _ => utc.date(),
    }
}

#[cfg(not(unix))]
fn local_date_now(utc: OffsetDateTime) -> Date {
    utc.date()
}

fn ensure_private_directory(path: &Path) -> Result<(), EntitlementError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
            return Err(EntitlementError::Invalid(
                "private entitlement directory is unsafe".into(),
            ));
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => fs::create_dir(path)?,
        Err(error) => return Err(error.into()),
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(value: &str) -> OffsetDateTime {
        OffsetDateTime::parse(value, &Rfc3339).unwrap()
    }

    fn date(value: &str) -> Date {
        parse_date(value).unwrap()
    }

    fn store() -> (tempfile::TempDir, EntitlementStore) {
        let root = tempfile::tempdir().unwrap();
        let store = EntitlementStore::open(root.path().join("service")).unwrap();
        (root, store)
    }

    fn evidence(day: usize) -> SuccessfulDayEvidence {
        SuccessfulDayEvidence {
            local_date: format!("2026-08-{day:02}"),
            command_id: format!("command_{day}"),
            execution_id: format!("execution_{day}"),
            accepted_version_id: format!("version_{day}"),
            accepted_at: format!("2026-08-{day:02}T12:00:00Z"),
        }
    }

    #[test]
    fn genesis_starts_the_clock_but_not_a_successful_day() {
        let (_root, store) = store();
        let status = store
            .complete_genesis_at(at("2026-08-01T12:00:00Z"), date("2026-08-01"))
            .unwrap();
        assert_eq!(status.phase, EntitlementPhase::TrialActive);
        assert_eq!(status.successful_days, 0);
        assert!(status.factory_mutations_allowed);
    }

    #[test]
    fn accepted_versions_count_once_per_day_and_retries_are_idempotent() {
        let (_root, store) = store();
        store
            .complete_genesis_at(at("2026-08-01T00:00:00Z"), date("2026-08-01"))
            .unwrap();
        let first = evidence(1);
        store
            .record_successful_day_at(
                first.clone(),
                at("2026-08-01T12:00:00Z"),
                date("2026-08-01"),
            )
            .unwrap();
        store
            .record_successful_day_at(first, at("2026-08-01T12:01:00Z"), date("2026-08-01"))
            .unwrap();
        let mut second_acceptance = evidence(2);
        second_acceptance.local_date = "2026-08-01".into();
        second_acceptance.accepted_at = "2026-08-01T18:00:00Z".into();
        let status = store
            .record_successful_day_at(
                second_acceptance,
                at("2026-08-01T18:00:00Z"),
                date("2026-08-01"),
            )
            .unwrap();
        assert_eq!(status.successful_days, 1);
    }

    #[test]
    fn five_distinct_days_qualify_and_lock_the_next_admission() {
        let (_root, store) = store();
        store
            .complete_genesis_at(at("2026-08-01T00:00:00Z"), date("2026-08-01"))
            .unwrap();
        let mut status = store
            .status_at(at("2026-08-01T00:00:00Z"), date("2026-08-01"))
            .unwrap();
        for day in 1..=5 {
            status = store
                .record_successful_day_at(
                    evidence(day),
                    at(&format!("2026-08-{day:02}T12:00:00Z")),
                    date(&format!("2026-08-{day:02}")),
                )
                .unwrap();
        }
        assert_eq!(status.phase, EntitlementPhase::TrialQualified);
        assert!(!status.factory_mutations_allowed);
        assert!(status.purchase_allowed);
    }

    #[test]
    fn seven_calendar_days_expire_without_a_purchase_offer() {
        let (_root, store) = store();
        store
            .complete_genesis_at(at("2026-08-01T23:59:00Z"), date("2026-08-01"))
            .unwrap();
        let status = store
            .status_at(at("2026-08-08T00:01:00Z"), date("2026-08-08"))
            .unwrap();
        assert_eq!(status.phase, EntitlementPhase::TrialExpired);
        assert!(!status.purchase_allowed);
        assert!(!status.factory_mutations_allowed);
    }

    #[test]
    fn restart_preserves_state_and_timezone_boundary_is_injected() {
        let (root, store) = store();
        store
            .complete_genesis_at(at("2026-08-02T02:30:00Z"), date("2026-08-01"))
            .unwrap();
        store
            .record_successful_day_at(evidence(1), at("2026-08-02T02:45:00Z"), date("2026-08-01"))
            .unwrap();
        drop(store);
        let reopened = EntitlementStore::open(root.path().join("service")).unwrap();
        assert_eq!(reopened.state().unwrap().successful_days.len(), 1);
    }

    #[test]
    fn verified_subscription_unlocks_then_lapses_without_erasing_days() {
        let (_root, store) = store();
        store
            .complete_genesis_at(at("2026-08-01T00:00:00Z"), date("2026-08-01"))
            .unwrap();
        for day in 1..=5 {
            store
                .record_successful_day_at(
                    evidence(day),
                    at(&format!("2026-08-{day:02}T12:00:00Z")),
                    date(&format!("2026-08-{day:02}")),
                )
                .unwrap();
        }
        let subscription = VerifiedSubscription {
            entitlement_id: "entitlement_fixture".into(),
            plan: SubscriptionPlan::Monthly,
            issued_at: "2026-08-05T12:01:00Z".into(),
            paid_through: "2026-09-05T12:01:00Z".into(),
            cancellation_at_period_end: true,
            provider_revision: 2,
            receipt_digest: "ab".repeat(32),
        };
        let active = store
            .install_verified_subscription(
                subscription.clone(),
                at("2026-08-05T12:01:00Z"),
                date("2026-08-05"),
            )
            .unwrap();
        assert_eq!(active.phase, EntitlementPhase::ProMonthly);
        let durable_revision = store.state().unwrap().revision;
        store
            .install_verified_subscription(
                subscription.clone(),
                at("2026-08-05T12:02:00Z"),
                date("2026-08-05"),
            )
            .unwrap();
        assert_eq!(store.state().unwrap().revision, durable_revision);
        let mut stale = subscription;
        stale.provider_revision = 1;
        stale.receipt_digest = "cd".repeat(32);
        assert!(store
            .install_verified_subscription(stale, at("2026-08-05T12:03:00Z"), date("2026-08-05"),)
            .is_err());
        let lapsed = store
            .status_at(at("2026-09-05T12:01:00Z"), date("2026-09-05"))
            .unwrap();
        assert_eq!(lapsed.phase, EntitlementPhase::ProLapsed);
        assert_eq!(lapsed.successful_days, 5);
    }

    #[test]
    fn existing_pairing_migration_never_fabricates_days() {
        let (_root, store) = store();
        let status = store
            .migrate_existing_pairing_at(at("2026-08-21T12:00:00Z"), date("2026-08-21"))
            .unwrap();
        assert_eq!(status.phase, EntitlementPhase::TrialActive);
        assert_eq!(status.successful_days, 0);
        assert_eq!(
            store.state().unwrap().migration.as_deref(),
            Some("existing_paired_installation_v1")
        );
    }
}
