use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;
use p256::ecdsa::signature::Signer;
use p256::ecdsa::{Signature, SigningKey};
use tempfile::tempdir;
use time::format_description::well_known::Rfc3339;
use time::{Date, Duration, Month, OffsetDateTime};
use tohseno_application::{
    installation_binding, verify_receipt, EntitlementPhase, EntitlementReceiptPayload,
    EntitlementStore, SignedEntitlementReceipt, SubscriptionPlan, SuccessfulDayEvidence,
};

#[test]
fn local_1_0_0_entitlement_golden_path() {
    let root = tempdir().unwrap();
    let store = EntitlementStore::open(root.path().join("service")).unwrap();
    let start = OffsetDateTime::from_unix_timestamp(1_800_000_000).unwrap();
    let first_date = Date::from_calendar_date(2027, Month::January, 15).unwrap();

    assert!(store.require_new_factory_mutation().is_err());
    let active = store.complete_genesis_at(start, first_date).unwrap();
    assert_eq!(active.phase, EntitlementPhase::TrialActive);
    assert_eq!(active.successful_days, 0);

    for day in 0..5 {
        let now = start + Duration::days(day);
        let date = first_date + Duration::days(day);
        let status = store
            .record_successful_day_at(
                SuccessfulDayEvidence {
                    local_date: date.to_string(),
                    command_id: format!("command_golden_{day}"),
                    execution_id: format!("execution_golden_{day}"),
                    accepted_version_id: format!("version_golden_{day}"),
                    accepted_at: now.format(&Rfc3339).unwrap(),
                },
                now,
                date,
            )
            .unwrap();
        assert_eq!(status.successful_days, day as usize + 1);
    }
    assert_eq!(
        store
            .status_at(start + Duration::days(4), first_date + Duration::days(4))
            .unwrap()
            .phase,
        EntitlementPhase::TrialQualified
    );
    assert!(store.require_new_factory_mutation().is_err());

    let signing = SigningKey::from_bytes((&[9_u8; 32]).into()).unwrap();
    let receipt_now = start + Duration::days(4);
    let payload = EntitlementReceiptPayload {
        schema: "tohseno.private-entitlement-receipt/1".into(),
        receipt_id: "receipt_golden_1".into(),
        entitlement_id: "entitlement_golden_1".into(),
        installation_binding: installation_binding("workspace_golden").unwrap(),
        plan: SubscriptionPlan::Yearly,
        issued_at: receipt_now.format(&Rfc3339).unwrap(),
        paid_through: (receipt_now + Duration::days(365))
            .format(&Rfc3339)
            .unwrap(),
        cancellation_at_period_end: false,
        provider_revision: 1,
    };
    let payload = tohseno_protocol::canonical::to_vec(&payload).unwrap();
    let signature: Signature = signing.sign(&payload);
    let envelope = SignedEntitlementReceipt {
        schema: "tohseno.private-entitlement-envelope/1".into(),
        payload_base64url: URL_SAFE_NO_PAD.encode(&payload),
        signature_base64url: URL_SAFE_NO_PAD.encode(signature.to_bytes()),
    };
    let envelope = serde_json::to_vec(&envelope).unwrap();
    let public = URL_SAFE_NO_PAD.encode(signing.verifying_key().to_encoded_point(true));
    let verified = verify_receipt(&envelope, &public, "workspace_golden", receipt_now).unwrap();
    let pro = store
        .install_verified_subscription(verified, receipt_now, first_date + Duration::days(4))
        .unwrap();
    assert_eq!(pro.phase, EntitlementPhase::ProYearly);
    store.require_new_factory_mutation().unwrap();

    let expired_root = tempdir().unwrap();
    let expired = EntitlementStore::open(expired_root.path().join("service")).unwrap();
    expired.complete_genesis_at(start, first_date).unwrap();
    let ended = expired
        .status_at(start + Duration::days(7), first_date + Duration::days(7))
        .unwrap();
    assert_eq!(ended.phase, EntitlementPhase::TrialExpired);
    assert!(!ended.purchase_allowed);
    assert!(expired.require_new_factory_mutation().is_err());
}
