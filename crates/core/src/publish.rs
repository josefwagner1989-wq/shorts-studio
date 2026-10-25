//! Publishing through official APIs only.
//!
//! - YouTube: Data API v3 resumable upload with OAuth (your own Google Cloud
//!   client, scope `youtube.upload`). Sets `containsSyntheticMedia` so the
//!   video carries YouTube's altered/synthetic content label.
//! - Outbox: copies the MP4 + caption into a folder for manual upload anywhere.

use crate::config::PublishSettings;
use crate::model::{Script, Video};
use crate::{Error, Result};
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};

pub trait Publisher {
    fn name(&self) -> &str;
    /// Returns the public link (or folder path) of the published video.
    fn publish(&self, video: &Video, script: &Script, file: &Path) -> Result<String>;
}

pub use crate::model::description;

/* ---------------- outbox ---------------- */

pub struct Outbox {
    pub dir: PathBuf,
}

impl Publisher for Outbox {
    fn name(&self) -> &str {
        "outbox"
    }
    fn publish(&self, video: &Video, script: &Script, file: &Path) -> Result<String> {
        std::fs::create_dir_all(&self.dir)?;
        let target = self.dir.join(format!("{}.mp4", video.id));
        std::fs::copy(file, &target)?;
        std::fs::write(self.dir.join(format!("{}.txt", video.id)), description(video, script))?;
        if let Some(srt) = video.srt_file.as_deref().filter(|p| Path::new(p).exists()) {
            std::fs::copy(srt, self.dir.join(format!("{}.srt", video.id)))?;
        }
        Ok(target.display().to_string())
    }
}

/* ---------------- YouTube ---------------- */

pub struct YouTube {
    pub client_id: String,
    pub client_secret: String,
    pub privacy: String,
    /// Where the OAuth refresh token is kept (0600). Move to libsecret before 1.0.
    pub token_file: PathBuf,
}

const AUTH_URL: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
/// Upload videos, and read the channel name and video statistics.
const SCOPE: &str = "https://www.googleapis.com/auth/youtube.upload https://www.googleapis.com/auth/youtube.readonly";

impl YouTube {
    pub fn from_settings(p: &PublishSettings, data_dir: &Path) -> Result<Self> {
        let secrets = crate::secrets::Secrets::load();
        let get = |name: &str, env: &str| {
            secrets.resolve(name, Some(env))
                .ok_or_else(|| Error::Config(format!("add the YouTube {name} in Preferences → Publishing")))
        };
        Ok(Self {
            client_id: get("youtube_client_id", &p.youtube_client_id_env)?,
            client_secret: get("youtube_client_secret", &p.youtube_client_secret_env)?,
            privacy: p.youtube_privacy.clone(),
            token_file: data_dir.join("youtube-token.json"),
        })
    }

    pub fn is_connected(&self) -> bool {
        self.token_file.exists()
    }

    /// Interactive one-time sign-in. `open_url` should open the system browser
    /// (in Flatpak, through the OpenURI portal). Blocks until Google redirects back.
    pub fn connect(&self, open_url: impl Fn(&str)) -> Result<()> {
        let listener = TcpListener::bind("127.0.0.1:0")?;
        let port = listener.local_addr()?.port();
        let redirect = format!("http://127.0.0.1:{port}");
        let state = format!("{:x}", crate::store::Store::now() ^ (port as u64) << 17);
        let url = reqwest::Url::parse_with_params(
            AUTH_URL,
            &[("client_id", self.client_id.as_str()), ("redirect_uri", &redirect), ("response_type", "code"),
              ("scope", SCOPE), ("access_type", "offline"), ("prompt", "consent"), ("state", &state)],
        )
        .map_err(|e| Error::Publish(e.to_string()))?;
        open_url(url.as_str());

        let (mut stream, _) = listener.accept()?;
        let mut line = String::new();
        BufReader::new(&stream).read_line(&mut line)?;
        let path = line.split_whitespace().nth(1).unwrap_or("");
        let params = reqwest::Url::parse(&format!("http://x{path}")).map_err(|e| Error::Publish(e.to_string()))?;
        let q = |k: &str| params.query_pairs().find(|(a, _)| a == k).map(|(_, v)| v.to_string());
        let ok = q("state").as_deref() == Some(state.as_str()) && q("code").is_some();
        let body = if ok { "Signed in. You can close this tab and return to Shorts Studio." } else { "Sign-in failed. Return to Shorts Studio and try again." };
        let _ = write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\n\r\n{body}", body.len());
        if !ok {
            return Err(Error::Publish(q("error").unwrap_or_else(|| "sign-in was cancelled".into())));
        }
        let tok: Value = reqwest::blocking::Client::new()
            .post(TOKEN_URL)
            .form(&[("code", q("code").unwrap()), ("client_id", self.client_id.clone()), ("client_secret", self.client_secret.clone()),
                    ("redirect_uri", redirect), ("grant_type", "authorization_code".into())])
            .send()?
            .json()?;
        let refresh = tok["refresh_token"].as_str().ok_or_else(|| Error::Publish(format!("no refresh token: {tok}")))?;
        if let Some(dir) = self.token_file.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(&self.token_file, json!({"refresh_token": refresh}).to_string())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&self.token_file, std::fs::Permissions::from_mode(0o600))?;
        }
        Ok(())
    }

    /// The connected channel: (name, link).
    pub fn channel(&self) -> Result<(String, String)> {
        let token = self.access_token()?;
        let v: Value = reqwest::blocking::Client::new()
            .get("https://www.googleapis.com/youtube/v3/channels?part=snippet&mine=true")
            .bearer_auth(token)
            .send()?
            .json()?;
        let item = &v["items"][0];
        let id = item["id"].as_str().ok_or_else(|| Error::Publish(format!("no channel on this Google account: {v}")))?;
        let name = item["snippet"]["title"].as_str().unwrap_or(id).to_string();
        Ok((name, format!("https://youtube.com/channel/{id}")))
    }

    /// View and like counts for uploaded videos, by video id.
    pub fn stats(&self, ids: &[String]) -> Result<Vec<(String, u64, u64)>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let token = self.access_token()?;
        let url = format!("https://www.googleapis.com/youtube/v3/videos?part=statistics&id={}", ids.join(","));
        let v: Value = reqwest::blocking::Client::new().get(url).bearer_auth(token).send()?.json()?;
        Ok(v["items"].as_array().into_iter().flatten().map(|i| {
            let n = |k: &str| i["statistics"][k].as_str().and_then(|x| x.parse().ok()).unwrap_or(0);
            (i["id"].as_str().unwrap_or_default().to_string(), n("viewCount"), n("likeCount"))
        }).collect())
    }

    fn access_token(&self) -> Result<String> {
        let saved: Value = serde_json::from_str(&std::fs::read_to_string(&self.token_file)
            .map_err(|_| Error::Publish("YouTube is not connected yet".into()))?)?;
        let refresh = saved["refresh_token"].as_str().unwrap_or_default().to_string();
        let tok: Value = reqwest::blocking::Client::new()
            .post(TOKEN_URL)
            .form(&[("refresh_token", refresh), ("client_id", self.client_id.clone()),
                    ("client_secret", self.client_secret.clone()), ("grant_type", "refresh_token".into())])
            .send()?
            .json()?;
        tok["access_token"].as_str().map(String::from)
            .ok_or_else(|| Error::Publish(format!("token refresh failed: {tok}")))
    }
}

impl Publisher for YouTube {
    fn name(&self) -> &str {
        "youtube"
    }

    fn publish(&self, video: &Video, script: &Script, file: &Path) -> Result<String> {
        let token = self.access_token()?;
        let title: String = video.idea.as_ref().map(|i| i.title.clone()).unwrap_or_else(|| "New short".into()).chars().take(100).collect();
        let meta = json!({
            "snippet": {"title": title, "description": description(video, script).chars().take(4900).collect::<String>(),
                        "tags": script.hashtags.iter().map(|h| h.trim_start_matches('#')).collect::<Vec<_>>(),
                        "categoryId": "27"},
            "status": {"privacyStatus": self.privacy, "selfDeclaredMadeForKids": false, "containsSyntheticMedia": true}
        });
        let http = reqwest::blocking::Client::builder().timeout(std::time::Duration::from_secs(900)).build()?;
        let init = http
            .post("https://www.googleapis.com/upload/youtube/v3/videos?uploadType=resumable&part=snippet,status")
            .bearer_auth(&token)
            .header("X-Upload-Content-Type", "video/mp4")
            .json(&meta)
            .send()?;
        if !init.status().is_success() {
            return Err(Error::Publish(format!("YouTube refused the upload: {}", init.text().unwrap_or_default())));
        }
        let location = init.headers().get("location").and_then(|l| l.to_str().ok())
            .ok_or_else(|| Error::Publish("YouTube did not return an upload URL".into()))?.to_string();
        let bytes = std::fs::read(file)?;
        let done: Value = http.put(location).bearer_auth(&token).header("Content-Type", "video/mp4").body(bytes).send()?.json()?;
        let id = done["id"].as_str().ok_or_else(|| Error::Publish(format!("upload finished without an id: {done}")))?;
        Ok(format!("https://youtube.com/shorts/{id}"))
    }
}

/// The publisher chosen in Preferences → Publishing.
pub fn from_settings(s: &crate::config::Settings) -> Result<Box<dyn Publisher>> {
    let data = s.data_path()?;
    match s.publish.target.as_str() {
        "youtube" => Ok(Box::new(YouTube::from_settings(&s.publish, &data)?)),
        _ => Ok(Box::new(Outbox { dir: data.join("outbox") })),
    }
}
