// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/11 17:52:05.350474
//+ Revised: 	2026/11/13 17:30:06.940655
// -----------------------------------------------------------------------------

use super::*;
use std::time::Duration;

/// How long an adapter may take to answer its first messages.
const STARTUP: Duration = Duration::from_secs(30);
const DOWNLOAD_STARTUP: Duration = Duration::from_secs(300);

impl AgentSession {
    /// Start on a background thread. Initialization progress appears in the snapshot.
    pub fn spawn(provider: Provider, cwd: &Path) -> Result<Self> {
        let (command, downloads) = provider.command()?;
        // The first start through npx downloads the adapter, which can take minutes.
        let startup = if downloads { DOWNLOAD_STARTUP } else { STARTUP };
        Self::spawn_with(provider, command, cwd, startup)
    }

    pub fn spawn_in_path(
        provider: Provider,
        cwd: &Path,
        path: Option<&std::ffi::OsStr>,
    ) -> Result<Self> {
        let (command, downloads) = provider.command_in(path)?;
        Self::spawn_with(
            provider,
            command,
            cwd,
            if downloads { DOWNLOAD_STARTUP } else { STARTUP },
        )
    }

    /// Use a custom ACP command, also useful for entirely offline protocol fixtures.
    pub fn spawn_command(provider: Provider, command: Command, cwd: &Path) -> Result<Self> {
        Self::spawn_with(provider, command, cwd, STARTUP)
    }

    fn spawn_with(
        provider: Provider,
        mut command: Command,
        cwd: &Path,
        startup: Duration,
    ) -> Result<Self> {
        let cwd = cwd
            .canonicalize()
            .context("The agent working directory does not exist.")?;
        ensure!(
            cwd.is_dir(),
            "The agent working directory is not a directory."
        );
        let cwd = crate::process::launch::native_path(&cwd);
        let cwd_text = protocol_cwd(&cwd)?;
        command
            .current_dir(&cwd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        process::configure(&mut command);
        let mut child = command
            .spawn()
            .context("Cannot launch the ACP adapter. Check its installation.")?;
        let tree = match ProcessTree::attach_child(&child) {
            Ok(tree) => Arc::new(tree),
            Err(error) => {
                if let Err(error) = child.kill() {
                    tracing::warn!(%error, "unsupervised agent could not be stopped");
                }
                if let Err(error) = child.wait() {
                    tracing::error!(%error, "unsupervised agent could not be reaped");
                }
                return Err(error);
            }
        };
        let process_id = child.id();
        let stdin = child.stdin.take().context("Agent stdin was not created.")?;
        let stdout = child
            .stdout
            .take()
            .context("Agent stdout was not created.")?;
        let stderr = child
            .stderr
            .take()
            .context("Agent stderr was not created.")?;
        let state = Arc::new(Mutex::new(AgentSnapshot::new(provider)));
        let shutdown = Arc::new(AtomicBool::new(false));
        let (events, receiver) = mpsc::sync_channel(QUEUE_CAPACITY);
        let (outbound, writer) = mpsc::sync_channel(32);
        let engine = Engine::new(EngineParts {
            child,
            tree: tree.clone(),
            state: state.clone(),
            shutdown: shutdown.clone(),
            outbound,
            events: receiver,
            cwd: cwd_text,
            startup,
        });
        let mut session = Self {
            events: events.clone(),
            state,
            shutdown: shutdown.clone(),
            tree,
            workers: Vec::new(),
            process_id,
        };
        session.start("orin-acp-output", {
            let events = events.clone();
            let shutdown = shutdown.clone();
            move || transport::read_stdout(stdout, events, shutdown)
        })?;
        session.start("orin-acp-input", {
            let events = events.clone();
            let shutdown = shutdown.clone();
            move || transport::write_stdin(stdin, writer, events, shutdown)
        })?;
        session.start("orin-acp-diagnostics", {
            let shutdown = shutdown.clone();
            move || transport::drain_stderr(stderr, shutdown)
        })?;
        session.start("orin-acp-session", move || engine.run())?;
        tracing::info!(provider = provider.label(), "ACP adapter launched");
        Ok(session)
    }

    fn start(&mut self, name: &str, task: impl FnOnce() + Send + 'static) -> Result<()> {
        self.workers
            .push(thread::Builder::new().name(name.into()).spawn(task)?);
        Ok(())
    }
}
fn protocol_cwd(cwd: &Path) -> Result<String> {
    crate::process::launch::native_path(cwd)
        .into_os_string()
        .into_string()
        .ok()
        .context("The project path must be valid Unicode for ACP.")
}

#[cfg(all(test, windows))]
mod tests {
    use super::protocol_cwd;
    use std::path::Path;

    #[test]
    fn unit_acp_working_directory_preserves_drive_and_unc_absolute_paths() {
        assert_eq!(
            protocol_cwd(Path::new(r"\\?\C:\project")).unwrap(),
            r"C:\project"
        );
        assert_eq!(
            protocol_cwd(Path::new(r"\\?\UNC\server\share\project")).unwrap(),
            r"\\server\share\project"
        );
    }
}
