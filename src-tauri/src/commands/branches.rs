use serde::Deserialize;
use tauri::{AppHandle, State};

use super::{Command, ExecCx, Risk};
use crate::domain::error::ErrorPayload;
use crate::domain::ids::RepoId;
use crate::domain::refs::{BranchFilter, BranchInfo, CheckoutOptions, CheckoutResult};
use crate::domain::revision::Revision;
use crate::domain::sender::SenderTag;
use crate::engine;
use crate::events::{self, AppEvent};
use crate::state::AppState;
use crate::validation::{self, ValidateCx, Violation};

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BranchList {
    pub repo: RepoId,
    #[serde(default)]
    pub filter: BranchFilter,
    #[serde(default)]
    pub search: Option<String>,
}

impl Command for BranchList {
    type Output = Vec<BranchInfo>;

    fn repo_id(&self) -> RepoId {
        self.repo.clone()
    }

    fn validate(&self, _cx: &ValidateCx<'_>) -> Result<(), Violation> {
        Ok(())
    }

    fn execute(self, cx: &ExecCx<'_>) -> Result<Self::Output, ErrorPayload> {
        let mut branches = engine::branches::list(cx.repo, self.filter)?;
        if let Some(search) = self
            .search
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            let needle = search.to_lowercase();
            branches.retain(|branch| branch.name.to_lowercase().contains(&needle));
        }
        Ok(branches)
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BranchCreate {
    pub repo: RepoId,
    pub name: String,
    #[serde(default)]
    pub start: Revision,
    #[serde(default)]
    pub checkout: bool,
    #[serde(default)]
    pub sender: SenderTag,
}

impl Command for BranchCreate {
    type Output = BranchInfo;

    fn repo_id(&self) -> RepoId {
        self.repo.clone()
    }

    fn sender(&self) -> SenderTag {
        self.sender
    }

    fn validate(&self, cx: &ValidateCx<'_>) -> Result<(), Violation> {
        validation::branches::validate_create(cx, &self.name, &self.start)
    }

    fn execute(self, cx: &ExecCx<'_>) -> Result<Self::Output, ErrorPayload> {
        let info = engine::branches::create(cx.repo, self.name.trim(), &self.start, self.checkout)?;
        events::emit(
            cx.app,
            AppEvent::RefsChanged {
                repo: cx.repo_id.clone(),
            },
        );
        if self.checkout {
            events::emit(
                cx.app,
                AppEvent::WorkingCopyChanged {
                    repo: cx.repo_id.clone(),
                },
            );
        }
        Ok(info)
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BranchCheckout {
    pub repo: RepoId,
    pub name: String,
    #[serde(default)]
    pub opts: CheckoutOptions,
}

impl Command for BranchCheckout {
    type Output = CheckoutResult;

    fn repo_id(&self) -> RepoId {
        self.repo.clone()
    }

    fn risk(&self) -> Risk {
        // The safe checkout strategy preserves local changes and reports
        // conflicts; nothing is discarded, so no snapshot is required.
        Risk::None
    }

    fn validate(&self, cx: &ValidateCx<'_>) -> Result<(), Violation> {
        validation::branches::validate_checkout(cx, &self.name)
    }

    fn execute(self, cx: &ExecCx<'_>) -> Result<Self::Output, ErrorPayload> {
        let result = engine::branches::checkout(cx.repo, &self.name, &self.opts)?;
        events::emit(
            cx.app,
            AppEvent::RefsChanged {
                repo: cx.repo_id.clone(),
            },
        );
        events::emit(
            cx.app,
            AppEvent::WorkingCopyChanged {
                repo: cx.repo_id.clone(),
            },
        );
        Ok(result)
    }
}

#[tauri::command]
pub async fn branch_list(
    state: State<'_, AppState>,
    app: AppHandle,
    args: BranchList,
) -> Result<Vec<BranchInfo>, ErrorPayload> {
    state.run(&app, args).await
}

#[tauri::command]
pub async fn branch_create(
    state: State<'_, AppState>,
    app: AppHandle,
    args: BranchCreate,
) -> Result<BranchInfo, ErrorPayload> {
    state.run(&app, args).await
}

#[tauri::command]
pub async fn branch_checkout(
    state: State<'_, AppState>,
    app: AppHandle,
    args: BranchCheckout,
) -> Result<CheckoutResult, ErrorPayload> {
    state.run(&app, args).await
}
