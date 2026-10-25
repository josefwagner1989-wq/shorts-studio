//! One JSON file per video under `<data>/videos/`. Simple, inspectable, git-friendly.

use crate::model::{Stage, Video};
use crate::Result;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub struct Store {
    root: PathBuf,
}

impl Store {
    pub fn open(data_dir: &Path) -> Result<Self> {
        std::fs::create_dir_all(data_dir.join("videos"))?;
        Ok(Self { root: data_dir.to_path_buf() })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn video_dir(&self, id: &str) -> PathBuf {
        self.root.join("media").join(id)
    }

    pub fn new_id() -> String {
        let t = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
        format!("v{}{:03}", t.as_secs(), t.subsec_millis())
    }

    pub fn now() -> u64 {
        SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
    }

    pub fn save(&self, v: &Video) -> Result<()> {
        let path = self.root.join("videos").join(format!("{}.json", v.id));
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_string_pretty(v)?)?;
        std::fs::rename(tmp, path)?;
        Ok(())
    }

    pub fn load(&self, id: &str) -> Result<Video> {
        Ok(serde_json::from_str(&std::fs::read_to_string(self.root.join("videos").join(format!("{id}.json")))?)?)
    }

    pub fn list(&self) -> Result<Vec<Video>> {
        let mut out = Vec::new();
        for e in std::fs::read_dir(self.root.join("videos"))? {
            let p = e?.path();
            if p.extension().is_some_and(|x| x == "json") {
                if let Ok(v) = serde_json::from_str::<Video>(&std::fs::read_to_string(&p)?) {
                    out.push(v);
                }
            }
        }
        out.sort_by_key(|v| v.created_at);
        Ok(out)
    }

    pub fn by_stage(&self, stage: Stage) -> Result<Vec<Video>> {
        Ok(self.list()?.into_iter().filter(|v| v.stage == stage).collect())
    }

    pub fn delete(&self, id: &str) -> Result<()> {
        let _ = std::fs::remove_dir_all(self.video_dir(id));
        std::fs::remove_file(self.root.join("videos").join(format!("{id}.json")))?;
        Ok(())
    }
}
