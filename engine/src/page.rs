//! Deterministic, private-by-default static Shot pages.
//!
//! A page is a projection of public protocol facts, never a copy of a Shot
//! directory. In particular, prompts, input images, source, build logs, and
//! application artifacts are not traversed or copied except that a declared
//! app icon may be selected from an asset catalog.

use crate::contract_generation::{
    resolve_current_contract_generation, CURRENT_GENERATION_REPOSITORY_PATH,
};
use crate::ledger::{validate_app_name, AppRecord, Evolution, Ledger, LedgerError};
use crate::{protocol_lifecycle, verifier};
use serde::Serialize;
use std::fmt;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use tohseno_protocol::canonical;
use tohseno_protocol::conformance::{CheckStatus, ConformanceReport};
use tohseno_protocol::digest::{Address20, Bytes32, ShotId};
use tohseno_protocol::fascia::{DistributionState, FasciaManifest};
use tohseno_protocol::identity::BuilderId;
use tohseno_protocol::record::ShotRecord;
use tohseno_protocol::signature::SignatureSidecar;

const MAX_PROTOCOL_FILE_BYTES: u64 = 4 * 1024 * 1024;
const MAX_ICON_BYTES: u64 = 32 * 1024 * 1024;
const FALLBACK_ICON: &[u8] = include_bytes!("../../brand/logos/tohseno-app-icon-1024.png");

/// The public state emitted by this local-only generator.
///
/// Publication requires a separate signed action and receipt. Merely
/// generating a directory can therefore never promote a Shot.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PagePublicState {
    Private,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ContractGenerationAudit {
    pub definition_repository_path: String,
    pub definition_sha256: Bytes32,
    pub generation: String,
    pub protocol_major: u64,
    pub chain_id: u64,
    pub source_commit: String,
    pub compiler: String,
    pub active_generation: Option<String>,
    pub inactive_reason: String,
    pub conditional_create2: ConditionalCreate2Coordinates,
}

/// These are mathematical EIP-1014 predictions from an immutable build
/// definition. They are not deployment evidence, RPC observations, or
/// authority coordinates.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ConditionalCreate2Coordinates {
    pub condition: String,
    pub deployer: Address20,
    pub builder_account_factory: Address20,
    pub shot_registry: Address20,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct PageBuildReport {
    pub output_path: PathBuf,
    pub slug: String,
    pub shot_id: ShotId,
    pub builder_id: BuilderId,
    pub sequence: u32,
    pub locally_verified: bool,
    pub public_state: PagePublicState,
    pub source_published: bool,
    pub contract_generation: ContractGenerationAudit,
}

#[derive(Debug)]
pub enum PageError {
    Ledger(LedgerError),
    Io(std::io::Error),
    Protocol(String),
    NoCompleteShot(String),
    UnsafePath(String),
    InvalidState(String),
}

impl fmt::Display for PageError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Ledger(error) => write!(formatter, "{error}"),
            Self::Io(error) => write!(formatter, "{error}"),
            Self::Protocol(error) => write!(formatter, "{error}"),
            Self::NoCompleteShot(app) => {
                write!(formatter, "{app} has no complete signed Shot")
            }
            Self::UnsafePath(path) => write!(formatter, "refusing unsafe path: {path}"),
            Self::InvalidState(message) => write!(formatter, "invalid Shot page state: {message}"),
        }
    }
}

impl std::error::Error for PageError {}

impl From<LedgerError> for PageError {
    fn from(value: LedgerError) -> Self {
        Self::Ledger(value)
    }
}

impl From<std::io::Error> for PageError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

/// Builds `ledger.root()/public/<slug>/` from the latest completed Shot.
///
/// Every protocol object is parsed as a closed JSON object, validated, and
/// cross-checked before any output is replaced. Output bytes depend only on
/// the signed Shot, its public sidecars, its app icon, and the embedded
/// immutable contract-generation definition.
pub fn build(ledger: &Ledger, app_name: &str) -> Result<PageBuildReport, PageError> {
    validate_app_name(app_name)?;
    let app = load_app_without_symlinks(ledger, app_name)?;
    let shot = latest_complete_evolution(ledger, &app)?;
    let fascia_reference = protocol_lifecycle::reference_fascia_root()
        .map_err(|error| PageError::InvalidState(error.to_string()))?;
    let offline = verifier::verify_shot_directory(&shot.path, &fascia_reference);
    if !offline.conformant {
        let failed = offline
            .checks
            .iter()
            .filter(|check| check.status != verifier::VerificationStatus::Pass)
            .map(|check| check.id.as_str())
            .collect::<Vec<_>>();
        return Err(PageError::InvalidState(format!(
            "offline verification did not pass: {}",
            failed.join(", ")
        )));
    }
    let verified = verify_sidecars(&app, &shot)?;
    let icon = select_icon(&shot.source_path())?;
    let contract_generation = contract_generation_audit()?;
    let world = fs::read_to_string(shot.source_path().join("WORLD.md")).ok();
    let preview = fs::read(shot.path.join("preview.png")).ok();

    let output_path = ledger
        .briefing_dir(app_name)
        .join("public")
        .join(&verified.record.slug);
    let page = render_html(
        &verified.record,
        &verified.fascia,
        &contract_generation,
        world.as_deref(),
        preview.is_some(),
    );
    let mut files = vec![
        ("index.html", page.into_bytes()),
        (
            "shot.json",
            canonical_json_line(&verified.record, "Shot record")?,
        ),
        (
            "fascia.json",
            canonical_json_line(&verified.fascia, "Fascia manifest")?,
        ),
        ("icon.png", icon),
        (
            "assets/signature.json",
            canonical_json_line(&verified.signature, "signature sidecar")?,
        ),
        (
            "assets/conformance.json",
            canonical_json_line(&verified.conformance, "conformance report")?,
        ),
    ];
    if let Some(preview) = preview {
        files.push(("preview.png", preview));
    }
    replace_page_directory(&ledger.briefing_dir(app_name), &output_path, &files)?;

    Ok(PageBuildReport {
        output_path,
        slug: verified.record.slug.clone(),
        shot_id: verified.record.shot_id,
        builder_id: verified.record.builder_id,
        sequence: verified.record.sequence,
        locally_verified: true,
        public_state: PagePublicState::Private,
        source_published: false,
        contract_generation,
    })
}

struct VerifiedPageInput {
    record: ShotRecord,
    signature: SignatureSidecar,
    fascia: FasciaManifest,
    conformance: ConformanceReport,
}

fn load_app_without_symlinks(ledger: &Ledger, app_name: &str) -> Result<AppRecord, PageError> {
    require_real_directory(ledger.root())?;
    let app_directory = ledger.working_tree(app_name);
    require_real_directory(&app_directory)?;
    let ledger_directory = app_directory.join(".tohseno");
    require_real_directory(&ledger_directory)?;
    require_real_directory(&ledger_directory.join("evolutions"))?;
    require_regular_file(&ledger_directory.join("app.toml"))?;

    let app = ledger.load_app(app_name)?;
    if app.name != app_name {
        return Err(PageError::InvalidState(
            "app.toml name does not match its directory".into(),
        ));
    }
    Ok(app)
}

fn latest_complete_evolution(ledger: &Ledger, app: &AppRecord) -> Result<Evolution, PageError> {
    let number = app
        .latest_evolution
        .ok_or_else(|| PageError::NoCompleteShot(app.name.clone()))?;
    let shot_path = ledger
        .working_tree(&app.name)
        .join(".tohseno")
        .join("evolutions")
        .join(format!("{number:04}"));
    require_real_directory(&shot_path)?;
    let marker = shot_path.join(".complete");
    require_regular_file(&marker)?;
    if read_regular_file(&marker, 64)? != b"complete\n" {
        return Err(PageError::InvalidState(format!(
            "Shot {number} has an invalid completion marker"
        )));
    }
    require_real_directory(&shot_path.join("TOHSENO"))?;
    Ok(Evolution {
        app_name: app.name.clone(),
        number,
        path: shot_path,
    })
}

fn verify_sidecars(app: &AppRecord, shot: &Evolution) -> Result<VerifiedPageInput, PageError> {
    let protocol_root = shot.path.join("TOHSENO");
    let record: ShotRecord = read_closed_json(&protocol_root.join("shot.json"), "Shot record")?;
    let signature: SignatureSidecar =
        read_closed_json(&protocol_root.join("signature.json"), "signature sidecar")?;
    let fascia: FasciaManifest =
        read_closed_json(&protocol_root.join("fascia.json"), "Fascia manifest")?;
    let conformance: ConformanceReport = read_closed_json(
        &protocol_root.join("conformance.json"),
        "conformance report",
    )?;

    record
        .validate()
        .map_err(|error| PageError::Protocol(error.to_string()))?;
    record
        .verify_signature(&signature)
        .map_err(|error| PageError::Protocol(format!("Shot signature: {error}")))?;
    fascia
        .validate()
        .map_err(|error| PageError::Protocol(error.to_string()))?;
    conformance
        .validate()
        .map_err(|error| PageError::Protocol(error.to_string()))?;

    if !conformance.conformant {
        return Err(PageError::InvalidState(
            "the latest Shot is not conformant".into(),
        ));
    }
    if !conformance
        .checks
        .iter()
        .any(|check| check.id == "record.signature" && check.status == CheckStatus::Pass)
    {
        return Err(PageError::InvalidState(
            "conformance has no passing record.signature check".into(),
        ));
    }
    if record.slug != app.target_name()
        || record.bundle_id != app.bundle_id
        || record.sequence != shot.number
        || record.bundle_version != shot.number
        || app.shot_id != Some(record.shot_id)
        || app.builder_id != Some(record.builder_id)
    {
        return Err(PageError::InvalidState(
            "signed Shot identity does not match the local append-only ledger".into(),
        ));
    }
    if fascia.fascia != record.fascia
        || fascia.distribution.bundle_id != record.bundle_id
        || fascia.distribution.bundle_version != record.bundle_version
        || conformance.shot_id != record.shot_id
        || conformance.sequence != record.sequence
    {
        return Err(PageError::InvalidState(
            "Fascia or conformance sidecar does not bind to the signed Shot".into(),
        ));
    }

    Ok(VerifiedPageInput {
        record,
        signature,
        fascia,
        conformance,
    })
}

fn read_closed_json<T: serde::de::DeserializeOwned>(
    path: &Path,
    label: &str,
) -> Result<T, PageError> {
    let bytes = read_regular_file(path, MAX_PROTOCOL_FILE_BYTES)?;
    canonical::from_slice(&bytes).map_err(|error| PageError::Protocol(format!("{label}: {error}")))
}

fn canonical_json_line<T: Serialize>(value: &T, label: &str) -> Result<Vec<u8>, PageError> {
    let mut bytes = canonical::to_vec(value)
        .map_err(|error| PageError::Protocol(format!("{label}: {error}")))?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn select_icon(source: &Path) -> Result<Vec<u8>, PageError> {
    match fs::symlink_metadata(source) {
        Ok(metadata) if metadata.file_type().is_dir() && !metadata.file_type().is_symlink() => {}
        Ok(_) => return Err(PageError::UnsafePath(source.display().to_string())),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(FALLBACK_ICON.to_vec())
        }
        Err(error) => return Err(error.into()),
    }
    let mut candidates = Vec::new();
    collect_icon_candidates(source, source, false, &mut candidates)?;
    candidates.sort_by(|left, right| {
        right
            .area
            .cmp(&left.area)
            .then_with(|| right.longest_edge.cmp(&left.longest_edge))
            .then_with(|| left.relative_path.cmp(&right.relative_path))
    });
    Ok(candidates
        .into_iter()
        .next()
        .map(|candidate| candidate.bytes)
        .unwrap_or_else(|| FALLBACK_ICON.to_vec()))
}

struct IconCandidate {
    relative_path: String,
    area: u64,
    longest_edge: u32,
    bytes: Vec<u8>,
}

fn collect_icon_candidates(
    root: &Path,
    directory: &Path,
    inside_app_icon_set: bool,
    candidates: &mut Vec<IconCandidate>,
) -> Result<(), PageError> {
    let mut entries = fs::read_dir(directory)?.collect::<Result<Vec<_>, _>>()?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let path = entry.path();
        let file_type = entry.file_type()?;
        if file_type.is_symlink() {
            return Err(PageError::UnsafePath(path.display().to_string()));
        }
        if file_type.is_dir() {
            let name = entry.file_name();
            let is_icon_set = name
                .to_str()
                .is_some_and(|name| name.to_ascii_lowercase().ends_with(".appiconset"));
            collect_icon_candidates(root, &path, inside_app_icon_set || is_icon_set, candidates)?;
        } else if file_type.is_file()
            && inside_app_icon_set
            && path
                .extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| extension.eq_ignore_ascii_case("png"))
        {
            let bytes = read_regular_file(&path, MAX_ICON_BYTES)?;
            let Some((width, height)) = png_dimensions(&bytes) else {
                continue;
            };
            let relative = path.strip_prefix(root).map_err(|_| {
                PageError::UnsafePath(format!("icon escaped source root: {}", path.display()))
            })?;
            let relative_path = relative.to_str().ok_or_else(|| {
                PageError::UnsafePath(format!("icon path is not valid UTF-8: {}", path.display()))
            })?;
            candidates.push(IconCandidate {
                relative_path: relative_path.replace('\\', "/"),
                area: u64::from(width) * u64::from(height),
                longest_edge: width.max(height),
                bytes,
            });
        } else if !file_type.is_file() {
            return Err(PageError::UnsafePath(path.display().to_string()));
        }
    }
    Ok(())
}

fn png_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    const PNG_SIGNATURE: &[u8; 8] = b"\x89PNG\r\n\x1a\n";
    if bytes.len() < 24
        || &bytes[..8] != PNG_SIGNATURE
        || &bytes[12..16] != b"IHDR"
        || u32::from_be_bytes(bytes[8..12].try_into().ok()?) != 13
    {
        return None;
    }
    let width = u32::from_be_bytes(bytes[16..20].try_into().ok()?);
    let height = u32::from_be_bytes(bytes[20..24].try_into().ok()?);
    (width > 0 && height > 0).then_some((width, height))
}

fn render_html(
    record: &ShotRecord,
    fascia: &FasciaManifest,
    contract_generation: &ContractGenerationAudit,
    world: Option<&str>,
    has_preview: bool,
) -> String {
    let app = escape_html(&record.slug);
    let shot_id = escape_html(&record.shot_id.to_string());
    let builder_id = escape_html(&record.builder_id.to_string());
    let bundle_id = escape_html(&record.bundle_id);
    let created_at = escape_html(record.created_at.as_str());
    let distribution = match fascia.distribution.state {
        DistributionState::Local => "Local",
        DistributionState::Published => "Published in app metadata",
        DistributionState::AppStore => "App Store in app metadata",
    };
    let world_markup = match world {
        Some(text) => format!(
            "<section><h2>World</h2><pre class=\"world\">{}</pre></section>\n",
            escape_html(text)
        ),
        None => String::new(),
    };
    let preview_markup = if has_preview {
        "<section><h2>Latest evolution</h2><img class=\"preview\" src=\"preview.png\" alt=\"the app's current first screen\" width=\"280\"></section>\n".to_owned()
    } else {
        String::new()
    };
    let generation_markup = format!(
        "<dl class=\"facts\">\
         <div><dt>Definition</dt><dd>{} (protocol major {})</dd></div>\
         <div><dt>Definition SHA-256</dt><dd class=\"mono\">{}</dd></div>\
         <div><dt>Definition path</dt><dd class=\"mono\">{}</dd></div>\
         <div><dt>Source commit</dt><dd class=\"mono\">{}</dd></div>\
         <div><dt>Compiler</dt><dd class=\"mono\">{}</dd></div>\
         <div><dt>Target chain</dt><dd class=\"mono\">eip155:{}</dd></div>\
         <div><dt>Active generation</dt><dd>None</dd></div>\
         <div><dt>CREATE2 deployer</dt><dd class=\"mono\">{}</dd></div>\
         <div><dt>Conditional factory</dt><dd class=\"mono\">{}</dd></div>\
         <div><dt>Conditional registry</dt><dd class=\"mono\">{}</dd></div>\
         </dl>\
         <p class=\"quiet\">Inactive: {}.</p>\
         <p class=\"quiet\">{} These offline predictions are not RPC observations, \
         deployment evidence, or public authority.</p>",
        escape_html(&contract_generation.generation),
        contract_generation.protocol_major,
        escape_html(&contract_generation.definition_sha256.to_string()),
        escape_html(&contract_generation.definition_repository_path),
        escape_html(&contract_generation.source_commit),
        escape_html(&contract_generation.compiler),
        contract_generation.chain_id,
        escape_html(&contract_generation.conditional_create2.deployer.to_string()),
        escape_html(
            &contract_generation
                .conditional_create2
                .builder_account_factory
                .to_string()
        ),
        escape_html(
            &contract_generation
                .conditional_create2
                .shot_registry
                .to_string()
        ),
        escape_html(&contract_generation.inactive_reason),
        escape_html(&contract_generation.conditional_create2.condition),
    );

    format!(
        "<!doctype html>\n\
<html lang=\"en\">\n\
<head>\n\
<meta charset=\"utf-8\">\n\
<meta name=\"viewport\" content=\"width=device-width,initial-scale=1\">\n\
<meta name=\"color-scheme\" content=\"dark\">\n\
<meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; img-src 'self'; style-src 'unsafe-inline'; base-uri 'none'; form-action 'none'\">\n\
<title>{app} · TOHSENO Evolution</title>\n\
<style>\n\
:root{{--ink:#f6f1e8;--muted:#aaa39a;--line:#35312d;--paper:#171513;--panel:#201d1a;--accent:#e2ff62;--blue:#80bfff}}\
*{{box-sizing:border-box}}html{{background:var(--paper);color:var(--ink);font-family:ui-sans-serif,-apple-system,BlinkMacSystemFont,\"Segoe UI\",sans-serif}}\
body{{margin:0}}main{{width:min(880px,calc(100% - 32px));margin:0 auto;padding:72px 0 96px}}\
header{{display:grid;grid-template-columns:112px 1fr;gap:28px;align-items:center;margin-bottom:52px}}\
.icon{{width:112px;height:112px;border-radius:25%;display:block}}h1{{font-size:clamp(2.25rem,8vw,5rem);line-height:.92;letter-spacing:-.06em;margin:.15em 0}}\
.eyebrow,.badge{{font-size:.72rem;letter-spacing:.13em;text-transform:uppercase}}.eyebrow{{color:var(--accent)}}\
.badges{{display:flex;gap:8px;flex-wrap:wrap}}.badge{{border:1px solid var(--line);border-radius:999px;padding:8px 11px}}.verified{{color:var(--accent)}}\
section{{padding:28px 0;border-top:1px solid var(--line)}}h2{{font-size:1rem;letter-spacing:.08em;text-transform:uppercase;margin:0 0 20px}}\
.facts{{margin:0}}.facts div{{display:grid;grid-template-columns:minmax(130px,.4fr) 1fr;gap:20px;padding:11px 0}}dt{{color:var(--muted)}}dd{{margin:0;overflow-wrap:anywhere}}\
.mono{{font-family:ui-monospace,SFMono-Regular,Menlo,monospace;font-size:.83rem}}.quiet{{color:var(--muted);line-height:1.6;max-width:64ch}}\
a{{color:var(--blue);text-underline-offset:3px}}footer{{color:var(--muted);font-size:.8rem;padding-top:28px;border-top:1px solid var(--line)}}\
@media(max-width:560px){{main{{padding-top:36px}}header{{grid-template-columns:72px 1fr;gap:18px}}.icon{{width:72px;height:72px}}.facts div{{grid-template-columns:1fr;gap:4px}}}}\n\
</style>\n\
</head>\n\
<body>\n\
<main>\n\
<header><img class=\"icon\" src=\"icon.png\" width=\"112\" height=\"112\" alt=\"{app} app icon\"><div><div class=\"eyebrow\">TOHSENO Shot</div><h1>{app}</h1><div class=\"badges\"><span class=\"badge verified\">Locally verified</span><span class=\"badge\">Private</span><span class=\"badge\">Evolution {sequence}</span></div></div></header>\n\
<section><h2>Identity</h2><dl class=\"facts\"><div><dt>ShotID</dt><dd class=\"mono\">{shot_id}</dd></div><div><dt>BuilderID</dt><dd class=\"mono\">{builder_id}</dd></div><div><dt>Bundle</dt><dd class=\"mono\">{bundle_id}</dd></div><div><dt>Signed at</dt><dd>{created_at}</dd></div></dl></section>\n\
<section><h2>Verification</h2><p>This Evolution’s closed record and low-s P-256 signature verified locally. Its Fascia declaration and conformance receipt are valid and bound to the same Shot.</p><p><a href=\"shot.json\">Shot record</a> · <a href=\"fascia.json\">Fascia</a> · <a href=\"assets/signature.json\">Signature</a> · <a href=\"assets/conformance.json\">Conformance</a></p></section>\n\
<section><h2>Public state</h2><dl class=\"facts\"><div><dt>Protocol state</dt><dd>Private</dd></div><div><dt>App metadata</dt><dd>{distribution}</dd></div><div><dt>Source</dt><dd>Not published</dd></div></dl><p class=\"quiet\">A local static page is not a publication receipt. No prompt, input image, full source tree, build log, or app artifact is included; the story and preview above, when present, are the only excerpts and come from the world’s own WORLD.md and recorded first screen.</p></section>\n\
<section><h2>Contract generation audit</h2>{generation_markup}</section>\n\
{preview_markup}\
{world_markup}\
<footer>Generated from public Evolution facts and the world’s own story. The embedded contract generation is an inactive offline build definition.</footer>\n\
</main>\n\
</body>\n\
</html>\n",
        sequence = record.sequence,
        distribution = escape_html(distribution),
    )
}

fn escape_html(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&#39;"),
            _ => escaped.push(character),
        }
    }
    escaped
}

fn replace_page_directory(
    owner_root: &Path,
    destination: &Path,
    files: &[(&str, Vec<u8>)],
) -> Result<(), PageError> {
    let public_root = owner_root.join("public");
    ensure_real_directory(&public_root)?;
    if destination.parent() != Some(public_root.as_path()) {
        return Err(PageError::UnsafePath(destination.display().to_string()));
    }
    let slug = destination
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| PageError::UnsafePath(destination.display().to_string()))?;
    validate_app_name(slug)?;

    let temporary = unused_sibling(&public_root, &format!(".{slug}.tmp"))?;
    fs::create_dir(&temporary)?;
    let build_result = (|| {
        fs::create_dir(temporary.join("assets"))?;
        for (relative, bytes) in files {
            write_new_file(&temporary, relative, bytes)?;
        }
        sync_directory(&temporary.join("assets"))?;
        sync_directory(&temporary)?;
        install_replacement(&temporary, destination, &public_root, slug)
    })();
    if build_result.is_err() && fs::symlink_metadata(&temporary).is_ok() {
        let _ = remove_tree_without_symlinks(&temporary);
    }
    build_result
}

fn install_replacement(
    temporary: &Path,
    destination: &Path,
    public_root: &Path,
    slug: &str,
) -> Result<(), PageError> {
    match fs::symlink_metadata(destination) {
        Ok(metadata) if metadata.file_type().is_dir() && !metadata.file_type().is_symlink() => {
            validate_tree_without_symlinks(destination)?;
            let backup = unused_sibling(public_root, &format!(".{slug}.previous"))?;
            fs::rename(destination, &backup)?;
            if let Err(error) = fs::rename(temporary, destination) {
                if let Err(rollback) = fs::rename(&backup, destination) {
                    return Err(PageError::InvalidState(format!(
                        "replacement failed ({error}); previous page remains recoverable at {} because rollback also failed ({rollback})",
                        backup.display()
                    )));
                }
                return Err(PageError::Io(error));
            }
            sync_directory(public_root)?;
            remove_tree_without_symlinks(&backup)?;
            sync_directory(public_root)?;
            Ok(())
        }
        Ok(_) => Err(PageError::UnsafePath(destination.display().to_string())),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            fs::rename(temporary, destination)?;
            sync_directory(public_root)?;
            Ok(())
        }
        Err(error) => Err(error.into()),
    }
}

fn unused_sibling(parent: &Path, prefix: &str) -> Result<PathBuf, PageError> {
    let process = std::process::id();
    for ordinal in 0_u32..10_000 {
        let candidate = parent.join(format!("{prefix}-{process}-{ordinal:04}"));
        match fs::symlink_metadata(&candidate) {
            Ok(_) => continue,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(candidate),
            Err(error) => return Err(error.into()),
        }
    }
    Err(PageError::InvalidState(format!(
        "no unused replacement path beneath {}",
        parent.display()
    )))
}

fn write_new_file(root: &Path, relative: &str, bytes: &[u8]) -> Result<(), PageError> {
    let relative_path = Path::new(relative);
    if relative_path.is_absolute()
        || relative_path.components().any(|component| {
            !matches!(
                component,
                std::path::Component::Normal(_) | std::path::Component::CurDir
            )
        })
    {
        return Err(PageError::UnsafePath(relative.into()));
    }
    let path = root.join(relative_path);
    let parent = path
        .parent()
        .ok_or_else(|| PageError::UnsafePath(path.display().to_string()))?;
    require_real_directory(parent)?;
    let mut file = File::options().create_new(true).write(true).open(&path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    Ok(())
}

fn ensure_real_directory(path: &Path) -> Result<(), PageError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_dir() && !metadata.file_type().is_symlink() => {
            Ok(())
        }
        Ok(_) => Err(PageError::UnsafePath(path.display().to_string())),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let parent = path
                .parent()
                .ok_or_else(|| PageError::UnsafePath(path.display().to_string()))?;
            require_real_directory(parent)?;
            fs::create_dir(path)?;
            require_real_directory(path)
        }
        Err(error) => Err(error.into()),
    }
}

fn require_real_directory(path: &Path) -> Result<(), PageError> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() || !metadata.file_type().is_dir() {
        return Err(PageError::UnsafePath(path.display().to_string()));
    }
    Ok(())
}

fn require_regular_file(path: &Path) -> Result<u64, PageError> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() || !metadata.file_type().is_file() {
        return Err(PageError::UnsafePath(path.display().to_string()));
    }
    Ok(metadata.len())
}

fn read_regular_file(path: &Path, limit: u64) -> Result<Vec<u8>, PageError> {
    let length = require_regular_file(path)?;
    if length > limit {
        return Err(PageError::InvalidState(format!(
            "{} exceeds the {} byte limit",
            path.display(),
            limit
        )));
    }
    let file = File::open(path)?;
    let mut bytes = Vec::with_capacity(usize::try_from(length).unwrap_or(0));
    file.take(limit + 1).read_to_end(&mut bytes)?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) != length {
        return Err(PageError::InvalidState(format!(
            "{} changed while it was read",
            path.display()
        )));
    }
    let observed = fs::symlink_metadata(path)?;
    if observed.file_type().is_symlink()
        || !observed.file_type().is_file()
        || observed.len() != length
    {
        return Err(PageError::InvalidState(format!(
            "{} changed while it was read",
            path.display()
        )));
    }
    Ok(bytes)
}

fn validate_tree_without_symlinks(path: &Path) -> Result<(), PageError> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() {
        return Err(PageError::UnsafePath(path.display().to_string()));
    }
    if metadata.file_type().is_file() {
        return Ok(());
    }
    if !metadata.file_type().is_dir() {
        return Err(PageError::UnsafePath(path.display().to_string()));
    }
    for entry in fs::read_dir(path)? {
        validate_tree_without_symlinks(&entry?.path())?;
    }
    Ok(())
}

fn remove_tree_without_symlinks(path: &Path) -> Result<(), PageError> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() {
        return Err(PageError::UnsafePath(path.display().to_string()));
    }
    if metadata.file_type().is_file() {
        fs::remove_file(path)?;
        return Ok(());
    }
    if !metadata.file_type().is_dir() {
        return Err(PageError::UnsafePath(path.display().to_string()));
    }
    let mut entries = fs::read_dir(path)?.collect::<Result<Vec<_>, _>>()?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        remove_tree_without_symlinks(&entry.path())?;
    }
    fs::remove_dir(path)?;
    Ok(())
}

fn sync_directory(path: &Path) -> Result<(), PageError> {
    File::open(path)?.sync_all()?;
    Ok(())
}

fn contract_generation_audit() -> Result<ContractGenerationAudit, PageError> {
    let resolved = resolve_current_contract_generation()
        .map_err(|error| PageError::Protocol(error.to_string()))?;
    let inactive_reason = resolved.inactive_reason().to_owned();
    let definition = resolved.definition;
    Ok(ContractGenerationAudit {
        definition_repository_path: CURRENT_GENERATION_REPOSITORY_PATH.into(),
        definition_sha256: resolved.definition_digest,
        generation: definition.generation,
        protocol_major: definition.protocol_major,
        chain_id: definition.chain.chain_id,
        source_commit: definition.source.commit,
        compiler: format!(
            "solc {} / {} / optimizer {}",
            definition.build.solc_version,
            definition.build.evm_version,
            definition.build.optimizer_runs
        ),
        active_generation: None,
        inactive_reason,
        conditional_create2: ConditionalCreate2Coordinates {
            condition: format!(
                "Conditional only if the exact declared init code and salts are used through the declared CREATE2 deployer on eip155:{}.",
                definition.chain.chain_id
            ),
            deployer: definition.create2.deployer,
            builder_account_factory: definition
                .create2
                .builder_account_factory
                .predicted_address,
            shot_registry: definition.create2.shot_registry.predicted_address,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tohseno_protocol::conformance::{ConformanceCheck, CONFORMANCE_SCHEMA};
    use tohseno_protocol::fascia::{
        AppleSurface, Capability, CapabilityDeclaration, DistributionDeclaration,
        InstallationIdentityDeclaration, PrivacyDeclaration, StorageDeclaration, StorageKind,
        FASCIA_SCHEMA, REQUIRED_FASCIA_FILES,
    };

    fn vector_record_and_signature() -> (ShotRecord, SignatureSidecar) {
        let vector: serde_json::Value =
            serde_json::from_str(include_str!("../../protocol/test-vectors/protocol-v1.json"))
                .unwrap();
        (
            serde_json::from_value(vector["record"]["value"].clone()).unwrap(),
            serde_json::from_value(vector["record"]["sidecar"].clone()).unwrap(),
        )
    }

    fn fascia(record: &ShotRecord) -> FasciaManifest {
        FasciaManifest {
            protocol: "tohseno".into(),
            schema: FASCIA_SCHEMA.into(),
            fascia: record.fascia.clone(),
            required_files: REQUIRED_FASCIA_FILES
                .iter()
                .map(|path| (*path).to_owned())
                .collect(),
            installation_identity: InstallationIdentityDeclaration {
                algorithm: "p256".into(),
                scope: "app_installation".into(),
                hardware_backed_when_available: true,
            },
            capabilities: vec![CapabilityDeclaration {
                capability: Capability::LocalStorage,
                purpose: "Local app state".into(),
                entitlement: None,
            }],
            storage: vec![StorageDeclaration {
                kind: StorageKind::Files,
                purpose: "Local documents".into(),
            }],
            network: vec![],
            privacy: PrivacyDeclaration {
                telemetry: false,
                tracking: false,
                account_required: false,
                silent_identity_linkage: false,
            },
            distribution: DistributionDeclaration {
                bundle_id: record.bundle_id.clone(),
                bundle_version: record.bundle_version,
                surfaces: vec![AppleSurface::Iphone],
                state: DistributionState::Local,
                app_store_id: None,
            },
        }
    }

    fn conformance(record: &ShotRecord) -> ConformanceReport {
        ConformanceReport {
            schema: CONFORMANCE_SCHEMA.into(),
            shot_id: record.shot_id,
            sequence: record.sequence,
            conformant: true,
            checks: vec![
                passing_check("record.schema"),
                passing_check("fascia.manifest"),
                passing_check("record.signature"),
            ],
        }
    }

    fn passing_check(id: &str) -> ConformanceCheck {
        ConformanceCheck {
            id: id.into(),
            status: CheckStatus::Pass,
            expected: "valid".into(),
            observed: "valid".into(),
            evidence_path: Some(format!("TOHSENO/{id}.json")),
        }
    }

    fn write_json<T: Serialize>(ledger: &Ledger, shot: &Evolution, path: &str, value: &T) {
        let mut bytes = serde_json::to_vec_pretty(value).unwrap();
        bytes.push(b'\n');
        ledger.write_evolution_file(shot, path, &bytes).unwrap();
    }

    fn complete_fixture() -> (tempfile::TempDir, Ledger, ShotRecord) {
        let temporary = tempfile::tempdir().unwrap();
        let ledger = Ledger::at(temporary.path());
        let (record, signature) = vector_record_and_signature();
        ledger.create_app(&record.slug, &record.bundle_id).unwrap();
        ledger
            .bind_protocol_identity(&record.slug, record.shot_id, record.builder_id)
            .unwrap();
        let shot = ledger.reserve_evolution(&record.slug, None).unwrap();
        ledger
            .write_evolution_file(
                &shot,
                "prompt.md",
                b"PRIVATE-PROMPT: launch the confidential paper press",
            )
            .unwrap();
        ledger
            .write_evolution_file(
                &shot,
                "images/private-reference.txt",
                b"PRIVATE-IMAGE-BYTES",
            )
            .unwrap();
        ledger
            .write_evolution_file(
                &shot,
                "src/Private.swift",
                b"let builderSecret = \"PRIVATE-SOURCE-BYTES\"",
            )
            .unwrap();
        write_json(&ledger, &shot, "TOHSENO/shot.json", &record);
        write_json(&ledger, &shot, "TOHSENO/signature.json", &signature);
        write_json(&ledger, &shot, "TOHSENO/fascia.json", &fascia(&record));
        write_json(
            &ledger,
            &shot,
            "TOHSENO/conformance.json",
            &conformance(&record),
        );
        ledger.finalize_evolution(&shot).unwrap();
        (temporary, ledger, record)
    }

    fn snapshot(directory: &Path) -> Vec<(String, Vec<u8>)> {
        fn visit(root: &Path, directory: &Path, output: &mut Vec<(String, Vec<u8>)>) {
            for entry in fs::read_dir(directory).unwrap() {
                let entry = entry.unwrap();
                if entry.file_type().unwrap().is_dir() {
                    visit(root, &entry.path(), output);
                } else {
                    output.push((
                        entry
                            .path()
                            .strip_prefix(root)
                            .unwrap()
                            .to_string_lossy()
                            .into_owned(),
                        fs::read(entry.path()).unwrap(),
                    ));
                }
            }
        }
        let mut output = Vec::new();
        visit(directory, directory, &mut output);
        output.sort_by(|left, right| left.0.cmp(&right.0));
        output
    }

    #[test]
    fn page_projection_is_byte_identical_and_private_by_default() {
        let (_temporary, ledger, record) = complete_fixture();
        let output = ledger.root().join("public").join(&record.slug);
        let generation = contract_generation_audit().unwrap();
        let html = render_html(&record, &fascia(&record), &generation, None, false);
        let files = [
            ("index.html", html.as_bytes().to_vec()),
            (
                "shot.json",
                canonical_json_line(&record, "Shot record").unwrap(),
            ),
            ("icon.png", FALLBACK_ICON.to_vec()),
        ];
        replace_page_directory(ledger.root(), &output, &files).unwrap();
        let first_snapshot = snapshot(&output);
        replace_page_directory(ledger.root(), &output, &files).unwrap();
        let second_snapshot = snapshot(&output);
        assert_eq!(first_snapshot, second_snapshot);

        let public_bytes = first_snapshot
            .iter()
            .flat_map(|(_, bytes)| bytes.iter().copied())
            .collect::<Vec<_>>();
        for private in [
            b"PRIVATE-PROMPT".as_slice(),
            b"PRIVATE-IMAGE-BYTES".as_slice(),
            b"PRIVATE-SOURCE-BYTES".as_slice(),
        ] {
            assert!(!public_bytes
                .windows(private.len())
                .any(|window| window == private));
        }
        let html = fs::read_to_string(output.join("index.html")).unwrap();
        assert!(html.contains("Private"));
        assert!(html.contains("Not published"));
        assert!(html.contains("Contract generation audit"));
        assert!(html.contains("0.8.0 (protocol major 2)"));
        assert!(html.contains("<dt>Active generation</dt><dd>None</dd>"));
        assert!(html.contains("Conditional factory"));
        assert!(html.contains("not RPC observations"));
        assert!(!html.contains("ShotRelations"));
        assert!(!html.contains("Deployment transaction"));
        assert!(!html.contains("protocol candidate 0.7.0"));
        assert!(!html.contains("0xe37fa0761c914814e7cdd0842dbda011b76ab387"));
        assert!(!html.contains("<script"));

        let audit = serde_json::to_value(&generation).unwrap();
        assert_eq!(audit["generation"], "0.8.0");
        assert_eq!(audit["active_generation"], serde_json::Value::Null);
        assert_eq!(
            audit["definition_repository_path"],
            "contracts/generations/0.8.0/generation.json"
        );
    }

    #[test]
    fn record_signature_fascia_and_conformance_tampering_are_rejected() {
        let (_temporary, ledger, record) = complete_fixture();
        let shot = ledger.latest_evolution(&record.slug).unwrap().unwrap();
        let record_path = shot.path.join("TOHSENO/shot.json");
        let original_record = fs::read(&record_path).unwrap();
        let mut changed_record = record.clone();
        changed_record.genesis_input_sha256 = Bytes32::new([0x77; 32]);
        fs::write(&record_path, serde_json::to_vec(&changed_record).unwrap()).unwrap();
        assert!(build(&ledger, &record.slug).is_err());
        fs::write(&record_path, original_record).unwrap();

        let fascia_path = shot.path.join("TOHSENO/fascia.json");
        let mut changed_fascia = fascia(&record);
        changed_fascia.distribution.bundle_version = 2;
        fs::write(&fascia_path, serde_json::to_vec(&changed_fascia).unwrap()).unwrap();
        assert!(build(&ledger, &record.slug).is_err());
        write_json_after_finalize(&fascia_path, &fascia(&record));

        let conformance_path = shot.path.join("TOHSENO/conformance.json");
        let mut changed_conformance = conformance(&record);
        changed_conformance.conformant = false;
        fs::write(
            &conformance_path,
            serde_json::to_vec(&changed_conformance).unwrap(),
        )
        .unwrap();
        assert!(build(&ledger, &record.slug).is_err());

        let mut unknown_member = serde_json::to_value(conformance(&record)).unwrap();
        unknown_member
            .as_object_mut()
            .unwrap()
            .insert("forged".into(), serde_json::Value::Bool(true));
        fs::write(
            &conformance_path,
            serde_json::to_vec(&unknown_member).unwrap(),
        )
        .unwrap();
        assert!(build(&ledger, &record.slug).is_err());
    }

    fn write_json_after_finalize<T: Serialize>(path: &Path, value: &T) {
        let mut bytes = serde_json::to_vec_pretty(value).unwrap();
        bytes.push(b'\n');
        fs::write(path, bytes).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn refuses_symlinked_sidecars_and_existing_public_trees() {
        use std::os::unix::fs::symlink;

        let (_temporary, ledger, record) = complete_fixture();
        let shot = ledger.latest_evolution(&record.slug).unwrap().unwrap();
        let sidecar = shot.path.join("TOHSENO/conformance.json");
        let target = shot.path.join("TOHSENO/conformance-target.json");
        fs::rename(&sidecar, &target).unwrap();
        symlink(&target, &sidecar).unwrap();
        assert!(build(&ledger, &record.slug).is_err());
        fs::remove_file(&sidecar).unwrap();
        fs::rename(&target, &sidecar).unwrap();

        let output = ledger.root().join("public").join(&record.slug);
        let files = [("index.html", b"private\n".to_vec())];
        replace_page_directory(ledger.root(), &output, &files).unwrap();
        let trap = output.join("assets/trap");
        symlink(shot.prompt_path(), &trap).unwrap();
        assert!(matches!(
            replace_page_directory(ledger.root(), &output, &files),
            Err(PageError::UnsafePath(_))
        ));
        assert!(trap.is_symlink());
    }

    #[test]
    fn escapes_every_html_metacharacter() {
        assert_eq!(escape_html("<&>\"'"), "&lt;&amp;&gt;&quot;&#39;");
    }
}
