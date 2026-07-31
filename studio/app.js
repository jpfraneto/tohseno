const ui = {
  studio: document.querySelector(".studio"),
  connection: document.querySelector("#connection"),
  onboarding: document.querySelector("#onboarding"),
  onboardingOpen: document.querySelector("#onboarding-open"),
  onboardingClose: document.querySelector("#onboarding-close"),
  onboardingBack: document.querySelector("#onboarding-back"),
  onboardingNext: document.querySelector("#onboarding-next"),
  onboardingPosition: document.querySelector("#onboarding-position"),
  onboardingXcode: document.querySelector("#onboarding-xcode"),
  onboardingXcodeDetail: document.querySelector("#onboarding-xcode-detail"),
  onboardingSigning: document.querySelector("#onboarding-signing"),
  onboardingSigningDetail: document.querySelector("#onboarding-signing-detail"),
  onboardingRefreshMac: document.querySelector("#onboarding-refresh-mac"),
  onboardingRefreshHarnesses: document.querySelector("#onboarding-refresh-harnesses"),
  onboardingCodex: document.querySelector("#onboarding-codex"),
  onboardingClaude: document.querySelector("#onboarding-claude"),
  onboardingReady: document.querySelector("#onboarding-ready"),
  newShot: document.querySelector("#new-shot"),
  appCount: document.querySelector("#app-count"),
  appGrid: document.querySelector("#app-grid"),
  noApps: document.querySelector("#no-apps"),
  selection: document.querySelector("#selection"),
  selectedIcon: document.querySelector("#selected-icon"),
  selectedName: document.querySelector("#selected-name"),
  selectedLocation: document.querySelector("#selected-location"),
  previousShot: document.querySelector("#previous-shot"),
  nextShot: document.querySelector("#next-shot"),
  shotPosition: document.querySelector("#shot-position"),
  evolve: document.querySelector("#evolve"),
  workingState: document.querySelector("#working-state"),
  recordEvolution: document.querySelector("#record-evolution"),
  openFolder: document.querySelector("#open-folder"),
  expiryState: document.querySelector("#expiry-state"),
  expiryLabel: document.querySelector("#expiry-label"),
  refreshApp: document.querySelector("#refresh-app"),
  memoryPanel: document.querySelector("#memory-panel"),
  memoryText: document.querySelector("#memory-text"),
  openSimulator: document.querySelector("#open-simulator"),
  slotLabel: document.querySelector("#slot-label"),
  slotDots: document.querySelector("#slot-dots"),
  latestEvent: document.querySelector("#latest-event"),
  simulatorTitle: document.querySelector("#simulator-title"),
  simulatorEmpty: document.querySelector("#simulator-empty"),
  runningApp: document.querySelector("#running-app"),
  simulatorLoading: document.querySelector("#simulator-loading"),
  simulatorScreen: document.querySelector("#simulator-screen"),
  showLibrary: document.querySelector("#show-library"),
  eventLog: document.querySelector("#events"),
  composer: document.querySelector("#composer"),
  composerSplitter: document.querySelector("#composer-splitter"),
  composerKicker: document.querySelector("#composer-kicker"),
  composerTitle: document.querySelector("#composer-title"),
  composerSupport: document.querySelector("#composer-support"),
  closeComposer: document.querySelector("#close-composer"),
  form: document.querySelector("#shot-form"),
  appNameLabel: document.querySelector("#app-name-label"),
  appName: document.querySelector("#app-name"),
  promptLabel: document.querySelector("#prompt-label"),
  prompt: document.querySelector("#prompt"),
  imageInput: document.querySelector("#images"),
  dropZone: document.querySelector("#drop-zone"),
  attachments: document.querySelector("#attachments"),
  harness: document.querySelector("#harness"),
  model: document.querySelector("#model"),
  route: document.querySelector("#route"),
  harnessStatus: document.querySelector("#harness-status"),
  planReview: document.querySelector("#plan-review"),
  planGenomeRevision: document.querySelector("#plan-genome-revision"),
  planGenome: document.querySelector("#plan-genome"),
  planExpression: document.querySelector("#plan-expression"),
  planCapabilities: document.querySelector("#plan-capabilities"),
  submit: document.querySelector("#submit"),
  librarySplitter: document.querySelector("#library-splitter"),
  protocolReadiness: document.querySelector("#protocol-readiness"),
  identityStatus: document.querySelector("#identity-status"),
  builderId: document.querySelector("#builder-id"),
  deviceStatus: document.querySelector("#device-status"),
  recoveryStatus: document.querySelector("#recovery-status"),
  identityDetail: document.querySelector("#identity-detail"),
  generationStatus: document.querySelector("#generation-status"),
  contractGeneration: document.querySelector("#contract-generation"),
  contractChain: document.querySelector("#contract-chain"),
  contractP256: document.querySelector("#contract-p256"),
  contractDefinitionDetail: document.querySelector("#contract-definition-detail"),
  nodeStatus: document.querySelector("#node-status"),
  nodeIdentity: document.querySelector("#node-identity"),
  nodeProtocol: document.querySelector("#node-protocol"),
  nodeReplicated: document.querySelector("#node-replicated"),
  nodeDetail: document.querySelector("#node-detail"),
  shotProtocol: document.querySelector("#shot-protocol"),
  shotState: document.querySelector("#shot-state"),
  shotId: document.querySelector("#shot-id"),
  evolutionStatus: document.querySelector("#evolution-status"),
  signatureStatus: document.querySelector("#signature-status"),
  fasciaStatus: document.querySelector("#fascia-status"),
  conformanceStatus: document.querySelector("#conformance-status"),
  shotAvailability: document.querySelector("#shot-availability"),
  shotProtocolDetail: document.querySelector("#shot-protocol-detail"),
  verifyShot: document.querySelector("#verify-shot"),
  protocolJson: document.querySelector("#protocol-json"),
  shotContinuity: document.querySelector("#shot-continuity"),
  continuityStatus: document.querySelector("#continuity-status"),
  continuityShotId: document.querySelector("#continuity-shot-id"),
  continuityExpression: document.querySelector("#continuity-expression"),
  continuityVersion: document.querySelector("#continuity-version"),
  continuityGenome: document.querySelector("#continuity-genome"),
  continuityLineage: document.querySelector("#continuity-lineage"),
  continuityToken: document.querySelector("#continuity-token"),
  continuityAvailability: document.querySelector("#continuity-availability"),
  intentionStatus: document.querySelector("#intention-status"),
  intentionText: document.querySelector("#intention-text"),
  genomeStatus: document.querySelector("#genome-status"),
  genomeText: document.querySelector("#genome-text"),
  continuityDetail: document.querySelector("#continuity-detail"),
  feedbackForm: document.querySelector("#feedback-form"),
  feedbackBinding: document.querySelector("#feedback-binding"),
  feedbackText: document.querySelector("#feedback-text"),
  feedbackSelectEvolution: document.querySelector("#feedback-select-evolution"),
  saveFeedback: document.querySelector("#save-feedback"),
  feedbackStatus: document.querySelector("#feedback-status"),
  launchToken: document.querySelector("#launch-token"),
  launchTokenLabel: document.querySelector("#launch-token-label"),
  launchTokenDetail: document.querySelector("#launch-token-detail"),
  bankrDialog: document.querySelector("#bankr-dialog"),
  bankrClose: document.querySelector("#bankr-close"),
  bankrForm: document.querySelector("#bankr-launch-form"),
  bankrStatus: document.querySelector("#bankr-status"),
  bankrConfiguration: document.querySelector("#bankr-configuration"),
  bankrShotName: document.querySelector("#bankr-shot-name"),
  bankrShotId: document.querySelector("#bankr-shot-id"),
  bankrShotVersion: document.querySelector("#bankr-shot-version"),
  bankrChain: document.querySelector("#bankr-chain"),
  bankrVesting: document.querySelector("#bankr-vesting"),
  bankrFeeMode: document.querySelector("#bankr-fee-mode"),
  bankrDescription: document.querySelector("#bankr-description"),
  bankrImage: document.querySelector("#bankr-image"),
  bankrWebsite: document.querySelector("#bankr-website"),
  bankrTweet: document.querySelector("#bankr-tweet"),
  bankrSimulate: document.querySelector("#bankr-simulate"),
  bankrSimulation: document.querySelector("#bankr-simulation"),
  bankrPredictedAddress: document.querySelector("#bankr-predicted-address"),
  bankrResolvedRecipient: document.querySelector("#bankr-resolved-recipient"),
  bankrConfigurationDigest: document.querySelector("#bankr-configuration-digest"),
  bankrFeeDistribution: document.querySelector("#bankr-fee-distribution"),
  bankrAcknowledge: document.querySelector("#bankr-acknowledge"),
  bankrConfirmationPhrase: document.querySelector("#bankr-confirmation-phrase"),
  bankrConfirmation: document.querySelector("#bankr-confirmation"),
  bankrDeploy: document.querySelector("#bankr-deploy"),
  bankrResult: document.querySelector("#bankr-result"),
  bankrResultTitle: document.querySelector("#bankr-result-title"),
  bankrResultSummary: document.querySelector("#bankr-result-summary"),
  bankrExplorer: document.querySelector("#bankr-explorer"),
  bankrResultJson: document.querySelector("#bankr-result-json"),
};

let library = { apps: [], iphone_slots_used: 0, iphone_slot_limit: 3 };
let selectedApp = null;
let selectedShot = null;
let composerMode = "create";
let composerAppName = null;
let reviewedInitialPlan = null;
let files = [];
let screenshotTimer = null;
let pendingShot = null;
let pressActive = false;
let shotCompleted = false;
let launchSequence = 0;
let protocolSequence = 0;
let protocolOverview = null;
let shotProtocol = null;
let nodeOverview = null;
let bankrOverview = null;
let bankrApproval = null;
let harnesses = [];
let onboardingFacts = null;
let onboardingStep = 1;
let executionPollToken = 0;
let observedExecutionEvents = 0;
const selectedFeedbackActions = new Map();

const studioJsonHeaders = {
  "content-type": "application/json",
  "x-tohseno-studio": "1",
};

const optionalBankrValue = (field) => {
  const value = field.value.trim();
  return value.length > 0 ? value : null;
};

const bankrParameters = () => ({
  description: ui.bankrDescription.value,
  image: optionalBankrValue(ui.bankrImage),
  tweet_url: optionalBankrValue(ui.bankrTweet),
  website_url: optionalBankrValue(ui.bankrWebsite),
  chain: ui.bankrChain.value,
  creator_vesting: ui.bankrVesting.value,
  creator_fee_mode: ui.bankrFeeMode.value,
});

const selectedLaunchBinding = () => {
  const ontology = shotProtocol?.ontology;
  const shotId = ontology?.shot_id;
  const versionOrdinal = Number(ontologyRecord(ontology?.version).ordinal);
  if (
    !selectedApp
    || !shotId
    || !/^0x[0-9a-f]{64}$/i.test(shotId)
    || !Number.isSafeInteger(versionOrdinal)
    || versionOrdinal < 1
    || versionOrdinal !== selectedShot
  ) return null;
  return {
    app_name: selectedApp.name,
    shot_id: shotId,
    version_ordinal: versionOrdinal,
  };
};

const renderTokenLaunchState = () => {
  const association = shotProtocol?.ontology?.token_association;
  if (association?.status === "associated") {
    ui.launchToken.disabled = true;
    ui.launchTokenLabel.textContent = "$TOHSENO is associated";
    ui.launchTokenDetail.textContent =
      `${association.symbol || "TOHSENO"} · ${displayIdentifier(association.token_address)}`;
    return;
  }
  const binding = selectedLaunchBinding();
  ui.launchToken.disabled = !binding;
  ui.launchTokenLabel.textContent = "Launch $TOHSENO for this Shot";
  ui.launchTokenDetail.textContent = binding
    ? `After deployment, record a private relation to ${displayIdentifier(binding.shot_id)}`
    : "A verified selected ShotID is required";
};

const resetBankrResult = () => {
  ui.bankrResult.hidden = true;
  ui.bankrResult.removeAttribute("data-status");
  ui.bankrResultTitle.textContent = "";
  ui.bankrResultSummary.textContent = "";
  ui.bankrResultJson.textContent = "";
  ui.bankrExplorer.hidden = true;
  ui.bankrExplorer.removeAttribute("href");
};

const updateBankrDeployState = () => {
  ui.bankrDeploy.disabled = !(
    bankrApproval
    && bankrOverview?.deploy_enabled
    && ui.bankrAcknowledge.checked
    && ui.bankrConfirmation.value === bankrApproval.confirmation_phrase
  );
};

const clearBankrApproval = () => {
  bankrApproval = null;
  ui.bankrSimulation.hidden = true;
  ui.bankrAcknowledge.checked = false;
  ui.bankrConfirmation.value = "";
  updateBankrDeployState();
};

const renderBankrStatus = () => {
  if (!bankrOverview) return;
  if (!bankrOverview.configured) {
    ui.bankrStatus.textContent = bankrOverview.configuration_error
      || "BANKR_API_KEY is not configured. This browser never receives the key.";
    ui.bankrStatus.dataset.status = "error";
    ui.bankrConfiguration.textContent =
      "Create a Bankr user API key with token-launch access and read-only disabled, export it in the terminal, then restart Studio.";
    ui.bankrSimulate.disabled = true;
    return;
  }
  ui.bankrStatus.textContent = bankrOverview.deploy_enabled
    ? "Bankr is configured. Simulation and the separately confirmed deployment are enabled."
    : "Bankr is configured for simulation. Deployment remains locked.";
  ui.bankrStatus.dataset.status = "ready";
  ui.bankrConfiguration.textContent = bankrOverview.deploy_enabled
    ? "The API key remains server-side. Every approval is single-use and expires after 10 minutes."
    : "To unlock the irreversible step, restart Studio with TOHSENO_ALLOW_BANKR_TOKEN_DEPLOY=1.";
  ui.bankrSimulate.disabled = false;
  updateBankrDeployState();
};

const loadBankrStatus = async () => {
  const response = await fetch("/api/bankr/launch", { cache: "no-store" });
  if (!response.ok) throw new Error(await response.text());
  bankrOverview = await response.json();
  renderBankrStatus();
};

const renderBankrSimulation = (approval) => {
  bankrApproval = approval;
  resetBankrResult();
  ui.bankrPredictedAddress.textContent =
    approval.bankr_simulation.tokenAddress || "Bankr did not return an address";
  ui.bankrResolvedRecipient.textContent =
    `${approval.fee_recipient_ens} · ${approval.fee_recipient_address}`;
  ui.bankrConfigurationDigest.textContent = approval.configuration_digest;
  ui.bankrFeeDistribution.textContent = JSON.stringify(
    approval.bankr_simulation.feeDistribution || {},
    null,
    2,
  );
  ui.bankrConfirmationPhrase.textContent = approval.confirmation_phrase;
  ui.bankrAcknowledge.checked = false;
  ui.bankrConfirmation.value = "";
  ui.bankrSimulation.hidden = false;
  updateBankrDeployState();
  ui.bankrSimulation.scrollIntoView({ behavior: "smooth", block: "nearest" });
};

const renderBankrDeployment = (outcome) => {
  const deployment = outcome.bankr_deployment || {};
  const tokenAddress = deployment.tokenAddress || "unknown token address";
  const transactionHash = deployment.txHash || "";
  const warnings = outcome.warnings || [];
  ui.bankrResult.hidden = false;
  ui.bankrResult.dataset.status = warnings.length > 0 ? "error" : "ready";
  ui.bankrResultTitle.textContent = warnings.length > 0
    ? "$TOHSENO deployed · verification attention"
    : "$TOHSENO deployed";
  ui.bankrResultSummary.textContent = warnings.length > 0
    ? `${tokenAddress}. ${warnings.join(" ")}`
    : `${tokenAddress} · private signed association recorded for Shot ${displayIdentifier(
      outcome.shot?.shot_id
    )}; no Shot registry transaction was sent.`;
  ui.bankrResultJson.textContent = JSON.stringify(outcome, null, 2);
  if (/^0x[0-9a-fA-F]{64}$/.test(transactionHash)) {
    const explorer = outcome.parameters.chain === "base"
      ? "https://basescan.org"
      : "https://robinhoodchain.blockscout.com";
    ui.bankrExplorer.href = `${explorer}/tx/${transactionHash}`;
    ui.bankrExplorer.hidden = false;
  }
  ui.bankrResult.scrollIntoView({ behavior: "smooth", block: "nearest" });
};

const escapeInitial = (name) => (name.trim()[0] || "T").toUpperCase();

const icon = (app, shot, className = "app-icon") => {
  const frame = document.createElement("span");
  frame.className = className;

  const fallback = document.createElement("span");
  fallback.className = "icon-fallback";
  fallback.textContent = escapeInitial(app.name);

  const image = document.createElement("img");
  image.alt = "";
  image.src = `/api/icon/${app.name}/${shot}`;
  image.addEventListener("error", () => image.remove());
  frame.append(fallback, image);

  if (app.retired && className === "app-icon") {
    const badge = document.createElement("span");
    badge.className = "local-badge";
    badge.title = "Saved locally, not installed on iPhone";
    frame.append(badge);
  }
  return frame;
};

const loadLibrary = async () => {
  const response = await fetch("/api/apps", { cache: "no-store" });
  if (!response.ok) throw new Error(await response.text());
  library = await response.json();

  if (selectedApp) {
    selectedApp = library.apps.find((app) => app.name === selectedApp.name) || null;
    if (selectedApp && !selectedApp.shots.includes(selectedShot)) {
      selectedShot = selectedApp.latest_evolution;
    }
  }
  renderLibrary();
  renderSelection();
  renderSlots();
};

const humanStatus = (value) => String(value || "unknown")
  .replaceAll("_", " ")
  .replace(/^\w/, (letter) => letter.toUpperCase());

const renderProtocolInspector = () => {
  ui.protocolJson.textContent = JSON.stringify({
    overview: protocolOverview,
    node: nodeOverview,
    selected_shot: shotProtocol,
  }, null, 2);
};

const displayIdentifier = (value) => {
  const text = String(value || "");
  return text.length > 24 ? `${text.slice(0, 12)}…${text.slice(-8)}` : text;
};

const valueStatus = (value, fallback = "unknown") => {
  if (typeof value === "string") return value;
  if (value && typeof value.status === "string") return value.status;
  if (value && typeof value.availability === "string") return value.availability;
  return fallback;
};

const ontologyRecord = (value) => value?.record || value || {};

const renderNodeStatus = () => {
  const node = nodeOverview || { configured: false };
  if (!node.configured) {
    ui.nodeStatus.textContent = "Optional";
    ui.nodeStatus.dataset.status = "pending";
    ui.nodeIdentity.textContent = "Not configured";
    ui.nodeIdentity.removeAttribute("title");
    ui.nodeProtocol.textContent = "Local only";
    ui.nodeReplicated.textContent = "Not queried";
    ui.nodeDetail.textContent =
      "Studio remains fully local. Configure a node only when this Mac should preserve eligible legacy evidence records.";
    renderProtocolInspector();
    return;
  }

  const reachable = node.reachable === true;
  ui.nodeStatus.textContent = reachable ? "Contributing" : "Unavailable";
  ui.nodeStatus.dataset.status = reachable ? "pass" : "pending";
  ui.nodeIdentity.textContent = displayIdentifier(node.identity) || "Configured";
  ui.nodeIdentity.title = node.identity || "";
  ui.nodeProtocol.textContent = node.protocol_version || "Unknown";
  const replicated = node.replicated_shots ?? node.replicated;
  ui.nodeReplicated.textContent = typeof replicated === "number"
    ? `${replicated} Shot${replicated === 1 ? "" : "s"} in local evidence`
    : humanStatus(replicated || "unknown");
  ui.nodeDetail.textContent = node.detail || (reachable
    ? "This node can validate and serve eligible evidence it possesses; active_generation is null."
    : "The configured node is unavailable. Studio and private Shot work remain usable.");
  renderProtocolInspector();
};

const loadNodeStatus = async () => {
  try {
    const response = await fetch("/api/node", { cache: "no-store" });
    if (response.status === 404) {
      nodeOverview = { configured: false };
    } else {
      if (!response.ok) throw new Error(await response.text());
      nodeOverview = await response.json();
    }
  } catch (error) {
    nodeOverview = {
      configured: true,
      reachable: false,
      detail: `Node status could not be read: ${error.message}`,
    };
  }
  renderNodeStatus();
};

const renderProtocolOverview = () => {
  if (!protocolOverview) return;
  const {
    identity,
    contract_definition: definition,
    active_generation: activeGeneration,
    publication,
  } = protocolOverview;
  const device = identity.device_keys[0];

  ui.protocolReadiness.textContent = `${definition.generation} · inactive`;
  ui.identityStatus.textContent = humanStatus(identity.status);
  ui.identityStatus.dataset.status = identity.status === "local_only" ? "pass" : "pending";
  ui.builderId.textContent = identity.builder_id || "Not created";
  ui.builderId.title = identity.builder_id || "";
  ui.deviceStatus.textContent = device
    ? `${device.label} · ${humanStatus(device.status)}`
    : "Pending";
  ui.deviceStatus.title = device?.key_id || "";
  ui.recoveryStatus.textContent = humanStatus(identity.recovery_status);
  ui.identityDetail.textContent = identity.detail;

  ui.generationStatus.textContent = activeGeneration ? "Active" : "Inactive";
  ui.generationStatus.dataset.status = activeGeneration ? "pass" : "pending";
  ui.contractGeneration.textContent =
    `${definition.generation} · protocol ${definition.protocol_major}`;
  ui.contractGeneration.title = definition.definition_digest;
  ui.contractChain.textContent = `eip155:${definition.chain_id}`;
  ui.contractP256.textContent =
    `${definition.p256.standard} · ${definition.p256.gas} gas`;
  ui.contractP256.title = definition.p256.address;
  ui.contractDefinitionDetail.textContent = activeGeneration
    ? definition.detail
    : `No public witness generation is active. ${definition.detail}. `
      + `Studio made no RPC call and has no deployment or broadcast path. ${publication.reason}`;
  renderProtocolInspector();
};

const loadProtocolOverview = async () => {
  const response = await fetch("/api/protocol", { cache: "no-store" });
  if (!response.ok) throw new Error(await response.text());
  protocolOverview = await response.json();
  renderProtocolOverview();
};

const exactIntentionText = (record) => {
  if (typeof record === "string") return record;
  if (!record || typeof record !== "object") return null;
  if (record.record) return exactIntentionText(record.record);
  for (const field of ["exact", "text", "inline_text"]) {
    if (typeof record[field] === "string") return record[field];
  }
  if (Array.isArray(record.materials)) {
    const inline = record.materials
      .map((material) => material?.inline_text)
      .filter((text) => typeof text === "string");
    if (inline.length > 0) return inline.join("\n\n");
  }
  return null;
};

const renderGenomeDocument = (accepted) => {
  const genome = accepted?.genome || ontologyRecord(accepted);
  if (!genome || typeof genome !== "object") return null;
  const lines = [];
  const add = (label, value) => {
    if (typeof value === "string" && value.length > 0) {
      lines.push(`${label}: ${value}`);
    } else if (Array.isArray(value) && value.length > 0) {
      lines.push(`${label}:\n${value.map((item) => `• ${item}`).join("\n")}`);
    }
  };
  add("Purpose", genome.purpose);
  add("For", genome.intended_for);
  add("Essential experience", genome.essential_experience);
  add("Must remain true", genome.behavioral_invariants);
  add("Must never happen", genome.forbidden_transformations);
  add("May change freely", genome.freely_changeable);
  add("Platform commitments", genome.platform_commitments);
  add("Privacy", genome.privacy_principles);
  add("Ownership", genome.ownership_principles);
  return lines.length > 0 ? lines.join("\n\n") : null;
};

const exactFeedbackBinding = (ontology) => {
  const version = ontologyRecord(ontology?.version);
  const expression = ontologyRecord(ontology?.expression);
  const ordinal = Number(version?.ordinal);
  const expressionId = version.expression_id || expression.expression_id;
  if (
    !version.version_id
    || !expressionId
    || (version.expression_id && expression.expression_id
      && version.expression_id !== expression.expression_id)
    || !Number.isSafeInteger(ordinal)
    || ordinal < 1
    || ordinal !== selectedShot
  ) return null;
  return {
    expressionId,
    ordinal,
    versionId: version.version_id,
  };
};

const setFeedbackAvailability = (ontology) => {
  const binding = exactFeedbackBinding(ontology);
  const canSelectForEvolution = Boolean(
    binding && selectedApp && binding.ordinal === selectedApp.latest_evolution
  );
  ui.feedbackText.disabled = !binding;
  ui.feedbackSelectEvolution.disabled = !canSelectForEvolution;
  if (!canSelectForEvolution) ui.feedbackSelectEvolution.checked = false;
  ui.saveFeedback.disabled = !binding || ui.feedbackText.value.trim().length === 0;
  ui.feedbackBinding.textContent = binding
    ? `Private feedback will bind to expression ${displayIdentifier(
      binding.expressionId
    )}, version ${String(binding.ordinal).padStart(4, "0")} (${displayIdentifier(binding.versionId)}).`
    : "This record does not expose an exact expression version, so Studio will not accept floating Shot-level feedback.";
  if (!binding) ui.feedbackText.value = "";
};

const renderShotContinuity = () => {
  if (!selectedApp || !selectedShot) {
    ui.shotContinuity.hidden = true;
    setFeedbackAvailability(null);
    return;
  }

  ui.shotContinuity.hidden = false;
  const ontology = shotProtocol?.ontology;
  if (!ontology) {
    const legacy = shotProtocol?.evolution;
    ui.continuityStatus.textContent = legacy ? "Legacy v1" : "Unavailable";
    ui.continuityStatus.dataset.status = "pending";
    ui.continuityShotId.textContent = displayIdentifier(legacy?.shot_id) || "Unavailable";
    ui.continuityShotId.title = legacy?.shot_id || "";
    ui.continuityExpression.textContent = `${selectedApp.name} · Apple app`;
    ui.continuityVersion.textContent = legacy
      ? `Evolution ${legacy.sequence}`
      : `Ledger evolution ${selectedShot}`;
    ui.continuityGenome.textContent = "Unknown";
    ui.continuityLineage.textContent = legacy?.commitment
      ? `v1 · ${displayIdentifier(legacy.commitment)}`
      : "Not available";
    ui.continuityToken.textContent = "Unknown in v1 compatibility record";
    ui.continuityToken.title = "";
    ui.continuityAvailability.textContent = shotProtocol
      ? humanStatus(shotProtocol.local_state)
      : "Checking";
    ui.intentionStatus.textContent = "Unknown";
    ui.intentionText.textContent =
      "This historical record does not contain the original coherent intention. Studio does not invent one during adoption.";
    ui.genomeStatus.textContent = "Unknown";
    ui.genomeText.textContent =
      "This historical record does not contain an accepted Shot genome. Existing v1 continuity remains valid without fabricated history.";
    ui.continuityDetail.textContent = legacy
      ? "This is a compatible signed v1 Evolution. Its source and signature remain authoritative while richer lineage facts are absent."
      : "No current lineage projection is available for this folder.";
    setFeedbackAvailability(null);
    return;
  }

  const expression = ontologyRecord(ontology.expression);
  const version = ontologyRecord(ontology.version);
  const lineage = ontology.lineage || {};
  const tokenAssociation = ontology.token_association || {};
  const intention = ontology.original_intention;
  const genome = ontology.accepted_genome;
  const intentionText = exactIntentionText(intention);
  const genomeText = renderGenomeDocument(genome);
  const lineageVerification = valueStatus(lineage.verification, ontology.status);
  const verified = ["pass", "valid", "verified"].includes(lineageVerification);
  const shotId = ontology.shot_id || shotProtocol?.evolution?.shot_id;
  const expressionId = expression.expression_id || version.expression_id;
  const ordinal = Number(version.ordinal);

  ui.continuityStatus.textContent = verified ? "Verified" : humanStatus(ontology.status);
  ui.continuityStatus.dataset.status = verified ? "pass" : "pending";
  ui.continuityShotId.textContent = displayIdentifier(shotId) || "Unavailable";
  ui.continuityShotId.title = shotId || "";
  ui.continuityExpression.textContent = [
    expression.name,
    expression.kind,
    displayIdentifier(expressionId),
  ].filter(Boolean).join(" · ") || "Unavailable";
  ui.continuityExpression.title = expressionId || "";
  ui.continuityVersion.textContent = Number.isSafeInteger(ordinal) && ordinal > 0
    ? `${String(ordinal).padStart(4, "0")} · ${displayIdentifier(version.version_id)}`
    : displayIdentifier(version.version_id) || "Unavailable";
  ui.continuityVersion.title = version.version_id || "";
  const accepted = genome?.genome || ontologyRecord(genome);
  ui.continuityGenome.textContent = accepted?.revision
    ? `Revision ${accepted.revision} · ${displayIdentifier(
      version.genome_digest || genome?.genome_digest
    )}`
    : humanStatus(valueStatus(genome));
  ui.continuityLineage.textContent = lineage.sequence
    ? `${lineage.sequence} actions · ${displayIdentifier(lineage.head)}`
    : displayIdentifier(lineage.head) || humanStatus(lineageVerification);
  ui.continuityLineage.title = lineage.head || "";
  if (
    tokenAssociation.status === "associated"
    && Number.isSafeInteger(Number(tokenAssociation.chain_id))
    && tokenAssociation.token_address
  ) {
    const symbol = tokenAssociation.symbol ? `${tokenAssociation.symbol} · ` : "";
    ui.continuityToken.textContent =
      `${symbol}eip155:${tokenAssociation.chain_id} · ${displayIdentifier(tokenAssociation.token_address)}`;
    ui.continuityToken.title =
      `${tokenAssociation.token_address} · relationship only; never Shot identity or ownership`;
  } else {
    const history = Number(tokenAssociation.history_count);
    ui.continuityToken.textContent = Number.isSafeInteger(history) && history > 0
      ? `None current · ${history} historical action${history === 1 ? "" : "s"}`
      : "None";
    ui.continuityToken.title = "A Shot does not require a token.";
  }
  ui.continuityAvailability.textContent = humanStatus(
    valueStatus(lineage.availability, valueStatus(ontology.availability))
  );
  ui.intentionStatus.textContent = humanStatus(
    valueStatus(intention, intentionText ? "locally_available" : "unknown")
  );
  ui.intentionText.textContent = intentionText ||
    "The original intention is not available to this local Studio.";
  ui.genomeStatus.textContent = accepted?.revision
    ? `Revision ${accepted.revision}`
    : humanStatus(valueStatus(genome, genomeText ? "locally_available" : "unknown"));
  ui.genomeText.textContent = genomeText ||
    "The accepted genome is not available to this local Studio.";
  ui.continuityDetail.textContent = ontology.detail ||
    "These are derived views of signed append-only lineage. The advanced inspector retains the exact local response.";
  setFeedbackAvailability(ontology);
};

const renderShotProtocol = () => {
  if (!shotProtocol) {
    ui.shotProtocol.hidden = true;
    renderTokenLaunchState();
    renderShotContinuity();
    renderProtocolInspector();
    return;
  }
  ui.shotProtocol.hidden = false;
  const evolution = shotProtocol.evolution;
  const verified = shotProtocol.verification.status === "pass";
  ui.shotState.textContent = shotProtocol.adoption_required
    ? "Needs adoption"
    : verified
      ? "Verified · Private"
      : "Private";
  ui.shotState.dataset.status = verified ? "pass" : "pending";
  ui.shotId.textContent = evolution?.shot_id || "Legacy, unsigned";
  ui.shotId.title = evolution?.shot_id || "";
  ui.evolutionStatus.textContent = evolution
    ? `Evolution ${evolution.sequence}${shotProtocol.current ? " · current" : ""}`
    : "Not yet in protocol";
  ui.signatureStatus.textContent = shotProtocol.signature.status === "valid"
    ? "Verified"
    : humanStatus(shotProtocol.signature.status);
  ui.fasciaStatus.textContent = shotProtocol.fascia.status === "valid"
    ? shotProtocol.fascia.id
    : humanStatus(shotProtocol.fascia.status);
  ui.conformanceStatus.textContent = shotProtocol.conformance.status === "pass"
    ? `Verified · ${shotProtocol.conformance.passed} checks`
    : humanStatus(shotProtocol.conformance.status);
  ui.shotAvailability.textContent = humanStatus(shotProtocol.local_state);
  ui.shotProtocolDetail.textContent = shotProtocol.adoption_required
    ? shotProtocol.verification.detail
    : `${shotProtocol.verification.detail} No public witness generation is active.`;
  ui.verifyShot.disabled = false;
  ui.verifyShot.textContent = "Verify";
  renderTokenLaunchState();
  renderShotContinuity();
  renderProtocolInspector();
};

const loadShotProtocol = async (app, shot) => {
  const sequence = ++protocolSequence;
  ui.shotProtocol.hidden = false;
  ui.shotState.textContent = "Checking";
  ui.verifyShot.disabled = true;
  ui.verifyShot.textContent = "Verifying…";
  ui.feedbackText.value = "";
  ui.feedbackStatus.textContent = "";
  shotProtocol = null;
  renderTokenLaunchState();
  renderShotContinuity();
  try {
    const response = await fetch(`/api/protocol/shot/${app.name}/${shot}`, { cache: "no-store" });
    if (!response.ok) throw new Error(await response.text());
    const payload = await response.json();
    if (sequence !== protocolSequence) return;
    shotProtocol = payload;
    renderShotProtocol();
  } catch (error) {
    if (sequence !== protocolSequence) return;
    shotProtocol = null;
    ui.shotProtocol.hidden = false;
    ui.shotState.textContent = "Unavailable";
    ui.shotState.dataset.status = "fail";
    ui.shotProtocolDetail.textContent = `Protocol facts unavailable: ${error.message}`;
    ui.verifyShot.disabled = false;
    ui.verifyShot.textContent = "Try verification again";
    renderTokenLaunchState();
    renderShotContinuity();
    renderProtocolInspector();
  }
};

ui.feedbackText.addEventListener("input", () => {
  const binding = exactFeedbackBinding(shotProtocol?.ontology);
  ui.saveFeedback.disabled = !binding || ui.feedbackText.value.trim().length === 0;
  ui.feedbackStatus.textContent = "";
});

ui.feedbackForm.addEventListener("submit", async (event) => {
  event.preventDefault();
  const binding = exactFeedbackBinding(shotProtocol?.ontology);
  const text = ui.feedbackText.value.trim();
  if (!selectedApp || !binding || !text) return;

  ui.feedbackText.disabled = true;
  ui.saveFeedback.disabled = true;
  ui.saveFeedback.textContent = "Saving…";
  ui.feedbackStatus.textContent = "";
  try {
    const response = await fetch("/api/feedback", {
      method: "POST",
      headers: studioJsonHeaders,
      body: JSON.stringify({
        app_name: selectedApp.name,
        version_ordinal: binding.ordinal,
        text,
      }),
    });
    if (!response.ok) throw new Error(await response.text());
    const saved = await response.json();
    const selectForEvolution = ui.feedbackSelectEvolution.checked;
    if (selectForEvolution) {
      if (!/^0x[0-9a-f]{64}$/.test(saved.action_commitment)) {
        throw new Error("the signed Feedback action commitment was invalid");
      }
      const key = `${selectedApp.name}:${binding.ordinal}`;
      const selected = selectedFeedbackActions.get(key) || new Set();
      selected.add(saved.action_commitment);
      selectedFeedbackActions.set(key, selected);
    }
    ui.feedbackText.value = "";
    ui.feedbackSelectEvolution.checked = false;
    ui.feedbackStatus.textContent =
      `Saved privately as ${displayIdentifier(saved.feedback_id)} for version ${String(binding.ordinal).padStart(4, "0")}`
      + (selectForEvolution
        ? ` and selected signed action ${displayIdentifier(saved.action_commitment)} for its next evolution.`
        : ".");
    appendEvent("status", `private feedback saved for ${selectedApp.name} version ${String(binding.ordinal).padStart(4, "0")}.`);
  } catch (error) {
    ui.feedbackStatus.textContent = `Feedback was not saved: ${error.message}`;
  } finally {
    ui.feedbackText.disabled = false;
    ui.saveFeedback.textContent = "Save private feedback";
    ui.saveFeedback.disabled = ui.feedbackText.value.trim().length === 0;
  }
});

ui.recordEvolution.addEventListener("click", async () => {
  if (!selectedApp) return;
  ui.recordEvolution.disabled = true;
  try {
    await fetch("/api/evolve", {
      method: "POST",
      headers: studioJsonHeaders,
      body: JSON.stringify({ app_name: selectedApp.name }),
    });
  } finally {
    setTimeout(() => {
      ui.recordEvolution.disabled = false;
      loadLibrary();
    }, 1500);
  }
});

ui.openFolder.addEventListener("click", () => {
  if (!selectedApp) return;
  fetch("/api/open", {
    method: "POST",
    headers: studioJsonHeaders,
    body: JSON.stringify({ app_name: selectedApp.name }),
  });
});

ui.refreshApp.addEventListener("click", async () => {
  if (!selectedApp) return;
  ui.refreshApp.disabled = true;
  try {
    await fetch("/api/refresh", {
      method: "POST",
      headers: studioJsonHeaders,
      body: JSON.stringify({ app_name: selectedApp.name }),
    });
  } finally {
    setTimeout(() => {
      ui.refreshApp.disabled = false;
      loadLibrary();
    }, 1500);
  }
});

ui.verifyShot.addEventListener("click", () => {
  if (selectedApp && selectedShot) loadShotProtocol(selectedApp, selectedShot);
});

const renderLibrary = () => {
  const tiles = library.apps.map((app) => {
    const tile = document.createElement("button");
    tile.type = "button";
    tile.className = "app-tile";
    if (selectedApp?.name === app.name) tile.classList.add("selected");
    tile.setAttribute("aria-label", `Run ${app.name}, evolution ${app.latest_evolution}`);
    tile.setAttribute("aria-pressed", selectedApp?.name === app.name ? "true" : "false");

    const name = document.createElement("strong");
    name.className = "app-name";
    name.textContent = app.name;

    const meta = document.createElement("span");
    meta.className = "app-meta";
    meta.textContent = app.shots.length === 1 ? "1 evolution" : `${app.shots.length} evolutions`;

    tile.append(icon(app, app.latest_evolution), name, meta);
    tile.addEventListener("click", () => selectApp(app, app.latest_evolution));
    return tile;
  });

  ui.appGrid.replaceChildren(...tiles);
  ui.noApps.hidden = library.apps.length > 0;
  ui.appCount.textContent = library.apps.length === 1 ? "1 app" : `${library.apps.length} apps`;
};

const renderSlots = () => {
  ui.slotLabel.textContent = `${library.iphone_slots_used} of ${library.iphone_slot_limit} installed`;
  ui.slotDots.replaceChildren(...Array.from({ length: library.iphone_slot_limit }, (_, index) => {
    const dot = document.createElement("span");
    dot.className = `slot-dot${index < library.iphone_slots_used ? " used" : ""}`;
    return dot;
  }));
};

const renderSelection = () => {
  if (!selectedApp) {
    ui.selection.hidden = true;
    renderTokenLaunchState();
    return;
  }

  const index = selectedApp.shots.indexOf(selectedShot);
  ui.selection.hidden = false;
  ui.selectedIcon.replaceChildren(...icon(selectedApp, selectedShot, "selected-icon").childNodes);
  ui.selectedName.textContent = selectedApp.name;
  ui.selectedLocation.textContent = selectedApp.retired ? "Local library" : "Installed on iPhone";
  ui.shotPosition.textContent = `Evolution ${selectedShot} of ${selectedApp.latest_evolution}`;
  ui.workingState.hidden = !selectedApp.unrecorded_changes;
  const days = selectedApp.expires_in_days;
  if (typeof days === "number" && days <= 7) {
    ui.expiryState.hidden = false;
    ui.expiryState.classList.toggle("expired", days <= 0);
    ui.expiryLabel.textContent =
      days <= 0 ? "this install has died" :
      days === 1 ? "dies tomorrow" :
      `dies in ${days} days`;
  } else {
    ui.expiryState.hidden = true;
  }
  if (selectedApp.memory) {
    ui.memoryPanel.hidden = false;
    ui.memoryText.textContent = selectedApp.memory;
  } else {
    ui.memoryPanel.hidden = true;
  }
  ui.previousShot.disabled = index <= 0;
  ui.nextShot.disabled = index < 0 || index >= selectedApp.shots.length - 1;
  renderTokenLaunchState();
};

const selectApp = async (app, shot) => {
  selectedApp = app;
  selectedShot = shot;
  shotProtocol = null;
  renderLibrary();
  renderSelection();
  loadShotProtocol(app, shot);

  ui.simulatorEmpty.hidden = true;
  ui.runningApp.hidden = false;
  ui.showLibrary.hidden = false;
  ui.simulatorTitle.textContent = `${app.name} · Evolution ${shot}`;
  ui.simulatorLoading.hidden = false;
  ui.simulatorLoading.querySelector("strong").textContent = "Opening evolution…";
  ui.simulatorLoading.querySelector("span").textContent = "Building for Simulator";
  ui.simulatorScreen.removeAttribute("src");

  stopScreenshots();
  const sequence = ++launchSequence;
  try {
    const response = await fetch("/api/simulator/launch", {
      method: "POST",
      headers: studioJsonHeaders,
      body: JSON.stringify({ app_name: app.name, shot }),
    });
    if (!response.ok) throw new Error(await response.text());
    if (sequence !== launchSequence) return;
    refreshScreenshot(sequence);
    screenshotTimer = setInterval(() => refreshScreenshot(sequence), 850);
  } catch (error) {
    if (sequence !== launchSequence) return;
    appendEvent("status", `Simulator stopped: ${error.message}`);
    ui.simulatorLoading.querySelector("strong").textContent = "Could not open this evolution";
    ui.simulatorLoading.querySelector("span").textContent = "Read the live press for details";
  }
};

const refreshScreenshot = (sequence) => {
  const image = new Image();
  image.onload = () => {
    if (sequence !== launchSequence) return;
    ui.simulatorScreen.src = image.src;
    ui.simulatorLoading.hidden = true;
  };
  image.src = `/api/simulator/screen?t=${Date.now()}`;
};

const stopScreenshots = () => {
  if (screenshotTimer) clearInterval(screenshotTimer);
  screenshotTimer = null;
};

const showSimulatorEmpty = () => {
  stopScreenshots();
  launchSequence += 1;
  selectedApp = null;
  selectedShot = null;
  protocolSequence += 1;
  shotProtocol = null;
  ui.feedbackText.value = "";
  ui.feedbackStatus.textContent = "";
  renderShotProtocol();
  renderLibrary();
  renderSelection();
  ui.runningApp.hidden = true;
  ui.simulatorEmpty.hidden = false;
  ui.showLibrary.hidden = true;
  ui.simulatorTitle.textContent = "No Shot selected";
};

ui.previousShot.addEventListener("click", () => {
  const index = selectedApp.shots.indexOf(selectedShot);
  if (index > 0) selectApp(selectedApp, selectedApp.shots[index - 1]);
});

ui.nextShot.addEventListener("click", () => {
  const index = selectedApp.shots.indexOf(selectedShot);
  if (index < selectedApp.shots.length - 1) selectApp(selectedApp, selectedApp.shots[index + 1]);
});

ui.showLibrary.addEventListener("click", showSimulatorEmpty);

ui.openSimulator.addEventListener("click", async () => {
  await fetch("/api/simulator/focus", {
    method: "POST",
    headers: studioJsonHeaders,
    body: "{}",
  });
});

const selectedHarness = () => harnesses.find((item) => item.id === ui.harness.value);
const selectedRoute = () => selectedHarness()?.routes.find((item) => item.id === ui.route.value);

const costLabel = () => {
  const estimate = selectedRoute()?.estimated_additional_cost_usd;
  return typeof estimate === "number" ? `$${estimate.toFixed(2)}` : "USAGE-BASED";
};

const renderRoutes = () => {
  const harness = selectedHarness();
  ui.route.replaceChildren(...(harness?.routes || []).map((route) => {
    const option = document.createElement("option");
    option.value = route.id;
    option.disabled = !route.available;
    const estimate = typeof route.estimated_additional_cost_usd === "number"
      ? `$${route.estimated_additional_cost_usd.toFixed(2)} additional`
      : "usage-based";
    option.textContent = `${route.label} · ${estimate}`;
    return option;
  }));
  const available = harness?.routes.find((route) => route.available);
  if (available) ui.route.value = available.id;
  const route = selectedRoute();
  ui.harnessStatus.textContent = route
    ? `${harness.label} · ${harness.authentication.replaceAll("_", " ")} · ${route.billing} billing`
    : `${harness?.label || "Harness"} has no authenticated route on this machine.`;
  updateSubmitState();
};

const renderModels = () => {
  const harness = selectedHarness();
  ui.model.replaceChildren(...(harness?.models || []).map((model) => {
    const option = document.createElement("option");
    option.value = model.id;
    option.textContent = model.label;
    return option;
  }));
  renderRoutes();
};

const loadHarnesses = async () => {
  const response = await fetch("/api/harnesses");
  if (!response.ok) throw new Error(await response.text());
  harnesses = (await response.json()).harnesses;
  ui.harness.replaceChildren(...harnesses.map((harness) => {
    const option = document.createElement("option");
    option.value = harness.id;
    option.disabled = !harness.installed;
    option.textContent = `${harness.label}${harness.installed ? "" : " · unavailable"}`;
    return option;
  }));
  const preferred = harnesses.find((harness) => harness.selected && harness.installed)
    || harnesses.find((harness) => harness.installed);
  if (preferred) ui.harness.value = preferred.id;
  renderModels();
  renderOnboarding();
};

const updateSubmitState = () => {
  const validName = composerMode === "evolve" || (ui.appName.validity.valid && ui.appName.value.length > 0);
  const ready = validName
    && ui.prompt.value.trim().length > 0
    && selectedHarness()?.installed
    && selectedRoute()?.available
    && !pressActive
    && !shotCompleted;
  ui.submit.disabled = !ready;
};

const restingSubmitLabel = () => (
  composerMode === "create" && !reviewedInitialPlan
    ? "REVIEW SHOT PLAN"
    : `PREPARE SHOT (${costLabel()})`
);

const setComposerBusy = (busy, busyLabel = "Preparing Shot…") => {
  pressActive = busy;
  ui.form.setAttribute("aria-busy", String(busy));
  ui.appName.disabled = busy;
  ui.prompt.disabled = busy;
  ui.imageInput.disabled = busy;
  ui.harness.disabled = busy;
  ui.model.disabled = busy;
  ui.route.disabled = busy;
  ui.dropZone.setAttribute("aria-disabled", String(busy));
  ui.submit.textContent = busy ? busyLabel : restingSubmitLabel();
  updateSubmitState();
};

const clearInitialPlanReview = () => {
  reviewedInitialPlan = null;
  ui.planReview.hidden = true;
  ui.planGenome.textContent = "";
  ui.planExpression.textContent = "";
  ui.planCapabilities.textContent = "";
  if (!pressActive && !shotCompleted) ui.submit.textContent = restingSubmitLabel();
};

const renderInitialPlanReview = (plan, appName, prompt) => {
  const expression = plan.expression_plan;
  reviewedInitialPlan = { appName, prompt, plan };
  ui.planGenomeRevision.textContent = `Revision ${plan.genome.revision}`;
  ui.planGenome.textContent = plan.genome_markdown;
  ui.planExpression.textContent =
    `${expression.name} · ${expression.kind.replaceAll("_", " ")} · ${expression.platforms.join(", ")}`;
  ui.planCapabilities.textContent = expression.organs
    .map((organ) => `${organ.organ_id}: ${organ.provides.join(", ")}`)
    .join(" · ");
  ui.planReview.hidden = false;
};

const openComposer = (mode) => {
  composerMode = mode;
  composerAppName = mode === "evolve" ? selectedApp.name : null;
  files = [];
  shotCompleted = false;
  clearInitialPlanReview();
  renderFiles();
  ui.form.reset();

  if (mode === "create") {
    ui.composerKicker.textContent = "CREATE";
    ui.composerTitle.textContent = "New Shot";
    ui.composerSupport.textContent = "Make the app no company would.";
    ui.appNameLabel.hidden = false;
    ui.appName.required = true;
    ui.promptLabel.textContent = "Make the intention exact";
  } else {
    const selectionKey = `${composerAppName}:${selectedApp.latest_evolution}`;
    const selectedCount = selectedFeedbackActions.get(selectionKey)?.size || 0;
    ui.composerKicker.textContent = `SHOT ${selectedApp.latest_evolution + 1}`;
    ui.composerTitle.textContent = `Evolve ${composerAppName}`;
    ui.composerSupport.textContent = selectedCount > 0
      ? `${selectedCount} exact-version feedback action${selectedCount === 1 ? "" : "s"} selected.`
      : "Use teaches the next evolution.";
    ui.appNameLabel.hidden = true;
    ui.appName.required = false;
    ui.appName.value = composerAppName;
    ui.promptLabel.textContent = "What should change?";
  }

  ui.composer.hidden = false;
  ui.composerSplitter.hidden = false;
  ui.studio.classList.add("composer-open");
  setComposerBusy(false);
  setTimeout(() => (mode === "create" ? ui.appName : ui.prompt).focus(), 0);
};

const closeComposer = () => {
  ui.composer.hidden = true;
  ui.composerSplitter.hidden = true;
  ui.studio.classList.remove("composer-open");
};

ui.newShot.addEventListener("click", () => openComposer("create"));
ui.evolve.addEventListener("click", () => openComposer("evolve"));
ui.closeComposer.addEventListener("click", closeComposer);

const onboardingProgress = [...document.querySelectorAll("[data-onboarding-progress]")];
const onboardingSteps = [...document.querySelectorAll("[data-onboarding-step]")];
const onboardingCompletionKey = "tohseno-first-shot-onboarding-complete";
const onboardingStepKey = "tohseno-first-shot-onboarding-step";

const subscriptionReady = (harness) => harness?.installed && harness.routes.some((route) => (
  route.available
  && route.billing === "subscription"
  && route.estimated_additional_cost_usd === 0
));

const renderHarnessOnboarding = (card, harness) => {
  const status = card.querySelector(".harness-onboarding-status");
  if (!harness?.installed) {
    card.dataset.status = "action";
    status.textContent = "Not installed";
  } else if (subscriptionReady(harness)) {
    card.dataset.status = "ready";
    status.textContent = "$0.00 subscription route ready";
  } else {
    card.dataset.status = "action";
    status.textContent = "Installed · sign in, then check again";
  }
};

const onboardingStepReady = () => {
  if (onboardingStep === 1) return true;
  if (onboardingStep === 2) {
    return onboardingFacts?.xcode.ready && onboardingFacts?.apple_signing.ready;
  }
  if (onboardingStep === 3) return onboardingFacts?.harness_ready === true;
  return onboardingFacts?.ready_for_first_shot === true;
};

const renderOnboarding = () => {
  onboardingSteps.forEach((step) => {
    step.hidden = Number(step.dataset.onboardingStep) !== onboardingStep;
  });
  onboardingProgress.forEach((marker) => {
    const position = Number(marker.dataset.onboardingProgress);
    marker.dataset.status = position < onboardingStep
      ? "complete"
      : (position === onboardingStep ? "current" : "pending");
  });
  ui.onboardingBack.hidden = onboardingStep === 1;
  ui.onboardingPosition.textContent = `Step ${onboardingStep} of 4`;
  ui.onboardingNext.textContent = onboardingStep === 4 ? "BEGIN FIRST SHOT" : "Continue";
  ui.onboardingNext.disabled = !onboardingStepReady();

  if (!onboardingFacts) return;
  ui.onboardingXcode.dataset.status = onboardingFacts.xcode.ready ? "ready" : "action";
  ui.onboardingXcodeDetail.textContent = onboardingFacts.xcode.detail;
  ui.onboardingSigning.dataset.status = onboardingFacts.apple_signing.ready ? "ready" : "action";
  ui.onboardingSigningDetail.textContent = onboardingFacts.apple_signing.detail;
  renderHarnessOnboarding(
    ui.onboardingCodex,
    harnesses.find((harness) => harness.id === "codex"),
  );
  renderHarnessOnboarding(
    ui.onboardingClaude,
    harnesses.find((harness) => harness.id === "claude-code"),
  );
  ui.onboardingReady.dataset.status = onboardingFacts.ready_for_first_shot
    ? "ready"
    : "action";
  ui.onboardingReady.textContent = onboardingFacts.ready_for_first_shot
    ? "This Mac is ready. The first protocol action will create your local Builder identity (private to this Mac; public authority stays closed until a contract generation is activated)."
    : "Finish the Apple and harness steps before taking the first Shot.";
};

const showOnboarding = () => {
  const savedStep = Number.parseInt(localStorage.getItem(onboardingStepKey), 10);
  onboardingStep = Number.isInteger(savedStep) && savedStep >= 1 && savedStep <= 4
    ? savedStep
    : 1;
  ui.onboarding.hidden = false;
  ui.studio.inert = true;
  renderOnboarding();
  ui.onboardingNext.focus();
};

const hideOnboarding = () => {
  ui.onboarding.hidden = true;
  ui.studio.inert = false;
  ui.onboardingOpen.focus();
};

const loadOnboarding = async ({ autoOpen = false } = {}) => {
  const response = await fetch("/api/onboarding", { cache: "no-store" });
  if (!response.ok) throw new Error(await response.text());
  onboardingFacts = await response.json();
  renderOnboarding();
  if (
    autoOpen
    && onboardingFacts.first_run
    && localStorage.getItem(onboardingCompletionKey) !== "1"
  ) {
    showOnboarding();
  }
};

const refreshOnboarding = async (button) => {
  const previous = button.textContent;
  button.disabled = true;
  button.textContent = "Checking…";
  try {
    await Promise.all([loadHarnesses(), loadOnboarding()]);
  } catch (error) {
    appendEvent("status", `readiness check failed: ${error.message}`);
  } finally {
    button.disabled = false;
    button.textContent = previous;
    renderOnboarding();
  }
};

ui.onboardingOpen.addEventListener("click", showOnboarding);
ui.onboardingClose.addEventListener("click", hideOnboarding);
ui.onboardingBack.addEventListener("click", () => {
  onboardingStep = Math.max(1, onboardingStep - 1);
  localStorage.setItem(onboardingStepKey, String(onboardingStep));
  renderOnboarding();
});
ui.onboardingNext.addEventListener("click", () => {
  if (!onboardingStepReady()) return;
  if (onboardingStep < 4) {
    onboardingStep += 1;
    localStorage.setItem(onboardingStepKey, String(onboardingStep));
    renderOnboarding();
    return;
  }
  localStorage.setItem(onboardingCompletionKey, "1");
  localStorage.removeItem(onboardingStepKey);
  hideOnboarding();
  openComposer("create");
});
ui.onboardingRefreshMac.addEventListener(
  "click",
  () => refreshOnboarding(ui.onboardingRefreshMac),
);
ui.onboardingRefreshHarnesses.addEventListener(
  "click",
  () => refreshOnboarding(ui.onboardingRefreshHarnesses),
);
document.querySelectorAll(".copy-harness-command").forEach((button) => {
  button.addEventListener("click", async () => {
    const previous = button.textContent;
    try {
      await navigator.clipboard.writeText(button.dataset.command);
      button.textContent = "Copied · paste in Terminal";
    } catch (error) {
      appendEvent("status", `could not copy the install command: ${error.message}`);
      button.textContent = "Copy failed";
    }
    setTimeout(() => {
      button.textContent = previous;
    }, 1800);
  });
});

const renderFiles = () => {
  ui.attachments.replaceChildren(...files.map((file) => {
    const chip = document.createElement("span");
    chip.className = "attachment";
    chip.textContent = file.name;
    return chip;
  }));
};

const acceptFiles = (incoming) => {
  if (pressActive) return;
  const supported = /\.(png|jpe?g|heic|webp)$/i;
  const incomingFiles = Array.from(incoming);
  const accepted = incomingFiles.filter((file) => supported.test(file.name));
  const rejected = incomingFiles.filter((file) => !supported.test(file.name));
  if (rejected.length > 0) {
    appendEvent(
      "status",
      `${rejected.length} attachment${rejected.length === 1 ? "" : "s"} rejected: use PNG, JPEG, HEIC, or WebP.`
    );
  }
  const available = 8 - files.length;
  files = [...files, ...accepted.slice(0, available)];
  if (accepted.length > available) {
    appendEvent("status", "Eight reference images are attached; additional files were not attached.");
  }
  renderFiles();
};

ui.dropZone.addEventListener("click", () => {
  if (!pressActive) ui.imageInput.click();
});
ui.dropZone.addEventListener("keydown", (event) => {
  if (!pressActive && (event.key === "Enter" || event.key === " ")) ui.imageInput.click();
});
ui.imageInput.addEventListener("change", () => acceptFiles(ui.imageInput.files));
ui.harness.addEventListener("change", renderModels);
ui.model.addEventListener("change", updateSubmitState);
ui.route.addEventListener("change", () => {
  const harness = selectedHarness();
  const route = selectedRoute();
  ui.harnessStatus.textContent = route
    ? `${harness.label} · ${harness.authentication.replaceAll("_", " ")} · ${route.billing} billing`
    : `${harness?.label || "Harness"} has no authenticated route on this machine.`;
  ui.submit.textContent = restingSubmitLabel();
  updateSubmitState();
});
for (const name of ["dragenter", "dragover"]) {
  ui.dropZone.addEventListener(name, (event) => {
    event.preventDefault();
    if (!pressActive) ui.dropZone.classList.add("dragging");
  });
}
for (const name of ["dragleave", "drop"]) {
  ui.dropZone.addEventListener(name, (event) => {
    event.preventDefault();
    ui.dropZone.classList.remove("dragging");
  });
}
ui.dropZone.addEventListener("drop", (event) => acceptFiles(event.dataTransfer.files));

for (const field of [ui.appName, ui.prompt]) {
  field.addEventListener("input", () => {
    shotCompleted = false;
    clearInitialPlanReview();
    updateSubmitState();
  });
}

const filePayload = (file) => new Promise((resolve, reject) => {
  const reader = new FileReader();
  reader.onerror = reject;
  reader.onload = () => resolve({ name: file.name, data: reader.result.split(",", 2)[1] });
  reader.readAsDataURL(file);
});

ui.form.addEventListener("submit", async (event) => {
  event.preventDefault();
  if (ui.submit.disabled) return;

  const appName = composerMode === "create" ? ui.appName.value : composerAppName;
  const prompt = ui.prompt.value;
  if (composerMode === "create" && !reviewedInitialPlan) {
    setComposerBusy(true, "Preparing review…");
    try {
      const response = await fetch("/api/plan", {
        method: "POST",
        headers: studioJsonHeaders,
        body: JSON.stringify({ app_name: appName, prompt }),
      });
      if (!response.ok) throw new Error(await response.text());
      renderInitialPlanReview(await response.json(), appName, prompt);
      setComposerBusy(false);
      ui.planReview.scrollIntoView({ behavior: "smooth", block: "nearest" });
    } catch (error) {
      appendEvent("status", `plan rejected: ${error.message}`);
      clearInitialPlanReview();
      setComposerBusy(false);
    }
    return;
  }
  if (composerMode === "create"
    && (reviewedInitialPlan.appName !== appName || reviewedInitialPlan.prompt !== prompt)) {
    clearInitialPlanReview();
    appendEvent("status", "The intention changed; review the regenerated Genome before committing.");
    updateSubmitState();
    return;
  }
  pendingShot = { mode: composerMode, appName };
  setComposerBusy(true);
  try {
    const selectionKey = `${appName}:${selectedApp?.latest_evolution}`;
    const selected = composerMode === "evolve"
      ? [...(selectedFeedbackActions.get(selectionKey) || [])].sort()
      : [];
    const response = await fetch("/shots", {
      method: "POST",
      headers: studioJsonHeaders,
      body: JSON.stringify({
        mode: composerMode,
        app_name: appName,
        prompt,
        harness: ui.harness.value,
        model: ui.model.value,
        route: ui.route.value,
        accept_genome: composerMode === "create",
        selected_feedback_actions: selected,
        images: await Promise.all(files.map(filePayload)),
      }),
    });
    if (!response.ok) throw new Error(await response.text());
    const execution = await response.json();
    pendingShot.executionId = execution.execution_id;
    observedExecutionEvents = 0;
    appendEvent(
      "handoff",
      `SHOT PREPARED\n\nHarness: ${execution.harness_display_name}\nModel: ${execution.model}\nAdditional cost: ${typeof execution.estimated_additional_cost_usd === "number" ? `$${execution.estimated_additional_cost_usd.toFixed(2)}` : "usage-based"}\n\nWaiting for confirmation in Terminal…`
    );
    ui.submit.textContent = "SHOT PREPARED";
    void followExecution(appName, execution.execution_id);
  } catch (error) {
    appendEvent("status", `intake rejected: ${error.message}`);
    pendingShot = null;
    setComposerBusy(false);
  }
});

const appendEvent = (kind, message) => {
  ui.latestEvent.textContent = message;
  ui.eventLog.querySelector(".empty")?.remove();

  const line = document.createElement("p");
  line.className = kind;
  line.textContent = kind === "harness_line" ? `  ${message}` : message;
  ui.eventLog.append(line);

  if (kind === "status" && message.startsWith("engine stopped:")) {
    pendingShot = null;
    setComposerBusy(false);
  }
};

const finishExecution = (completion) => {
  const completed = pendingShot;
  pendingShot = null;
  pressActive = false;
  shotCompleted = true;
  ui.form.setAttribute("aria-busy", "false");
  ui.appName.disabled = false;
  ui.prompt.disabled = false;
  ui.imageInput.disabled = false;
  ui.harness.disabled = false;
  ui.model.disabled = false;
  ui.route.disabled = false;
  ui.dropZone.setAttribute("aria-disabled", "false");
  ui.submit.textContent = completion.landed ? "SHOT LANDED" : "SHOT NOT LANDED";
  ui.submit.disabled = true;
  const validation = completion.validation_results
    .map((result) => `${result.command}: ${result.status}`)
    .join(" · ");
  appendEvent(
    "result",
    `${completion.landed ? "SHOT LANDED" : "SHOT NOT LANDED"}\n\n${completion.files_changed.length} files changed\n${validation}\nAdditional cost: ${typeof completion.actual_additional_cost_usd === "number" ? `$${completion.actual_additional_cost_usd.toFixed(2)}` : (typeof completion.estimated_additional_cost_usd === "number" ? `$${completion.estimated_additional_cost_usd.toFixed(2)} estimated` : "unavailable")}\n\n${completion.authoritative_next_action}`
  );
  if (completed) {
    loadLibrary().then(() => {
      const app = library.apps.find((candidate) => candidate.name === completed.appName);
      if (app) selectApp(app, app.latest_evolution);
    });
  }
};

const followExecution = async (appName, executionId) => {
  const token = ++executionPollToken;
  while (pendingShot?.executionId === executionId && token === executionPollToken) {
    try {
      const response = await fetch(
        `/api/executions/${encodeURIComponent(appName)}/${encodeURIComponent(executionId)}`
      );
      if (!response.ok) throw new Error(await response.text());
      const state = await response.json();
      for (const event of state.events.slice(observedExecutionEvents)) {
        appendEvent("status", `${event.event} · ${event.report}`);
        if (event.event === "execution.started") ui.submit.textContent = "SHOT IN FLIGHT";
      }
      observedExecutionEvents = state.events.length;
      if (state.completion) {
        finishExecution(state.completion);
        return;
      }
    } catch (error) {
      appendEvent("status", `execution state unavailable: ${error.message}`);
    }
    await new Promise((resolve) => setTimeout(resolve, 750));
  }
};

document.querySelector("#clear").addEventListener("click", () => {
  ui.eventLog.replaceChildren();
  appendEvent("status", "The display is clear.");
});

const clamp = (value, minimum, maximum) => Math.min(Math.max(value, minimum), maximum);

const configureSplitter = (splitter, property, minimum, maximum, storageKey) => {
  const stored = Number.parseInt(localStorage.getItem(storageKey), 10);
  if (Number.isFinite(stored)) {
    document.documentElement.style.setProperty(property, `${clamp(stored, minimum, maximum)}px`);
  }

  const currentWidth = () => Number.parseFloat(getComputedStyle(document.documentElement).getPropertyValue(property));
  const setWidth = (width) => {
    const next = clamp(width, minimum, maximum);
    document.documentElement.style.setProperty(property, `${next}px`);
    splitter.setAttribute("aria-valuenow", String(Math.round(next)));
    localStorage.setItem(storageKey, String(Math.round(next)));
  };
  setWidth(currentWidth());

  splitter.addEventListener("pointerdown", (event) => {
    if (event.button !== 0) return;
    event.preventDefault();
    const startX = event.clientX;
    const startWidth = currentWidth();
    splitter.setPointerCapture(event.pointerId);
    document.body.classList.add("resizing");

    const move = (moveEvent) => setWidth(startWidth + moveEvent.clientX - startX);
    const finish = () => {
      document.body.classList.remove("resizing");
      splitter.removeEventListener("pointermove", move);
      splitter.removeEventListener("pointerup", finish);
      splitter.removeEventListener("pointercancel", finish);
    };
    splitter.addEventListener("pointermove", move);
    splitter.addEventListener("pointerup", finish);
    splitter.addEventListener("pointercancel", finish);
  });

  splitter.addEventListener("keydown", (event) => {
    if (!["ArrowLeft", "ArrowRight"].includes(event.key)) return;
    event.preventDefault();
    setWidth(currentWidth() + (event.key === "ArrowRight" ? 16 : -16));
  });
};

configureSplitter(ui.librarySplitter, "--library-width", 240, 520, "tohseno-library-width");
configureSplitter(ui.composerSplitter, "--composer-width", 320, 640, "tohseno-composer-width");

ui.launchToken.addEventListener("click", () => {
  const binding = selectedLaunchBinding();
  if (!binding || shotProtocol?.ontology?.token_association?.status === "associated") return;
  clearBankrApproval();
  resetBankrResult();
  ui.bankrShotName.textContent = binding.app_name;
  ui.bankrShotId.textContent = binding.shot_id;
  ui.bankrShotVersion.textContent = String(binding.version_ordinal).padStart(4, "0");
  ui.bankrDialog.dataset.appName = binding.app_name;
  ui.bankrDialog.dataset.shotId = binding.shot_id;
  ui.bankrDialog.dataset.versionOrdinal = String(binding.version_ordinal);
  if (!ui.bankrDialog.open) ui.bankrDialog.showModal();
});

ui.bankrClose.addEventListener("click", () => ui.bankrDialog.close());

for (const field of [
  ui.bankrChain,
  ui.bankrVesting,
  ui.bankrFeeMode,
  ui.bankrDescription,
  ui.bankrImage,
  ui.bankrWebsite,
  ui.bankrTweet,
]) {
  field.addEventListener("input", () => {
    clearBankrApproval();
    resetBankrResult();
  });
}

ui.bankrAcknowledge.addEventListener("change", updateBankrDeployState);
ui.bankrConfirmation.addEventListener("input", updateBankrDeployState);

ui.bankrForm.addEventListener("submit", async (event) => {
  event.preventDefault();
  if (ui.bankrSimulate.disabled) return;
  clearBankrApproval();
  resetBankrResult();
  ui.bankrSimulate.disabled = true;
  ui.bankrSimulate.textContent = "Bankr is simulating…";
  ui.bankrStatus.textContent = "Bankr is simulating the exact launch. Nothing is being broadcast.";
  ui.bankrStatus.dataset.status = "ready";
  try {
    const response = await fetch("/api/bankr/launch/simulate", {
      method: "POST",
      headers: studioJsonHeaders,
      body: JSON.stringify({
        app_name: ui.bankrDialog.dataset.appName,
        version_ordinal: Number(ui.bankrDialog.dataset.versionOrdinal),
        parameters: bankrParameters(),
      }),
    });
    if (!response.ok) throw new Error(await response.text());
    renderBankrSimulation(await response.json());
    ui.bankrStatus.textContent =
      "Simulation verified the chain, predicted token address, and pinned jpfraneto.eth recipient.";
  } catch (error) {
    ui.bankrStatus.textContent = error.message;
    ui.bankrStatus.dataset.status = "error";
  } finally {
    ui.bankrSimulate.textContent = "Simulate with Bankr";
    ui.bankrSimulate.disabled = !bankrOverview?.configured;
  }
});

ui.bankrDeploy.addEventListener("click", async () => {
  if (ui.bankrDeploy.disabled || !bankrApproval) return;
  const approval = bankrApproval;
  ui.bankrDeploy.disabled = true;
  ui.bankrDeploy.textContent = "Bankr is deploying…";
  ui.bankrStatus.textContent =
    "Deployment submitted to Bankr. Keep this window open until a receipt appears.";
  ui.bankrStatus.dataset.status = "ready";
  try {
    const response = await fetch("/api/bankr/launch/deploy", {
      method: "POST",
      headers: studioJsonHeaders,
      body: JSON.stringify({
        approval_id: approval.approval_id,
        confirmation: ui.bankrConfirmation.value,
        shot: approval.shot,
      }),
    });
    if (!response.ok) throw new Error(await response.text());
    bankrApproval = null;
    renderBankrDeployment(await response.json());
    await loadShotProtocol(selectedApp, selectedShot);
    ui.bankrStatus.textContent =
      "Bankr returned a deployment receipt and Studio recorded a private local Shot relation. Verify the token transaction before announcing the address.";
  } catch (error) {
    bankrApproval = null;
    ui.bankrResult.hidden = false;
    ui.bankrResult.dataset.status = "error";
    ui.bankrResultTitle.textContent = error.message.startsWith("DEPLOYMENT OUTCOME UNKNOWN")
      ? "Deployment outcome unknown"
      : "Deployment not completed";
    ui.bankrResultSummary.textContent = error.message;
    ui.bankrResultJson.textContent =
      "Do not click deploy again. If the outcome is unknown, inspect Bankr's recent launches before simulating a new request.";
    ui.bankrStatus.textContent = error.message;
    ui.bankrStatus.dataset.status = "error";
  } finally {
    ui.bankrDeploy.textContent = "Deploy $TOHSENO through Bankr";
    updateBankrDeployState();
  }
});

const stream = new EventSource("/events");
stream.onopen = () => {
  ui.connection.textContent = "press connected";
  ui.connection.classList.add("online");
};
stream.onerror = () => {
  ui.connection.textContent = "reconnecting";
  ui.connection.classList.remove("online");
};
stream.onmessage = (event) => {
  const item = JSON.parse(event.data);
  appendEvent(item.kind, item.message);
};

Promise.all([
  loadHarnesses(),
  loadLibrary(),
  loadOnboarding({ autoOpen: true }),
  loadProtocolOverview(),
  loadNodeStatus(),
  loadBankrStatus(),
]).catch((error) => {
  appendEvent("status", `studio data unavailable: ${error.message}`);
});
