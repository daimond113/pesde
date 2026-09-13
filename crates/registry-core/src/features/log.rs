use async_trait::async_trait;
use pesde::source::pesde::registry::*;

#[async_trait]
pub trait Repository {
	async fn global_size(&self) -> anyhow::Result<u64>;

	async fn global_entry(&self, pos: u64) -> anyhow::Result<Option<GlobalEntry>>;

	async fn scope_size(&self, scope: &ScopeId) -> anyhow::Result<u64>;

	async fn scope_entry(&self, scope: &ScopeId, pos: u64) -> anyhow::Result<Option<ScopeEntry>>;
}
