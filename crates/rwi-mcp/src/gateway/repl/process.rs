use rwi_js::protocol::{Event, FRAME_CAP};
use std::{io, process::Stdio, time::Duration};
use tokio::{
    io::BufReader,
    process::{Child, ChildStdin, ChildStdout, Command},
};

pub(super) struct Worker {
    child: Child,
    input: ChildStdin,
    output: BufReader<ChildStdout>,
    pub generation: String,
}
impl Worker {
    pub fn start() -> io::Result<Self> {
        let mut command = Command::new(std::env::current_exe()?);
        command
            .arg("--js-worker")
            .env_clear()
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true);
        #[cfg(windows)]
        command.creation_flags(0x08000000); // CREATE_NO_WINDOW
        let mut child = command.spawn()?;
        let input = child
            .stdin
            .take()
            .ok_or_else(|| io::Error::other("worker_stdin_missing"))?;
        let output = BufReader::new(
            child
                .stdout
                .take()
                .ok_or_else(|| io::Error::other("worker_stdout_missing"))?,
        );
        Ok(Self {
            child,
            input,
            output,
            generation: uuid::Uuid::new_v4().to_string(),
        })
    }
    pub async fn send(&mut self, value: &impl serde::Serialize) -> io::Result<()> {
        rwi_native_link::device::write(&mut self.input, value, FRAME_CAP).await
    }
    pub async fn read(&mut self) -> io::Result<Event> {
        let bytes = rwi_native_link::wire::read_frame(&mut self.output, FRAME_CAP)
            .await?
            .ok_or_else(|| io::Error::other("js_worker_exited"))?;
        serde_json::from_slice(&bytes).map_err(io::Error::other)
    }
    pub async fn stop(&mut self) {
        let _ = self.child.start_kill();
        let _ = tokio::time::timeout(Duration::from_secs(1), self.child.wait()).await;
    }
}
