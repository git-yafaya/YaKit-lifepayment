use super::*;
use aes_gcm::{aead::Aead, Aes256Gcm, KeyInit, Nonce};
use rand::RngCore;

impl LedgerService {
    pub(crate) fn backup(&mut self, action: &str, p: Value) -> Result<Value, String> {
        match action {
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
                    "applied",
                    "cursors",
                    "outbox",
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
                let independent = ["applied", "cursors", "outbox"]
                    .iter()
                    .any(|table| tables[*table].is_null());
                for table in [
                    "records",
                    "captures",
                    "pending",
                    "history",
                    "settings",
                    "aliases",
                    "field_versions",
                    "sources",
                    "applied",
                    "cursors",
                    "outbox",
                ] {
                    // 旧备份没有同步水位，仍允许恢复原有账本内容。
                    let empty = vec![];
                    let rows = if ["applied", "cursors", "outbox"].contains(&table)
                        && tables[table].is_null()
                    {
                        &empty
                    } else {
                        tables[table].as_array().ok_or("备份表缺失")?
                    };
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
                    .execute_batch("DELETE FROM sequences;")
                    .map_err(err)?;
                for r in self.rows()? {
                    self.validate(&r)?;
                }
                let mut identity = self.identity()?;
                identity["deviceId"] = json!(id());
                if independent {
                    // 旧备份缺少历史水位，独立空间避免把远端旧操作覆盖到恢复内容上。
                    identity["personalSpaceId"] = json!(id());
                    identity["sharedSpaceId"] = json!(id());
                    self.db.execute_batch("DELETE FROM field_versions; DELETE FROM applied; DELETE FROM cursors; DELETE FROM outbox;").map_err(err)?;
                    self.db
                        .execute(
                            "INSERT OR REPLACE INTO settings VALUES('independentRecovery','true')",
                            [],
                        )
                        .map_err(err)?;
                }
                self.db
                    .execute(
                        "UPDATE settings SET data=? WHERE id='identity'",
                        [identity.to_string()],
                    )
                    .map_err(err)?;
                if independent {
                    self.queue_independent_snapshot(&identity)?;
                } else {
                    self.restore_outbox(&identity)?;
                }
                Ok(json!({"restored":true,"requiresPairing":true,"independentLedger":independent}))
            }
            _ => Err("不支持的备份操作".into()),
        }
    }
    fn queue_independent_snapshot(&self, identity: &Value) -> Result<(), String> {
        let space = s(identity, "personalSpaceId");
        let rows = self.rows()?;
        let mut record_ids: std::collections::BTreeSet<String> = rows
            .iter()
            .filter(|row| row["ownerId"] == identity["memberId"])
            .map(|row| s(row, "id").to_string())
            .collect();
        // 本人的退款可能关联共同账单；保留其原交易，另一台个人设备才能校验引用。
        for row in rows
            .iter()
            .filter(|row| row["ownerId"] == identity["memberId"] && s(row, "kind") == "refund")
        {
            if let Ok(original) = self.record(s(row, "originalTransactionId")) {
                record_ids.insert(s(&original, "id").to_string());
            }
        }
        for row in rows
            .iter()
            .filter(|row| record_ids.contains(s(row, "id")) && s(row, "kind") != "refund")
        {
            self.queue(row, space, &Value::Null)?;
        }
        // 先发布归并引用，再发布退款；退款可能仍使用归并前的原交易编号。
        let aliases = {
            let mut statement = self
                .db
                .prepare("SELECT id,target FROM aliases")
                .map_err(err)?;
            let rows = statement
                .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
                .map_err(err)?;
            rows.collect::<Result<Vec<_>, _>>().map_err(err)?
        };
        for (key, target) in aliases {
            if record_ids.contains(&target) {
                self.queue(
                    &json!({"id":key,"entityType":"alias","targetId":target}),
                    space,
                    &Value::Null,
                )?;
            }
        }
        for row in rows
            .iter()
            .filter(|row| record_ids.contains(s(row, "id")) && s(row, "kind") == "refund")
        {
            self.queue(row, space, &Value::Null)?;
        }
        let settings = {
            let mut statement = self.db.prepare("SELECT id,data FROM settings WHERE id LIKE 'account:%' OR id LIKE 'category:%' OR id LIKE 'rule:%' OR id LIKE 'merge:%'").map_err(err)?;
            let rows = statement
                .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
                .map_err(err)?;
            rows.collect::<Result<Vec<_>, _>>().map_err(err)?
        };
        for (key, text) in settings {
            let value: Value = serde_json::from_str(&text).map_err(err)?;
            if !key.starts_with("account:") || value["ownerId"] == identity["memberId"] {
                self.queue(
                    &json!({"id":key,"entityType":"setting","value":value}),
                    space,
                    &Value::Null,
                )?;
            }
        }
        Ok(())
    }

    fn restore_outbox(&self, identity: &Value) -> Result<(), String> {
        let operations = {
            let mut statement = self
                .db
                .prepare("SELECT payload,uploaded FROM outbox ORDER BY rowid")
                .map_err(err)?;
            let rows = statement
                .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))
                .map_err(err)?;
            rows.collect::<Result<Vec<_>, _>>().map_err(err)?
        };
        self.db.execute("DELETE FROM outbox", []).map_err(err)?;
        for (text, uploaded) in operations {
            let mut operation: Value = serde_json::from_str(&text).map_err(err)?;
            let space = s(&operation, "spaceId").to_string();
            let cursor = format!("{space}:{}", s(&operation, "deviceId"));
            let sequence = operation["sequence"].as_i64().ok_or("备份操作缺少序号")?;
            // 备份已经包含这些旧设备操作，重新配对时直接从后续历史继续。
            self.db.execute("INSERT INTO cursors VALUES(?,?) ON CONFLICT(device) DO UPDATE SET sequence=MAX(sequence,excluded.sequence)", params![cursor,sequence]).map_err(err)?;
            self.db
                .execute(
                    "INSERT OR IGNORE INTO applied VALUES(?)",
                    [s(&operation, "id")],
                )
                .map_err(err)?;
            if uploaded != 0 {
                continue;
            }
            // 保留操作标识和因果依赖，只为新设备重新安排连续发送序号。
            self.db.execute("INSERT INTO sequences VALUES(?,1) ON CONFLICT(space) DO UPDATE SET sequence=sequence+1", [&space]).map_err(err)?;
            let sequence: i64 = self
                .db
                .query_row(
                    "SELECT sequence FROM sequences WHERE space=?",
                    [&space],
                    |r| r.get(0),
                )
                .map_err(err)?;
            operation["deviceId"] = identity["deviceId"].clone();
            operation["sequence"] = json!(sequence);
            self.db
                .execute(
                    "INSERT INTO outbox(id,space,entity,payload) VALUES(?,?,?,?)",
                    params![
                        s(&operation, "id"),
                        space,
                        s(&operation, "entityId"),
                        operation.to_string()
                    ],
                )
                .map_err(err)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod checks {
    use super::*;

    #[test]
    fn legacy_backup_creates_an_independent_syncable_ledger() {
        let mut source = LedgerService::open(Path::new(":memory:"), &[1; 32]).unwrap();
        let original_identity = source.identity().unwrap();
        let account = source
            .dispatch("addAccount", json!({"name":"原账户"}))
            .unwrap();
        source
            .dispatch("addCategory", json!({"name":"原分类"}))
            .unwrap();
        source
            .dispatch("learnRule", json!({"match":"测试来源","category":"原分类"}))
            .unwrap();
        let expense = json!({"id":"original","amountMinor":"1000","currencyCode":"CNY","kind":"expense","occurredAt":"2026-09-10T12:00:00Z","merchant":"原交易","payerId":"local","accountId":account["id"],"note":"个人备注"});
        source.dispatch("create", expense.clone()).unwrap();
        let mut duplicate = expense.clone();
        duplicate["id"] = json!("old-reference");
        duplicate["merchant"] = json!("归并来源");
        source.dispatch("create", duplicate).unwrap();
        source
            .dispatch(
                "mergeTransactions",
                json!({"sourceId":"old-reference","targetId":"original"}),
            )
            .unwrap();
        let mut refund = expense;
        refund["id"] = json!("refund");
        refund["amountMinor"] = json!("200");
        refund["kind"] = json!("refund");
        refund["merchant"] = json!("退款");
        refund["originalTransactionId"] = json!("old-reference");
        source.dispatch("create", refund).unwrap();
        source.dispatch("share", json!({"id":"original"})).unwrap();
        // 模拟备份中已经收到的他人共同账单，独立恢复时只保留本地内容。
        let mut foreign = source.record("original").unwrap();
        foreign["id"] = json!("foreign-shared");
        foreign["ownerId"] = json!(id());
        foreign["payerId"] = foreign["ownerId"].clone();
        source
            .db
            .execute(
                "INSERT INTO records(id,data) VALUES(?,?)",
                params![s(&foreign, "id"), foreign.to_string()],
            )
            .unwrap();
        let rows = source.rows().unwrap();
        let history = source
            .dispatch("history", json!({"id":"original"}))
            .unwrap();
        let path = std::env::temp_dir().join(format!("lightledger-legacy-{}.backup", id()));
        let payload = json!({"path":path,"password":"legacy-password"});
        source.dispatch("exportBackup", payload.clone()).unwrap();

        // 使用真实加密容器构造旧格式：旧版只缺少同步水位和发件箱三张表。
        let bytes = std::fs::read(&path).unwrap();
        let mut key = [0u8; 32];
        pbkdf2::pbkdf2_hmac::<sha2::Sha256>(b"legacy-password", &bytes[9..25], 600_000, &mut key);
        let cipher = Aes256Gcm::new_from_slice(&key).unwrap();
        let clear = cipher
            .decrypt(Nonce::from_slice(&bytes[25..37]), &bytes[37..])
            .unwrap();
        let mut tables: Value = serde_json::from_slice(&clear).unwrap();
        for table in ["applied", "cursors", "outbox"] {
            tables.as_object_mut().unwrap().remove(table);
        }
        let mut nonce = [0u8; 12];
        rand::thread_rng().fill_bytes(&mut nonce);
        let encrypted = cipher
            .encrypt(
                Nonce::from_slice(&nonce),
                serde_json::to_vec(&tables).unwrap().as_slice(),
            )
            .unwrap();
        let mut legacy = bytes[..25].to_vec();
        legacy.extend(nonce);
        legacy.extend(encrypted);
        std::fs::write(&path, legacy).unwrap();
        let mut restored = LedgerService::open(Path::new(":memory:"), &[2; 32]).unwrap();
        let result = restored.dispatch("restoreBackup", payload).unwrap();
        std::fs::remove_file(path).unwrap();
        assert_eq!(
            result,
            json!({"restored":true,"requiresPairing":true,"independentLedger":true})
        );
        let identity = restored.identity().unwrap();
        assert_eq!(identity["memberId"], original_identity["memberId"]);
        for field in ["personalSpaceId", "sharedSpaceId", "deviceId"] {
            assert_ne!(identity[field], original_identity[field], "{field}");
        }
        assert_eq!(restored.rows().unwrap(), rows);
        assert_eq!(
            restored
                .dispatch("history", json!({"id":"original"}))
                .unwrap(),
            history
        );
        assert!(restored
            .dispatch("adoptIdentity", original_identity)
            .unwrap_err()
            .contains("独立账本"));
        assert_eq!(
            restored
                .dispatch("adoptIdentity", identity.clone())
                .unwrap(),
            identity
        );
        for table in ["applied", "cursors"] {
            assert_eq!(
                restored
                    .db
                    .query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r
                        .get::<_, i64>(0))
                    .unwrap(),
                0
            );
        }
        let pending = restored.dispatch("pendingUploads", json!({})).unwrap();
        let mut empty_device = LedgerService::open(Path::new(":memory:"), &[3; 32]).unwrap();
        empty_device
            .dispatch("adoptIdentity", identity.clone())
            .unwrap();
        for operation in pending.as_array().unwrap() {
            assert_eq!(operation["spaceId"], identity["personalSpaceId"]);
            assert_ne!(operation["entityId"], foreign["id"]);
            empty_device
                .dispatch("applyOperation", operation.clone())
                .unwrap();
        }
        for key in ["original", "refund", "old-reference"] {
            assert_eq!(
                empty_device.record(key).unwrap(),
                restored.record(key).unwrap()
            );
        }
        for action in ["accounts", "categories", "rules"] {
            assert_eq!(
                empty_device.dispatch(action, json!({})).unwrap(),
                restored.dispatch(action, json!({})).unwrap()
            );
        }
        assert_eq!(
            empty_device.record("refund").unwrap()["originalTransactionId"],
            "old-reference"
        );
        assert!(empty_device
            .dispatch("pending", json!({}))
            .unwrap()
            .as_array()
            .unwrap()
            .is_empty());
    }
}
