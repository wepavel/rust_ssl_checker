pub(crate) mod file;
mod selectel;

use async_trait::async_trait;
use base::prelude::anyhow;
pub use file::FileSourceService;
pub use selectel::SelectelSourceService;

#[async_trait]
pub(crate) trait DomainSourceTrait: Send + Sync {
    async fn get_domains(&self) -> anyhow::Result<Vec<String>>;
    fn get_source_name(&self) -> &'static str;
}
