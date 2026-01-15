use super::DomainSourceTrait;
use base::prelude::{
    anyhow::{Context, Result},
    tokio::fs,
};
use async_trait::async_trait;

pub struct FileSourceService {
    filename: String,
    #[allow(dead_code)]
    dcl: &'static str,
}

impl FileSourceService {
    pub fn new(filename: &str) -> Self {
        Self { filename: filename.to_string(), dcl: "FileSourceService" }
    }
}

#[async_trait]
impl DomainSourceTrait for FileSourceService {
    async fn get_domains(&self) -> Result<Vec<String>> {
        let path = format!("./{}", self.filename);

        let content = fs::read_to_string(&path)
            .await
            .with_context(|| format!("Не удалось прочитать файл: {}", path))?;

        let domains = content
            .lines()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();

        Ok(domains)
    }

    fn get_source_name(&self) -> &'static str {
        self.dcl
    }
}
