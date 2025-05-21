// sync/entity_merger/document_type.rs
use super::{DomainEntityMerger, BaseDomainMerger};
use async_trait::async_trait;
use crate::auth::AuthContext;
use crate::errors::{DomainError, DomainResult};
use crate::domains::document::repository::DocumentTypeRepository;
use crate::domains::document::types::DocumentType;
use crate::domains::sync::types::{ChangeLogEntry, Tombstone};
use crate::domains::core::delete_service::{DeleteService, DeleteOptions};
use sqlx::{SqlitePool, Transaction, Sqlite};
use std::sync::Arc;

/// Minimal stub merger for document_types to satisfy compilation.
/// It currently performs no-op merges and delegates hard deletes to DeleteService if available.
pub struct DocumentTypeEntityMerger {
    repo: Arc<dyn DocumentTypeRepository + Send + Sync>,
    pool: SqlitePool,
    delete_service: Arc<dyn DeleteService<DocumentType> + Send + Sync>,
}

impl DocumentTypeEntityMerger {
    pub fn new(
        repo: Arc<dyn DocumentTypeRepository + Send + Sync>,
        pool: SqlitePool,
        delete_service: Arc<dyn DeleteService<DocumentType> + Send + Sync>,
    ) -> Self {
        Self { repo, pool, delete_service }
    }
}

#[async_trait]
impl DomainEntityMerger for DocumentTypeEntityMerger {
    fn entity_table(&self) -> &'static str { "document_types" }

    async fn apply_create(&self, _change: &ChangeLogEntry, _auth: &AuthContext) -> DomainResult<()> {
        // For now we ignore remote create/update/delete of document types.
        Ok(())
    }

    async fn apply_update(&self, _change: &ChangeLogEntry, _auth: &AuthContext) -> DomainResult<()> {
        Ok(())
    }

    async fn apply_soft_delete(&self, _change: &ChangeLogEntry, _auth: &AuthContext) -> DomainResult<()> {
        Ok(())
    }

    async fn apply_hard_delete(&self, tombstone: &Tombstone, auth: &AuthContext) -> DomainResult<()> {
        if tombstone.entity_type != self.entity_table() {
            return Err(DomainError::Internal("Tombstone entity type mismatch for document_types".into()));
        }
        let opts = DeleteOptions { allow_hard_delete: true, fallback_to_soft_delete: false, force: true };
        let _ = self.delete_service.delete(tombstone.entity_id, auth, opts).await?;
        Ok(())
    }

    async fn apply_change_with_tx<'t>(&self, _change: &ChangeLogEntry, _auth: &AuthContext, _tx: &mut Transaction<'t, Sqlite>) -> DomainResult<()> {
        Ok(())
    }
} 