//! HTTP/JSON RPC client for the standalone wallet-monitor-service.

use wallet_monitor_types::*;

pub struct WalletMonitorClient {
    base_url: String,
    client: reqwest::Client,
}

impl WalletMonitorClient {
    pub fn new(base_url: &str) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            client: reqwest::Client::new(),
        }
    }

    /// Default client pointing to localhost:9100
    pub fn default_local() -> Self {
        Self::new("http://127.0.0.1:9100")
    }

    pub async fn add_wallet(
        &self,
        address: &str,
        label: Option<&str>,
        chain: &str,
        threshold_usd: f64,
    ) -> Result<WatchlistEntry, String> {
        let req = AddWalletRequest {
            address: address.to_string(),
            label: label.map(|s| s.to_string()),
            chain: Some(chain.to_string()),
            threshold_usd: Some(threshold_usd),
        };
        let resp: RpcResponse<WatchlistEntry> = self
            .post("/rpc/watchlist/add", &req)
            .await?;
        resp.data.ok_or_else(|| resp.error.unwrap_or_else(|| "Unknown error".to_string()))
    }

    pub async fn remove_wallet(&self, id: i64) -> Result<bool, String> {
        let req = RemoveWalletRequest { id };
        let resp: RpcResponse<bool> = self
            .post("/rpc/watchlist/remove", &req)
            .await?;
        if resp.success {
            Ok(true)
        } else {
            Err(resp.error.unwrap_or_else(|| "Unknown error".to_string()))
        }
    }

    pub async fn list_watchlist(&self) -> Result<Vec<WatchlistEntry>, String> {
        let resp: RpcResponse<Vec<WatchlistEntry>> = self
            .get("/rpc/watchlist/list")
            .await?;
        resp.data.ok_or_else(|| resp.error.unwrap_or_else(|| "Unknown error".to_string()))
    }

    pub async fn update_wallet(
        &self,
        id: i64,
        label: Option<&str>,
        threshold_usd: Option<f64>,
        monitor_enabled: Option<bool>,
        notes: Option<&str>,
    ) -> Result<bool, String> {
        let req = UpdateWalletRequest {
            id,
            label: label.map(|s| s.to_string()),
            threshold_usd,
            monitor_enabled,
            notes: notes.map(|s| s.to_string()),
        };
        let resp: RpcResponse<bool> = self
            .post("/rpc/watchlist/update", &req)
            .await?;
        if resp.success {
            Ok(true)
        } else {
            Err(resp.error.unwrap_or_else(|| "Unknown error".to_string()))
        }
    }

    pub async fn query_activity(
        &self,
        filter: &ActivityFilter,
    ) -> Result<Vec<ActivityEntry>, String> {
        let resp: RpcResponse<Vec<ActivityEntry>> = self
            .post("/rpc/activity/query", filter)
            .await?;
        resp.data.ok_or_else(|| resp.error.unwrap_or_else(|| "Unknown error".to_string()))
    }

    pub async fn get_activity_stats(&self) -> Result<ActivityStats, String> {
        let resp: RpcResponse<ActivityStats> = self
            .get("/rpc/activity/stats")
            .await?;
        resp.data.ok_or_else(|| resp.error.unwrap_or_else(|| "Unknown error".to_string()))
    }

    pub async fn get_status(&self) -> Result<ServiceStatus, String> {
        let resp: RpcResponse<ServiceStatus> = self
            .get("/rpc/status")
            .await?;
        resp.data.ok_or_else(|| resp.error.unwrap_or_else(|| "Unknown error".to_string()))
    }

    pub async fn backup_export(&self) -> Result<Vec<BackupEntry>, String> {
        let resp: RpcResponse<Vec<BackupEntry>> = self.post_empty("/rpc/backup/export").await?;
        resp.data.ok_or_else(|| resp.error.unwrap_or_else(|| "Unknown error".to_string()))
    }

    pub async fn backup_restore(&self, wallets: Vec<BackupEntry>) -> Result<usize, String> {
        let req = BackupRestoreRequest { wallets };
        let resp: RpcResponse<usize> = self.post("/rpc/backup/restore", &req).await?;
        resp.data.ok_or_else(|| resp.error.unwrap_or_else(|| "Unknown error".to_string()))
    }

    async fn get<T: serde::de::DeserializeOwned>(&self, path: &str) -> Result<T, String> {
        let url = format!("{}{}", self.base_url, path);
        self.client
            .get(&url)
            .send()
            .await
            .map_err(|e| format!("Wallet monitor service unavailable: {}", e))?
            .json::<T>()
            .await
            .map_err(|e| format!("Invalid response from wallet monitor service: {}", e))
    }

    async fn post<B: serde::Serialize, T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<T, String> {
        let url = format!("{}{}", self.base_url, path);
        self.client
            .post(&url)
            .json(body)
            .send()
            .await
            .map_err(|e| format!("Wallet monitor service unavailable: {}", e))?
            .json::<T>()
            .await
            .map_err(|e| format!("Invalid response from wallet monitor service: {}", e))
    }

    async fn post_empty<T: serde::de::DeserializeOwned>(&self, path: &str) -> Result<T, String> {
        let url = format!("{}{}", self.base_url, path);
        self.client
            .post(&url)
            .send()
            .await
            .map_err(|e| format!("Wallet monitor service unavailable: {}", e))?
            .json::<T>()
            .await
            .map_err(|e| format!("Invalid response from wallet monitor service: {}", e))
    }
}
