//! HTTP/JSON RPC client for the standalone discord-tipping-service.

use discord_tipping_types::*;

pub struct DiscordTippingClient {
    base_url: String,
    client: reqwest::Client,
}

impl DiscordTippingClient {
    pub fn new(base_url: &str) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            client: reqwest::Client::new(),
        }
    }

    pub fn default_local() -> Self {
        Self::new("http://127.0.0.1:9101")
    }

    pub async fn get_or_create_profile(
        &self,
        discord_user_id: &str,
        username: &str,
    ) -> Result<DiscordUserProfile, String> {
        let req = GetOrCreateProfileRequest {
            discord_user_id: discord_user_id.to_string(),
            username: username.to_string(),
        };
        let resp: RpcResponse<DiscordUserProfile> = self.post("/rpc/profile/get_or_create", &req).await?;
        resp.data.ok_or_else(|| resp.error.unwrap_or_else(|| "Unknown error".to_string()))
    }

    pub async fn get_profile(
        &self,
        discord_user_id: &str,
    ) -> Result<Option<DiscordUserProfile>, String> {
        let req = GetProfileRequest {
            discord_user_id: discord_user_id.to_string(),
        };
        let resp: RpcResponse<Option<DiscordUserProfile>> = self.post("/rpc/profile/get", &req).await?;
        if resp.success {
            // data is Option<Option<T>>; serde deserializes null as None (outer),
            // so flatten: Some(Some(x)) -> Some(x), None -> None
            Ok(resp.data.flatten())
        } else {
            Err(resp.error.unwrap_or_else(|| "Unknown error".to_string()))
        }
    }

    pub async fn get_profile_by_address(
        &self,
        address: &str,
    ) -> Result<Option<DiscordUserProfile>, String> {
        let req = GetProfileByAddressRequest {
            address: address.to_string(),
        };
        let resp: RpcResponse<Option<DiscordUserProfile>> =
            self.post("/rpc/profile/get_by_address", &req).await?;
        if resp.success {
            Ok(resp.data.flatten())
        } else {
            Err(resp.error.unwrap_or_else(|| "Unknown error".to_string()))
        }
    }

    pub async fn register_address(
        &self,
        discord_user_id: &str,
        address: &str,
    ) -> Result<(), String> {
        let req = RegisterAddressRequest {
            discord_user_id: discord_user_id.to_string(),
            address: address.to_string(),
        };
        let resp: RpcResponse<bool> = self.post("/rpc/profile/register", &req).await?;
        if resp.success {
            Ok(())
        } else {
            Err(resp.error.unwrap_or_else(|| "Unknown error".to_string()))
        }
    }

    pub async fn unregister_address(
        &self,
        discord_user_id: &str,
    ) -> Result<(), String> {
        let req = UnregisterAddressRequest {
            discord_user_id: discord_user_id.to_string(),
        };
        let resp: RpcResponse<bool> = self.post("/rpc/profile/unregister", &req).await?;
        if resp.success {
            Ok(())
        } else {
            Err(resp.error.unwrap_or_else(|| "Unknown error".to_string()))
        }
    }

    pub async fn list_all_profiles(&self) -> Result<Vec<DiscordUserProfile>, String> {
        let resp: RpcResponse<Vec<DiscordUserProfile>> = self.get("/rpc/profiles/all").await?;
        resp.data.ok_or_else(|| resp.error.unwrap_or_else(|| "Unknown error".to_string()))
    }

    pub async fn list_registered_profiles(&self) -> Result<Vec<DiscordUserProfile>, String> {
        let resp: RpcResponse<Vec<DiscordUserProfile>> =
            self.get("/rpc/profiles/registered").await?;
        resp.data.ok_or_else(|| resp.error.unwrap_or_else(|| "Unknown error".to_string()))
    }

    pub async fn get_stats(&self) -> Result<ProfileStats, String> {
        let resp: RpcResponse<ProfileStats> = self.get("/rpc/stats").await?;
        resp.data.ok_or_else(|| resp.error.unwrap_or_else(|| "Unknown error".to_string()))
    }

    pub async fn get_status(&self) -> Result<ServiceStatus, String> {
        let resp: RpcResponse<ServiceStatus> = self.get("/rpc/status").await?;
        resp.data.ok_or_else(|| resp.error.unwrap_or_else(|| "Unknown error".to_string()))
    }

    pub async fn backup_export(&self) -> Result<Vec<BackupEntry>, String> {
        let resp: RpcResponse<Vec<BackupEntry>> = self.post_empty("/rpc/backup/export").await?;
        resp.data.ok_or_else(|| resp.error.unwrap_or_else(|| "Unknown error".to_string()))
    }

    pub async fn backup_restore(&self, profiles: Vec<BackupEntry>) -> Result<usize, String> {
        let req = BackupRestoreRequest { profiles };
        let resp: RpcResponse<usize> = self.post("/rpc/backup/restore", &req).await?;
        resp.data.ok_or_else(|| resp.error.unwrap_or_else(|| "Unknown error".to_string()))
    }

    async fn get<T: serde::de::DeserializeOwned>(&self, path: &str) -> Result<T, String> {
        let url = format!("{}{}", self.base_url, path);
        self.client
            .get(&url)
            .send()
            .await
            .map_err(|e| format!("Discord tipping service unavailable: {}", e))?
            .json::<T>()
            .await
            .map_err(|e| format!("Invalid response from discord tipping service: {}", e))
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
            .map_err(|e| format!("Discord tipping service unavailable: {}", e))?
            .json::<T>()
            .await
            .map_err(|e| format!("Invalid response from discord tipping service: {}", e))
    }

    async fn post_empty<T: serde::de::DeserializeOwned>(&self, path: &str) -> Result<T, String> {
        let url = format!("{}{}", self.base_url, path);
        self.client
            .post(&url)
            .send()
            .await
            .map_err(|e| format!("Discord tipping service unavailable: {}", e))?
            .json::<T>()
            .await
            .map_err(|e| format!("Invalid response from discord tipping service: {}", e))
    }
}
