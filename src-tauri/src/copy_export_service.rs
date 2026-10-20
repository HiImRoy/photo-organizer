//! Authorization and process-local coordination for safe copy exports.
//!
//! The database freezes plans and journals jobs. This module binds explicit
//! user previews to those frozen records, holds short-lived target-parent
//! leases, consumes each authorization at most once, and dispatches the
//! handle-bound runner.

use std::collections::HashMap;
use std::io;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::copy_export_runner::{
    CopyExportProgress, run_copy_export_job_after_recovery_gate,
    run_copy_export_rollback_job_after_recovery_gate,
};
use crate::db::{
    CatalogRuntimeIdentity, CopyExportJobContext, CopyExportJobItem, CopyExportJobPage,
    CopyExportJobPageItem, CopyExportPlanBinding, RecentCopyExportJobRecord, Repository,
};
use crate::error::{AppError, AppResult};
use crate::export_file_safety::{
    CommittedTargetIdentity, DurableFileIdentity, FileOperationService, FileSnapshot,
    GrantedParent, ProtectedRoot,
};
use crate::models::{
    CopyExportRules, CreateOrganizationPlanV2Request, OrganizationPlanScopeV2, OrganizationPlanV2,
    OrganizationTargetV2, RecentOrganizationPlanV2,
};
use crate::source_identity::{identity_key, paths_overlap};

const GRANT_TTL: Duration = Duration::from_secs(5 * 60);
const AUTHORIZATION_TTL: Duration = Duration::from_secs(5 * 60);
const MAX_GRANTS: usize = 64;
const MAX_AUTHORIZATIONS: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CopyExportOperationKind {
    Copy,
    Resume,
    Rollback,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CopyExportPreflight {
    /// Plan ID for `copy`, job ID for `resume` and `rollback`.
    pub resource_id: String,
    pub operation: CopyExportOperationKind,
    pub authorization_token: String,
    pub expires_at_unix_ms: u64,
    pub target_root: String,
    pub plan_revision: String,
    pub snapshot_blake3: String,
    pub item_count: u64,
    pub estimated_bytes: u64,
    pub blocked_count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CopyExportStartResponse {
    pub job_id: String,
    pub accepted: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CopyExportRuntimeStatus {
    pub job_id: String,
    pub operation: CopyExportOperationKind,
    pub running: bool,
    pub job_state: Option<String>,
    pub progress: Option<CopyExportProgress>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CopyItemReconcileDecision {
    RetryWhenTargetAbsent,
    AdoptVerifiedComplete,
    KeepQuarantined,
}

impl CopyItemReconcileDecision {
    fn db_value(&self) -> &'static str {
        match self {
            Self::RetryWhenTargetAbsent => "retry_when_target_absent",
            Self::AdoptVerifiedComplete => "adopt_verified_complete",
            Self::KeepQuarantined => "keep_quarantined",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CopyExportJobItemView {
    pub ordinal: u32,
    pub asset_id: i64,
    pub state: String,
    pub attempts: u32,
    pub target_relative_path: PathBuf,
    pub source_snapshot: FileSnapshot,
    pub partial_target_identity: Option<DurableFileIdentity>,
    pub partial_snapshot: Option<FileSnapshot>,
    pub output_identity: Option<CommittedTargetIdentity>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CopyExportJobView {
    pub job_id: String,
    pub plan_id: String,
    pub target_root: String,
    pub state: String,
    pub progress_completed_items: u64,
    pub progress_total_items: u64,
    pub progress_completed_bytes: u64,
    pub progress_total_bytes: u64,
    pub current_item_ordinal: Option<u32>,
    pub cancel_requested: bool,
    pub created_at: String,
    pub updated_at: String,
    pub item_total: u64,
    pub item_page: u32,
    pub item_page_size: u32,
    pub items: Vec<CopyExportJobItemView>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RecentCopyExportJob {
    pub job_id: String,
    pub plan_id: String,
    pub target_root: String,
    pub state: String,
    pub progress_completed_items: u64,
    pub progress_total_items: u64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct AuthorizationKey {
    resource_id: String,
    operation: CopyExportOperationKind,
}

#[derive(Debug, Clone)]
struct AuthorizationBinding {
    resource_id: String,
    operation: CopyExportOperationKind,
    expires_at: Instant,
    catalog: CatalogRuntimeIdentity,
    plan_id: String,
    plan_revision: String,
    snapshot_blake3: String,
    target_parent_path: PathBuf,
    target_parent_identity: DurableFileIdentity,
    target_child_name: String,
    target_root_path: PathBuf,
    target_root_identity: Option<DurableFileIdentity>,
    protected_roots: Vec<ProtectedRoot>,
}

#[derive(Default)]
struct AuthorizationStore {
    by_token: HashMap<String, AuthorizationBinding>,
    latest_by_key: HashMap<AuthorizationKey, String>,
}

impl AuthorizationStore {
    fn revoke_latest(&mut self, key: &AuthorizationKey) {
        if let Some(old_token) = self.latest_by_key.remove(key) {
            self.by_token.remove(&old_token);
        }
        self.by_token
            .retain(|_, authorization| authorization.expires_at > Instant::now());
        self.latest_by_key
            .retain(|_, token| self.by_token.contains_key(token));
    }

    fn issue(
        &mut self,
        key: AuthorizationKey,
        mut binding: AuthorizationBinding,
    ) -> AppResult<String> {
        self.revoke_latest(&key);
        if self.by_token.len() >= MAX_AUTHORIZATIONS {
            return Err(AppError::InvalidArgument(
                "too many active export authorizations; wait for one to expire".into(),
            ));
        }
        let token = Uuid::new_v4().to_string();
        binding.expires_at = Instant::now() + AUTHORIZATION_TTL;
        self.by_token.insert(token.clone(), binding);
        self.latest_by_key.insert(key, token.clone());
        self.latest_by_key
            .retain(|_, active_token| self.by_token.contains_key(active_token));
        Ok(token)
    }

    /// Remove the token before validating it, making every attempted use
    /// one-shot, including stale or mismatched attempts.
    fn consume(&mut self, token: &str) -> Option<AuthorizationBinding> {
        let binding = self.by_token.remove(token)?;
        let key = AuthorizationKey {
            resource_id: binding.resource_id.clone(),
            operation: binding.operation,
        };
        if self
            .latest_by_key
            .get(&key)
            .is_some_and(|latest| latest == token)
        {
            self.latest_by_key.remove(&key);
        }
        (binding.expires_at > Instant::now()).then_some(binding)
    }
}

struct TargetGrant {
    path: PathBuf,
    identity: DurableFileIdentity,
    expires_at: Instant,
}

struct RunnerRuntime {
    operation: CopyExportOperationKind,
    cancel: Arc<AtomicBool>,
    progress: Mutex<Option<CopyExportProgress>>,
    error: Mutex<Option<String>>,
    finished: AtomicBool,
}

#[derive(Default)]
struct ServiceRuntime {
    grants: HashMap<String, TargetGrant>,
    authorizations: AuthorizationStore,
    runners: HashMap<String, Arc<RunnerRuntime>>,
}

/// Cloneable service handle. All mutable state is process-local and shared by
/// the IPC facade and background runner threads.
#[derive(Clone)]
pub struct CopyExportService {
    repository: Repository,
    app_data_path: PathBuf,
    runtime: Arc<Mutex<ServiceRuntime>>,
    preview_gate: Arc<Mutex<()>>,
    operation_gate: Arc<Mutex<()>>,
}

impl CopyExportService {
    pub fn new(repository: Repository) -> AppResult<Self> {
        let app_data_path = repository
            .database_path()
            .parent()
            .ok_or_else(|| AppError::InvalidArgument("database path has no parent".into()))?
            .to_path_buf();
        Ok(Self {
            repository,
            app_data_path,
            runtime: Arc::new(Mutex::new(ServiceRuntime::default())),
            preview_gate: Arc::new(Mutex::new(())),
            operation_gate: Arc::new(Mutex::new(())),
        })
    }

    /// Grant an existing, non-reparse directory as the parent for a new copy
    /// target. The held lease expires after five minutes and is bounded to 64
    /// concurrent grants.
    pub fn grant(&self, path: &Path) -> AppResult<crate::models::TargetParentGrantResponse> {
        let protected = self.current_protected_roots()?;
        let lease = GrantedParent::grant_existing(path, &protected)?;
        let grant_path = lease.path().to_path_buf();
        let grant_identity = lease.identity()?.clone();
        let id = Uuid::new_v4().to_string();
        let mut runtime = self.lock_runtime()?;
        runtime
            .grants
            .retain(|_, grant| grant.expires_at > Instant::now());
        if runtime.grants.len() >= MAX_GRANTS {
            return Err(AppError::InvalidArgument(
                "too many active target grants; wait for one to expire".into(),
            ));
        }
        runtime.grants.insert(
            id.clone(),
            TargetGrant {
                path: grant_path.clone(),
                identity: grant_identity,
                expires_at: Instant::now() + GRANT_TTL,
            },
        );
        Ok(crate::models::TargetParentGrantResponse {
            grant_id: id,
            display_path: grant_path.to_string_lossy().into_owned(),
        })
    }

    /// Build and persist a frozen plan from the granted parent. The target
    /// grant is re-opened and its physical directory identity compared before
    /// the planner reads the catalog.
    pub fn create_plan(
        &self,
        request: &CreateOrganizationPlanV2Request,
    ) -> AppResult<OrganizationPlanV2> {
        validate_child_name(&request.target.child_name)?;
        let grant = self.get_grant(&request.target.parent_grant_id)?;
        let protected = self.current_protected_roots()?;
        let parent = GrantedParent::grant_existing(&grant.path, &protected)?;
        if parent.identity()? != &grant.identity
            || identity_key(parent.path()) != identity_key(&grant.path)
        {
            return Err(unsafe_export("granted target parent identity changed"));
        }
        let root_path = parent.path().join(&request.target.child_name);
        reject_output_overlap(&root_path, &protected)?;
        let plan = crate::copy_export_plan::build_plan_v2(
            &self.repository,
            request,
            parent.path(),
            parent.identity()?,
            &protected,
        )?;
        self.repository.save_organization_plan_v2(
            request,
            &plan,
            parent.path(),
            parent.identity()?,
            &protected,
        )?;
        self.repository
            .read_organization_plan_v2(&plan.summary.plan_id, 1, 120)
    }

    /// Flat IPC contract used by the planner screen.
    pub fn create_plan_v2(
        &self,
        scope: OrganizationPlanScopeV2,
        target: OrganizationTargetV2,
        rules: Option<CopyExportRules>,
    ) -> AppResult<OrganizationPlanV2> {
        self.create_plan(&CreateOrganizationPlanV2Request {
            scope,
            target,
            rules: rules.unwrap_or_default(),
        })
    }

    pub fn get_plan(
        &self,
        plan_id: &str,
        item_page: u32,
        item_page_size: u32,
    ) -> AppResult<OrganizationPlanV2> {
        let page_size = if item_page_size == 0 {
            120
        } else {
            item_page_size.min(120)
        };
        self.repository
            .read_organization_plan_v2(plan_id, item_page.max(1), page_size)
    }

    pub fn list_recent_plans(&self, limit: u32) -> AppResult<Vec<RecentOrganizationPlanV2>> {
        self.repository
            .list_recent_organization_plans_v2(limit.min(100))
    }

    /// Inspect current identities and mint a one-use authorization token.
    /// `resource_id` is a plan ID for copy and a job ID for resume/rollback.
    pub fn preflight(
        &self,
        resource_id: &str,
        operation: CopyExportOperationKind,
    ) -> AppResult<CopyExportPreflight> {
        let _preview_guard = self
            .preview_gate
            .lock()
            .map_err(|_| internal_error("preview lock was poisoned"))?;
        let key = AuthorizationKey {
            resource_id: resource_id.to_owned(),
            operation,
        };
        self.lock_runtime()?.authorizations.revoke_latest(&key);

        let catalog = self.repository.catalog_runtime_identity()?;
        let latest_roots = self.current_protected_roots()?;
        let (plan, binding, item_count, estimated_bytes, blocked_count) = match operation {
            CopyExportOperationKind::Copy => {
                let plan = self.load_plan_header(resource_id)?;
                self.repository
                    .validate_copy_export_plan_fresh(resource_id)?;
                validate_copy_plan(&plan)?;
                let binding = self
                    .repository
                    .copy_export_plan_binding(resource_id)?
                    .ok_or_else(|| {
                        AppError::NotFound(format!("copy plan binding {resource_id}"))
                    })?;
                self.verify_plan_binding(&binding, &plan, &latest_roots, true)?;
                (
                    plan.clone(),
                    binding,
                    plan.summary.eligible_count,
                    plan.summary.estimated_bytes,
                    plan.summary.blocked_count,
                )
            }
            CopyExportOperationKind::Resume | CopyExportOperationKind::Rollback => {
                let context = self.repository.load_copy_export_context(resource_id)?;
                if operation == CopyExportOperationKind::Resume {
                    self.repository
                        .validate_copy_export_plan_fresh(&context.plan_id)?;
                }
                let job_state = self
                    .repository
                    .copy_export_job_state(resource_id)?
                    .ok_or_else(|| AppError::NotFound(format!("copy export job {resource_id}")))?;
                let plan = self.load_plan_header(&context.plan_id)?;
                let binding = self
                    .repository
                    .copy_export_plan_binding(&context.plan_id)?
                    .ok_or_else(|| {
                        AppError::NotFound(format!("copy plan binding {}", context.plan_id))
                    })?;
                self.verify_job_binding(&context, &binding, &plan, &latest_roots)?;
                validate_job_operation(&job_state, &context.items, operation)?;
                if operation == CopyExportOperationKind::Rollback {
                    self.verify_rollback_targets(&context, &latest_roots)?;
                }
                let item_count = if operation == CopyExportOperationKind::Rollback {
                    context
                        .items
                        .iter()
                        .filter(|item| item.state == "copied")
                        .count() as u64
                } else {
                    context
                        .items
                        .iter()
                        .filter(|item| item.state != "copied")
                        .count() as u64
                };
                let estimated_bytes = context
                    .items
                    .iter()
                    .filter(|item| match operation {
                        CopyExportOperationKind::Rollback => item.state == "copied",
                        CopyExportOperationKind::Resume => item.state != "copied",
                        CopyExportOperationKind::Copy => false,
                    })
                    .fold(0u64, |sum, item| {
                        sum.saturating_add(item.source_snapshot.size_bytes)
                    });
                (plan, binding, item_count, estimated_bytes, 0)
            }
        };

        let target_root = binding.target_parent_path.join(&binding.target_child_name);
        let auth_binding = AuthorizationBinding {
            resource_id: resource_id.to_owned(),
            operation,
            expires_at: Instant::now() + AUTHORIZATION_TTL,
            catalog,
            plan_id: plan.summary.plan_id.clone(),
            plan_revision: plan.summary.revision.clone(),
            snapshot_blake3: plan.summary.snapshot_blake3.clone(),
            target_parent_path: binding.target_parent_path.clone(),
            target_parent_identity: binding.target_parent_identity.clone(),
            target_child_name: binding.target_child_name.clone(),
            target_root_path: target_root.clone(),
            target_root_identity: self.root_identity_for_operation(resource_id, operation)?,
            protected_roots: merge_protected_roots(&binding.protected_roots, &latest_roots)?,
        };
        let token = self
            .lock_runtime()?
            .authorizations
            .issue(key, auth_binding)?;
        Ok(CopyExportPreflight {
            resource_id: resource_id.to_owned(),
            operation,
            authorization_token: token,
            expires_at_unix_ms: unix_ms_after(AUTHORIZATION_TTL),
            target_root: target_root.to_string_lossy().into_owned(),
            plan_revision: plan.summary.revision,
            snapshot_blake3: plan.summary.snapshot_blake3,
            item_count,
            estimated_bytes,
            blocked_count,
        })
    }

    pub fn start(&self, plan_id: &str, token: &str) -> AppResult<CopyExportStartResponse> {
        let _operation_guard = self
            .operation_gate
            .lock()
            .map_err(|_| internal_error("operation lock was poisoned"))?;
        let authorization =
            self.consume_authorization(token, plan_id, CopyExportOperationKind::Copy)?;
        self.ensure_runner_idle()?;
        self.validate_consumed_authorization(&authorization)?;
        self.repository.ensure_copy_recovery_gate()?;
        // The recovery backup can take long enough for a source, protected
        // root, or target parent to change. Recheck the consumed grant at the
        // last point before the database creates the job intent.
        self.validate_consumed_authorization(&authorization)?;
        let protected = self.current_protected_roots()?;
        let protected = merge_protected_roots(&authorization.protected_roots, &protected)?;
        let context = self
            .repository
            .prepare_copy_export_job(plan_id, &protected)?;
        if identity_key(&context.target_parent_path)
            != identity_key(&authorization.target_parent_path)
            || context.target_parent_identity != authorization.target_parent_identity
            || context.target_child_name != authorization.target_child_name
        {
            return Err(unsafe_export(
                "prepared job target differs from the authorized plan snapshot",
            ));
        }
        self.dispatch(context.job_id.clone(), CopyExportOperationKind::Copy)?;
        Ok(CopyExportStartResponse {
            job_id: context.job_id,
            accepted: true,
        })
    }

    pub fn get_job(
        &self,
        job_id: &str,
        item_page: u32,
        item_page_size: u32,
    ) -> AppResult<Option<CopyExportJobView>> {
        let page_size = if item_page_size == 0 {
            100
        } else {
            item_page_size.min(100)
        };
        Ok(self
            .repository
            .copy_export_job_page(job_id, item_page.max(1), page_size)?
            .map(copy_export_job_page_to_view))
    }

    pub fn list_recent_jobs(&self, limit: u32) -> AppResult<Vec<RecentCopyExportJob>> {
        Ok(self
            .repository
            .list_recent_copy_export_jobs(limit.min(100))?
            .into_iter()
            .map(recent_copy_export_job_to_view)
            .collect())
    }

    pub fn runtime_status(&self, job_id: &str) -> AppResult<Option<CopyExportRuntimeStatus>> {
        let runtime = self.lock_runtime()?.runners.get(job_id).cloned();
        let Some(runtime) = runtime else {
            return Ok(None);
        };
        let job_state = self.repository.copy_export_job_state(job_id)?;
        Ok(Some(CopyExportRuntimeStatus {
            job_id: job_id.to_owned(),
            operation: runtime.operation,
            running: !runtime.finished.load(Ordering::Acquire),
            job_state,
            progress: runtime
                .progress
                .lock()
                .map_err(|_| internal_error("runner progress lock was poisoned"))?
                .clone(),
            error: runtime
                .error
                .lock()
                .map_err(|_| internal_error("runner error lock was poisoned"))?
                .clone(),
        }))
    }

    pub fn cancel(&self, job_id: &str) -> AppResult<bool> {
        let runtime = self.lock_runtime()?.runners.get(job_id).cloned();
        if let Some(runtime) = runtime {
            runtime.cancel.store(true, Ordering::Release);
        }
        self.repository.request_cancel_copy_export(job_id)?;
        Ok(true)
    }

    pub fn preview_resume(&self, job_id: &str) -> AppResult<CopyExportPreflight> {
        self.preflight(job_id, CopyExportOperationKind::Resume)
    }

    pub fn resume(&self, job_id: &str, token: &str) -> AppResult<CopyExportStartResponse> {
        let _operation_guard = self
            .operation_gate
            .lock()
            .map_err(|_| internal_error("operation lock was poisoned"))?;
        let authorization =
            self.consume_authorization(token, job_id, CopyExportOperationKind::Resume)?;
        self.ensure_runner_idle()?;
        self.validate_consumed_authorization(&authorization)?;
        self.repository.ensure_copy_recovery_gate()?;
        // Revalidate after the potentially slow recovery proof and before the
        // resume queue transition or runner dispatch.
        self.validate_consumed_authorization(&authorization)?;
        let protected = self.current_protected_roots()?;
        let _ = merge_protected_roots(&authorization.protected_roots, &protected)?;
        self.repository.queue_copy_export_resume(job_id)?;
        self.dispatch(job_id.to_owned(), CopyExportOperationKind::Resume)?;
        Ok(CopyExportStartResponse {
            job_id: job_id.to_owned(),
            accepted: true,
        })
    }

    pub fn preview_rollback(&self, job_id: &str) -> AppResult<CopyExportPreflight> {
        self.preflight(job_id, CopyExportOperationKind::Rollback)
    }

    pub fn confirm_rollback(
        &self,
        job_id: &str,
        token: &str,
    ) -> AppResult<CopyExportStartResponse> {
        let _operation_guard = self
            .operation_gate
            .lock()
            .map_err(|_| internal_error("operation lock was poisoned"))?;
        let authorization =
            self.consume_authorization(token, job_id, CopyExportOperationKind::Rollback)?;
        self.ensure_runner_idle()?;
        self.validate_consumed_authorization(&authorization)?;
        self.repository.ensure_copy_recovery_gate()?;
        // Rollback is bound to the recorded outputs, parent/root identities,
        // and current protection roots; source catalog freshness is
        // intentionally not part of this authorization.
        self.validate_consumed_authorization(&authorization)?;
        self.dispatch(job_id.to_owned(), CopyExportOperationKind::Rollback)?;
        Ok(CopyExportStartResponse {
            job_id: job_id.to_owned(),
            accepted: true,
        })
    }

    /// Reconcile only through a verified absence, a fully inspected completed
    /// output, or an explicit quarantine decision. The service never deletes a
    /// restart-discovered partial file.
    pub fn reconcile_item(
        &self,
        job_id: &str,
        ordinal: u32,
        decision: CopyItemReconcileDecision,
    ) -> AppResult<()> {
        let context = self.repository.load_copy_export_context(job_id)?;
        let item = context
            .items
            .iter()
            .find(|item| item.ordinal == ordinal)
            .ok_or_else(|| AppError::NotFound(format!("copy export item {job_id}:{ordinal}")))?;
        let latest = self.current_protected_roots()?;
        let protected = merge_protected_roots(&context.protected_roots, &latest)?;
        let verified_output = match decision {
            CopyItemReconcileDecision::KeepQuarantined => None,
            CopyItemReconcileDecision::RetryWhenTargetAbsent => {
                self.verify_source_metadata(item)?;
                if self
                    .inspect_reconciled_target(&context, item, &protected)?
                    .is_some()
                {
                    return Err(unsafe_export(
                        "target still exists; retry is allowed only after its absence is verified",
                    ));
                }
                None
            }
            CopyItemReconcileDecision::AdoptVerifiedComplete => {
                self.verify_source_metadata(item)?;
                let snapshot = self
                    .inspect_reconciled_target(&context, item, &protected)?
                    .ok_or_else(|| unsafe_export("target is absent and cannot be adopted"))?;
                if snapshot.size_bytes != item.source_snapshot.size_bytes
                    || snapshot.blake3 != item.source_snapshot.blake3
                {
                    return Err(unsafe_export(
                        "target bytes do not match the frozen source fingerprint",
                    ));
                }
                let identity = snapshot
                    .identity
                    .clone()
                    .ok_or_else(|| unsafe_export("target has no durable physical identity"))?;
                Some(CommittedTargetIdentity {
                    identity,
                    size_bytes: snapshot.size_bytes,
                    blake3: snapshot.blake3,
                    final_path: target_path(&context, item)?,
                })
            }
        };
        self.repository.reconcile_copy_export_item(
            job_id,
            ordinal,
            decision.db_value(),
            verified_output.as_ref(),
        )
    }

    fn load_plan_header(&self, plan_id: &str) -> AppResult<OrganizationPlanV2> {
        self.repository.read_organization_plan_v2(plan_id, 1, 1)
    }

    fn verify_plan_binding(
        &self,
        binding: &CopyExportPlanBinding,
        plan: &OrganizationPlanV2,
        latest_roots: &[ProtectedRoot],
        expect_root_absent: bool,
    ) -> AppResult<()> {
        validate_child_name(&binding.target_child_name)?;
        let expected_root = binding.target_parent_path.join(&binding.target_child_name);
        if identity_key(&expected_root) != identity_key(Path::new(&plan.summary.target_root)) {
            return Err(unsafe_export(
                "plan target root differs from its frozen parent",
            ));
        }
        let protected = merge_protected_roots(&binding.protected_roots, latest_roots)?;
        reject_output_overlap(&expected_root, &protected)?;
        let parent = GrantedParent::grant_existing(&binding.target_parent_path, &protected)?;
        if parent.identity()? != &binding.target_parent_identity
            || identity_key(parent.path()) != identity_key(&binding.target_parent_path)
        {
            return Err(unsafe_export("frozen target parent identity changed"));
        }
        if expect_root_absent {
            match std::fs::symlink_metadata(&expected_root) {
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => return Err(error.into()),
                Ok(_) => {
                    return Err(unsafe_export(
                        "planned output root already exists and cannot be claimed",
                    ));
                }
            }
        }
        Ok(())
    }

    fn verify_job_binding(
        &self,
        context: &CopyExportJobContext,
        binding: &CopyExportPlanBinding,
        plan: &OrganizationPlanV2,
        latest_roots: &[ProtectedRoot],
    ) -> AppResult<()> {
        if identity_key(&context.target_parent_path) != identity_key(&binding.target_parent_path)
            || context.target_parent_identity != binding.target_parent_identity
            || context.target_child_name != binding.target_child_name
            || identity_key(&context.target_root_path)
                != identity_key(Path::new(&plan.summary.target_root))
        {
            return Err(unsafe_export("job target differs from its frozen plan"));
        }
        self.verify_plan_binding(binding, plan, latest_roots, false)?;
        let protected = merge_protected_roots(&context.protected_roots, latest_roots)?;
        let parent = GrantedParent::grant_existing(&context.target_parent_path, &protected)?;
        if parent.identity()? != &context.target_parent_identity {
            return Err(unsafe_export("job target parent identity changed"));
        }
        match context.target_root_identity.as_ref() {
            Some(root_identity) => {
                FileOperationService.open_existing_output_root(
                    &parent,
                    std::ffi::OsStr::new(&context.target_child_name),
                    root_identity,
                    &protected,
                )?;
            }
            None => match std::fs::symlink_metadata(parent.path().join(&context.target_child_name))
            {
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => return Err(error.into()),
                Ok(_) => {
                    return Err(unsafe_export(
                        "output root exists without a recorded identity; it remains quarantined",
                    ));
                }
            },
        }
        Ok(())
    }

    /// Validate every output that the rollback runner would delete before
    /// minting an executable rollback token. The runner repeats these checks
    /// against its deletion handle after authorization, so changes between
    /// preview and execution remain blocked there as well.
    fn verify_rollback_targets(
        &self,
        context: &CopyExportJobContext,
        latest_roots: &[ProtectedRoot],
    ) -> AppResult<()> {
        let copied_items = context
            .items
            .iter()
            .filter(|item| item.state == "copied")
            .collect::<Vec<_>>();
        if copied_items.is_empty() {
            return Err(unsafe_export("job has no committed outputs to roll back"));
        }

        let protected = merge_protected_roots(&context.protected_roots, latest_roots)?;
        let parent = GrantedParent::grant_existing(&context.target_parent_path, &protected)?;
        if parent.identity()? != &context.target_parent_identity {
            return Err(unsafe_export("rollback target parent identity changed"));
        }
        let root_identity = context
            .target_root_identity
            .as_ref()
            .ok_or_else(|| unsafe_export("rollback output root has no recorded identity"))?;
        let root = FileOperationService.open_existing_output_root(
            &parent,
            std::ffi::OsStr::new(&context.target_child_name),
            root_identity,
            &protected,
        )?;

        for item in copied_items {
            let expected = item
                .output_identity
                .as_ref()
                .ok_or_else(|| unsafe_export("copied item has no committed output identity"))?;
            let actual = FileOperationService
                .inspect_target_with_protected(&root, &item.target_relative_path, &protected)
                .map_err(|error| {
                    unsafe_export(format!(
                        "rollback target {} could not be verified: {error}",
                        item.ordinal
                    ))
                })?;
            if actual.identity.as_ref() != Some(&expected.identity)
                || actual.size_bytes != expected.size_bytes
                || actual.blake3 != expected.blake3
            {
                return Err(unsafe_export(format!(
                    "rollback target {} identity, size, or BLAKE3 changed",
                    item.ordinal
                )));
            }
        }
        Ok(())
    }

    fn root_identity_for_operation(
        &self,
        resource_id: &str,
        operation: CopyExportOperationKind,
    ) -> AppResult<Option<DurableFileIdentity>> {
        match operation {
            CopyExportOperationKind::Copy => Ok(None),
            CopyExportOperationKind::Resume | CopyExportOperationKind::Rollback => self
                .repository
                .load_copy_export_context(resource_id)
                .map(|context| context.target_root_identity),
        }
    }

    fn consume_authorization(
        &self,
        token: &str,
        resource_id: &str,
        operation: CopyExportOperationKind,
    ) -> AppResult<AuthorizationBinding> {
        let binding = self
            .lock_runtime()?
            .authorizations
            .consume(token)
            .ok_or_else(|| {
                AppError::InvalidArgument("authorization token is expired or already used".into())
            })?;
        if binding.resource_id != resource_id || binding.operation != operation {
            return Err(AppError::InvalidArgument(
                "authorization token does not match this operation".into(),
            ));
        }
        Ok(binding)
    }

    fn validate_consumed_authorization(
        &self,
        authorization: &AuthorizationBinding,
    ) -> AppResult<()> {
        let current_catalog = self.repository.catalog_runtime_identity()?;
        if !same_catalog(&authorization.catalog, &current_catalog) {
            return Err(unsafe_export(
                "catalog identity or application instance changed after preview",
            ));
        }
        let latest = self.current_protected_roots()?;
        match authorization.operation {
            CopyExportOperationKind::Copy => {
                let plan = self.load_plan_header(&authorization.plan_id)?;
                if plan.summary.revision != authorization.plan_revision
                    || plan.summary.snapshot_blake3 != authorization.snapshot_blake3
                {
                    return Err(unsafe_export("plan revision changed after preview"));
                }
                validate_copy_plan(&plan)?;
                self.repository
                    .validate_copy_export_plan_fresh(&authorization.plan_id)?;
                let binding = self
                    .repository
                    .copy_export_plan_binding(&authorization.plan_id)?
                    .ok_or_else(|| AppError::NotFound("copy plan binding".into()))?;
                if !same_binding(authorization, &binding, None) {
                    return Err(unsafe_export("copy target binding changed after preview"));
                }
                self.verify_plan_binding(&binding, &plan, &latest, true)?;
            }
            CopyExportOperationKind::Resume | CopyExportOperationKind::Rollback => {
                let context = self
                    .repository
                    .load_copy_export_context(&authorization.resource_id)?;
                let plan = self.load_plan_header(&authorization.plan_id)?;
                if plan.summary.revision != authorization.plan_revision
                    || plan.summary.snapshot_blake3 != authorization.snapshot_blake3
                {
                    return Err(unsafe_export("plan revision changed after preview"));
                }
                if authorization.operation == CopyExportOperationKind::Resume {
                    self.repository
                        .validate_copy_export_plan_fresh(&authorization.plan_id)?;
                }
                let binding = self
                    .repository
                    .copy_export_plan_binding(&authorization.plan_id)?
                    .ok_or_else(|| AppError::NotFound("copy plan binding".into()))?;
                if !same_binding(
                    authorization,
                    &binding,
                    context.target_root_identity.as_ref(),
                ) {
                    return Err(unsafe_export("job target binding changed after preview"));
                }
                self.verify_job_binding(&context, &binding, &plan, &latest)?;
            }
        }
        Ok(())
    }

    fn ensure_runner_idle(&self) -> AppResult<()> {
        let mut runtime = self.lock_runtime()?;
        runtime
            .runners
            .retain(|_, worker| !worker.finished.load(Ordering::Acquire));
        if runtime.runners.is_empty() {
            Ok(())
        } else {
            Err(AppError::InvalidArgument(
                "another copy export runner is already active".into(),
            ))
        }
    }

    fn dispatch(&self, job_id: String, operation: CopyExportOperationKind) -> AppResult<()> {
        let runtime = Arc::new(RunnerRuntime {
            operation,
            cancel: Arc::new(AtomicBool::new(false)),
            progress: Mutex::new(None),
            error: Mutex::new(None),
            finished: AtomicBool::new(false),
        });
        if let Ok(mut state) = self.runtime.lock() {
            state
                .runners
                .retain(|_, worker| !worker.finished.load(Ordering::Acquire));
            if !state.runners.is_empty() {
                return Err(AppError::InvalidArgument(
                    "another copy export runner is already active".into(),
                ));
            }
            state.runners.insert(job_id.clone(), Arc::clone(&runtime));
        } else {
            return Err(internal_error("copy export service lock was poisoned"));
        }
        let repository = self.repository.clone();
        let worker_id = job_id.clone();
        thread::spawn(move || {
            let progress_runtime = Arc::clone(&runtime);
            let result = match operation {
                CopyExportOperationKind::Copy | CopyExportOperationKind::Resume => {
                    run_copy_export_job_after_recovery_gate(
                        &repository,
                        &worker_id,
                        &runtime.cancel,
                        move |progress| {
                            if let Ok(mut slot) = progress_runtime.progress.lock() {
                                *slot = Some(progress);
                            }
                        },
                    )
                }
                CopyExportOperationKind::Rollback => {
                    run_copy_export_rollback_job_after_recovery_gate(
                        &repository,
                        &worker_id,
                        &runtime.cancel,
                        move |progress| {
                            if let Ok(mut slot) = progress_runtime.progress.lock() {
                                *slot = Some(progress);
                            }
                        },
                    )
                }
            };
            if let Err(error) = result {
                let interruption =
                    repository.mark_copy_export_job_interrupted(&worker_id, &error.to_string());
                if let Ok(mut slot) = runtime.error.lock() {
                    *slot = Some(match interruption {
                        Ok(()) => error.to_string(),
                        Err(mark_error) => {
                            format!("{error}; failed to persist interrupted state: {mark_error}")
                        }
                    });
                }
            }
            runtime.finished.store(true, Ordering::Release);
        });
        Ok(())
    }

    fn get_grant(&self, grant_id: &str) -> AppResult<TargetGrant> {
        let mut runtime = self.lock_runtime()?;
        runtime
            .grants
            .retain(|_, grant| grant.expires_at > Instant::now());
        runtime
            .grants
            .get(grant_id)
            .map(|grant| TargetGrant {
                path: grant.path.clone(),
                identity: grant.identity.clone(),
                expires_at: grant.expires_at,
            })
            .ok_or_else(|| AppError::NotFound("target parent grant expired or missing".into()))
    }

    fn current_protected_roots(&self) -> AppResult<Vec<ProtectedRoot>> {
        self.repository.current_protected_roots(&self.app_data_path)
    }

    fn verify_source_metadata(&self, item: &CopyExportJobItem) -> AppResult<()> {
        let metadata = FileOperationService.inspect_source_metadata(&item.source_path)?;
        if metadata.identity != item.source_snapshot.identity
            || metadata.size_bytes != item.source_snapshot.size_bytes
            || metadata.modified_time != item.source_snapshot.modified_time
        {
            return Err(unsafe_export(
                "source metadata changed since the frozen plan",
            ));
        }
        Ok(())
    }

    fn inspect_reconciled_target(
        &self,
        context: &CopyExportJobContext,
        item: &CopyExportJobItem,
        protected: &[ProtectedRoot],
    ) -> AppResult<Option<FileSnapshot>> {
        let parent = GrantedParent::grant_existing(&context.target_parent_path, protected)?;
        if parent.identity()? != &context.target_parent_identity {
            return Err(unsafe_export("target parent identity changed"));
        }
        let Some(root_identity) = &context.target_root_identity else {
            match std::fs::symlink_metadata(parent.path().join(&context.target_child_name)) {
                Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
                Err(error) => return Err(error.into()),
                Ok(_) => {
                    return Err(unsafe_export(
                        "output root exists without a recorded identity; it remains quarantined",
                    ));
                }
            }
        };
        let root = FileOperationService.open_existing_output_root(
            &parent,
            std::ffi::OsStr::new(&context.target_child_name),
            root_identity,
            protected,
        )?;
        match FileOperationService.inspect_target_with_protected(
            &root,
            &item.target_relative_path,
            protected,
        ) {
            Ok(snapshot) => Ok(Some(snapshot)),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error.into()),
        }
    }

    fn lock_runtime(&self) -> AppResult<MutexGuard<'_, ServiceRuntime>> {
        self.runtime
            .lock()
            .map_err(|_| internal_error("copy export service lock was poisoned"))
    }
}

fn copy_export_job_page_to_view(page: CopyExportJobPage) -> CopyExportJobView {
    CopyExportJobView {
        job_id: page.job_id,
        plan_id: page.plan_id,
        target_root: page.target_root,
        state: page.state,
        progress_completed_items: page.progress_completed_items,
        progress_total_items: page.progress_total_items,
        progress_completed_bytes: page.progress_completed_bytes,
        progress_total_bytes: page.progress_total_bytes,
        current_item_ordinal: page.current_item_ordinal,
        cancel_requested: page.cancel_requested,
        created_at: page.created_at,
        updated_at: page.updated_at,
        item_total: page.item_total,
        item_page: page.item_page,
        item_page_size: page.item_page_size,
        items: page
            .items
            .into_iter()
            .map(copy_export_job_item_to_view)
            .collect(),
    }
}

fn copy_export_job_item_to_view(item: CopyExportJobPageItem) -> CopyExportJobItemView {
    CopyExportJobItemView {
        ordinal: item.ordinal,
        asset_id: item.asset_id,
        state: item.state,
        attempts: item.attempts,
        target_relative_path: item.target_relative_path,
        source_snapshot: item.source_snapshot,
        partial_target_identity: item.partial_target_identity,
        partial_snapshot: item.partial_snapshot,
        output_identity: item.output_identity,
    }
}

fn recent_copy_export_job_to_view(record: RecentCopyExportJobRecord) -> RecentCopyExportJob {
    RecentCopyExportJob {
        job_id: record.job_id,
        plan_id: record.plan_id,
        target_root: record.target_root,
        state: record.state,
        progress_completed_items: record.progress_completed_items,
        progress_total_items: record.progress_total_items,
        created_at: record.created_at,
        updated_at: record.updated_at,
    }
}

fn validate_copy_plan(plan: &OrganizationPlanV2) -> AppResult<()> {
    if plan.summary.status != crate::models::CopyPlanStatus::Ready
        || plan.summary.eligible_count == 0
        || plan.summary.blocked_count > 0
    {
        return Err(AppError::InvalidArgument(
            "copy plan is not ready for execution".into(),
        ));
    }
    Ok(())
}

fn validate_job_operation(
    job_state: &str,
    items: &[CopyExportJobItem],
    operation: CopyExportOperationKind,
) -> AppResult<()> {
    let allowed = match operation {
        CopyExportOperationKind::Copy => false,
        CopyExportOperationKind::Resume => matches!(
            job_state,
            "queued" | "interrupted" | "cancelled" | "partial" | "failed" | "reconcile_required"
        ),
        CopyExportOperationKind::Rollback => {
            matches!(
                job_state,
                "completed"
                    | "partial"
                    | "cancelled"
                    | "failed"
                    | "interrupted"
                    | "reconcile_required"
            ) && items.iter().any(|item| item.state == "copied")
        }
    };
    if !allowed {
        return Err(AppError::InvalidArgument(format!(
            "job state {} does not allow {:?}",
            job_state, operation
        )));
    }
    if operation == CopyExportOperationKind::Resume
        && items.iter().any(|item| {
            matches!(
                item.state.as_str(),
                "copying"
                    | "partial"
                    | "reconcile_required"
                    | "rollback_pending"
                    | "rollback_blocked"
            ) || item.partial_target_identity.is_some()
                || item.partial_snapshot.is_some()
        })
    {
        return Err(AppError::InvalidArgument(
            "job has unresolved output identities and requires reconciliation".into(),
        ));
    }
    Ok(())
}

fn same_catalog(left: &CatalogRuntimeIdentity, right: &CatalogRuntimeIdentity) -> bool {
    left.catalog_id == right.catalog_id
        && left.instance_epoch == right.instance_epoch
        && left.database_file_identity == right.database_file_identity
        && identity_key(&left.database_path) == identity_key(&right.database_path)
}

fn same_binding(
    authorization: &AuthorizationBinding,
    binding: &CopyExportPlanBinding,
    root_identity: Option<&DurableFileIdentity>,
) -> bool {
    identity_key(&authorization.target_parent_path) == identity_key(&binding.target_parent_path)
        && authorization.target_parent_identity == binding.target_parent_identity
        && authorization.target_child_name == binding.target_child_name
        && identity_key(&authorization.target_root_path)
            == identity_key(&binding.target_parent_path.join(&binding.target_child_name))
        && authorization.target_root_identity.as_ref() == root_identity
}

fn merge_protected_roots(
    frozen: &[ProtectedRoot],
    latest: &[ProtectedRoot],
) -> AppResult<Vec<ProtectedRoot>> {
    let mut roots: HashMap<String, ProtectedRoot> = HashMap::new();
    for root in frozen.iter().chain(latest) {
        let key = identity_key(&root.path);
        if let Some(existing) = roots.get(&key) {
            if existing.identity != root.identity {
                return Err(unsafe_export("protected root identity changed"));
            }
        } else {
            roots.insert(key, root.clone());
        }
    }
    let mut result = roots.into_iter().collect::<Vec<_>>();
    result.sort_by(|left, right| left.0.cmp(&right.0));
    Ok(result.into_iter().map(|(_, root)| root).collect())
}

fn reject_output_overlap(path: &Path, protected: &[ProtectedRoot]) -> AppResult<()> {
    let candidate = identity_key(path);
    if protected
        .iter()
        .any(|root| paths_overlap(&candidate, &identity_key(&root.path)))
    {
        return Err(AppError::UnsafePath(path.to_path_buf()));
    }
    Ok(())
}

fn validate_child_name(name: &str) -> AppResult<()> {
    let path = Path::new(name);
    let mut components = path.components();
    if name.is_empty()
        || !matches!(components.next(), Some(Component::Normal(_)))
        || components.next().is_some()
        || name
            .chars()
            .any(|character| matches!(character, '/' | '\\' | ':'))
    {
        return Err(AppError::InvalidArgument(
            "target folder name must be one plain path component".into(),
        ));
    }
    Ok(())
}

fn target_path(context: &CopyExportJobContext, item: &CopyExportJobItem) -> AppResult<PathBuf> {
    let path = context.target_root_path.join(&item.target_relative_path);
    if !path.starts_with(&context.target_root_path) {
        return Err(unsafe_export("target path escapes the output root"));
    }
    Ok(path)
}

fn unix_ms_after(duration: Duration) -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .saturating_add(duration)
        .as_millis()
        .min(u64::MAX as u128) as u64
}

fn unsafe_export(message: impl Into<String>) -> AppError {
    AppError::InvalidArgument(message.into())
}

fn internal_error(message: impl Into<String>) -> AppError {
    AppError::InvalidArgument(message.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn catalog(identity: DurableFileIdentity, epoch: i64) -> CatalogRuntimeIdentity {
        CatalogRuntimeIdentity {
            catalog_id: "test-catalog".into(),
            instance_epoch: epoch,
            database_path: PathBuf::from("test-data/catalog.sqlite3"),
            database_file_identity: identity,
        }
    }

    fn binding(catalog: CatalogRuntimeIdentity) -> AuthorizationBinding {
        AuthorizationBinding {
            resource_id: "plan-1".into(),
            operation: CopyExportOperationKind::Copy,
            expires_at: Instant::now() + AUTHORIZATION_TTL,
            catalog,
            plan_id: "plan-1".into(),
            plan_revision: "rev-1".into(),
            snapshot_blake3: "a".repeat(64),
            target_parent_path: PathBuf::from("test-data/parent"),
            target_parent_identity: DurableFileIdentity {
                volume_serial: 1,
                file_index_high: 0,
                file_index_low: 2,
                creation_filetime: 3,
            },
            target_child_name: "output".into(),
            target_root_path: PathBuf::from("test-data/parent/output"),
            target_root_identity: None,
            protected_roots: vec![],
        }
    }

    fn file_id(low: u32) -> DurableFileIdentity {
        DurableFileIdentity {
            volume_serial: 1,
            file_index_high: 0,
            file_index_low: low,
            creation_filetime: 3,
        }
    }

    fn service_without_catalog() -> CopyExportService {
        CopyExportService {
            repository: Repository::new(PathBuf::from("test-data/not-opened.sqlite3")),
            app_data_path: PathBuf::from("test-data"),
            runtime: Arc::new(Mutex::new(ServiceRuntime::default())),
            preview_gate: Arc::new(Mutex::new(())),
            operation_gate: Arc::new(Mutex::new(())),
        }
    }

    #[test]
    fn a_new_preview_revokes_older_token_for_the_same_operation() {
        let mut store = AuthorizationStore::default();
        let key = AuthorizationKey {
            resource_id: "plan-1".into(),
            operation: CopyExportOperationKind::Copy,
        };
        let old = store
            .issue(key.clone(), binding(catalog(file_id(10), 1)))
            .unwrap();
        let new = store.issue(key, binding(catalog(file_id(10), 1))).unwrap();
        assert!(store.consume(&old).is_none());
        assert!(store.consume(&new).is_some());
        assert!(store.consume(&new).is_none());
    }

    #[test]
    fn mismatched_resource_or_operation_attempt_consumes_authorization() {
        for (resource_id, operation) in [
            ("another-plan", CopyExportOperationKind::Copy),
            ("plan-1", CopyExportOperationKind::Resume),
        ] {
            let service = service_without_catalog();
            let key = AuthorizationKey {
                resource_id: "plan-1".into(),
                operation: CopyExportOperationKind::Copy,
            };
            let token = service
                .runtime
                .lock()
                .expect("runtime")
                .authorizations
                .issue(key, binding(catalog(file_id(10), 1)))
                .expect("issue one-use token");

            assert!(
                service
                    .consume_authorization(&token, resource_id, operation)
                    .is_err()
            );
            assert!(
                service
                    .runtime
                    .lock()
                    .expect("runtime")
                    .authorizations
                    .consume(&token)
                    .is_none()
            );
        }
    }

    #[test]
    fn authorization_capacity_is_bounded_and_expired_entries_are_pruned() {
        let mut store = AuthorizationStore::default();
        for index in 0..MAX_AUTHORIZATIONS {
            let resource_id = format!("plan-{index}");
            store
                .issue(
                    AuthorizationKey {
                        resource_id: resource_id.clone(),
                        operation: CopyExportOperationKind::Copy,
                    },
                    binding(catalog(file_id(10), 1)),
                )
                .expect("fill bounded authorization store");
        }
        assert_eq!(store.by_token.len(), MAX_AUTHORIZATIONS);
        assert!(
            store
                .issue(
                    AuthorizationKey {
                        resource_id: "plan-over-capacity".into(),
                        operation: CopyExportOperationKind::Copy,
                    },
                    binding(catalog(file_id(10), 1)),
                )
                .is_err()
        );

        for binding in store.by_token.values_mut() {
            binding.expires_at = Instant::now() - Duration::from_secs(1);
        }
        store
            .issue(
                AuthorizationKey {
                    resource_id: "plan-after-expiry".into(),
                    operation: CopyExportOperationKind::Copy,
                },
                binding(catalog(file_id(10), 1)),
            )
            .expect("expired entries should free capacity");
        assert_eq!(store.by_token.len(), 1);
        assert_eq!(store.latest_by_key.len(), 1);
    }

    #[test]
    fn second_runner_is_rejected_after_its_token_is_consumed() {
        let service = service_without_catalog();
        let key = AuthorizationKey {
            resource_id: "plan-1".into(),
            operation: CopyExportOperationKind::Copy,
        };
        let token = service
            .runtime
            .lock()
            .expect("runtime")
            .authorizations
            .issue(key, binding(catalog(file_id(10), 1)))
            .expect("issue token");
        service.runtime.lock().expect("runtime").runners.insert(
            "active-job".into(),
            Arc::new(RunnerRuntime {
                operation: CopyExportOperationKind::Copy,
                cancel: Arc::new(AtomicBool::new(false)),
                progress: Mutex::new(None),
                error: Mutex::new(None),
                finished: AtomicBool::new(false),
            }),
        );

        assert!(service.start("plan-1", &token).is_err());
        assert!(
            service
                .runtime
                .lock()
                .expect("runtime")
                .authorizations
                .consume(&token)
                .is_none()
        );
    }

    #[test]
    fn authorization_binds_database_file_identity_and_instance_epoch() {
        let authorized = catalog(file_id(10), 7);
        assert!(same_catalog(&authorized, &catalog(file_id(10), 7)));
        assert!(!same_catalog(&authorized, &catalog(file_id(11), 7)));
        assert!(!same_catalog(&authorized, &catalog(file_id(10), 8)));
        let mut other_catalog = catalog(file_id(10), 7);
        other_catalog.catalog_id = "other-catalog".into();
        assert!(!same_catalog(&authorized, &other_catalog));
    }

    #[cfg(windows)]
    #[test]
    fn replacing_the_catalog_file_at_the_same_path_invalidates_authorization() {
        use std::fs;

        let fixture_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../test-data");
        fs::create_dir_all(&fixture_root).expect("test-data directory");
        let temporary = tempfile::Builder::new()
            .prefix("copy-export-service-")
            .tempdir_in(&fixture_root)
            .expect("fixture-local temporary directory");
        let database_path = temporary.path().join("catalog.sqlite3");
        let preserved_path = temporary.path().join("catalog.previous.sqlite3");
        fs::write(&database_path, b"first catalog bytes").expect("write first file");
        let first_identity = FileOperationService
            .inspect_source_metadata(&database_path)
            .expect("inspect initial file")
            .identity
            .expect("Windows durable identity");
        fs::rename(&database_path, &preserved_path).expect("retain original file ID");
        fs::write(&database_path, b"replacement catalog bytes")
            .expect("write replacement at same path");
        let replacement_identity = FileOperationService
            .inspect_source_metadata(&database_path)
            .expect("inspect replacement file")
            .identity
            .expect("Windows durable identity");

        let authorized = CatalogRuntimeIdentity {
            catalog_id: "same-stable-catalog".into(),
            instance_epoch: 3,
            database_path: database_path.clone(),
            database_file_identity: first_identity,
        };
        let current = CatalogRuntimeIdentity {
            catalog_id: "same-stable-catalog".into(),
            instance_epoch: 3,
            database_path,
            database_file_identity: replacement_identity,
        };
        assert!(!same_catalog(&authorized, &current));
    }

    #[test]
    fn protected_root_replacement_at_the_same_path_fails_closed() {
        let path = PathBuf::from("test-data/Source");
        let frozen = ProtectedRoot {
            path: path.clone(),
            identity: Some(file_id(12)),
        };
        let replacement = ProtectedRoot {
            path,
            identity: Some(file_id(13)),
        };
        assert!(merge_protected_roots(&[frozen], &[replacement]).is_err());
    }

    #[test]
    fn output_name_rejects_traversal_and_absolute_components() {
        assert!(validate_child_name("Vacation 旅行").is_ok());
        assert!(validate_child_name("../escape").is_err());
        assert!(validate_child_name("C:\\output").is_err());
        assert!(validate_child_name("a/b").is_err());
    }

    #[test]
    fn paged_job_dto_serializes_nested_identity_contract_in_camel_case() {
        let identity = DurableFileIdentity {
            creation_filetime: u64::MAX,
            ..file_id(42)
        };
        let snapshot = FileSnapshot {
            identity: Some(identity.clone()),
            size_bytes: 9,
            modified_time: 17,
            blake3: "ab".repeat(32),
        };
        let page = CopyExportJobPage {
            job_id: "job-1".into(),
            plan_id: "plan-1".into(),
            target_root: "E:/target-parent/出口 旅行".into(),
            state: "completed".into(),
            progress_completed_items: 1,
            progress_total_items: 1,
            progress_completed_bytes: 9,
            progress_total_bytes: 9,
            current_item_ordinal: None,
            cancel_requested: false,
            created_at: "created".into(),
            updated_at: "updated".into(),
            item_total: 1,
            item_page: 1,
            item_page_size: 100,
            items: vec![CopyExportJobPageItem {
                ordinal: 0,
                asset_id: 7,
                state: "copied".into(),
                attempts: 1,
                target_relative_path: PathBuf::from("旅行/Фото 01.jpg"),
                source_snapshot: snapshot.clone(),
                partial_target_identity: Some(identity.clone()),
                partial_snapshot: None,
                output_identity: Some(CommittedTargetIdentity {
                    identity,
                    size_bytes: 9,
                    blake3: snapshot.blake3.clone(),
                    final_path: PathBuf::from("E:/output/旅行/Фото 01.jpg"),
                }),
            }],
        };

        let view = copy_export_job_page_to_view(page);
        let json = serde_json::to_value(view).expect("serialize public job DTO");
        let item = &json["items"][0];
        let source_snapshot = &item["sourceSnapshot"];
        let durable_id = &source_snapshot["identity"];
        assert_eq!(json["itemPageSize"], 100);
        assert_eq!(json["targetRoot"], "E:/target-parent/出口 旅行");
        assert_eq!(item["targetRelativePath"], "旅行/Фото 01.jpg");
        assert_eq!(item["partialTargetIdentity"]["fileIndexLow"], 42);
        assert_eq!(source_snapshot["sizeBytes"], 9);
        assert_eq!(source_snapshot["modifiedTime"], "17");
        assert_eq!(durable_id["volumeSerial"], 1);
        assert_eq!(durable_id["fileIndexHigh"], 0);
        assert_eq!(durable_id["fileIndexLow"], 42);
        assert_eq!(durable_id["creationFiletime"], u64::MAX.to_string());
        assert_eq!(item["outputIdentity"]["sizeBytes"], 9);
        assert_eq!(
            item["outputIdentity"]["finalPath"],
            "E:/output/旅行/Фото 01.jpg"
        );
    }

    #[test]
    fn old_snake_case_snapshot_json_remains_readable() {
        let value = serde_json::json!({
            "identity": {
                "volume_serial": 1,
                "file_index_high": 2,
                "file_index_low": 3,
                "creation_filetime": u64::MAX,
            },
            "size_bytes": 4,
            "modified_time": 5,
            "blake3": "cd".repeat(32),
        });
        let snapshot: FileSnapshot = serde_json::from_value(value).expect("legacy JSON parses");
        assert_eq!(
            snapshot.identity.expect("file identity").creation_filetime,
            u64::MAX
        );
        assert_eq!(snapshot.size_bytes, 4);
        assert_eq!(snapshot.modified_time, 5);
    }

    #[cfg(windows)]
    #[test]
    fn scanned_fixture_can_be_planned_and_copied_through_the_service() {
        use std::fs;
        use std::sync::atomic::AtomicBool;

        use crate::models::{AssetFilter, AssetSortField, SortDirection};
        use crate::paths::AppPaths;

        let repository_root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("repository root");
        let test_data = repository_root.join("test-data");
        fs::create_dir_all(&test_data).expect("test-data directory");
        let temporary = tempfile::Builder::new()
            .prefix("copy-export-service-e2e-")
            .tempdir_in(&test_data)
            .expect("fixture-local temporary directory");
        let source_root = temporary.path().join("来源 图库");
        let source_file = source_root.join("旅行").join("Фото 01.jpg");
        let stale_source_file = source_root.join("旅行").join("Фото 02.jpg");
        let app_data = temporary.path().join("app-data");
        let target_parent = temporary.path().join("目标 parent");
        fs::create_dir_all(source_file.parent().expect("source parent")).expect("source folder");
        fs::create_dir_all(stale_source_file.parent().expect("stale source parent"))
            .expect("stale source folder");
        fs::create_dir_all(&target_parent).expect("target parent");
        let fixture = test_data
            .join("manual-verification-20260808")
            .join("Parent 库")
            .join("parent-landscape.jpg");
        fs::copy(&fixture, &source_file).expect("copy approved test fixture");
        fs::copy(&fixture, &stale_source_file).expect("copy second approved test fixture");

        let paths = AppPaths::initialize(&app_data).expect("app paths");
        let repository = Repository::new(&paths.database_path);
        repository.initialize().expect("initialize test catalog");
        let scan = crate::scanner::scan_library(
            &repository,
            &paths.thumbnail_dir,
            &source_root,
            "copy-export-service-e2e",
            &AtomicBool::new(false),
            |_| {},
        )
        .expect("scan fixture library");
        assert_eq!(scan.status, "completed");
        assert_eq!(scan.succeeded, 2);

        let source_metadata = FileOperationService
            .inspect_source_metadata(&source_file)
            .expect("read actual source metadata from its handle");
        let assets = repository
            .list_assets(
                scan.library_id,
                AssetSortField::FileName,
                SortDirection::Asc,
                1,
                10,
                &AssetFilter::default(),
            )
            .expect("list scanned asset");
        let asset_id = assets
            .items
            .iter()
            .find(|asset| asset.file_name == "Фото 01.jpg")
            .expect("primary scanned asset")
            .id;
        let stale_asset_id = assets
            .items
            .iter()
            .find(|asset| asset.file_name == "Фото 02.jpg")
            .expect("stale-check scanned asset")
            .id;

        let service = CopyExportService::new(repository.clone()).expect("copy service");
        let grant = service.grant(&target_parent).expect("grant target parent");
        let plan = service
            .create_plan_v2(
                OrganizationPlanScopeV2::ExplicitAssets {
                    asset_ids: vec![asset_id],
                },
                OrganizationTargetV2 {
                    parent_grant_id: grant.grant_id.clone(),
                    child_name: "出口 旅行".into(),
                },
                None,
            )
            .expect("build and persist frozen plan");
        assert_eq!(plan.summary.eligible_count, 1);
        assert_eq!(
            plan.items[0].source_modified_filetime,
            source_metadata.modified_time
        );

        let stale_plan = service
            .create_plan_v2(
                OrganizationPlanScopeV2::ExplicitAssets {
                    asset_ids: vec![stale_asset_id],
                },
                OrganizationTargetV2 {
                    parent_grant_id: grant.grant_id.clone(),
                    child_name: "stale target".into(),
                },
                None,
            )
            .expect("freeze second plan before source mutation");
        let tamper_plan = service
            .create_plan_v2(
                OrganizationPlanScopeV2::ExplicitAssets {
                    asset_ids: vec![asset_id],
                },
                OrganizationTargetV2 {
                    parent_grant_id: grant.grant_id.clone(),
                    child_name: "预览损坏".into(),
                },
                None,
            )
            .expect("freeze a second target for tampered-preview coverage");
        fs::write(&stale_source_file, b"changed after the plan snapshot")
            .expect("modify only test-owned source copy");
        assert!(
            service
                .preflight(&stale_plan.summary.plan_id, CopyExportOperationKind::Copy)
                .is_err()
        );

        let approval = service
            .preflight(&plan.summary.plan_id, CopyExportOperationKind::Copy)
            .expect("authorize frozen plan");
        let started = service
            .start(&plan.summary.plan_id, &approval.authorization_token)
            .expect("start authorized copy");

        let deadline = Instant::now() + Duration::from_secs(20);
        let completed = loop {
            let page = service
                .get_job(&started.job_id, 1, 10)
                .expect("read job page")
                .expect("job remains in catalog");
            if matches!(
                page.state.as_str(),
                "completed" | "partial" | "failed" | "interrupted"
            ) {
                break page;
            }
            assert!(Instant::now() < deadline, "copy job did not finish in time");
            thread::sleep(Duration::from_millis(50));
        };
        assert_eq!(
            completed.state,
            "completed",
            "copy runtime status: {:?}",
            service
                .runtime_status(&started.job_id)
                .expect("read copy runtime status")
        );
        let copied_item = completed.items.first().expect("copied item");
        assert_eq!(copied_item.state, "copied");
        assert_eq!(
            copied_item.source_snapshot.modified_time,
            source_metadata.modified_time
        );
        let output = target_parent
            .join("出口 旅行")
            .join(&copied_item.target_relative_path);
        let original_bytes = fs::read(&source_file).expect("read original fixture bytes");
        let copied_bytes = fs::read(&output).expect("read generated copy bytes");
        assert_eq!(copied_bytes, original_bytes);
        let expected_blake3 = blake3::hash(&original_bytes).to_hex().to_string();
        assert_eq!(copied_item.source_snapshot.blake3, expected_blake3);
        assert_eq!(
            copied_item
                .output_identity
                .as_ref()
                .expect("committed output identity")
                .blake3,
            expected_blake3
        );

        let tamper_approval = service
            .preflight(&tamper_plan.summary.plan_id, CopyExportOperationKind::Copy)
            .expect("authorize second fixture-local copy");
        let tamper_started = service
            .start(
                &tamper_plan.summary.plan_id,
                &tamper_approval.authorization_token,
            )
            .expect("start second authorized copy");
        let tamper_deadline = Instant::now() + Duration::from_secs(20);
        let tamper_completed = loop {
            let page = service
                .get_job(&tamper_started.job_id, 1, 10)
                .expect("read second job page")
                .expect("second job remains in catalog");
            if matches!(
                page.state.as_str(),
                "completed" | "partial" | "failed" | "interrupted"
            ) {
                break page;
            }
            assert!(
                Instant::now() < tamper_deadline,
                "second copy job did not finish in time"
            );
            thread::sleep(Duration::from_millis(50));
        };
        assert_eq!(
            tamper_completed.state,
            "completed",
            "second copy runtime status: {:?}",
            service
                .runtime_status(&tamper_started.job_id)
                .expect("read second copy runtime status")
        );
        let tamper_item = tamper_completed.items.first().expect("second copied item");
        let tamper_output = target_parent
            .join("预览损坏")
            .join(&tamper_item.target_relative_path);
        assert_eq!(
            fs::read(&tamper_output).expect("read second generated copy"),
            original_bytes
        );
        let tampered_preview_bytes = b"tampered before rollback preview";
        fs::write(&tamper_output, tampered_preview_bytes)
            .expect("tamper only the second generated fixture copy");
        assert!(service.preview_rollback(&tamper_started.job_id).is_err());

        fs::write(&source_file, b"source changed after completed copy")
            .expect("modify only test-owned source after verifying original copy");

        let rollback_approval = service
            .preview_rollback(&started.job_id)
            .expect("preview unchanged committed output before rollback");
        let tampered_bytes = b"changed after the copy committed";
        fs::write(&output, tampered_bytes).expect("tamper only the generated fixture copy");
        service
            .confirm_rollback(&started.job_id, &rollback_approval.authorization_token)
            .expect("start authorized rollback");
        let rollback_deadline = Instant::now() + Duration::from_secs(20);
        let rollback_result = loop {
            let page = service
                .get_job(&started.job_id, 1, 10)
                .expect("read rollback job page")
                .expect("rollback job remains in catalog");
            if matches!(
                page.state.as_str(),
                "rolled_back" | "partial" | "failed" | "interrupted"
            ) {
                break page;
            }
            assert!(
                Instant::now() < rollback_deadline,
                "rollback job did not finish in time"
            );
            thread::sleep(Duration::from_millis(50));
        };
        assert_eq!(rollback_result.state, "partial");
        assert_eq!(rollback_result.items[0].state, "rollback_blocked");
        assert_eq!(
            fs::read(&output).expect("tampered copy remains"),
            tampered_bytes
        );
    }
}
