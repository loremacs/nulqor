use std::path::PathBuf;
use std::process;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let root: PathBuf = if args.len() > 1 {
        PathBuf::from(&args[1])
    } else {
        std::env::current_dir().expect("cannot determine current directory")
    };

    if !root.exists() {
        eprintln!("FAIL: {}: path does not exist", root.display());
        process::exit(1);
    }

    let errors = nulqor_lint::lint_extensions(&root);

    if errors.is_empty() {
        eprintln!("OK: no linter errors in {}", root.display());
        process::exit(0);
    }

    for e in &errors {
        println!("{e}");
    }
    process::exit(1);
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;
    use std::path::PathBuf;

    fn tmp_dir() -> PathBuf {
        let base = std::env::temp_dir().join(format!(
            "nulqor-lint-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .subsec_nanos()
        ));
        fs::create_dir_all(&base).unwrap();
        base
    }

    fn write(root: &Path, rel: &str, content: &str) {
        let path = root.join(rel);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, content).unwrap();
    }

    fn valid_manifest(id: &str) -> String {
        format!(
            r#"[extension]
id            = "{id}"
version       = "0.1.0"
kind          = "Panel"
api-version   = "v1"
schema-version = "1.0.0"
min-core      = "0.1.0"
"#
        )
    }

    #[test]
    fn passes_valid_extension() {
        let root = tmp_dir();
        write(&root, "extensions/hello-panel/extension.toml", &valid_manifest("hello-panel"));
        let errors = nulqor_lint::lint_extensions(&root);
        assert!(errors.is_empty(), "Expected no errors, got: {errors:?}");
    }

    #[test]
    fn fails_missing_manifest() {
        let root = tmp_dir();
        write(&root, "extensions/no-manifest/README.md", "# placeholder");
        let errors = nulqor_lint::lint_extensions(&root);
        assert!(
            errors.iter().any(|e| e.contains("extension.toml not found")),
            "Expected missing-manifest error, got: {errors:?}"
        );
    }

    #[test]
    fn fails_bad_toml() {
        let root = tmp_dir();
        write(&root, "extensions/bad-toml/extension.toml", "this is not toml ][[[");
        let errors = nulqor_lint::lint_extensions(&root);
        assert!(
            errors.iter().any(|e| e.contains("TOML parse error")),
            "Expected parse error, got: {errors:?}"
        );
    }

    #[test]
    fn fails_invalid_id_format() {
        let root = tmp_dir();
        write(
            &root,
            "extensions/bad-id/extension.toml",
            &valid_manifest("Bad ID With Spaces"),
        );
        let errors = nulqor_lint::lint_extensions(&root);
        assert!(
            errors.iter().any(|e| e.contains("id") && e.contains("kebab-case")),
            "Expected id format error, got: {errors:?}"
        );
    }

    #[test]
    fn fails_invalid_kind() {
        let root = tmp_dir();
        write(
            &root,
            "extensions/bad-kind/extension.toml",
            r#"[extension]
id            = "bad-kind"
version       = "0.1.0"
kind          = "Widget"
api-version   = "v1"
schema-version = "1.0.0"
min-core      = "0.1.0"
"#,
        );
        let errors = nulqor_lint::lint_extensions(&root);
        assert!(
            errors.iter().any(|e| e.contains("kind") && e.contains("Widget")),
            "Expected kind error, got: {errors:?}"
        );
    }

    #[test]
    fn fails_command_missing_version() {
        let root = tmp_dir();
        write(
            &root,
            "extensions/bad-cmd/extension.toml",
            &format!(
                "{}\n[[commands]]\nid = \"hello:ping\"\nowner = \"bad-cmd\"\npermission = \"read\"\n",
                valid_manifest("bad-cmd")
            ),
        );
        let errors = nulqor_lint::lint_extensions(&root);
        assert!(
            errors.iter().any(|e| e.contains("@version")),
            "Expected @version error, got: {errors:?}"
        );
    }

    #[test]
    fn fails_command_version_zero() {
        let root = tmp_dir();
        write(
            &root,
            "extensions/bad-v0/extension.toml",
            &format!(
                "{}\n[[commands]]\nid = \"hello:ping@0\"\nowner = \"bad-v0\"\npermission = \"read\"\n",
                valid_manifest("bad-v0")
            ),
        );
        let errors = nulqor_lint::lint_extensions(&root);
        assert!(
            errors.iter().any(|e| e.contains("version must be >= 1")),
            "Expected version >= 1 error, got: {errors:?}"
        );
    }

    #[test]
    fn fails_bad_semver() {
        let root = tmp_dir();
        write(
            &root,
            "extensions/bad-semver/extension.toml",
            r#"[extension]
id            = "bad-semver"
version       = "not-semver"
kind          = "Service"
api-version   = "v1"
schema-version = "1.0.0"
min-core      = "0.1.0"
"#,
        );
        let errors = nulqor_lint::lint_extensions(&root);
        assert!(
            errors.iter().any(|e| e.contains("not valid semver")),
            "Expected semver error, got: {errors:?}"
        );
    }

    #[test]
    fn fails_cross_extension_fs_scope() {
        let root = tmp_dir();
        write(
            &root,
            "extensions/escape-scope/extension.toml",
            &format!(
                "{}\nfs-scopes = [\"../other-extension/data\"]\n",
                valid_manifest("escape-scope")
            ),
        );
        let errors = nulqor_lint::lint_extensions(&root);
        assert!(
            errors.iter().any(|e| e.contains("BOUNDARY")),
            "Expected boundary error, got: {errors:?}"
        );
    }

    #[test]
    fn passes_valid_command_and_event() {
        let root = tmp_dir();
        write(
            &root,
            "extensions/full-ext/extension.toml",
            r#"[extension]
id            = "full-ext"
version       = "0.1.0"
kind          = "Service"
api-version   = "v1"
schema-version = "1.0.0"
min-core      = "0.1.0"

[[commands]]
id           = "full-ext:do-work@1"
owner        = "full-ext"
input-schema  = "{ task: string }"
output-schema = "{ result: string }"
callable-by  = ["agent"]
permission   = "write"

[[publishes]]
id = "full-ext:work-done@1"

[[subscribes]]
namespace = "canvas"
name      = "ready"
"#,
        );
        let errors = nulqor_lint::lint_extensions(&root);
        assert!(errors.is_empty(), "Expected no errors, got: {errors:?}");
    }

    #[test]
    fn lint_extension_dir_accepts_valid() {
        let root = tmp_dir();
        write(
            &root,
            "extensions/one/extension.toml",
            &valid_manifest("one"),
        );
        let ext_dir = root.join("extensions/one");
        let errors = nulqor_lint::lint_extension_dir(&ext_dir, &root);
        assert!(errors.is_empty(), "Expected no errors, got: {errors:?}");
    }

    #[test]
    fn parse_manifest_returns_id() {
        let root = tmp_dir();
        write(
            &root,
            "extensions/my-ext/extension.toml",
            &valid_manifest("my-ext"),
        );
        let path = root.join("extensions/my-ext/extension.toml");
        let manifest = nulqor_lint::parse_manifest(&path).expect("parse failed");
        assert_eq!(manifest.extension.id, "my-ext");
    }
}
