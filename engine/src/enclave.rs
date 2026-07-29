//! Earns real Secure Enclave access for the identity helper.
//!
//! macOS grants Secure Enclave keys only to code whose signature carries a
//! keychain access group backed by an embedded provisioning profile — an
//! entitlement a bare command-line binary can never hold, which is why an
//! ad-hoc-signed helper always lands on `secure_enclave_unavailable`. The
//! builder's signed-in Xcode session can mint that profile, exactly as it
//! already mints iOS signing for every Shot. This module lets Xcode build a
//! minimal app bundle with the keychain entitlement, swaps the real helper
//! into it, re-signs the bundle, proves an Enclave key round-trip, and
//! installs the earned bundle under the machine's identity root.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const BUNDLE_NAME: &str = "TohsenoIdentity.app";
const PRODUCT_NAME: &str = "TohsenoIdentity";
const PROBE_TAG: &str = "enclave-earn-probe";

#[derive(Debug)]
pub struct EnclaveEarnError(pub String);

impl std::fmt::Display for EnclaveEarnError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}", self.0)
    }
}

impl std::error::Error for EnclaveEarnError {}

/// The earned helper executable, when a previous earn installed one.
pub fn earned_helper(identity_root: &Path) -> Option<PathBuf> {
    let executable = identity_root
        .join(BUNDLE_NAME)
        .join("Contents/MacOS")
        .join(PRODUCT_NAME);
    let metadata = fs::symlink_metadata(&executable).ok()?;
    use std::os::unix::fs::PermissionsExt;
    (metadata.is_file() && metadata.permissions().mode() & 0o100 != 0).then_some(executable)
}

/// Wraps `helper` in a provisioned app bundle and installs it under
/// `identity_root`, returning the earned executable. Requires a signed-in
/// Xcode account; the first team it lists is used.
pub fn earn(helper: &Path, identity_root: &Path) -> Result<PathBuf, EnclaveEarnError> {
    let team = first_xcode_team()?;
    let scratch = tempfile::tempdir()
        .map_err(|error| EnclaveEarnError(format!("scratch directory: {error}")))?;
    let project_root = scratch.path();

    write_scratch_project(project_root, &team)?;
    let built = build_scratch_app(project_root)?;
    let identity = signing_identity(&built)?;
    let entitlements = extract_entitlements(project_root, &built)?;

    fs::copy(helper, built.join("Contents/MacOS").join(PRODUCT_NAME))
        .map_err(|error| EnclaveEarnError(format!("installing helper into bundle: {error}")))?;
    run(
        "codesign",
        &[
            "--force".as_ref(),
            "--sign".as_ref(),
            identity.as_ref(),
            "--entitlements".as_ref(),
            entitlements.as_os_str(),
            built.as_os_str(),
        ],
        "re-signing the identity bundle",
    )?;

    fs::create_dir_all(identity_root)
        .map_err(|error| EnclaveEarnError(format!("identity root: {error}")))?;
    let installed = identity_root.join(BUNDLE_NAME);
    if installed.exists() {
        fs::remove_dir_all(&installed)
            .map_err(|error| EnclaveEarnError(format!("replacing earned bundle: {error}")))?;
    }
    copy_bundle(&built, &installed)?;
    let executable = installed.join("Contents/MacOS").join(PRODUCT_NAME);
    prove_enclave(&executable)?;
    Ok(executable)
}

/// The first team of the signed-in Xcode account. Any team works: a free
/// personal team mints Mac development profiles like a paid one.
fn first_xcode_team() -> Result<String, EnclaveEarnError> {
    let output = Command::new("defaults")
        .args(["read", "com.apple.dt.Xcode", "IDEProvisioningTeamByIdentifier"])
        .output()
        .map_err(|error| EnclaveEarnError(format!("reading Xcode accounts: {error}")))?;
    let listing = String::from_utf8_lossy(&output.stdout);
    listing
        .lines()
        .find_map(|line| {
            let trimmed = line.trim();
            trimmed
                .strip_prefix("teamID = ")?
                .strip_suffix(';')
                .map(|team| team.trim_matches('"').to_owned())
        })
        .ok_or_else(|| {
            EnclaveEarnError(
                "no Xcode account found — open Xcode, sign in under Settings → Accounts, \
                 then run tohseno again"
                    .into(),
            )
        })
}

fn write_scratch_project(root: &Path, team: &str) -> Result<(), EnclaveEarnError> {
    let write = |relative: &str, contents: &str| -> Result<(), EnclaveEarnError> {
        let path = root.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| EnclaveEarnError(format!("scratch project: {error}")))?;
        }
        fs::write(&path, contents)
            .map_err(|error| EnclaveEarnError(format!("scratch project: {error}")))
    };
    write("Sources/main.swift", "// placeholder executable; replaced by the real helper\n")?;
    write(
        "Identity.entitlements",
        concat!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n",
            "<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n",
            "<plist version=\"1.0\">\n<dict>\n",
            "\t<key>keychain-access-groups</key>\n\t<array>\n",
            "\t\t<string>$(AppIdentifierPrefix)com.tohseno.apple-identity</string>\n",
            "\t</array>\n</dict>\n</plist>\n"
        ),
    )?;
    write(
        "Proof.xcodeproj/project.pbxproj",
        &PBXPROJ_TEMPLATE.replace("@TEAM@", team),
    )
}

fn build_scratch_app(root: &Path) -> Result<PathBuf, EnclaveEarnError> {
    let output = Command::new("xcodebuild")
        .current_dir(root)
        .args([
            "-project",
            "Proof.xcodeproj",
            "-scheme",
            "Proof",
            "-configuration",
            "Release",
            "-derivedDataPath",
            "build",
            "-allowProvisioningUpdates",
            "build",
        ])
        .output()
        .map_err(|error| EnclaveEarnError(format!("running xcodebuild: {error}")))?;
    if !output.status.success() {
        let log = String::from_utf8_lossy(&output.stdout);
        let reason = log
            .lines()
            .filter(|line| line.contains("error:"))
            .collect::<Vec<_>>()
            .join("; ");
        return Err(EnclaveEarnError(format!(
            "Xcode could not mint the identity profile: {}",
            if reason.is_empty() { "unknown xcodebuild failure" } else { &reason }
        )));
    }
    let app = root.join(format!("build/Build/Products/Release/{PRODUCT_NAME}.app"));
    if !app.join("Contents/embedded.provisionprofile").is_file() {
        return Err(EnclaveEarnError(
            "the built bundle carries no provisioning profile".into(),
        ));
    }
    Ok(app)
}

/// The exact certificate xcodebuild chose, read back from the bundle.
fn signing_identity(app: &Path) -> Result<String, EnclaveEarnError> {
    let output = Command::new("codesign")
        .arg("-dvv")
        .arg(app)
        .output()
        .map_err(|error| EnclaveEarnError(format!("reading bundle signature: {error}")))?;
    let details = String::from_utf8_lossy(&output.stderr);
    details
        .lines()
        .find_map(|line| line.strip_prefix("Authority="))
        .map(str::to_owned)
        .ok_or_else(|| EnclaveEarnError("the built bundle has no signing authority".into()))
}

fn extract_entitlements(root: &Path, app: &Path) -> Result<PathBuf, EnclaveEarnError> {
    let output = Command::new("codesign")
        .arg("-d")
        .arg("--entitlements")
        .arg("-")
        .arg("--xml")
        .arg(app)
        .output()
        .map_err(|error| EnclaveEarnError(format!("reading bundle entitlements: {error}")))?;
    if !output.status.success() || output.stdout.is_empty() {
        return Err(EnclaveEarnError(
            "the built bundle has no entitlements".into(),
        ));
    }
    let path = root.join("minted.entitlements");
    fs::write(&path, &output.stdout)
        .map_err(|error| EnclaveEarnError(format!("saving entitlements: {error}")))?;
    Ok(path)
}

fn copy_bundle(source: &Path, destination: &Path) -> Result<(), EnclaveEarnError> {
    run(
        "cp",
        &[
            "-R".as_ref(),
            source.as_os_str(),
            destination.as_os_str(),
        ],
        "installing the earned bundle",
    )
}

/// A full create → delete round-trip on the real Enclave; anything less than
/// `test_only: false` fails the earn.
fn prove_enclave(executable: &Path) -> Result<(), EnclaveEarnError> {
    let _ = Command::new(executable)
        .args(["delete", "--tag", PROBE_TAG])
        .output();
    let output = Command::new(executable)
        .args(["create", "--tag", PROBE_TAG, "--backend", "secure-enclave"])
        .output()
        .map_err(|error| EnclaveEarnError(format!("probing the earned bundle: {error}")))?;
    let report = String::from_utf8_lossy(&output.stdout);
    let earned = output.status.success() && report.contains("\"test_only\":false");
    let _ = Command::new(executable)
        .args(["delete", "--tag", PROBE_TAG])
        .output();
    if earned {
        Ok(())
    } else {
        Err(EnclaveEarnError(format!(
            "the earned bundle still cannot reach the Secure Enclave: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )))
    }
}

fn run(
    program: &str,
    arguments: &[&std::ffi::OsStr],
    action: &str,
) -> Result<(), EnclaveEarnError> {
    let output = Command::new(program)
        .args(arguments)
        .output()
        .map_err(|error| EnclaveEarnError(format!("{action}: {error}")))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(EnclaveEarnError(format!(
            "{action}: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )))
    }
}

/// A minimal one-target macOS app project; Xcode's automatic signing mints
/// the Mac provisioning profile that backs the keychain entitlement.
const PBXPROJ_TEMPLATE: &str = r#"// !$*UTF8*$!
{
	archiveVersion = 1;
	classes = {
	};
	objectVersion = 56;
	objects = {
		AA0000000000000000000001 /* main.swift */ = {isa = PBXFileReference; lastKnownFileType = sourcecode.swift; path = main.swift; sourceTree = "<group>"; };
		AA0000000000000000000002 /* TohsenoIdentity.app */ = {isa = PBXFileReference; explicitFileType = wrapper.application; includeInIndex = 0; path = TohsenoIdentity.app; sourceTree = BUILT_PRODUCTS_DIR; };
		AA0000000000000000000003 /* main.swift in Sources */ = {isa = PBXBuildFile; fileRef = AA0000000000000000000001; };
		AA0000000000000000000010 = {
			isa = PBXGroup;
			children = (
				AA0000000000000000000011,
				AA0000000000000000000012,
			);
			sourceTree = "<group>";
		};
		AA0000000000000000000011 /* Sources */ = {
			isa = PBXGroup;
			children = (
				AA0000000000000000000001,
			);
			path = Sources;
			sourceTree = "<group>";
		};
		AA0000000000000000000012 /* Products */ = {
			isa = PBXGroup;
			children = (
				AA0000000000000000000002,
			);
			name = Products;
			sourceTree = "<group>";
		};
		AA0000000000000000000020 /* Sources */ = {
			isa = PBXSourcesBuildPhase;
			buildActionMask = 2147483647;
			files = (
				AA0000000000000000000003,
			);
			runOnlyForDeploymentPostprocessing = 0;
		};
		AA0000000000000000000030 /* Proof */ = {
			isa = PBXNativeTarget;
			buildConfigurationList = AA0000000000000000000041;
			buildPhases = (
				AA0000000000000000000020,
			);
			buildRules = (
			);
			dependencies = (
			);
			name = Proof;
			productName = Proof;
			productReference = AA0000000000000000000002;
			productType = "com.apple.product-type.application";
		};
		AA0000000000000000000040 /* Build configuration list for PBXProject */ = {
			isa = XCConfigurationList;
			buildConfigurations = (
				AA0000000000000000000050,
			);
			defaultConfigurationIsVisible = 0;
			defaultConfigurationName = Release;
		};
		AA0000000000000000000041 /* Build configuration list for PBXNativeTarget */ = {
			isa = XCConfigurationList;
			buildConfigurations = (
				AA0000000000000000000051,
			);
			defaultConfigurationIsVisible = 0;
			defaultConfigurationName = Release;
		};
		AA0000000000000000000050 /* Release */ = {
			isa = XCBuildConfiguration;
			buildSettings = {
				SDKROOT = macosx;
				SWIFT_VERSION = 5.0;
			};
			name = Release;
		};
		AA0000000000000000000051 /* Release */ = {
			isa = XCBuildConfiguration;
			buildSettings = {
				CODE_SIGN_ENTITLEMENTS = Identity.entitlements;
				CODE_SIGN_STYLE = Automatic;
				DEVELOPMENT_TEAM = @TEAM@;
				GENERATE_INFOPLIST_FILE = YES;
				MACOSX_DEPLOYMENT_TARGET = 13.0;
				PRODUCT_BUNDLE_IDENTIFIER = "com.tohseno.apple-identity";
				PRODUCT_NAME = TohsenoIdentity;
				PROVISIONING_PROFILE_SPECIFIER = "";
				SWIFT_VERSION = 5.0;
			};
			name = Release;
		};
		AA0000000000000000000060 /* Project object */ = {
			isa = PBXProject;
			attributes = {
				LastUpgradeCheck = 1500;
			};
			buildConfigurationList = AA0000000000000000000040;
			compatibilityVersion = "Xcode 14.0";
			developmentRegion = en;
			hasScannedForEncodings = 0;
			knownRegions = (
				en,
				Base,
			);
			mainGroup = AA0000000000000000000010;
			productRefGroup = AA0000000000000000000012;
			projectDirPath = "";
			projectRoot = "";
			targets = (
				AA0000000000000000000030,
			);
		};
	};
	rootObject = AA0000000000000000000060;
}
"#;
