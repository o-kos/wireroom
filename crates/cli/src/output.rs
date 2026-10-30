#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
pub use linux::Output;

#[cfg(not(target_os = "linux"))]
pub struct Output;
#[cfg(not(target_os = "linux"))]
impl Output {
    pub fn open(_: &str) -> anyhow::Result<Self> {
        anyhow::bail!("Safe output ancestry is currently supported only on Linux")
    }
    pub fn temporary(&self) -> anyhow::Result<std::fs::File> {
        anyhow::bail!("Unsupported output platform")
    }
    pub fn materialize(
        &mut self,
        _: &dmd_replay::Recording,
        _: &dmd_protocol::ResultEvent,
    ) -> anyhow::Result<()> {
        anyhow::bail!("Unsupported output platform")
    }
}
