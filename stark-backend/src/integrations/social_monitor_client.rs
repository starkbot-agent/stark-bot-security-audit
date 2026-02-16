//! HTTP/JSON RPC client for the standalone social-monitor-service.

use social_monitor_types::*;

pub struct SocialMonitorClient {
    base_url: String,
    client: reqwest::Client,
}

impl SocialMonitorClient {
    pub fn new(base_url: &str) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            client: reqwest::Client::new(),
        }
    }

    /// Default client pointing to localhost:9102
    pub fn default_local() -> Self {
        Self::new("http://127.0.0.1:9102")
    }

    // =====================================================
    // Account Operations
    // =====================================================

    pub async fn add_account(
        &self,
        username: &str,
        notes: Option<&str>,
        custom_keywords: Option<&str>,
    ) -> Result<MonitoredAccount, String> {
        let req = AddAccountRequest {
            username: username.to_string(),
            notes: notes.map(|s| s.to_string()),
            custom_keywords: custom_keywords.map(|s| s.to_string()),
        };
        let resp: RpcResponse<MonitoredAccount> =
            self.post("/rpc/accounts/add", &req).await?;
        resp.data
            .ok_or_else(|| resp.error.unwrap_or_else(|| "Unknown error".to_string()))
    }

    pub async fn remove_account(&self, id: i64) -> Result<bool, String> {
        let req = RemoveAccountRequest { id };
        let resp: RpcResponse<bool> = self.post("/rpc/accounts/remove", &req).await?;
        if resp.success {
            Ok(true)
        } else {
            Err(resp.error.unwrap_or_else(|| "Unknown error".to_string()))
        }
    }

    pub async fn list_accounts(&self) -> Result<Vec<MonitoredAccount>, String> {
        let resp: RpcResponse<Vec<MonitoredAccount>> =
            self.get("/rpc/accounts/list").await?;
        resp.data
            .ok_or_else(|| resp.error.unwrap_or_else(|| "Unknown error".to_string()))
    }

    pub async fn update_account(
        &self,
        id: i64,
        monitor_enabled: Option<bool>,
        custom_keywords: Option<&str>,
        notes: Option<&str>,
    ) -> Result<bool, String> {
        let req = UpdateAccountRequest {
            id,
            monitor_enabled,
            custom_keywords: custom_keywords.map(|s| s.to_string()),
            notes: notes.map(|s| s.to_string()),
        };
        let resp: RpcResponse<bool> = self.post("/rpc/accounts/update", &req).await?;
        if resp.success {
            Ok(true)
        } else {
            Err(resp.error.unwrap_or_else(|| "Unknown error".to_string()))
        }
    }

    // =====================================================
    // Keyword Operations
    // =====================================================

    pub async fn add_keyword(
        &self,
        keyword: &str,
        category: Option<&str>,
        aliases: Option<Vec<String>>,
    ) -> Result<TrackedKeyword, String> {
        let req = AddKeywordRequest {
            keyword: keyword.to_string(),
            category: category.map(|s| s.to_string()),
            aliases,
        };
        let resp: RpcResponse<TrackedKeyword> =
            self.post("/rpc/keywords/add", &req).await?;
        resp.data
            .ok_or_else(|| resp.error.unwrap_or_else(|| "Unknown error".to_string()))
    }

    pub async fn remove_keyword(&self, id: i64) -> Result<bool, String> {
        let req = RemoveKeywordRequest { id };
        let resp: RpcResponse<bool> = self.post("/rpc/keywords/remove", &req).await?;
        if resp.success {
            Ok(true)
        } else {
            Err(resp.error.unwrap_or_else(|| "Unknown error".to_string()))
        }
    }

    pub async fn list_keywords(&self) -> Result<Vec<TrackedKeyword>, String> {
        let resp: RpcResponse<Vec<TrackedKeyword>> =
            self.get("/rpc/keywords/list").await?;
        resp.data
            .ok_or_else(|| resp.error.unwrap_or_else(|| "Unknown error".to_string()))
    }

    // =====================================================
    // Tweet Operations
    // =====================================================

    pub async fn query_tweets(
        &self,
        filter: &TweetFilter,
    ) -> Result<Vec<CapturedTweet>, String> {
        let resp: RpcResponse<Vec<CapturedTweet>> =
            self.post("/rpc/tweets/query", filter).await?;
        resp.data
            .ok_or_else(|| resp.error.unwrap_or_else(|| "Unknown error".to_string()))
    }

    pub async fn get_tweet_stats(&self) -> Result<TweetStats, String> {
        let resp: RpcResponse<TweetStats> = self.get("/rpc/tweets/stats").await?;
        resp.data
            .ok_or_else(|| resp.error.unwrap_or_else(|| "Unknown error".to_string()))
    }

    // =====================================================
    // Forensics Operations
    // =====================================================

    pub async fn query_topics(
        &self,
        filter: &TopicFilter,
    ) -> Result<Vec<TopicScore>, String> {
        let resp: RpcResponse<Vec<TopicScore>> =
            self.post("/rpc/topics/query", filter).await?;
        resp.data
            .ok_or_else(|| resp.error.unwrap_or_else(|| "Unknown error".to_string()))
    }

    pub async fn query_sentiment(
        &self,
        filter: &SentimentFilter,
    ) -> Result<Vec<SentimentSnapshot>, String> {
        let resp: RpcResponse<Vec<SentimentSnapshot>> =
            self.post("/rpc/sentiment/query", filter).await?;
        resp.data
            .ok_or_else(|| resp.error.unwrap_or_else(|| "Unknown error".to_string()))
    }

    pub async fn forensics_report(
        &self,
        account_id: Option<i64>,
        username: Option<&str>,
    ) -> Result<AccountForensicsReport, String> {
        let req = ForensicsReportRequest {
            account_id,
            username: username.map(|s| s.to_string()),
        };
        let resp: RpcResponse<AccountForensicsReport> =
            self.post("/rpc/forensics/report", &req).await?;
        resp.data
            .ok_or_else(|| resp.error.unwrap_or_else(|| "Unknown error".to_string()))
    }

    // =====================================================
    // Service Operations
    // =====================================================

    pub async fn get_status(&self) -> Result<ServiceStatus, String> {
        let resp: RpcResponse<ServiceStatus> = self.get("/rpc/status").await?;
        resp.data
            .ok_or_else(|| resp.error.unwrap_or_else(|| "Unknown error".to_string()))
    }

    pub async fn backup_export(&self) -> Result<BackupData, String> {
        let resp: RpcResponse<BackupData> =
            self.post_empty("/rpc/backup/export").await?;
        resp.data
            .ok_or_else(|| resp.error.unwrap_or_else(|| "Unknown error".to_string()))
    }

    pub async fn backup_restore(&self, data: BackupData) -> Result<usize, String> {
        let req = BackupRestoreRequest { data };
        let resp: RpcResponse<usize> = self.post("/rpc/backup/restore", &req).await?;
        resp.data
            .ok_or_else(|| resp.error.unwrap_or_else(|| "Unknown error".to_string()))
    }

    // =====================================================
    // HTTP helpers
    // =====================================================

    async fn get<T: serde::de::DeserializeOwned>(&self, path: &str) -> Result<T, String> {
        let url = format!("{}{}", self.base_url, path);
        self.client
            .get(&url)
            .send()
            .await
            .map_err(|e| format!("Social monitor service unavailable: {}", e))?
            .json::<T>()
            .await
            .map_err(|e| format!("Invalid response from social monitor service: {}", e))
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
            .map_err(|e| format!("Social monitor service unavailable: {}", e))?
            .json::<T>()
            .await
            .map_err(|e| format!("Invalid response from social monitor service: {}", e))
    }

    async fn post_empty<T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
    ) -> Result<T, String> {
        let url = format!("{}{}", self.base_url, path);
        self.client
            .post(&url)
            .send()
            .await
            .map_err(|e| format!("Social monitor service unavailable: {}", e))?
            .json::<T>()
            .await
            .map_err(|e| format!("Invalid response from social monitor service: {}", e))
    }
}
