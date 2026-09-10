use super::*;
use aes_gcm::{aead::Aead, Aes256Gcm, KeyInit, Nonce};
use rand::RngCore;
impl LedgerService {
    pub(crate) fn transfer(&mut self, action: &str, p: Value) -> Result<Value, String> {
        match action {
            "exportJson" => {
                Ok(json!({"text":serde_json::to_string_pretty(&self.export_rows()?).map_err(err)?}))
            }
            "importJson" => {
                let rows: Vec<Value> = serde_json::from_str(s(&p, "text")).map_err(err)?;
                let mut results = vec![];
                for (index, mut row) in rows.into_iter().enumerate() {
                    use sha2::Digest;
                    row["captureId"] = json!(format!(
                        "json:{:x}:{index}",
                        sha2::Sha256::digest(s(&p, "text").as_bytes())
                    ));
                    results.push(self.import_row(row, index + 1));
                }
                Ok(json!(results))
            }
            "exportCsv" => {
                let mut w = csv::Writer::from_writer(vec![]);
                let fields = [
                    "id",
                    "amountMinor",
                    "currencyCode",
                    "kind",
                    "occurredAt",
                    "merchant",
                    "category",
                    "payerId",
                    "accountId",
                    "note",
                    "originalTransactionId",
                ];
                w.write_record(fields).map_err(err)?;
                for r in self.export_rows()? {
                    w.write_record(fields.map(|f| s(&r, f))).map_err(err)?;
                }
                Ok(json!({"text":String::from_utf8(w.into_inner().map_err(err)?).map_err(err)?}))
            }
            "importCsv" => {
                let mut reader = csv::Reader::from_reader(s(&p, "text").as_bytes());
                let headers = reader.headers().map_err(err)?.clone();
                for required in [
                    "amountMinor",
                    "currencyCode",
                    "kind",
                    "occurredAt",
                    "payerId",
                ] {
                    if !headers.iter().any(|h| h == required) {
                        return Err(format!("缺少列: {required}"));
                    }
                }
                let mut results = vec![];
                for (index, row) in reader.records().enumerate() {
                    let row = match row {
                        Ok(row) => row,
                        Err(e) => {
                            results.push(
                                json!({"status":"failed","row":index+1,"message":e.to_string()}),
                            );
                            continue;
                        }
                    };
                    let mut value = json!({});
                    for (h, v) in headers.iter().zip(row.iter()) {
                        if !v.is_empty() {
                            value[h] = json!(v);
                        }
                    } // 文件内容和行号共同标记，重试不会重复入账。
                    use sha2::Digest;
                    value["captureId"] = json!(format!(
                        "csv:{:x}:{index}",
                        sha2::Sha256::digest(s(&p, "text").as_bytes())
                    ));
                    results.push(self.import_row(value, index + 1));
                }
                Ok(json!(results))
            }
            "exportBackup" => {
                if s(&p, "password").len() < 8 {
                    return Err("备份密码至少8个字符".into());
                }
                let mut salt = [0u8; 16];
                let mut nonce = [0u8; 12];
                rand::thread_rng().fill_bytes(&mut salt);
                rand::thread_rng().fill_bytes(&mut nonce);
                let mut key = [0u8; 32];
                pbkdf2::pbkdf2_hmac::<sha2::Sha256>(
                    s(&p, "password").as_bytes(),
                    &salt,
                    600_000,
                    &mut key,
                );
                let mut tables = serde_json::Map::new();
                for table in [
                    "records",
                    "captures",
                    "pending",
                    "history",
                    "settings",
                    "aliases",
                    "field_versions",
                    "sources",
                ] {
                    let mut st = self
                        .db
                        .prepare(&format!("SELECT * FROM {table}"))
                        .map_err(err)?;
                    let cols = st.column_count();
                    let rows = st
                        .query_map([], |r| {
                            let mut cells = vec![];
                            for i in 0..cols {
                                cells.push(match r.get_ref(i)? {
                                    rusqlite::types::ValueRef::Integer(n) => json!(n),
                                    rusqlite::types::ValueRef::Text(t) => {
                                        json!(String::from_utf8_lossy(t))
                                    }
                                    _ => Value::Null,
                                });
                            }
                            Ok(cells)
                        })
                        .map_err(err)?
                        .collect::<Result<Vec<_>, _>>()
                        .map_err(err)?;
                    tables.insert(table.into(), json!(rows));
                }
                let clear = serde_json::to_vec(&tables).map_err(err)?;
                let encrypted = Aes256Gcm::new_from_slice(&key)
                    .map_err(err)?
                    .encrypt(Nonce::from_slice(&nonce), clear.as_ref())
                    .map_err(|_| "备份加密失败")?;
                let mut output = b"QACCOUNT1".to_vec();
                output.extend(salt);
                output.extend(nonce);
                output.extend(encrypted);
                let path = Path::new(s(&p, "path"));
                let mut file = std::fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(path)
                    .map_err(err)?;
                use std::io::Write;
                file.write_all(&output).map_err(err)?;
                file.sync_all().map_err(err)?;
                Ok(json!({"saved":true}))
            }
            "restoreBackup" => {
                let bytes = std::fs::read(s(&p, "path")).map_err(err)?;
                if bytes.len() < 53 || &bytes[..9] != b"QACCOUNT1" {
                    return Err("备份格式无效".into());
                }
                let mut key = [0u8; 32];
                pbkdf2::pbkdf2_hmac::<sha2::Sha256>(
                    s(&p, "password").as_bytes(),
                    &bytes[9..25],
                    600_000,
                    &mut key,
                );
                let clear = Aes256Gcm::new_from_slice(&key)
                    .map_err(err)?
                    .decrypt(Nonce::from_slice(&bytes[25..37]), &bytes[37..])
                    .map_err(|_| "密码错误或备份损坏")?;
                let tables: Value = serde_json::from_slice(&clear).map_err(err)?;
                for table in [
                    "records",
                    "captures",
                    "pending",
                    "history",
                    "settings",
                    "aliases",
                    "field_versions",
                    "sources",
                ] {
                    let rows = tables[table].as_array().ok_or("备份表缺失")?;
                    self.db
                        .execute(&format!("DELETE FROM {table}"), [])
                        .map_err(err)?;
                    for row in rows {
                        let cells = row.as_array().ok_or("备份行无效")?;
                        let sql = format!(
                            "INSERT INTO {table} VALUES({})",
                            vec!["?"; cells.len()].join(",")
                        );
                        let values = cells.iter().map(|v| {
                            if let Some(n) = v.as_i64() {
                                rusqlite::types::Value::Integer(n)
                            } else if let Some(t) = v.as_str() {
                                rusqlite::types::Value::Text(t.into())
                            } else {
                                rusqlite::types::Value::Null
                            }
                        });
                        self.db
                            .execute(&sql, rusqlite::params_from_iter(values))
                            .map_err(err)?;
                    }
                }
                self.db
                    .execute_batch("DELETE FROM outbox; DELETE FROM applied; DELETE FROM cursors; DELETE FROM sequences;")
                    .map_err(err)?;
                for r in self.rows()? {
                    self.validate(&r)?;
                }
                let mut identity = self.identity()?;
                identity["deviceId"] = json!(id());
                self.db
                    .execute(
                        "UPDATE settings SET data=? WHERE id='identity'",
                        [identity.to_string()],
                    )
                    .map_err(err)?;
                Ok(json!({"restored":true,"requiresPairing":true}))
            }
            _ => Err("不支持的数据操作".into()),
        }
    }
    fn import_row(&self, mut row: Value, index: usize) -> Value {
        if let Ok(existing) = self.record(s(&row, "id")) {
            return json!({"status":"existing","id":existing["id"],"row":index,"message":"账单ID已存在，保留已有数据"});
        }
        let result = (|| -> Result<Value, String> {
            self.db.execute_batch("SAVEPOINT import_row").map_err(err)?;
            for field in [
                "amountMinor",
                "currencyCode",
                "kind",
                "occurredAt",
                "payerId",
            ] {
                if s(&row, field).is_empty() {
                    return Err(format!("缺少字段{field}"));
                }
            }
            if row["id"].is_null() {
                row["id"] = json!(id());
            }
            self.create(row, false)
        })();
        match result {
            Ok(mut value) => {
                if let Err(e) = self.db.execute_batch("RELEASE import_row") {
                    return json!({"status":"failed","row":index,"message":e.to_string()});
                }
                value["row"] = json!(index);
                value
            }
            Err(e) => {
                let _ = self
                    .db
                    .execute_batch("ROLLBACK TO import_row; RELEASE import_row;");
                json!({"status":"failed","row":index,"message":e})
            }
        }
    }
    fn export_rows(&self) -> Result<Vec<Value>, String> {
        let member = self.identity()?;
        Ok(self
            .rows()?
            .into_iter()
            .filter(|r| r["ownerId"] == member["memberId"])
            .collect())
    }
}
