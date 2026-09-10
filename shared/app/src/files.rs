#[cfg(any(windows, target_os = "android"))]
use lightledger_core::LedgerService;
use serde_json::{Value, json};
use std::path::Path;

pub fn read_text(path: &str) -> Result<Value, String> {
    #[cfg(target_os = "android")]
    return tauri_plugin_device::read_text(path);
    #[cfg(not(target_os = "android"))]
    {
        use std::io::Read;
        let mut bytes = Vec::new();
        std::fs::File::open(path)
            .map_err(|e| e.to_string())?
            .take(20 * 1024 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        if bytes.len() > 20 * 1024 * 1024 {
            return Err("导入文件不能超过20MB".into());
        }
        Ok(
            json!({"text":String::from_utf8(bytes).map_err(|e|e.to_string())?,
            "name":Path::new(path).file_name().and_then(|v|v.to_str()).unwrap_or("")}),
        )
    }
}

pub fn write_text(path: &str, text: &str) -> Result<(), String> {
    #[cfg(target_os = "android")]
    return tauri_plugin_device::write_text(path, text);
    #[cfg(not(target_os = "android"))]
    std::fs::write(path, text).map_err(|e| e.to_string())
}

// 只暂存已加密的备份；独立目录在成功和失败时都会清理。
#[cfg(any(target_os = "android", test))]
struct BackupStage(std::path::PathBuf);
#[cfg(any(target_os = "android", test))]
impl BackupStage {
    fn new(dir: &Path) -> Result<Self, String> {
        let root = dir.join("backup-staging");
        std::fs::create_dir_all(&root).map_err(|e| e.to_string())?;
        let path = root.join(format!("{:016x}", rand::random::<u64>()));
        std::fs::create_dir(&path).map_err(|e| e.to_string())?;
        Ok(Self(path))
    }
    fn path(&self) -> std::path::PathBuf {
        self.0.join("backup.qaccount")
    }
}
#[cfg(any(target_os = "android", test))]
impl Drop for BackupStage {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[cfg(any(windows, target_os = "android"))]
pub fn dispatch(
    ledger: &mut LedgerService,
    action: &str,
    payload: Value,
    _dir: &Path,
) -> Result<Value, String> {
    #[cfg(target_os = "android")]
    if matches!(action, "exportBackup" | "restoreBackup") {
        let target = payload["path"]
            .as_str()
            .filter(|p| !p.is_empty())
            .ok_or("请选择备份文件")?
            .to_owned();
        let stage = BackupStage::new(_dir)?;
        let path = stage.path();
        let local = path.to_str().ok_or("备份临时路径无效")?;
        if action == "restoreBackup" {
            tauri_plugin_device::copy(&target, local)?;
        }
        let mut payload = payload;
        payload["path"] = json!(local);
        let result = ledger.dispatch(action, payload)?;
        if action == "exportBackup" {
            tauri_plugin_device::copy(local, &target)?;
        }
        return Ok(result);
    }
    ledger.dispatch(action, payload)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn files_and_backup_staging() {
        let dir =
            std::env::temp_dir().join(format!("lightledger-files-{:016x}", rand::random::<u64>()));
        std::fs::create_dir(&dir).unwrap();
        let file = dir.join("账单.csv");
        write_text(file.to_str().unwrap(), "金额,备注\n12,午餐").unwrap();
        let read = read_text(file.to_str().unwrap()).unwrap();
        assert_eq!(read["name"], "账单.csv");
        assert_eq!(read["text"], "金额,备注\n12,午餐");
        std::fs::write(&file, [0xff]).unwrap();
        assert!(read_text(file.to_str().unwrap()).is_err());
        let stage = BackupStage::new(&dir).unwrap();
        let path = stage.path();
        std::fs::write(&path, b"encrypted backup").unwrap();
        drop(stage);
        assert!(!path.exists());
        std::fs::remove_dir_all(dir).unwrap();
    }
}
