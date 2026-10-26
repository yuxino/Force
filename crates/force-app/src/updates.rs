use std::{io::Read as _, time::Duration};

use anyhow::{Context as _, ensure};
use gpui::Context;
use semver::Version;
use serde::Deserialize;

use crate::model::ForceView;

pub(crate) const RELEASES_URL: &str = "https://github.com/yuxino/Force/releases/latest";
const API_URL: &str = "https://api.github.com/repos/yuxino/Force/releases/latest";
const MAX_RESPONSE: u64 = 1024 * 1024;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) enum UpdateState {
    #[default]
    Idle,
    Checking,
    Current,
    NotPublished,
    Available {
        version: String,
        download: Option<String>,
    },
    Failed,
}

impl UpdateState {
    pub(crate) fn label(&self) -> String {
        match self {
            Self::Checking => "正在检查…".into(),
            Self::Available {
                version,
                download: Some(_),
            } => format!("下载 v{version}"),
            Self::Available { download: None, .. } => "查看新版".into(),
            Self::Failed => "重试更新检查".into(),
            _ => "检查更新".into(),
        }
    }

    pub(crate) fn message(&self) -> Option<&'static str> {
        match self {
            Self::Current => Some("已是最新版本"),
            Self::NotPublished => Some("暂未发布新版本"),
            Self::Available {
                download: Some(_), ..
            } => Some("下载后安装即可更新，任务和记录会保留。"),
            Self::Available { download: None, .. } => {
                Some("发现新版本，当前系统的安装包尚未就绪。")
            }
            Self::Failed => Some("暂时无法连接 GitHub，请检查网络后重试。"),
            _ => None,
        }
    }
}

#[derive(Deserialize)]
struct Release {
    tag_name: String,
    draft: bool,
    prerelease: bool,
    assets: Vec<Asset>,
}

#[derive(Deserialize)]
struct Asset {
    name: String,
    browser_download_url: String,
    size: u64,
    state: String,
}

fn asset_name(version: &str, os: &str, arch: &str) -> Option<String> {
    let extension = match (os, arch) {
        ("macos", "aarch64" | "x86_64") => "dmg",
        ("windows", "x86_64") => "zip",
        ("linux", "x86_64") => "tar.gz",
        _ => return None,
    };
    Some(format!("Force-v{version}-{os}-{arch}.{extension}"))
}

fn parse_release(body: &[u8], current: &str, os: &str, arch: &str) -> anyhow::Result<UpdateState> {
    let release: Release = serde_json::from_slice(body)?;
    let version = Version::parse(
        release
            .tag_name
            .strip_prefix('v')
            .context("Missing release prefix")?,
    )?;
    if release.draft || release.prerelease || !version.pre.is_empty() {
        return Ok(UpdateState::NotPublished);
    }
    if version <= Version::parse(current)? {
        return Ok(UpdateState::Current);
    }
    let version = version.to_string();
    // A release response must never turn the update button into an arbitrary
    // external link. Match our exact repository, tag and platform asset name.
    let download = asset_name(&version, os, arch).and_then(|name| {
        let expected =
            format!("https://github.com/yuxino/Force/releases/download/v{version}/{name}");
        release.assets.into_iter().find_map(|asset| {
            (asset.name == name
                && asset.browser_download_url == expected
                && asset.size > 0
                && asset.state == "uploaded")
                .then_some(expected.clone())
        })
    });
    Ok(UpdateState::Available { version, download })
}

fn fetch_latest(current: &str, os: &str, arch: &str) -> anyhow::Result<UpdateState> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(15))
        .connect_timeout(Duration::from_secs(8))
        .https_only(true)
        .redirect(reqwest::redirect::Policy::none())
        .user_agent(concat!("Force/", env!("CARGO_PKG_VERSION")))
        .build()?;
    let response = client
        .get(API_URL)
        .header("Accept", "application/vnd.github+json")
        .send()?;
    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(UpdateState::NotPublished);
    }
    let response = response.error_for_status()?;
    let mut body = Vec::new();
    response.take(MAX_RESPONSE + 1).read_to_end(&mut body)?;
    ensure!(
        body.len() as u64 <= MAX_RESPONSE,
        "Release response is too large"
    );
    parse_release(&body, current, os, arch)
}

impl ForceView {
    pub(crate) fn check_for_updates(&mut self, cx: &mut Context<Self>) {
        if self.update_state == UpdateState::Checking {
            return;
        }
        self.update_state = UpdateState::Checking;
        cx.notify();
        // The request is explicit, bounded and runs off the UI thread. No task,
        // record, local path or account information is sent to GitHub.
        let request = cx.background_executor().spawn(async {
            fetch_latest(
                env!("CARGO_PKG_VERSION"),
                std::env::consts::OS,
                std::env::consts::ARCH,
            )
        });
        cx.spawn(async move |this, cx| {
            let result = request.await;
            let _ = this.update(cx, |this, cx| {
                this.update_state = match result {
                    Ok(state) => state,
                    Err(error) => {
                        eprintln!("Force update check failed: {error:#}");
                        UpdateState::Failed
                    }
                };
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn update_action(&mut self, cx: &mut Context<Self>) {
        match &self.update_state {
            UpdateState::Available { download, .. } => {
                cx.open_url(download.as_deref().unwrap_or(RELEASES_URL));
            }
            _ => self.check_for_updates(cx),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn release(tag: &str, os: &str, arch: &str) -> serde_json::Value {
        let version = tag.trim_start_matches('v');
        let name = asset_name(version, os, arch).unwrap();
        json!({"tag_name":tag,"draft":false,"prerelease":false,"assets":[{
            "name":name,"browser_download_url":format!("https://github.com/yuxino/Force/releases/download/{tag}/{name}"),
            "size":1024,"state":"uploaded"
        }]})
    }

    #[test]
    fn semantic_versions_and_each_platform_select_the_exact_installer() {
        for (os, arch) in [
            ("macos", "aarch64"),
            ("macos", "x86_64"),
            ("windows", "x86_64"),
            ("linux", "x86_64"),
        ] {
            let data = release("v0.10.0", os, arch);
            let state =
                parse_release(&serde_json::to_vec(&data).unwrap(), "0.9.9", os, arch).unwrap();
            assert!(matches!(
                state,
                UpdateState::Available {
                    download: Some(_),
                    ..
                }
            ));
            assert_eq!(
                parse_release(&serde_json::to_vec(&data).unwrap(), "0.10.0", os, arch).unwrap(),
                UpdateState::Current
            );
            assert_eq!(
                parse_release(&serde_json::to_vec(&data).unwrap(), "0.11.0", os, arch).unwrap(),
                UpdateState::Current
            );
        }
    }

    #[test]
    fn draft_preview_incomplete_or_foreign_assets_cannot_offer_a_download() {
        for field in ["draft", "prerelease"] {
            let mut data = release("v0.2.0", "macos", "aarch64");
            data[field] = json!(true);
            assert_eq!(
                parse_release(
                    &serde_json::to_vec(&data).unwrap(),
                    "0.1.0",
                    "macos",
                    "aarch64"
                )
                .unwrap(),
                UpdateState::NotPublished
            );
        }
        for url in [
            "https://evil.example/Force.dmg",
            "http://github.com/yuxino/Force/releases/download/v0.2.0/Force-v0.2.0-macos-aarch64.dmg",
            "https://github.com/other/Force/releases/download/v0.2.0/Force-v0.2.0-macos-aarch64.dmg",
        ] {
            let mut data = release("v0.2.0", "macos", "aarch64");
            data["assets"][0]["browser_download_url"] = json!(url);
            assert!(matches!(
                parse_release(
                    &serde_json::to_vec(&data).unwrap(),
                    "0.1.0",
                    "macos",
                    "aarch64"
                )
                .unwrap(),
                UpdateState::Available { download: None, .. }
            ));
        }
        let mut data = release("v0.2.0", "macos", "aarch64");
        data["assets"][0]["size"] = json!(0);
        assert!(matches!(
            parse_release(
                &serde_json::to_vec(&data).unwrap(),
                "0.1.0",
                "macos",
                "aarch64"
            )
            .unwrap(),
            UpdateState::Available { download: None, .. }
        ));
        assert!(matches!(
            parse_release(
                &serde_json::to_vec(&release("v0.2.0", "macos", "x86_64")).unwrap(),
                "0.1.0",
                "macos",
                "aarch64"
            )
            .unwrap(),
            UpdateState::Available { download: None, .. }
        ));
        assert!(parse_release(b"not json", "0.1.0", "macos", "aarch64").is_err());
    }
}
