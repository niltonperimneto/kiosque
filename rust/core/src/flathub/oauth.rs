use serde::{Deserialize, Serialize};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum OAuthProvider {
    GitHub,
    GitLab,
    GnomeGitLab,
    KdeGitLab,
}

impl OAuthProvider {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::GitHub => "github",
            Self::GitLab => "gitlab",
            Self::GnomeGitLab => "gnome_gitlab",
            Self::KdeGitLab => "kde_gitlab",
        }
    }

    // Preserve the provider-selection API used by the Qt adapter.
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "github" => Some(Self::GitHub),
            "gitlab" => Some(Self::GitLab),
            "gnome_gitlab" => Some(Self::GnomeGitLab),
            "kde_gitlab" => Some(Self::KdeGitLab),
            _ => None,
        }
    }

    pub fn client_id(&self) -> String {
        match self {
            Self::GitHub => std::env::var("KIOSQUE_GITHUB_CLIENT_ID")
                .unwrap_or_else(|_| "Ov23lix0VtQ53LDFg8kH".to_string()),
            Self::GitLab => std::env::var("KIOSQUE_GITLAB_CLIENT_ID")
                .unwrap_or_else(|_| "client_id_gitlab_placeholder".to_string()),
            Self::GnomeGitLab => std::env::var("KIOSQUE_GNOME_CLIENT_ID")
                .unwrap_or_else(|_| "client_id_gnome_placeholder".to_string()),
            Self::KdeGitLab => std::env::var("KIOSQUE_KDE_CLIENT_ID")
                .unwrap_or_else(|_| "client_id_kde_placeholder".to_string()),
        }
    }

    fn authorization_url(
        &self,
        state: &str,
        redirect_uri: &str,
        challenge: &str,
    ) -> Result<String, String> {
        let base = match self {
            Self::GitHub => return Err("GitHub uses device authorization".into()),
            Self::GitLab => "https://gitlab.com/oauth/authorize",
            Self::GnomeGitLab => "https://gitlab.gnome.org/oauth/authorize",
            Self::KdeGitLab => "https://invent.kde.org/oauth/authorize",
        };
        let mut url = reqwest::Url::parse(base).map_err(|_| "Invalid OAuth endpoint")?;
        url.query_pairs_mut().extend_pairs([
            ("client_id", self.client_id().as_str()),
            ("redirect_uri", redirect_uri),
            ("response_type", "code"),
            ("state", state),
            ("scope", "read_user"),
            ("code_challenge_method", "S256"),
            ("code_challenge", challenge),
        ]);
        Ok(url.into())
    }

    pub fn token_url(&self) -> &'static str {
        match self {
            Self::GitHub => "https://github.com/login/oauth/access_token",
            Self::GitLab => "https://gitlab.com/oauth/token",
            Self::GnomeGitLab => "https://gitlab.gnome.org/oauth/token",
            Self::KdeGitLab => "https://invent.kde.org/oauth/token",
        }
    }

    pub fn user_profile_url(&self) -> &'static str {
        match self {
            Self::GitHub => "https://api.github.com/user",
            Self::GitLab => "https://gitlab.com/api/v4/user",
            Self::GnomeGitLab => "https://gitlab.gnome.org/api/v4/user",
            Self::KdeGitLab => "https://invent.kde.org/api/v4/user",
        }
    }
}

const LOGIN_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(300);
const RESPONSE_LIMIT: usize = 64 * 1024;

#[derive(Clone, Debug)]
pub struct DevicePrompt {
    pub user_code: String,
    pub verification_uri: String,
}

fn random_url_token() -> String {
    use base64::Engine;
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(rand::random::<[u8; 32]>())
}

fn pkce_challenge(verifier: &str) -> String {
    use base64::Engine;
    use sha2::{Digest, Sha256};
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}

fn same_state(left: &str, right: &str) -> bool {
    left.len() == right.len()
        && left
            .bytes()
            .zip(right.bytes())
            .fold(0, |difference, (a, b)| difference | (a ^ b))
            == 0
}

fn callback_code(request: &str, path: &str, state: &str) -> Option<Result<String, String>> {
    let first = request.lines().next()?;
    let mut words = first.split_whitespace();
    if words.next()? != "GET" {
        return None;
    }
    let target = words.next()?;
    if !target.starts_with('/')
        || target.starts_with("//")
        || !matches!(words.next(), Some("HTTP/1.1" | "HTTP/1.0"))
        || words.next().is_some()
    {
        return None;
    }
    let url = reqwest::Url::parse(&format!("http://127.0.0.1{target}")).ok()?;
    if url.path() != path {
        return None;
    }
    let parameters: Vec<_> = url.query_pairs().collect();
    let states: Vec<_> = parameters
        .iter()
        .filter(|(key, _)| key == "state")
        .collect();
    if states.len() != 1 || !same_state(&states[0].1, state) {
        return None;
    }
    if parameters.iter().any(|(key, _)| key == "error") {
        return Some(Err("Sign-in was declined".into()));
    }
    let codes: Vec<_> = parameters.iter().filter(|(key, _)| key == "code").collect();
    if codes.len() != 1
        || codes[0].1.is_empty()
        || codes[0].1.len() > 1024
        || codes[0].1.chars().any(char::is_control)
    {
        return None;
    }
    Some(Ok(codes[0].1.to_string()))
}

async fn listen_for_code(listener: TcpListener, path: &str, state: &str) -> Result<String, String> {
    loop {
        let (mut socket, _) = listener
            .accept()
            .await
            .map_err(|_| "Could not receive sign-in response")?;
        let result = tokio::time::timeout(std::time::Duration::from_secs(2), async {
            let mut request = Vec::new();
            let mut chunk = [0u8; 1024];
            while request.len() < 8192 && !request.windows(4).any(|bytes| bytes == b"\r\n\r\n") {
                let count = socket.read(&mut chunk).await.ok()?;
                if count == 0 { return None; }
                request.extend_from_slice(&chunk[..count]);
            }
            if request.len() > 8192 { return None; }
            let outcome = std::str::from_utf8(&request).ok().and_then(|request| callback_code(request, path, state));
            let body = if outcome.is_some() { "Return to Kiosque. Your sign-in is being verified." } else { "Invalid sign-in response." };
            let response = format!("HTTP/1.1 {}\r\nContent-Type: text/plain; charset=utf-8\r\nCache-Control: no-store\r\nContent-Security-Policy: default-src 'none'\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", if outcome.is_some() { "200 OK" } else { "400 Bad Request" }, body.len());
            let _ = socket.write_all(response.as_bytes()).await;
            outcome
        }).await;
        if let Ok(Some(outcome)) = result {
            return outcome;
        }
    }
}

fn open_browser(url: &str) -> Result<(), String> {
    std::process::Command::new("xdg-open")
        .arg(url)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map(|_| ())
        .map_err(|_| "Could not open your browser".into())
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: Option<String>,
    error: Option<String>,
}

async fn token_response(response: reqwest::Response) -> Result<TokenResponse, String> {
    let success = response.status().is_success();
    let body = zeroize::Zeroizing::new(
        super::client::read_limited(response, RESPONSE_LIMIT)
            .await
            .map_err(|_| "Could not read authentication response")?,
    );
    if !success {
        return Err("Authentication server rejected the request".into());
    }
    serde_json::from_slice(&body)
        .map_err(|_| "Authentication server returned an invalid response".into())
}

async fn github_device_token(
    prompt: impl Fn(DevicePrompt),
) -> Result<zeroize::Zeroizing<String>, String> {
    #[derive(Deserialize)]
    struct Device {
        device_code: String,
        user_code: String,
        verification_uri: String,
        expires_in: u64,
        interval: u64,
    }
    let client = super::client::shared_client();
    let response = client
        .post("https://github.com/login/device/code")
        .header("Accept", "application/json")
        .form(&[
            ("client_id", OAuthProvider::GitHub.client_id()),
            ("scope", "read:user".into()),
        ])
        .send()
        .await
        .map_err(|_| "Could not start GitHub sign-in")?;
    let success = response.status().is_success();
    let body = zeroize::Zeroizing::new(
        super::client::read_limited(response, RESPONSE_LIMIT)
            .await
            .map_err(|_| "Could not read GitHub sign-in response")?,
    );
    if !success {
        return Err("GitHub rejected device authorization".into());
    }
    let device: Device = serde_json::from_slice(&body).map_err(
        |_| "GitHub device authorization is unavailable; enable device flow for the OAuth app",
    )?;
    let uri = crate::security::https_url(&device.verification_uri)?;
    if uri.host_str() != Some("github.com")
        || uri.path() != "/login/device"
        || device.user_code.len() > 32
    {
        return Err("Invalid GitHub verification URL or code".into());
    }
    let code = zeroize::Zeroizing::new(device.device_code);
    prompt(DevicePrompt {
        user_code: device.user_code,
        verification_uri: device.verification_uri.clone(),
    });
    open_browser(&device.verification_uri)?;
    let deadline =
        tokio::time::Instant::now() + std::time::Duration::from_secs(device.expires_in.min(300));
    let mut interval = device.interval.clamp(5, 30);
    while tokio::time::Instant::now() < deadline {
        tokio::time::sleep(std::time::Duration::from_secs(interval)).await;
        let response = client
            .post(OAuthProvider::GitHub.token_url())
            .header("Accept", "application/json")
            .form(&[
                ("client_id", OAuthProvider::GitHub.client_id().as_str()),
                ("device_code", code.as_str()),
                ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
            ])
            .send()
            .await
            .map_err(|_| "Could not complete GitHub sign-in")?;
        let token = token_response(response).await?;
        if let Some(token) = token.access_token {
            return Ok(zeroize::Zeroizing::new(token));
        }
        match token.error.as_deref() {
            Some("authorization_pending") => {}
            Some("slow_down") => interval = (interval + 5).min(30),
            Some("access_denied") => return Err("GitHub sign-in was declined".into()),
            Some("device_flow_disabled") => {
                return Err("Enable device authorization in the GitHub OAuth application".into());
            }
            _ => return Err("GitHub sign-in failed or expired".into()),
        }
    }
    Err("GitHub sign-in expired".into())
}

pub async fn perform_login(provider: OAuthProvider) -> Result<(), String> {
    perform_login_with_prompt(provider, |_| {}).await
}

pub async fn perform_login_with_prompt(
    provider: OAuthProvider,
    prompt: impl Fn(DevicePrompt) + Send,
) -> Result<(), String> {
    tokio::time::timeout(LOGIN_TIMEOUT, login(provider, prompt))
        .await
        .map_err(|_| "Sign-in timed out".to_string())?
}

async fn login(provider: OAuthProvider, prompt: impl Fn(DevicePrompt)) -> Result<(), String> {
    let client_id = provider.client_id();
    if client_id.contains("placeholder") || client_id.is_empty() {
        return Err("This sign-in provider has not been configured".into());
    }
    let client = super::client::shared_client();
    let token = if matches!(provider, OAuthProvider::GitHub) {
        github_device_token(prompt).await?
    } else {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .map_err(|_| "Could not open a local sign-in listener")?;
        let port = listener
            .local_addr()
            .map_err(|_| "Could not determine the sign-in port")?
            .port();
        let path = "/oauth/callback".to_string();
        let redirect = format!("http://127.0.0.1:{port}{path}");
        let state = random_url_token();
        let verifier = zeroize::Zeroizing::new(random_url_token());
        let authorization =
            provider.authorization_url(&state, &redirect, &pkce_challenge(&verifier))?;
        open_browser(&authorization)?;
        let code = zeroize::Zeroizing::new(listen_for_code(listener, &path, &state).await?);
        let response = client
            .post(provider.token_url())
            .header("Accept", "application/json")
            .form(&[
                ("client_id", client_id.as_str()),
                ("code", code.as_str()),
                ("redirect_uri", redirect.as_str()),
                ("grant_type", "authorization_code"),
                ("code_verifier", verifier.as_str()),
            ])
            .send()
            .await
            .map_err(|_| "Could not exchange the authorization code")?;
        let token = token_response(response).await?;
        if token.error.is_some() {
            return Err("Authentication server rejected sign-in".into());
        }
        zeroize::Zeroizing::new(
            token
                .access_token
                .ok_or("Authentication response has no access token")?,
        )
    };
    let response = client
        .get(provider.user_profile_url())
        .bearer_auth(token.as_str())
        .send()
        .await
        .map_err(|_| "Could not verify your profile")?;
    let success = response.status().is_success();
    let body = super::client::read_limited(response, 256 * 1024)
        .await
        .map_err(|_| "Could not read the profile response")?;
    if !success {
        return Err("Profile verification was rejected".into());
    }
    #[derive(Deserialize)]
    struct Profile {
        id: i64,
        login: Option<String>,
        username: Option<String>,
        avatar_url: Option<String>,
    }
    let profile: Profile = serde_json::from_slice(&body).map_err(|_| "Invalid profile response")?;
    if profile.id <= 0 {
        return Err("Invalid profile identifier".into());
    }
    let mut settings = crate::settings::load_settings();
    settings.oauth_provider = Some(provider.as_str().into());
    settings.oauth_username = Some(
        profile
            .login
            .or(profile.username)
            .ok_or("Profile has no user name")?,
    );
    settings.oauth_user_id = Some(profile.id.to_string());
    settings.oauth_avatar_url = profile
        .avatar_url
        .filter(|url| crate::security::https_url(url).is_ok());
    // The provider token has completed its only purpose. It is never persisted.
    crate::settings::save_settings(&settings)
}

pub fn perform_logout() -> Result<(), String> {
    let mut settings = crate::settings::load_settings();
    settings.oauth_provider = None;
    settings.oauth_username = None;
    settings.oauth_user_id = None;
    settings.oauth_avatar_url = None;
    crate::settings::save_settings(&settings)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pkce_rfc7636_vector() {
        assert_eq!(
            pkce_challenge("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
    }
    #[test]
    fn callbacks_reject_duplicates_paths_and_invalid_state() {
        let path = "/oauth/callback/random";
        let request = |query| format!("GET {path}?{query} HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n");
        assert_eq!(
            callback_code(&request("code=abc%2B123&state=random"), path, "random"),
            Some(Ok("abc+123".into()))
        );
        for query in [
            "code=abc&state=wrong",
            "code=a&code=b&state=random",
            "code=a&state=random&state=random",
            "code=&state=random",
        ] {
            assert!(callback_code(&request(query), path, "random").is_none());
        }
        assert!(callback_code(&request("code=a&state=random"), "/different", "random").is_none());
    }
    #[tokio::test]
    async fn invalid_callback_does_not_end_the_login_attempt() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let task =
            tokio::spawn(async move { listen_for_code(listener, "/callback", "expected").await });
        for state in ["wrong", "expected"] {
            let mut socket = tokio::net::TcpStream::connect(address).await.unwrap();
            socket
                .write_all(
                    format!(
                        "GET /callback?code=code&state={state} HTTP/1.1\r\nHost: {address}\r\n\r\n"
                    )
                    .as_bytes(),
                )
                .await
                .unwrap();
            let mut response = Vec::new();
            socket.read_to_end(&mut response).await.unwrap();
        }
        assert_eq!(
            tokio::time::timeout(std::time::Duration::from_secs(3), task)
                .await
                .unwrap()
                .unwrap()
                .unwrap(),
            "code"
        );
    }
}
