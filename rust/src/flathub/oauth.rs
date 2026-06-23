use serde::{Deserialize, Serialize};
use tokio::net::TcpListener;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

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

    pub fn client_secret(&self) -> Option<String> {
        match self {
            Self::GitHub => Some(std::env::var("KIOSQUE_GITHUB_CLIENT_SECRET")
                .unwrap_or_else(|_| "dummy_secret_placeholder".to_string())),
            Self::GitLab => std::env::var("KIOSQUE_GITLAB_CLIENT_SECRET").ok(),
            Self::GnomeGitLab => std::env::var("KIOSQUE_GNOME_CLIENT_SECRET").ok(),
            Self::KdeGitLab => std::env::var("KIOSQUE_KDE_CLIENT_SECRET").ok(),
        }
    }

    pub fn auth_url(&self, state: &str, redirect_uri: &str) -> String {
        match self {
            Self::GitHub => format!(
                "https://github.com/login/oauth/authorize?client_id={}&redirect_uri={}&state={}&scope=read:user",
                self.client_id(), redirect_uri, state
            ),
            Self::GitLab => format!(
                "https://gitlab.com/oauth/authorize?client_id={}&redirect_uri={}&response_type=code&state={}&scope=read_user",
                self.client_id(), redirect_uri, state
            ),
            Self::GnomeGitLab => format!(
                "https://gitlab.gnome.org/oauth/authorize?client_id={}&redirect_uri={}&response_type=code&state={}&scope=read_user",
                self.client_id(), redirect_uri, state
            ),
            Self::KdeGitLab => format!(
                "https://invent.kde.org/oauth/authorize?client_id={}&redirect_uri={}&response_type=code&state={}&scope=read_user",
                self.client_id(), redirect_uri, state
            ),
        }
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

async fn listen_for_code(port: u16, expected_state: &str) -> Result<String, String> {
    let listener = TcpListener::bind(format!("127.0.0.1:{}", port))
        .await
        .map_err(|e| format!("Failed to bind to local port: {}", e))?;
        
    loop {
        let (mut socket, _) = match listener.accept().await {
            Ok(s) => s,
            Err(_) => continue,
        };
        
        let mut buffer = [0; 2048];
        let n = match socket.read(&mut buffer).await {
            Ok(n) if n > 0 => n,
            _ => continue,
        };
        
        let request = String::from_utf8_lossy(&buffer[..n]);
        let first_line = match request.lines().next() {
            Some(line) => line,
            None => continue,
        };
        
        // If the browser pre-connected or requested something without oauth query params,
        // send a 404 (e.g. for /favicon.ico) and continue the loop.
        if !first_line.contains("code=") || !first_line.contains("state=") {
            let response = "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
            let _ = socket.write_all(response.as_bytes()).await;
            let _ = socket.flush().await;
            continue;
        }

        let mut req_code = None;
        let mut req_state = None;

        if let Some(query_start) = first_line.find('?') {
            if let Some(query_end) = first_line[query_start..].find(' ') {
                let query = &first_line[query_start + 1 .. query_start + query_end];
                
                for part in query.split('&') {
                    let mut kv = part.splitn(2, '=');
                    if let (Some(k), Some(v)) = (kv.next(), kv.next()) {
                        if k == "code" {
                            req_code = Some(v.to_string());
                        } else if k == "state" {
                            req_state = Some(v.to_string());
                        }
                    }
                }
            }
        }
        
        let is_valid = req_state.as_deref() == Some(expected_state) && req_code.is_some();
        
        let response_body = if is_valid {
            r#"
            <!DOCTYPE html>
            <html>
            <head><title>Kiosque Login Successful</title></head>
            <body style="font-family: sans-serif; text-align: center; margin-top: 50px; background-color: #f6f8fa;">
                <div style="display: inline-block; padding: 30px; background: white; border-radius: 8px; box-shadow: 0 4px 12px rgba(0,0,0,0.1);">
                    <h2 style="color: #2da44e;">Login Successful!</h2>
                    <p>You have successfully authenticated with ODRS.</p>
                    <p>You can now close this tab and return to the Kiosque storefront application.</p>
                </div>
            </body>
            </html>
            "#
        } else {
            r#"
            <!DOCTYPE html>
            <html>
            <head><title>Kiosque Login Failed</title></head>
            <body style="font-family: sans-serif; text-align: center; margin-top: 50px; background-color: #f6f8fa;">
                <div style="display: inline-block; padding: 30px; background: white; border-radius: 8px; box-shadow: 0 4px 12px rgba(0,0,0,0.1);">
                    <h2 style="color: #cf222e;">Login Failed</h2>
                    <p>Authentication failed or the verification state was invalid.</p>
                    <p>Please try logging in again from Kiosque.</p>
                </div>
            </body>
            </html>
            "#
        };
        
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            response_body.len(),
            response_body
        );
        let _ = socket.write_all(response.as_bytes()).await;
        let _ = socket.flush().await;
        
        if is_valid {
            return Ok(req_code.unwrap());
        } else {
            return Err("State mismatch or missing auth code".to_string());
        }
    }
}

fn open_browser(url: &str) {
    let _ = std::process::Command::new("xdg-open")
        .arg(url)
        .spawn();
}

#[derive(Deserialize)]
struct GitHubUser {
    id: i64,
    login: String,
    avatar_url: Option<String>,
}

#[derive(Deserialize)]
struct GitLabUser {
    id: i64,
    username: String,
    avatar_url: Option<String>,
}

pub async fn perform_login(provider: OAuthProvider) -> Result<(), String> {
    let port = 43210;
    let redirect_uri = format!("http://localhost:{}/", port);
    
    let state_bytes: [u8; 16] = rand::random();
    let state: String = state_bytes.iter().map(|b| format!("{:02x}", b)).collect();
    
    let auth_url = provider.auth_url(&state, &redirect_uri);
    
    let server_fut = listen_for_code(port, &state);
    
    open_browser(&auth_url);
    
    let code = server_fut.await?;
    
    let client = super::client::shared_client();
    
    let mut params = std::collections::HashMap::new();
    params.insert("client_id", provider.client_id().to_string());
    params.insert("code", code);
    params.insert("redirect_uri", redirect_uri);
    params.insert("state", state);
    if let Some(secret) = provider.client_secret() {
        if secret == "dummy_secret_placeholder" {
            return Err("GitHub Client Secret is not set. Please export the KIOSQUE_GITHUB_CLIENT_SECRET environment variable before starting Kiosque (e.g. export KIOSQUE_GITHUB_CLIENT_SECRET=\"your_actual_secret\").".to_string());
        }
        params.insert("client_secret", secret.to_string());
    }
    if !matches!(provider, OAuthProvider::GitHub) {
        params.insert("grant_type", "authorization_code".to_string());
    }

    let token_resp = client.post(provider.token_url())
        .header("Accept", "application/json")
        .form(&params)
        .send()
        .await
        .map_err(|e| format!("Failed to send token request: {}", e))?;

    let token_status = token_resp.status();
    let token_body = token_resp.text().await
        .map_err(|e| format!("Failed to read token response: {}", e))?;

    if !token_status.is_success() {
        return Err(format!("Token server returned error status {}: {}", token_status.as_u16(), token_body));
    }

    #[derive(Deserialize)]
    struct TokenResponse {
        access_token: Option<String>,
        error: Option<String>,
        error_description: Option<String>,
    }

    let token_data: TokenResponse = serde_json::from_str(&token_body)
        .map_err(|e| format!("Failed to parse token JSON: {}\nResponse: {}", e, token_body))?;
    
    if let Some(err) = token_data.error {
        let desc = token_data.error_description.unwrap_or_default();
        return Err(format!("OAuth exchange failed: {} - {}", err, desc));
    }

    let access_token = token_data.access_token
        .ok_or_else(|| "Missing access_token in successful response".to_string())?;

    let profile_resp = client.get(provider.user_profile_url())
        .bearer_auth(&access_token)
        .send()
        .await
        .map_err(|e| format!("Failed to fetch user profile: {}", e))?;

    let profile_status = profile_resp.status();
    let profile_body = profile_resp.text().await
        .map_err(|e| format!("Failed to read profile response: {}", e))?;

    if !profile_status.is_success() {
        return Err(format!("User profile request failed with status {}: {}", profile_status.as_u16(), profile_body));
    }

    let (user_id, username, avatar_url) = match provider {
        OAuthProvider::GitHub => {
            let user: GitHubUser = serde_json::from_str(&profile_body)
                .map_err(|e| format!("Failed to parse GitHub profile: {}", e))?;
            (user.id.to_string(), user.login, user.avatar_url)
        }
        _ => {
            let user: GitLabUser = serde_json::from_str(&profile_body)
                .map_err(|e| format!("Failed to parse GitLab profile: {}", e))?;
            (user.id.to_string(), user.username, user.avatar_url)
        }
    };

    let mut settings = crate::settings::load_settings();
    settings.oauth_provider = Some(provider.as_str().to_string());
    settings.oauth_username = Some(username);
    settings.oauth_user_id = Some(user_id);
    settings.oauth_token = Some(access_token);
    settings.oauth_avatar_url = avatar_url;
    
    crate::settings::save_settings(&settings)
        .map_err(|e| format!("Failed to save settings: {}", e))?;

    Ok(())
}

pub fn perform_logout() -> Result<(), String> {
    let mut settings = crate::settings::load_settings();
    settings.oauth_provider = None;
    settings.oauth_username = None;
    settings.oauth_user_id = None;
    settings.oauth_token = None;
    settings.oauth_avatar_url = None;
    crate::settings::save_settings(&settings)?;
    Ok(())
}
