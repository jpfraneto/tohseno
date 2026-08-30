use crate::catalog::{BuildSafety, BuildSafetyClassification, DependencyLock};
use crate::{NetworkError, Result};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use tohseno_protocol::digest::Bytes32;

const MAX_PROJECT_BYTES: u64 = 16 * 1024 * 1024;

pub fn classify_xcode_project(source_root: &Path, container_path: &Path) -> Result<BuildSafety> {
    let relative = container_path
        .strip_prefix(source_root)
        .map_err(|_| NetworkError::Invalid("Xcode container is outside the source root".into()))?;
    let is_workspace = relative.extension().and_then(|value| value.to_str()) == Some("xcworkspace");
    let project_file = if relative.extension().and_then(|value| value.to_str()) == Some("xcodeproj")
    {
        container_path.join("project.pbxproj")
    } else {
        let contents = container_path.join("contents.xcworkspacedata");
        if !contents.is_file() {
            return unsupported("workspace metadata is unavailable");
        }
        let projects = find_project_files(source_root)?;
        if projects.len() != 1 {
            return review("workspace contains an ambiguous set of Xcode projects");
        }
        projects[0].join("project.pbxproj")
    };
    let metadata = fs::symlink_metadata(&project_file)?;
    if !metadata.file_type().is_file() || metadata.len() > MAX_PROJECT_BYTES {
        return unsupported("Xcode project metadata is not a bounded regular file");
    }
    let text = fs::read_to_string(project_file)
        .map_err(|_| NetworkError::Invalid("Xcode project metadata is not UTF-8".into()))?;
    let mut unsupported = Vec::new();
    let mut review = Vec::new();
    if is_workspace {
        review.push("workspace references require visible Mac review".to_owned());
    }
    for (needle, reason) in [
        ("PBXShellScriptBuildPhase", "Run Script build phase"),
        ("PBXBuildRule", "custom Xcode build rule"),
        ("PBXAppleScriptBuildPhase", "AppleScript build phase"),
        ("PBXRezBuildPhase", "Rez build phase"),
        (
            "com.apple.product-type.app-extension",
            "application extension with coordinated signing",
        ),
        (
            "com.apple.product-type.system-extension",
            "system extension",
        ),
    ] {
        if text.contains(needle) {
            if needle == "com.apple.product-type.system-extension" {
                unsupported.push(reason.to_owned());
            } else {
                review.push(reason.to_owned());
            }
        }
    }
    if text.contains("PBXLegacyTarget") || text.contains("externalBuildToolPath") {
        unsupported.push("external legacy build target".to_owned());
    }
    let package_dependency = text.contains("XCSwiftPackageProductDependency")
        || text.contains("XCRemoteSwiftPackageReference")
        || text.contains("XCLocalSwiftPackageReference");
    if package_dependency {
        if collect_dependency_locks(source_root)?.is_empty() {
            unsupported
                .push("Swift Package dependency has no published resolution lock".to_owned());
        } else {
            review.push(
                "Swift Package dependency and build plugins require visible Mac review".to_owned(),
            );
        }
    }
    if text.contains("XCLocalSwiftPackageReference") {
        review.push("local Swift Package reference".to_owned());
    }
    let product_types = text
        .lines()
        .filter_map(|line| {
            let (_, value) = line.split_once("productType =")?;
            Some(
                value
                    .trim()
                    .trim_matches(|character| matches!(character, '"' | ';'))
                    .to_owned(),
            )
        })
        .collect::<Vec<_>>();
    if !product_types
        .iter()
        .any(|value| value == "com.apple.product-type.application")
    {
        unsupported.push("no ordinary native application target".to_owned());
    }
    if product_types
        .iter()
        .any(|value| value != "com.apple.product-type.application")
    {
        review.push("additional non-application Xcode target".to_owned());
    }
    if source_has_extension(source_root, "entitlements")? {
        review.push("explicit entitlement file".to_owned());
    }
    if source_contains(source_root, b"-load-plugin-executable")?
        || source_contains(source_root, b"-plugin-path")?
    {
        review.push("custom compiler plugin loading".to_owned());
    }
    for (needle, reason) in [
        ("aps-environment", "Push Notifications entitlement"),
        (
            "com.apple.security.application-groups",
            "App Groups entitlement",
        ),
        ("com.apple.developer.icloud", "iCloud entitlement"),
        (
            "com.apple.developer.applesignin",
            "Sign in with Apple entitlement",
        ),
        (
            "com.apple.developer.associated-domains",
            "Associated Domains entitlement",
        ),
    ] {
        if source_contains(source_root, needle.as_bytes())? {
            review.push(reason.to_owned());
        }
    }
    unsupported.sort();
    unsupported.dedup();
    review.sort();
    review.dedup();
    if !unsupported.is_empty() {
        return Ok(BuildSafety {
            classification: BuildSafetyClassification::Unsupported,
            reasons: unsupported,
        });
    }
    if !review.is_empty() {
        return Ok(BuildSafety {
            classification: BuildSafetyClassification::RequiresMacReview,
            reasons: review,
        });
    }
    Ok(BuildSafety {
        classification: BuildSafetyClassification::Green,
        reasons: Vec::new(),
    })
}

pub fn collect_dependency_locks(source_root: &Path) -> Result<Vec<DependencyLock>> {
    let root = source_root.canonicalize()?;
    let mut locks = Vec::new();
    visit(&root, 0, &mut |path, metadata| {
        if !metadata.is_file() || metadata.len() > 16 * 1024 * 1024 {
            return Ok(());
        }
        let name = path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or_default();
        if !matches!(
            name,
            "Package.resolved" | "Podfile.lock" | "Cartfile.resolved"
        ) {
            return Ok(());
        }
        let relative = path
            .strip_prefix(&root)
            .map_err(|_| NetworkError::Invalid("dependency lock escaped the source root".into()))?
            .to_str()
            .ok_or_else(|| NetworkError::Invalid("dependency lock path is not UTF-8".into()))?
            .replace('\\', "/");
        let mut file = fs::File::open(path)?;
        let mut hasher = Sha256::new();
        let mut buffer = [0_u8; 64 * 1024];
        loop {
            let count = file.read(&mut buffer)?;
            if count == 0 {
                break;
            }
            hasher.update(&buffer[..count]);
        }
        locks.push(DependencyLock {
            path: relative,
            sha256: Bytes32::new(hasher.finalize().into()),
        });
        Ok(())
    })?;
    locks.sort_by(|left, right| left.path.cmp(&right.path));
    if locks.windows(2).any(|items| items[0].path == items[1].path) {
        return Err(NetworkError::Invalid(
            "duplicate dependency lock path".into(),
        ));
    }
    Ok(locks)
}

fn review(reason: &str) -> Result<BuildSafety> {
    Ok(BuildSafety {
        classification: BuildSafetyClassification::RequiresMacReview,
        reasons: vec![reason.to_owned()],
    })
}

fn unsupported(reason: &str) -> Result<BuildSafety> {
    Ok(BuildSafety {
        classification: BuildSafetyClassification::Unsupported,
        reasons: vec![reason.to_owned()],
    })
}

fn find_project_files(root: &Path) -> Result<Vec<PathBuf>> {
    let mut found = Vec::new();
    visit(root, 0, &mut |path, metadata| {
        if metadata.is_dir()
            && path.extension().and_then(|value| value.to_str()) == Some("xcodeproj")
        {
            found.push(path.to_path_buf());
        }
        Ok(())
    })?;
    found.sort();
    Ok(found)
}

fn source_contains(root: &Path, needle: &[u8]) -> Result<bool> {
    let mut matched = false;
    visit(root, 0, &mut |path, metadata| {
        if matched || !metadata.is_file() || metadata.len() > 4 * 1024 * 1024 {
            return Ok(());
        }
        let extension = path
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or_default();
        if matches!(extension, "entitlements" | "pbxproj" | "plist") {
            let bytes = fs::read(path)?;
            matched = bytes.windows(needle.len()).any(|window| window == needle);
        }
        Ok(())
    })?;
    Ok(matched)
}

fn source_has_extension(root: &Path, expected: &str) -> Result<bool> {
    let mut matched = false;
    visit(root, 0, &mut |path, metadata| {
        if metadata.is_file() && path.extension().and_then(|value| value.to_str()) == Some(expected)
        {
            matched = true;
        }
        Ok(())
    })?;
    Ok(matched)
}

fn visit(
    root: &Path,
    depth: usize,
    callback: &mut impl FnMut(&Path, &fs::Metadata) -> Result<()>,
) -> Result<()> {
    if depth > 32 {
        return Err(NetworkError::Invalid(
            "source directory exceeds maximum depth".into(),
        ));
    }
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path)?;
        if metadata.file_type().is_symlink() {
            continue;
        }
        callback(&path, &metadata)?;
        if metadata.is_dir() && !ignored_directory(&path) {
            visit(&path, depth + 1, callback)?;
        }
    }
    Ok(())
}

fn ignored_directory(path: &Path) -> bool {
    matches!(
        path.file_name().and_then(|value| value.to_str()),
        Some(".git" | ".build" | ".swiftpm" | "DerivedData" | "build" | "xcuserdata")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project(contents: &str) -> (tempfile::TempDir, PathBuf) {
        let root = tempfile::tempdir().unwrap();
        let container = root.path().join("Fixture.xcodeproj");
        fs::create_dir(&container).unwrap();
        fs::write(container.join("project.pbxproj"), contents).unwrap();
        (root, container)
    }

    #[test]
    fn green_profile_is_deliberately_narrow() {
        let (root, container) = project("productType = com.apple.product-type.application;\n");
        let safety = classify_xcode_project(root.path(), &container).unwrap();
        assert_eq!(safety.classification, BuildSafetyClassification::Green);
        assert!(safety.reasons.is_empty());
    }

    #[test]
    fn unpinned_packages_are_unsupported_and_locks_are_content_bound() {
        let (root, container) = project(
            "productType = com.apple.product-type.application;\nisa = XCSwiftPackageProductDependency;\n",
        );
        let safety = classify_xcode_project(root.path(), &container).unwrap();
        assert_eq!(
            safety.classification,
            BuildSafetyClassification::Unsupported
        );
        fs::write(root.path().join("Package.resolved"), b"{\"version\":2}\n").unwrap();
        let locks = collect_dependency_locks(root.path()).unwrap();
        assert_eq!(locks.len(), 1);
        let safety = classify_xcode_project(root.path(), &container).unwrap();
        assert_eq!(
            safety.classification,
            BuildSafetyClassification::RequiresMacReview
        );
    }

    #[test]
    fn executable_build_hooks_require_visible_mac_review() {
        let (root, container) = project(
            "productType = com.apple.product-type.application;\nisa = PBXShellScriptBuildPhase; shellScript = evil;\n",
        );
        let safety = classify_xcode_project(root.path(), &container).unwrap();
        assert_eq!(
            safety.classification,
            BuildSafetyClassification::RequiresMacReview
        );
        assert_eq!(safety.reasons, ["Run Script build phase"]);
    }

    #[test]
    fn unsupported_system_extensions_never_enter_automatic_build() {
        let (root, container) = project(
            "productType = com.apple.product-type.application;\nproductType = com.apple.product-type.system-extension;\n",
        );
        let safety = classify_xcode_project(root.path(), &container).unwrap();
        assert_eq!(
            safety.classification,
            BuildSafetyClassification::Unsupported
        );
        assert_eq!(safety.reasons, ["system extension"]);
    }
}
