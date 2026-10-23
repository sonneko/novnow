//! Release matrix, naming, version propagation, and archive contract (§20).

mod sandbox;

use std::io::Write;
use std::path::Path;
use std::process::{Command, Output};

use sandbox::{minimal_project, TempDir};
use serde_json::Value;

fn root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

fn run(args: &[&str]) -> Output {
    let output = Command::new("python3")
        .arg(root().join("scripts/release-matrix.py"))
        .args(args)
        .current_dir(root())
        .output()
        .expect("python3 must run the release matrix");
    assert!(
        output.status.success(),
        "release-matrix {args:?} failed\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

fn json(args: &[&str]) -> Value {
    let output = run(args);
    serde_json::from_slice(&output.stdout).expect("release matrix must emit JSON")
}

fn write_zip(path: &Path, entries: &[(&str, &str)]) {
    let file = std::fs::File::create(path).expect("create zip");
    let mut writer = zip::ZipWriter::new(file);
    for (name, text) in entries {
        writer
            .start_file(*name, zip::write::SimpleFileOptions::default())
            .expect("start zip entry");
        writer.write_all(text.as_bytes()).expect("write zip entry");
    }
    writer.finish().expect("finish zip");
}

#[test]
fn desktop_matrix_has_exactly_five_targets_times_two_gui_modes() {
    let matrix = json(&["--desktop"]);
    let rows = matrix["include"].as_array().expect("matrix include");
    assert_eq!(rows.len(), 10);
    let mut pairs: Vec<(&str, &str)> = rows
        .iter()
        .map(|row| {
            (
                row["target"].as_str().expect("target"),
                row["gui"].as_str().expect("gui"),
            )
        })
        .collect();
    pairs.sort_unstable();
    pairs.dedup();
    assert_eq!(pairs.len(), 10, "target/gui pairs must be unique");

    let version = env!("CARGO_PKG_VERSION");
    for row in rows {
        let target = row["target"].as_str().expect("target");
        let gui = row["gui"].as_str().expect("gui");
        let artifact = row["artifact"].as_str().expect("artifact");
        let extension = if target.ends_with("pc-windows-msvc") {
            ".zip"
        } else {
            ".tar.gz"
        };
        assert_eq!(
            artifact,
            format!("novnow-{version}-{target}-{gui}{extension}")
        );
        assert_eq!(
            row["features"].as_str(),
            Some(if gui == "gui" { "ui" } else { "" })
        );
        let named = run(&["--artifact", "--target", target, "--gui", gui]);
        assert_eq!(
            String::from_utf8(named.stdout).expect("utf8").trim(),
            artifact
        );
    }
}

#[test]
fn wheel_matrix_has_five_abi3_wheels_and_expected_platform_tags() {
    let matrix = json(&["--wheels"]);
    let rows = matrix["include"].as_array().expect("matrix include");
    assert_eq!(rows.len(), 5);
    let expected = [
        "macosx_11_0_arm64",
        "macosx_11_0_x86_64",
        "manylinux_2_28_aarch64",
        "manylinux_2_28_x86_64",
        "win_amd64",
    ];
    let mut actual: Vec<&str> = rows
        .iter()
        .map(|row| row["wheel_tag"].as_str().expect("wheel tag"))
        .collect();
    actual.sort_unstable();
    assert_eq!(actual, expected);
    for row in rows {
        let artifact = row["artifact"].as_str().expect("artifact");
        assert!(artifact.contains("-cp39-abi3-"), "{artifact}");
        assert!(artifact.starts_with(&format!("novnow-{}-", env!("CARGO_PKG_VERSION"))));
    }
}

#[test]
fn derived_versions_and_windows_subsystem_stay_in_sync() {
    run(&["--check"]);
    let main = include_str!("../src/main.rs");
    assert!(main
        .contains("#![cfg_attr(all(windows, feature = \"ui\"), windows_subsystem = \"windows\")]"));
    assert!(!main.contains("all(windows, not(debug_assertions))"));
}

#[test]
fn all_archives_are_self_contained_and_release_checksums_cover_every_artifact() {
    let temp = TempDir::new("distribution-archives");
    let release = temp.mkdir("release");
    let binary = temp.write("binary", "fake novnow executable\n");
    let version = env!("CARGO_PKG_VERSION");
    let matrix = json(&["--desktop"]);
    for row in matrix["include"].as_array().expect("desktop rows") {
        let target = row["target"].as_str().expect("target");
        let gui = row["gui"].as_str().expect("gui");
        let artifact = row["artifact"].as_str().expect("artifact");
        let output = release.join(artifact);
        let app = temp.join(&format!("{target}/novnow.app"));
        let macos_gui = target.ends_with("apple-darwin") && gui == "gui";
        if macos_gui {
            run(&[
                "--bundle-macos",
                "--binary",
                binary.to_str().expect("utf8 path"),
                "--output",
                app.to_str().expect("utf8 path"),
            ]);
        }
        let mut args = vec![
            "--package",
            "--target",
            target,
            "--gui",
            gui,
            "--output",
            output.to_str().expect("utf8 path"),
        ];
        if macos_gui {
            args.extend(["--app", app.to_str().expect("utf8 path")]);
        } else {
            args.extend(["--binary", binary.to_str().expect("utf8 path")]);
        }
        run(&args);
        run(&[
            "--verify-archive",
            output.to_str().expect("utf8 path"),
            "--target",
            target,
            "--gui",
            gui,
        ]);
    }

    let wheels = json(&["--wheels"]);
    for row in wheels["include"].as_array().expect("wheel rows") {
        let tag = format!(
            "cp39-abi3-{}",
            row["wheel_tag"].as_str().expect("wheel tag")
        );
        let metadata = format!("Metadata-Version: 2.1\nName: novnow\nVersion: {version}\n");
        let wheel = format!("Wheel-Version: 1.0\nRoot-Is-Purelib: false\nTag: {tag}\n");
        let metadata_path = format!("novnow-{version}.dist-info/METADATA");
        let wheel_path = format!("novnow-{version}.dist-info/WHEEL");
        write_zip(
            &release.join(row["artifact"].as_str().expect("wheel artifact")),
            &[
                (metadata_path.as_str(), &metadata),
                (wheel_path.as_str(), &wheel),
            ],
        );
    }
    run(&[
        "--checksums",
        release.to_str().expect("utf8 path"),
        "--signature-note",
        "signatures not configured in this test",
    ]);
    run(&[
        "--verify-release",
        release.to_str().expect("utf8 path"),
        "--artifacts-only",
    ]);

    let vsix_manifest =
        format!("{{\"name\":\"novnow\",\"publisher\":\"sonneko\",\"version\":\"{version}\"}}");
    write_zip(
        &release.join(format!("novnow-{version}.vsix")),
        &[("extension/package.json", &vsix_manifest)],
    );
    run(&["--checksums", release.to_str().expect("utf8 path")]);
    run(&["--verify-release", release.to_str().expect("utf8 path")]);
}

#[test]
fn composite_action_resolves_runner_targets_and_action_version() {
    let temp = TempDir::new("action-resolve");
    let cases = [
        ("Linux", "X64", "x86_64-unknown-linux-gnu"),
        ("Linux", "ARM64", "aarch64-unknown-linux-gnu"),
        ("macOS", "X64", "x86_64-apple-darwin"),
        ("macOS", "ARM64", "aarch64-apple-darwin"),
        ("Windows", "X64", "x86_64-pc-windows-msvc"),
    ];
    for (index, (os, arch, target)) in cases.into_iter().enumerate() {
        let output_file = temp.join(&format!("output-{index}"));
        let output = Command::new("bash")
            .arg(root().join("scripts/action/install.sh"))
            .arg("--resolve")
            .env("RUNNER_OS", os)
            .env("RUNNER_ARCH", arch)
            .env("NOVNOW_ACTION_VERSION", env!("CARGO_PKG_VERSION"))
            .env("GITHUB_OUTPUT", &output_file)
            .output()
            .expect("resolve action target");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let values = std::fs::read_to_string(output_file).expect("output values");
        assert!(values.contains(&format!("version={}\n", env!("CARGO_PKG_VERSION"))));
        assert!(values.contains(&format!("target={target}\n")));
    }
}

#[test]
fn composite_action_preserves_check_and_export_results() {
    let temp = TempDir::new("action-run");
    let project = minimal_project(&temp, "novel");
    let output_file = temp.join("output");
    let action = || {
        let mut command = Command::new("bash");
        command
            .arg(root().join("scripts/action/install.sh"))
            .arg("--run")
            .env("NOVNOW_BINARY", env!("CARGO_BIN_EXE_novnow"))
            .env("INPUT_PATH", &project)
            .env("GITHUB_OUTPUT", &output_file);
        command
    };
    let output = action()
        .env("INPUT_COMMAND", "check")
        .env("INPUT_ARGS", "")
        .output()
        .expect("run action check");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let values = std::fs::read_to_string(&output_file).expect("check outputs");
    assert!(values.contains("exit-code=0\n"));
    assert!(values.contains("findings="));

    std::fs::write(
        project.join("contents/01-01.txt"),
        "@01-01-01\n'action_unknown_term\n",
    )
    .expect("write E diagnostic fixture");
    let output = action()
        .env("INPUT_COMMAND", "check")
        .env("INPUT_ARGS", "")
        .output()
        .expect("run failed action check");
    assert_eq!(output.status.code(), Some(5));
    let values = std::fs::read_to_string(&output_file).expect("failed check outputs");
    assert!(values.contains("exit-code=5\n"));
    assert!(values.contains("findings=1\n"));

    let exported = temp.join("exported.txt");
    let output = action()
        .env("INPUT_COMMAND", "export")
        .env("INPUT_ARGS", format!("--out {}", exported.display()))
        .output()
        .expect("run action export");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(exported.is_file());
    let values = std::fs::read_to_string(&output_file).expect("export outputs");
    assert!(values.contains("exit-code=0\n"));
    assert!(values.contains("findings=\n"));
    assert!(values.contains(&format!("export-path={}\n", exported.display())));
}

#[test]
fn status_distinguishes_published_browser_from_local_release_readiness() {
    let status = json(&["--status"]);
    assert_eq!(status["version"].as_str(), Some(env!("CARGO_PKG_VERSION")));
    let forms = status["forms"].as_array().expect("forms");
    assert_eq!(forms.len(), 5);
    assert_eq!(forms[0]["id"].as_str(), Some("browser"));
    assert_eq!(forms[0]["status"].as_str(), Some("published"));
    assert_eq!(
        forms
            .iter()
            .map(|row| row["choices"].as_u64().expect("choices"))
            .sum::<u64>(),
        18
    );
}
