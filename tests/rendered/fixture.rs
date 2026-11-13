// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/11 15:18:35.471132
//+ Revised: 	2026/11/12 22:43:40.639666
// -----------------------------------------------------------------------------

use anyhow::{Context, Result, ensure};
use std::{
    fs::{self, File},
    os::windows::process::CommandExt,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
};

/// What a test changes about how Orin is started.
#[derive(Clone, Debug, Default)]
pub struct LaunchOptions {
    pub open_project: bool,
    pub agent_program: Option<PathBuf>,
    /// Extra environment, such as the updater's debug-build overrides.
    pub env: Vec<(String, String)>,
    /// Run this copy instead of the built binary (the updater replaces the file it runs from).
    pub executable: Option<PathBuf>,
}

pub fn launch(scratch: &Path, options: &LaunchOptions) -> Result<Child> {
    let fixture = create_fixture(scratch)?;
    let executable = options
        .executable
        .clone()
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_BIN_EXE_orin")));
    let mut command = Command::new(executable);
    command
        .args(["--test-driver", "--settings"])
        .arg(scratch.join("settings.json"));
    if options.open_project {
        command.arg("--project").arg(fixture);
    }
    let system =
        PathBuf::from(std::env::var_os("SystemRoot").context("SystemRoot is required on Windows")?);
    command.env("ComSpec", system.join("System32/cmd.exe"));
    // The shell starts with the project; a fixed prompt keeps its temporary path out of captures.
    command.env("PROMPT", "$G");
    // A stand-in `codex` CLI: signed out, and its "login" only prints a marker, so the account
    // page and its sign-in flow never reach a real agent or account.
    let agents = scratch.join("agent-bin");
    fs::create_dir_all(&agents)?;
    fs::write(
        agents.join("codex.cmd"),
        concat!(
            "@echo off\r\n",
            "if \"%1 %2\"==\"login status\" (echo Not logged in & exit /b 1)\r\n",
            "if \"%1\"==\"login\" (echo FAKE CODEX LOGIN & exit /b 0)\r\n",
            "exit /b 2\r\n",
        ),
    )?;
    let mut paths = vec![agents, system.join("System32"), system];
    if let Some(path) = std::env::var_os("PATH")
        && let Some(git) = std::env::split_paths(&path).find(|path| path.join("git.exe").is_file())
    {
        paths.push(git);
    }
    command.env("PATH", std::env::join_paths(paths)?);
    command.env_remove("ORIN_TEST_AGENT");
    command.env_remove("ORIN_ACP_FIXTURE_MODE");
    // The updater only sees what a test asks for, and never writes to the real cache.
    for name in ["ORIN_RELEASE_TARGET", "ORIN_UPDATE_SOURCES"] {
        command.env_remove(name);
    }
    command.env("ORIN_UPDATE_DIR", scratch.join("updates"));
    if let Some(program) = &options.agent_program {
        command.env("ORIN_TEST_AGENT", program);
    }
    for (name, value) in &options.env {
        command.env(name, value);
    }
    command.stdin(Stdio::piped()).stdout(Stdio::piped());
    command.stderr(File::create(scratch.join("app.log"))?);
    command.creation_flags(0x0800_0000);
    command.spawn().context("launch native Orin test process")
}

pub fn create_fixture(scratch: &Path) -> Result<PathBuf> {
    let fixture = scratch.join("fixture");
    fs::create_dir_all(fixture.join("src"))?;
    fs::write(
        fixture.join("README.md"),
        concat!(
            "# Fixture\n",
            "A native Orin UI test project with **bold**, *soft*, and `inline code`.\n\n",
            "## Checklist\n\n",
            "- [x] Render headings\n",
            "- [ ] Follow [links](src/main.rs)\n\n",
            "> Supervise, don't babysit.\n\n",
            "| Column | Purpose |\n| --- | --- |\n| Code | Read and edit |\n\n",
            "```rust\nfn main() {}\n```\n",
        ),
    )?;
    fs::write(
        fixture.join("src/main.rs"),
        "fn main() {\n    println!(\"before\");\n}\n",
    )?;
    // Files of different kinds side by side (README, Config, Git, Code), a well-known tests
    // folder (Git skips empty folders, so it holds a file), and ignored build output.
    fs::write(
        fixture.join("Cargo.toml"),
        "[package]\nname = \"fixture\"\n",
    )?;
    fs::write(fixture.join(".gitignore"), "target/\n")?;
    fs::create_dir_all(fixture.join("tests"))?;
    fs::write(fixture.join("tests/smoke.rs"), "#[test]\nfn smoke() {}\n")?;
    fs::create_dir_all(fixture.join("target/debug"))?;
    fs::write(
        fixture.join("target/debug/app.bin"),
        [0x7f, b'E', b'L', b'F'],
    )?;
    for args in [
        vec!["init", "--initial-branch=main", "--quiet"],
        vec!["add", "."],
    ] {
        let status = Command::new("git")
            .current_dir(&fixture)
            .args(args)
            .status()?;
        ensure!(status.success(), "create fixture Git index failed");
    }
    fs::write(
        fixture.join("src/main.rs"),
        "fn main() {\n    println!(\"after\");\n}\n",
    )?;
    Ok(fixture)
}
