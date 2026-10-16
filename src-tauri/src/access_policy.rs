use std::fs;
use std::path::Path;

#[test]
fn capabilities_do_not_grant_filesystem_or_dialog_plugins() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("capabilities/default.json");
    let json = fs::read_to_string(path).unwrap();
    assert!(json.contains("core:default"));
    assert!(json.contains("core:window:allow-destroy"));
    for forbidden in ["fs:", "dialog:", "shell:", "http:", "plugin:fs"] {
        assert!(!json.contains(forbidden), "{forbidden} must stay out of capabilities");
    }
}

#[test]
fn frontend_has_no_direct_filesystem_api() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let package_json = fs::read_to_string(root.join("package.json")).unwrap();
    for forbidden in [
        "@tauri-apps/plugin-fs",
        "@tauri-apps/plugin-dialog",
        "@tauri-apps/plugin-store",
    ] {
        assert!(
            !package_json.contains(forbidden),
            "{forbidden} must not be installed"
        );
    }

    let mut files = Vec::new();
    collect_source_files(&root.join("src"), &mut files);
    for path in files {
        let text = fs::read_to_string(&path).unwrap();
        for forbidden in [
            "@tauri-apps/plugin-fs",
            "@tauri-apps/plugin-dialog",
            "readTextFile",
            "readDir",
        ] {
            assert!(
                !text.contains(forbidden),
                "{} contains {forbidden}",
                path.display()
            );
        }
    }
}

fn collect_source_files(dir: &Path, files: &mut Vec<std::path::PathBuf>) {
    let entries = fs::read_dir(dir).unwrap();
    for entry in entries {
        let path = entry.unwrap().path();
        if path.is_dir() {
            collect_source_files(&path, files);
            continue;
        }
        let extension = path.extension().and_then(|ext| ext.to_str()).unwrap_or("");
        if matches!(extension, "ts" | "js" | "svelte") {
            files.push(path);
        }
    }
}
