//! Microsoft account authentication for Minecraft: Java Edition.
//!
//! The flow is intentionally device-code based: no embedded browser, no
//! redirect URI and no client secret. The public client id belongs to the
//! CreeperCore Entra application; it is not a secret.

use anyhow::{anyhow, bail, Context, Result};
use base64::{
    engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD},
    Engine as _,
};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::time::Duration;
use uuid::Uuid;

pub const CLIENT_ID: &str = "4fe694bb-536a-497b-ac37-6ce5e089b95e";
const AUTHORITY: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0";
const SCOPES: &str = "XboxLive.signin offline_access openid profile";
const REDIRECT_URI: &str = "http://localhost";
const MAX_SKIN_BYTES: u64 = 2 * 1024 * 1024;

#[derive(Debug, Clone)]
pub struct AuthRequest {
    pub url: String,
    pub redirect_uri: String,
    pub code_verifier: String,
    pub state: String,
}

pub fn build_auth_request() -> Result<AuthRequest> {
    let verifier = format!(
        "{}{}{}",
        Uuid::new_v4().simple(),
        Uuid::new_v4().simple(),
        Uuid::new_v4().simple()
    );
    let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
    let state = Uuid::new_v4().to_string();
    let mut url = reqwest::Url::parse(&format!("{AUTHORITY}/authorize"))?;
    url.query_pairs_mut()
        .append_pair("client_id", CLIENT_ID)
        .append_pair("response_type", "code")
        .append_pair("redirect_uri", REDIRECT_URI)
        .append_pair("response_mode", "query")
        .append_pair("scope", SCOPES)
        .append_pair("state", &state)
        .append_pair("code_challenge", &challenge)
        .append_pair("code_challenge_method", "S256")
        .append_pair("prompt", "select_account");
    Ok(AuthRequest {
        url: url.to_string(),
        redirect_uri: REDIRECT_URI.to_string(),
        code_verifier: verifier,
        state,
    })
}

pub fn authorization_code_from_url(url: &str, expected_state: &str) -> Result<Option<String>> {
    let parsed = reqwest::Url::parse(url).context("Microsoft 回调地址无效")?;
    if parsed.scheme() != "http" || parsed.host_str() != Some("localhost") {
        return Ok(None);
    }
    let mut code = None;
    let mut state = None;
    let mut error = None;
    let mut description = None;
    for (key, value) in parsed.query_pairs() {
        match key.as_ref() {
            "code" => code = Some(value.into_owned()),
            "state" => state = Some(value.into_owned()),
            "error" => error = Some(value.into_owned()),
            "error_description" => description = Some(value.into_owned()),
            _ => {}
        }
    }
    if let Some(error) = error {
        let description = description.unwrap_or_else(|| "Microsoft 授权失败".to_string());
        bail!("{error}: {description}");
    }
    if state.as_deref() != Some(expected_state) {
        bail!("Microsoft 回调 state 不匹配，请重新登录");
    }
    Ok(code)
}

pub async fn login_with_authorization_code(
    client: &Client,
    code: &str,
    redirect_uri: &str,
    code_verifier: &str,
) -> Result<MsaIdentity> {
    let token = token_request(
        client,
        &[
            ("grant_type", "authorization_code".to_string()),
            ("client_id", CLIENT_ID.to_string()),
            ("code", code.to_string()),
            ("redirect_uri", redirect_uri.to_string()),
            ("code_verifier", code_verifier.to_string()),
        ],
    )
    .await?;
    let access_token = token
        .access_token
        .ok_or_else(|| anyhow!("Microsoft 没有返回 access token"))?;
    let refresh_token = token
        .refresh_token
        .ok_or_else(|| anyhow!("Microsoft 没有返回 refresh token"))?;
    complete_chain(client, &access_token, &refresh_token).await
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceCodeInfo {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub expires_in: u64,
    pub interval: u64,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct MsaIdentity {
    pub username: String,
    pub uuid: String,
    /// Minecraft access token, not the Microsoft token.
    pub access_token: String,
    pub refresh_token: String,
    /// Unix epoch seconds for the Minecraft access token.
    pub expires_at: i64,
    pub skin_data_url: Option<String>,
}

pub enum DevicePoll {
    Pending,
    Complete(MsaIdentity),
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    #[serde(default)]
    access_token: Option<String>,
    #[serde(default)]
    refresh_token: Option<String>,
}

async fn token_request(client: &Client, form: &[(&str, String)]) -> Result<TokenResponse> {
    let response = client
        .post(format!("{AUTHORITY}/token"))
        .form(form)
        .timeout(Duration::from_secs(25))
        .send()
        .await
        .context("无法连接 Microsoft 登录服务")?;
    let status = response.status();
    let value: Value = response
        .json()
        .await
        .context("Microsoft 登录响应格式无效")?;
    if !status.is_success() {
        let code = value
            .get("error")
            .and_then(Value::as_str)
            .unwrap_or("unknown_error");
        let desc = value
            .get("error_description")
            .and_then(Value::as_str)
            .unwrap_or("Microsoft 登录失败");
        let short = desc.split(['\r', '\n']).next().unwrap_or(desc);
        bail!("{code}: {short}");
    }
    serde_json::from_value(value).context("Microsoft token 响应缺少字段")
}

pub async fn start_device_code(client: &Client) -> Result<DeviceCodeInfo> {
    #[derive(Deserialize)]
    struct Response {
        device_code: String,
        user_code: String,
        verification_uri: String,
        expires_in: u64,
        #[serde(default = "default_interval")]
        interval: u64,
        #[serde(default)]
        message: String,
    }
    fn default_interval() -> u64 {
        5
    }
    let response = client
        .post(format!("{AUTHORITY}/devicecode"))
        .form(&[("client_id", CLIENT_ID), ("scope", SCOPES)])
        .timeout(Duration::from_secs(25))
        .send()
        .await
        .context("无法连接 Microsoft 设备登录服务")?;
    if !response.status().is_success() {
        let text = response.text().await.unwrap_or_default();
        bail!("无法开始 Microsoft 登录：{text}");
    }
    let value: Response = response.json().await.context("设备登录响应格式无效")?;
    Ok(DeviceCodeInfo {
        device_code: value.device_code,
        user_code: value.user_code,
        verification_uri: value.verification_uri,
        expires_in: value.expires_in,
        interval: value.interval.max(3),
        message: if value.message.is_empty() {
            "请在浏览器中输入代码完成登录".to_string()
        } else {
            value.message
        },
    })
}

pub async fn poll_device_code(client: &Client, device_code: &str) -> Result<DevicePoll> {
    let response = client
        .post(format!("{AUTHORITY}/token"))
        .form(&[
            (
                "grant_type",
                "urn:ietf:params:oauth:grant-type:device_code".to_string(),
            ),
            ("client_id", CLIENT_ID.to_string()),
            ("device_code", device_code.to_string()),
        ])
        .timeout(Duration::from_secs(25))
        .send()
        .await
        .context("无法轮询 Microsoft 登录状态")?;
    let status = response.status();
    let value: Value = response.json().await.context("Microsoft 登录轮询格式无效")?;
    if !status.is_success() {
        let code = value
            .get("error")
            .and_then(Value::as_str)
            .unwrap_or("unknown_error");
        if code == "authorization_pending" || code == "slow_down" {
            return Ok(DevicePoll::Pending);
        }
        let description = value
            .get("error_description")
            .and_then(Value::as_str)
            .unwrap_or("Microsoft 登录未完成");
        let short = description.split(['\r', '\n']).next().unwrap_or(description);
        bail!("{code}: {short}");
    }
    let token: TokenResponse = serde_json::from_value(value)?;
    let access_token = token
        .access_token
        .ok_or_else(|| anyhow!("Microsoft 没有返回 access token"))?;
    let refresh_token = token
        .refresh_token
        .ok_or_else(|| anyhow!("Microsoft 没有返回 refresh token"))?;
    let identity = complete_chain(client, &access_token, &refresh_token).await?;
    Ok(DevicePoll::Complete(identity))
}

pub async fn refresh_session(client: &Client, refresh_token: &str) -> Result<MsaIdentity> {
    let token = token_request(
        client,
        &[
            ("grant_type", "refresh_token".to_string()),
            ("client_id", CLIENT_ID.to_string()),
            ("refresh_token", refresh_token.to_string()),
            ("scope", SCOPES.to_string()),
        ],
    )
    .await?;
    let access_token = token
        .access_token
        .ok_or_else(|| anyhow!("Microsoft refresh 没有返回 access token"))?;
    let rotated = token.refresh_token.unwrap_or_else(|| refresh_token.to_string());
    complete_chain(client, &access_token, &rotated).await
}

async fn complete_chain(
    client: &Client,
    microsoft_access_token: &str,
    refresh_token: &str,
) -> Result<MsaIdentity> {
    let xbox = xbox_user_auth(client, microsoft_access_token).await?;
    let xsts = xsts_authorize(client, &xbox).await?;
    let minecraft = login_with_xbox(client, &xsts).await?;
    entitlements(client, &minecraft).await?;
    let profile = minecraft_profile(client, &minecraft).await?;
    let username = profile
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or("Player")
        .to_string();
    let compact_uuid = profile
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let uuid = dash_uuid(&compact_uuid)?;
    let skin_data_url = skin_from_profile(client, &profile).await;
    let expires_in = minecraft
        .get("expires_in")
        .and_then(Value::as_u64)
        .unwrap_or(86_400);
    Ok(MsaIdentity {
        username,
        uuid,
        access_token: minecraft
            .get("access_token")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        refresh_token: refresh_token.to_string(),
        expires_at: chrono::Utc::now().timestamp() + expires_in as i64,
        skin_data_url,
    })
}

async fn xbox_user_auth(client: &Client, microsoft_access_token: &str) -> Result<String> {
    let response = client
        .post("https://user.auth.xboxlive.com/user/authenticate")
        .json(&json!({
            "Properties": {
                "AuthMethod": "RPS",
                "SiteName": "user.auth.xboxlive.com",
                "RpsTicket": format!("d={microsoft_access_token}")
            },
            "RelyingParty": "http://auth.xboxlive.com",
            "TokenType": "JWT"
        }))
        .timeout(Duration::from_secs(25))
        .send()
        .await
        .context("无法连接 Xbox Live")?;
    let status = response.status();
    let value: Value = response.json().await.context("Xbox Live 响应格式无效")?;
    if !status.is_success() {
        bail!("Xbox Live 身份验证失败：{}", compact_error(&value));
    }
    value
        .get("Token")
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| anyhow!("Xbox Live 没有返回 token"))
}

struct XstsToken {
    token: String,
    uhs: String,
}

async fn xsts_authorize(client: &Client, xbox_token: &str) -> Result<XstsToken> {
    let payload = json!({
        "Properties": {
            "SandboxId": "RETAIL",
            "UserTokens": [xbox_token]
        },
        "RelyingParty": "rp://api.minecraftservices.com/",
        "TokenType": "JWT"
    });
    let mut response = None;
    let mut last_error = None;
    for attempt in 0..3 {
        match client
            .post("https://xsts.auth.xboxlive.com/xsts/authorize")
            .json(&payload)
            .timeout(Duration::from_secs(10))
            .send()
            .await
        {
            Ok(value) => {
                response = Some(value);
                break;
            }
            Err(error) => {
                last_error = Some(error);
                if attempt < 2 {
                    tokio::time::sleep(Duration::from_millis(500 * (attempt + 1) as u64)).await;
                }
            }
        }
    }
    let response = response.ok_or_else(|| {
        anyhow!(
            "无法连接 Xbox XSTS：{}。请确认网络可以访问 xsts.auth.xboxlive.com；如果使用代理或 VPN，请开启系统代理/TUN 后重启启动器",
            last_error
                .map(|error| error.to_string())
                .unwrap_or_else(|| "未知网络错误".to_string())
        )
    })?;
    let status = response.status();
    let value: Value = response.json().await.context("XSTS 响应格式无效")?;
    if !status.is_success() {
        let xerr = value.get("XErr").and_then(Value::as_i64);
        let message = match xerr {
            Some(2148916233) => "这个 Microsoft 账户还没有 Xbox 档案，请先在 xbox.com 登录一次",
            Some(2148916235) => "Xbox Live 在你所在地区不可用",
            Some(2148916236) => "需要完成 Xbox 成人验证",
            Some(2148916238) => "该账户是儿童账户，需要加入家庭组",
            _ => "XSTS 授权失败",
        };
        bail!("{message}");
    }
    let token = value
        .get("Token")
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| anyhow!("XSTS 没有返回 token"))?;
    let uhs = value
        .get("DisplayClaims")
        .and_then(|claims| claims.get("xui"))
        .and_then(Value::as_array)
        .and_then(|items| items.first())
        .and_then(|item| item.get("uhs"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| anyhow!("XSTS 没有返回用户哈希"))?;
    Ok(XstsToken { token, uhs })
}

async fn login_with_xbox(client: &Client, xsts: &XstsToken) -> Result<Value> {
    let response = client
        .post("https://api.minecraftservices.com/authentication/login_with_xbox")
        .json(&json!({
            "identityToken": format!("XBL3.0 x={};{}", xsts.uhs, xsts.token)
        }))
        .timeout(Duration::from_secs(25))
        .send()
        .await
        .context("无法连接 Minecraft 登录服务")?;
    let status = response.status();
    let value: Value = response.json().await.context("Minecraft 登录响应格式无效")?;
    if !status.is_success() {
        let message = value
            .get("errorMessage")
            .and_then(Value::as_str)
            .unwrap_or("Minecraft 登录失败；请确认 Azure 应用已获得 Minecraft API 权限");
        if message.contains("Invalid app registration")
            || message.contains("AppRegInfo")
        {
            bail!(
                "Microsoft 账号已授权，但这个 Azure 应用没有 Minecraft API 权限（Invalid app registration）。\
                 这不是账号或密码问题；请先提交 https://aka.ms/mce-reviewappid 审核。"
            );
        }
        bail!("{message}");
    }
    Ok(value)
}

async fn entitlements(client: &Client, minecraft: &Value) -> Result<()> {
    let token = minecraft
        .get("access_token")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("Minecraft 登录没有返回 access token"))?;
    let response = client
        .get("https://api.minecraftservices.com/entitlements/mcstore")
        .bearer_auth(token)
        .timeout(Duration::from_secs(25))
        .send()
        .await
        .context("无法检查 Minecraft 所有权")?;
    let status = response.status();
    let value: Value = response.json().await.unwrap_or_default();
    if !status.is_success() {
        bail!("无法检查 Minecraft 所有权（HTTP {status}）");
    }
    let items = value
        .get("items")
        .and_then(Value::as_array)
        .map(Vec::len)
        .unwrap_or(0);
    if items == 0 {
        bail!("这个 Microsoft 账户没有 Minecraft Java 版所有权");
    }
    Ok(())
}

async fn minecraft_profile(client: &Client, minecraft: &Value) -> Result<Value> {
    let token = minecraft
        .get("access_token")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("Minecraft 登录没有返回 access token"))?;
    fetch_profile(client, token).await
}

pub async fn fetch_profile(client: &Client, token: &str) -> Result<Value> {
    let response = client
        .get("https://api.minecraftservices.com/minecraft/profile")
        .bearer_auth(token)
        .timeout(Duration::from_secs(25))
        .send()
        .await
        .context("无法读取 Minecraft 档案")?;
    let status = response.status();
    if status.as_u16() == 404 {
        bail!("Microsoft 账户没有 Minecraft Java 版档案");
    }
    if status.as_u16() == 401 || status.as_u16() == 403 {
        bail!("Minecraft 登录已失效，请重新登录");
    }
    if !status.is_success() {
        bail!("无法读取 Minecraft 档案（HTTP {status}）");
    }
    response.json().await.context("Minecraft 档案格式无效")
}

pub async fn upload_skin(
    client: &Client,
    token: &str,
    bytes: Vec<u8>,
    variant: &str,
) -> Result<Value> {
    if !matches!(variant, "classic" | "slim") {
        bail!("皮肤模型必须是 classic 或 slim");
    }
    let form = reqwest::multipart::Form::new()
        .text("variant", variant.to_string())
        .part(
            "file",
            reqwest::multipart::Part::bytes(bytes)
                .file_name("skin.png")
                .mime_str("image/png")?,
        );
    let response = client
        .post("https://api.minecraftservices.com/minecraft/profile/skins")
        .bearer_auth(token)
        .multipart(form)
        .timeout(Duration::from_secs(40))
        .send()
        .await
        .context("无法上传 Minecraft 皮肤")?;
    let status = response.status();
    if !status.is_success() {
        let value: Value = response.json().await.unwrap_or_default();
        let message = value
            .get("errorMessage")
            .and_then(Value::as_str)
            .unwrap_or("Minecraft 拒绝了皮肤上传");
        bail!("{message}");
    }
    fetch_profile(client, token).await
}

pub async fn delete_skin(client: &Client, token: &str) -> Result<Value> {
    let response = client
        .delete("https://api.minecraftservices.com/minecraft/profile/skins/active")
        .bearer_auth(token)
        .timeout(Duration::from_secs(25))
        .send()
        .await
        .context("无法恢复默认皮肤")?;
    if !response.status().is_success() {
        bail!(
            "Minecraft 拒绝了恢复默认皮肤（HTTP {}）",
            response.status()
        );
    }
    fetch_profile(client, token).await
}

pub async fn set_cape(client: &Client, token: &str, cape_id: Option<&str>) -> Result<Value> {
    let response = if let Some(cape_id) = cape_id {
        client
            .put("https://api.minecraftservices.com/minecraft/profile/capes/active")
            .bearer_auth(token)
            .json(&json!({ "capeId": cape_id }))
            .timeout(Duration::from_secs(25))
            .send()
            .await
            .context("无法切换披风")?
    } else {
        client
            .delete("https://api.minecraftservices.com/minecraft/profile/capes/active")
            .bearer_auth(token)
            .timeout(Duration::from_secs(25))
            .send()
            .await
            .context("无法隐藏披风")?
    };
    if !response.status().is_success() {
        let value: Value = response.json().await.unwrap_or_default();
        let message = value
            .get("errorMessage")
            .and_then(Value::as_str)
            .unwrap_or("Minecraft 拒绝了披风设置");
        bail!("{message}");
    }
    fetch_profile(client, token).await
}

pub async fn download_texture(client: &Client, url: &str) -> Option<String> {
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return None;
    }
    let response = client
        .get(url)
        .timeout(Duration::from_secs(20))
        .send()
        .await
        .ok()?;
    if !response.status().is_success() {
        return None;
    }
    if response.content_length().unwrap_or(0) > MAX_SKIN_BYTES {
        return None;
    }
    let bytes = response.bytes().await.ok()?;
    if bytes.len() as u64 > MAX_SKIN_BYTES {
        return None;
    }
    Some(format!("data:image/png;base64,{}", STANDARD.encode(bytes)))
}

async fn skin_from_profile(client: &Client, profile: &Value) -> Option<String> {
    let skins = profile.get("skins")?.as_array()?;
    let skin = skins
        .iter()
        .find(|skin| {
            skin.get("state")
                .and_then(Value::as_str)
                .map(|state| state.eq_ignore_ascii_case("ACTIVE"))
                .unwrap_or(true)
        })
        .or_else(|| skins.first())?;
    let url = skin.get("url")?.as_str()?;
    let response = client
        .get(url)
        .timeout(Duration::from_secs(20))
        .send()
        .await
        .ok()?;
    if !response.status().is_success() {
        return None;
    }
    if response.content_length().unwrap_or(0) > MAX_SKIN_BYTES {
        return None;
    }
    let bytes = response.bytes().await.ok()?;
    if bytes.len() as u64 > MAX_SKIN_BYTES {
        return None;
    }
    Some(format!("data:image/png;base64,{}", STANDARD.encode(bytes)))
}

fn dash_uuid(compact: &str) -> Result<String> {
    if compact.len() != 32 || !compact.chars().all(|ch| ch.is_ascii_hexdigit()) {
        bail!("Minecraft 返回了无效的 UUID");
    }
    Ok(format!(
        "{}-{}-{}-{}-{}",
        &compact[0..8],
        &compact[8..12],
        &compact[12..16],
        &compact[16..20],
        &compact[20..32]
    ))
}

fn compact_error(value: &Value) -> String {
    value
        .get("Message")
        .or_else(|| value.get("errorMessage"))
        .and_then(Value::as_str)
        .unwrap_or("未知错误")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_dashed_uuid() {
        assert_eq!(
            dash_uuid("0123456789abcdef0123456789abcdef").unwrap(),
            "01234567-89ab-cdef-0123-456789abcdef"
        );
    }

    #[test]
    fn rejects_invalid_uuid() {
        assert!(dash_uuid("not-a-uuid").is_err());
    }
}
