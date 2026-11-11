use crate::output::checklist::generate_restore_checklist;
use crate::output::script::generate_backup_script;
use crate::scanner::ignore::IgnoreList;
use crate::scanner::traversal::{ScannedItem, Scanner};
use rmcp::handler::server::wrapper::Json;
use rmcp::{ServiceExt, tool, tool_router};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Serialize, Deserialize, JsonSchema, Default)]
pub struct ScanParams {
    /// The path of the directory to scan (defaults to the current directory if empty).
    pub path: String,
    /// Optional path to a custom .nix-migrate-ignore file.
    pub ignore_file: Option<String>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct ScanResult {
    /// Scanned and classified items.
    pub items: Vec<ScannedItem>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct BackupParams {
    /// The list of scanned items to generate output for.
    pub items: Vec<ScannedItem>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct TextResult {
    /// The generated script or document content.
    pub content: String,
}

#[derive(Debug, Clone, Default)]
pub struct NixMigrateServer;

#[tool_router(server_handler)]
impl NixMigrateServer {
    /// Create a new server instance.
    pub fn new() -> Self {
        Self
    }

    /// Scans a directory and classifies all non-managed files versus Nix-managed files.
    #[tool(
        name = "scan_home",
        description = "Scan a directory and classify files by Nix-management state and category"
    )]
    pub async fn scan_home(
        &self,
        params: rmcp::handler::server::wrapper::Parameters<ScanParams>,
    ) -> Result<Json<ScanResult>, String> {
        let path_str = if params.0.path.is_empty() {
            ".".to_string()
        } else {
            params.0.path.clone()
        };

        let root_path = PathBuf::from(&path_str);
        if !root_path.exists() {
            return Err(format!("Path does not exist: {:?}", root_path));
        }

        // Determine ignore file path
        let ignore_file_path = match &params.0.ignore_file {
            Some(f) => PathBuf::from(f),
            None => root_path.join(".nix-migrate-ignore"),
        };

        let ignore_list = IgnoreList::load_from_file(&ignore_file_path)
            .map_err(|e| format!("Failed to load ignore file: {}", e))?;

        let scanner = Scanner::new(root_path, ignore_list);
        let items = scanner.scan();

        Ok(Json(ScanResult { items }))
    }

    /// Generates a bash backup script targeting the scanned files.
    #[tool(
        name = "generate_backup_script",
        description = "Generate a bash script to back up unmanaged configurations, secrets and data"
    )]
    pub async fn get_backup_script(
        &self,
        params: rmcp::handler::server::wrapper::Parameters<BackupParams>,
    ) -> Result<Json<TextResult>, String> {
        let content = generate_backup_script(&params.0.items);
        Ok(Json(TextResult { content }))
    }

    /// Generates a Markdown restoration checklist for the scanned files.
    #[tool(
        name = "generate_restore_checklist",
        description = "Generate a Markdown checklist detailing what was backed up and manual restoration steps"
    )]
    pub async fn get_restore_checklist(
        &self,
        params: rmcp::handler::server::wrapper::Parameters<BackupParams>,
    ) -> Result<Json<TextResult>, String> {
        let content = generate_restore_checklist(&params.0.items);
        Ok(Json(TextResult { content }))
    }
}

/// Runs the Nix Migrate MCP Server over stdio.
pub async fn run_server() -> anyhow::Result<()> {
    let server = NixMigrateServer::new();
    let service = server
        .serve((tokio::io::stdin(), tokio::io::stdout()))
        .await?;
    service.waiting().await?;
    Ok(())
}
