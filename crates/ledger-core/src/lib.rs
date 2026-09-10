mod capture;
mod commands;
mod queries;
mod sync;
mod transfer;
mod workflow;
use rusqlite::{params, Connection};
use serde_json::{json, Value};
use std::path::Path;
use uuid::Uuid;

pub struct LedgerService {
    pub(crate) db: Connection,
}
fn id() -> String {
    Uuid::new_v4().to_string()
}
fn s<'a>(v: &'a Value, key: &str) -> &'a str {
    v[key].as_str().unwrap_or("")
}
fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}
impl LedgerService {
    pub fn open(path: &Path, key: &[u8]) -> Result<Self, String> {
        if key.len() < 16 {
            return Err("数据库密钥至少需要16字节".into());
        }
        let db = Connection::open(path).map_err(err)?;
        let hex = key.iter().map(|b| format!("{b:02x}")).collect::<String>();
        db.execute_batch(&format!("PRAGMA key=\"x'{hex}'\"; PRAGMA foreign_keys=ON;"))
            .map_err(err)?;
        let cipher: String = db
            .query_row("PRAGMA cipher_version", [], |r| r.get(0))
            .map_err(err)?;
        if cipher.is_empty() {
            return Err("SQLCipher不可用".into());
        }
        db.query_row("SELECT count(*) FROM sqlite_master", [], |r| {
            r.get::<_, i64>(0)
        })
        .map_err(err)?;
        db.execute_batch("CREATE TABLE IF NOT EXISTS records(id TEXT PRIMARY KEY,data TEXT NOT NULL,version INTEGER NOT NULL DEFAULT 1); CREATE TABLE IF NOT EXISTS captures(id TEXT PRIMARY KEY,record_id TEXT NOT NULL); CREATE TABLE IF NOT EXISTS pending(id TEXT PRIMARY KEY,kind TEXT,record_id TEXT,payload TEXT,actor TEXT); CREATE TABLE IF NOT EXISTS history(id TEXT PRIMARY KEY,record_id TEXT,actor TEXT,action TEXT,before_data TEXT,after_data TEXT,at TEXT); CREATE TABLE IF NOT EXISTS outbox(id TEXT PRIMARY KEY,space TEXT,entity TEXT,payload TEXT,uploaded INTEGER DEFAULT 0); CREATE TABLE IF NOT EXISTS applied(id TEXT PRIMARY KEY); CREATE TABLE IF NOT EXISTS cursors(device TEXT PRIMARY KEY,sequence INTEGER); CREATE TABLE IF NOT EXISTS settings(id TEXT PRIMARY KEY,data TEXT); CREATE TABLE IF NOT EXISTS aliases(id TEXT PRIMARY KEY,target TEXT); CREATE TABLE IF NOT EXISTS field_versions(entity TEXT,field TEXT,version TEXT,PRIMARY KEY(entity,field)); CREATE TABLE IF NOT EXISTS sequences(space TEXT PRIMARY KEY,sequence INTEGER); CREATE TABLE IF NOT EXISTS sources(id TEXT PRIMARY KEY,record_id TEXT,data TEXT,expires_at TEXT); CREATE INDEX IF NOT EXISTS history_record ON history(record_id); CREATE INDEX IF NOT EXISTS record_time ON records(json_extract(data,'$.deleted'),json_extract(data,'$.occurredAt')); CREATE INDEX IF NOT EXISTS record_dedup ON records(json_extract(data,'$.amountMinor'),json_extract(data,'$.merchant')); CREATE INDEX IF NOT EXISTS record_key ON records(json_extract(data,'$.transactionKey')); CREATE INDEX IF NOT EXISTS record_refund ON records(json_extract(data,'$.originalTransactionId'));").map_err(err)?;
        Ok(Self { db })
    }
    fn record(&self, key: &str) -> Result<Value, String> {
        let key = self
            .db
            .query_row("SELECT target FROM aliases WHERE id=?", [key], |r| {
                r.get::<_, String>(0)
            })
            .unwrap_or(key.into());
        let text: String = self
            .db
            .query_row("SELECT data FROM records WHERE id=?", [key], |r| r.get(0))
            .map_err(err)?;
        serde_json::from_str(&text).map_err(err)
    }
    fn rows(&self) -> Result<Vec<Value>, String> {
        let mut st = self
            .db
            .prepare("SELECT data FROM records ORDER BY json_extract(data,'$.occurredAt') DESC,id")
            .map_err(err)?;
        let iter = st.query_map([], |r| r.get::<_, String>(0)).map_err(err)?;
        iter.map(|v| serde_json::from_str(&v.map_err(err)?).map_err(err))
            .collect()
    }
    fn pending(
        &self,
        kind: &str,
        record: &str,
        payload: Value,
        actor: &str,
    ) -> Result<String, String> {
        let key = id();
        self.db
            .execute(
                "INSERT INTO pending VALUES(?,?,?,?,?)",
                params![key, kind, record, payload.to_string(), actor],
            )
            .map_err(err)?;
        if ["sharedDelete", "coreModification"].contains(&kind) {
            let value = json!({"id":key,"entityType":"pending","kind":kind,"transactionId":record,"payload":payload,"requestedBy":actor,"resolved":false});
            self.queue(&value, s(&self.identity()?, "sharedSpaceId"), &Value::Null)?;
        }
        Ok(key)
    }
    fn validate(&self, v: &Value) -> Result<(), String> {
        let amount = s(v, "amountMinor")
            .parse::<i64>()
            .map_err(|_| "金额必须是非负整数分字符串")?;
        if amount < 0 {
            return Err("金额不能为负数".into());
        }
        if !["CNY", "USD", "EUR", "JPY"].contains(&s(v, "currencyCode")) {
            return Err("不支持的币种".into());
        }
        if !["expense", "income", "transfer", "refund"].contains(&s(v, "kind")) {
            return Err("账务类型无效".into());
        }
        chrono::DateTime::parse_from_rfc3339(s(v, "occurredAt"))
            .map_err(|_| "发生时间必须包含时区")?;
        if Uuid::parse_str(s(v, "payerId")).is_err() {
            return Err("请确认付款人".into());
        }
        if s(v, "kind") == "refund" && v["deleted"] != true {
            let original = self.record(s(v, "originalTransactionId"))?;
            if original["deleted"] == true
                || s(&original, "kind") != "expense"
                || s(&original, "currencyCode") != s(v, "currencyCode")
            {
                return Err("退款必须关联同币种支出".into());
            }
            let refunded = self
                .rows()?
                .iter()
                .filter(|r| {
                    s(r, "originalTransactionId") == s(v, "originalTransactionId")
                        && s(r, "id") != s(v, "id")
                        && r["deleted"] != true
                })
                .try_fold(0i64, |a, r| {
                    a.checked_add(s(r, "amountMinor").parse::<i64>().unwrap_or(0))
                })
                .ok_or("退款金额溢出")?;
            if refunded.checked_add(amount).ok_or("退款金额溢出")?
                > s(&original, "amountMinor").parse::<i64>().unwrap_or(0)
            {
                return Err("累计退款超过原始支出".into());
            }
        }
        let refunds:i64=self.db.query_row("SELECT COALESCE(SUM(CAST(json_extract(data,'$.amountMinor') AS INTEGER)),0) FROM records WHERE json_extract(data,'$.originalTransactionId')=? AND COALESCE(json_extract(data,'$.deleted'),0)=0 AND id!=?",params![s(v,"id"),s(v,"id")],|r|r.get(0)).map_err(err)?;
        if refunds > 0 && (v["deleted"] == true || s(v, "kind") != "expense" || amount < refunds) {
            return Err("请先处理关联退款，原账单不能删除、改类型或小于已退款金额".into());
        }
        Ok(())
    }
    fn save(&self, v: &Value, action: &str, actor: &str, old: Value) -> Result<(), String> {
        let mut normalized = v.clone();
        let time = chrono::DateTime::parse_from_rfc3339(s(v, "occurredAt")).map_err(err)?;
        normalized["occurredAt"] = json!(time
            .with_timezone(&chrono::Utc)
            .to_rfc3339_opts(chrono::SecondsFormat::Secs, true));
        // 个人回收站和共同视角分别保存；个人删除不会撤回共同事实。
        if normalized["deleted"] == true && normalized["shared"] == true {
            let mut snapshot = if old["sharedSnapshot"].is_object() {
                old["sharedSnapshot"].clone()
            } else {
                old.clone()
            };
            if let Some(fields) = snapshot.as_object_mut() {
                fields.remove("sharedSnapshot");
            }
            for field in ["sharedNote", "sharedCategory"] {
                if !normalized[field].is_null() {
                    snapshot[field] = normalized[field].clone();
                }
            }
            snapshot["deleted"] = json!(false);
            snapshot["shared"] = json!(true);
            normalized["sharedSnapshot"] = snapshot;
        } else if let Some(fields) = normalized.as_object_mut() {
            fields.remove("sharedSnapshot");
        }
        let v = &normalized;
        self.validate(v)?;
        let key = s(v, "id");
        self.db.execute("INSERT INTO records(id,data) VALUES(?,?) ON CONFLICT(id) DO UPDATE SET data=excluded.data,version=version+1",params![key,v.to_string()]).map_err(err)?;
        self.db
            .execute(
                "INSERT INTO history VALUES(?,?,?,?,?,?,?)",
                params![
                    id(),
                    key,
                    actor,
                    action,
                    old.to_string(),
                    v.to_string(),
                    chrono::Utc::now().to_rfc3339()
                ],
            )
            .map_err(err)?;
        let identity = self.identity()?;
        if v["ownerId"] == identity["memberId"] {
            self.queue(v, s(&identity, "personalSpaceId"), &old)?;
        }
        if v["shared"] == true || old["shared"] == true {
            let mut projection = if v["sharedSnapshot"].is_object() {
                v["sharedSnapshot"].clone()
            } else {
                v.clone()
            };
            let mut previous = if old["sharedSnapshot"].is_object() {
                old["sharedSnapshot"].clone()
            } else {
                old.clone()
            };
            for value in [&mut projection, &mut previous] {
                if let Some(fields) = value.as_object_mut() {
                    for field in [
                        "note",
                        "accountId",
                        "captureId",
                        "transactionKey",
                        "locks",
                        "sharedSnapshot",
                    ] {
                        fields.remove(field);
                    }
                }
            }
            self.queue(&projection, s(&identity, "sharedSpaceId"), &previous)?;
        }
        Ok(())
    }
    pub fn dispatch(&mut self, action: &str, p: Value) -> Result<Value, String> {
        self.db.execute_batch("BEGIN IMMEDIATE").map_err(err)?;
        let cleanup=self.db.execute("UPDATE sources SET data=json_remove(data,'$.rawText','$.rawImage'),expires_at='' WHERE expires_at!='' AND expires_at<=?",[chrono::Utc::now().to_rfc3339()]).map_err(err);
        let result = cleanup.and_then(|_| self.run(action, p));
        match result {
            Ok(v) => {
                self.db.execute_batch("COMMIT").map_err(err)?;
                Ok(v)
            }
            Err(e) => {
                let _ = self.db.execute_batch("ROLLBACK");
                Err(e)
            }
        }
    }
}

#[cfg(test)]
mod checks {
    use super::*;
    fn draft(amount: &str) -> Value {
        json!({"amountMinor":amount,"currencyCode":"CNY","kind":"expense","occurredAt":"2026-09-10T12:00:00Z","merchant":"午饭","payerId":"local"})
    }
    #[test]
    fn ledger_contract() {
        let directory = std::env::temp_dir().join(format!("lightledger-check-{}", id()));
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join("ledger.db");
        let key = [7u8; 32];
        let mut app = LedgerService::open(&path, &key).unwrap();
        assert!(
            !app.dispatch("cipherVersion", json!({})).unwrap()["version"]
                .as_str()
                .unwrap()
                .is_empty()
        );
        assert!(app.dispatch("create", draft("-1")).is_err());
        assert!(app.dispatch("create", draft("1.01")).is_err());
        assert!(app
            .dispatch("create", draft("9223372036854775808"))
            .is_err());
        let mut first = draft("2800");
        first["captureId"] = json!("capture-a");
        let created = app.dispatch("create", first.clone()).unwrap();
        let key_id = created["id"].as_str().unwrap();
        assert_eq!(
            app.dispatch("create", first.clone()).unwrap()["status"],
            "existing"
        );
        let mut repeated = draft("2800");
        repeated["id"] = json!(key_id);
        assert!(app.dispatch("create", repeated).is_err());
        let duplicate = app.dispatch("create", draft("2800")).unwrap();
        assert_eq!(duplicate["status"], "duplicate");
        assert_eq!(app.dispatch("summary", json!({})).unwrap()[0]["count"], 1);
        app.dispatch(
            "resolveDuplicate",
            json!({"id":duplicate["pendingId"],"merge":false}),
        )
        .unwrap();
        assert_eq!(app.dispatch("summary", json!({})).unwrap()[0]["count"], 2);
        app.dispatch("update", json!({"id":key_id,"note":"人工备注"}))
            .unwrap();
        assert!(app.record(key_id).unwrap()["locks"]
            .as_array()
            .unwrap()
            .contains(&json!("note")));
        app.dispatch("share", json!({"id":key_id})).unwrap();
        let pending = app
            .dispatch("requestSharedDelete", json!({"id":key_id}))
            .unwrap();
        assert!(app
            .dispatch(
                "approveSharedDelete",
                json!({"id":pending["pendingId"],"approve":true,"actorId":"spoof"})
            )
            .is_err());
        app.dispatch("delete", json!({"id":key_id})).unwrap();
        assert_eq!(
            app.dispatch("list", json!({"deleted":true}))
                .unwrap()
                .as_array()
                .unwrap()
                .len(),
            1
        );
        app.dispatch("restore", json!({"id":key_id})).unwrap();
        assert!(!app
            .dispatch("parseText", json!({"text":"2026-09-10 午饭 28"}))
            .unwrap()["missingFields"]
            .as_array()
            .unwrap()
            .is_empty());
        let backup = directory.join("saved.qaccount");
        app.dispatch(
            "exportBackup",
            json!({"path":backup,"password":"passphrase123"}),
        )
        .unwrap();
        assert!(app
            .dispatch(
                "restoreBackup",
                json!({"path":backup,"password":"wrong-password"})
            )
            .is_err());
        assert_eq!(app.dispatch("summary", json!({})).unwrap()[0]["count"], 2);
        app.dispatch(
            "restoreBackup",
            json!({"path":backup,"password":"passphrase123"}),
        )
        .unwrap();
        assert_eq!(
            app.dispatch("list", json!({}))
                .unwrap()
                .as_array()
                .unwrap()
                .len(),
            2
        );
        drop(app);
        assert!(!std::fs::read(&path)
            .unwrap()
            .starts_with(b"SQLite format 3"));
        assert!(LedgerService::open(&path, &[8u8; 32]).is_err());
        let mut app = LedgerService::open(&path, &key).unwrap();
        assert_eq!(
            app.dispatch("list", json!({}))
                .unwrap()
                .as_array()
                .unwrap()
                .len(),
            2
        );
        drop(app);
        std::fs::remove_dir_all(directory).unwrap();
    }
    #[test]
    fn operation_idempotence_and_fields() {
        let mut a = LedgerService::open(Path::new(":memory:"), &[1; 32]).unwrap();
        let mut b = LedgerService::open(Path::new(":memory:"), &[2; 32]).unwrap();
        a.dispatch("create", draft("1234")).unwrap();
        let ops = a.dispatch("pendingUploads", json!({})).unwrap();
        let first = ops[0].clone();
        assert_eq!(
            b.dispatch("applyOperation", first.clone()).unwrap()["applied"],
            true
        );
        assert_eq!(
            b.dispatch("applyOperation", first.clone()).unwrap()["duplicate"],
            true
        );
        let mut later = first.clone();
        later["id"] = json!(id());
        later["sequence"] = json!(3);
        assert!(b.dispatch("applyOperation", later).is_err());
        let key = first["entityId"].as_str().unwrap();
        a.dispatch("update", json!({"id":key,"note":"a"})).unwrap();
        let mut other = first.clone();
        other["id"] = json!(id());
        other["sequence"] = json!(2);
        other["patch"] = json!({"category":"交通"});
        other["baseVersions"] = json!({"category":first["id"]});
        other["payload"]["category"] = json!("交通");
        b.dispatch("applyOperation", other).unwrap();
        let next = a.dispatch("pendingUploads", json!({})).unwrap()[1].clone();
        let mut next = next;
        next["deviceId"] = json!(id());
        next["sequence"] = json!(1);
        assert_eq!(
            b.dispatch("applyOperation", next).unwrap()["conflict"],
            false
        );
        assert_eq!(b.record(key).unwrap()["category"], "交通");
        assert_eq!(b.record(key).unwrap()["note"], "a");
    }
    fn deliver(a: &mut LedgerService, b: &mut LedgerService, space: &str) {
        let ops = a.dispatch("pendingUploads", json!({})).unwrap();
        for op in ops.as_array().unwrap() {
            if op["spaceId"] == space {
                b.dispatch("applyOperation", op.clone()).unwrap();
                a.dispatch("markUploaded", json!({"id":op["id"]})).unwrap();
            }
        }
    }
    #[test]
    fn shared_scope_and_approval() {
        let mut a = LedgerService::open(Path::new(":memory:"), &[1; 32]).unwrap();
        let mut b = LedgerService::open(Path::new(":memory:"), &[2; 32]).unwrap();
        let ai = a.identity().unwrap();
        let bi = b.identity().unwrap();
        let space = s(&ai, "sharedSpaceId");
        b.dispatch("adoptSharedSpace", json!({"sharedSpaceId":space}))
            .unwrap();
        a.dispatch(
            "registerTrustedMember",
            json!({"memberId":bi["memberId"],"sharedSpaceId":space}),
        )
        .unwrap();
        b.dispatch(
            "registerTrustedMember",
            json!({"memberId":ai["memberId"],"sharedSpaceId":space}),
        )
        .unwrap();
        let created = a.dispatch("create", draft("1200")).unwrap();
        a.dispatch("share", json!({"id":created["id"]})).unwrap();
        deliver(&mut a, &mut b, space);
        assert!(b
            .dispatch("list", json!({}))
            .unwrap()
            .as_array()
            .unwrap()
            .is_empty());
        assert_eq!(
            b.dispatch("list", json!({"shared":true}))
                .unwrap()
                .as_array()
                .unwrap()
                .len(),
            1
        );
        assert_eq!(b.dispatch("exportJson", json!({})).unwrap()["text"], "[]");
        a.dispatch("delete", json!({"id":created["id"]})).unwrap();
        deliver(&mut a, &mut b, space);
        assert!(a
            .dispatch("list", json!({}))
            .unwrap()
            .as_array()
            .unwrap()
            .is_empty());
        assert_eq!(
            a.dispatch("list", json!({"deleted":true}))
                .unwrap()
                .as_array()
                .unwrap()
                .len(),
            1
        );
        for service in [&mut a, &mut b] {
            assert_eq!(
                service
                    .dispatch("list", json!({"shared":true}))
                    .unwrap()
                    .as_array()
                    .unwrap()
                    .len(),
                1
            );
            assert_eq!(
                service.dispatch("summary", json!({"shared":true})).unwrap()[0]["expenseMinor"],
                "1200"
            );
        }
        a.dispatch("restore", json!({"id":created["id"]})).unwrap();
        deliver(&mut a, &mut b, space);
        assert_eq!(
            a.dispatch("summary", json!({})).unwrap()[0]["expenseMinor"],
            "1200"
        );
        a.dispatch("delete", json!({"id":created["id"]})).unwrap();
        deliver(&mut a, &mut b, space);
        let pending = a
            .dispatch("requestSharedDelete", json!({"id":created["id"]}))
            .unwrap();
        deliver(&mut a, &mut b, space);
        b.dispatch(
            "approveSharedDelete",
            json!({"id":pending["pendingId"],"approve":true}),
        )
        .unwrap();
        deliver(&mut b, &mut a, space);
        assert_eq!(a.record(s(&created, "id")).unwrap()["shared"], false);
        assert_eq!(a.record(s(&created, "id")).unwrap()["deleted"], true);
        for service in [&mut a, &mut b] {
            assert!(service
                .dispatch("list", json!({"shared":true}))
                .unwrap()
                .as_array()
                .unwrap()
                .is_empty());
            assert!(service
                .dispatch("summary", json!({"shared":true}))
                .unwrap()
                .as_array()
                .unwrap()
                .is_empty());
        }
        a.dispatch("restore", json!({"id":created["id"]})).unwrap();
        assert_eq!(a.record(s(&created, "id")).unwrap()["shared"], false);
        assert_eq!(
            a.dispatch("summary", json!({})).unwrap()[0]["expenseMinor"],
            "1200"
        );
        assert!(a
            .dispatch("pending", json!({}))
            .unwrap()
            .as_array()
            .unwrap()
            .is_empty());
    }
    #[test]
    fn imports_refunds_and_resolution() {
        let mut a = LedgerService::open(Path::new(":memory:"), &[1; 32]).unwrap();
        let first = a.dispatch("create", draft("10000")).unwrap();
        let mut refund = draft("4000");
        refund["kind"] = json!("refund");
        refund["originalTransactionId"] = first["id"].clone();
        refund["merchant"] = json!("退款");
        let returned = a.dispatch("create", refund).unwrap();
        assert!(a
            .dispatch("update", json!({"id":first["id"],"amountMinor":"3000"}))
            .is_err());
        assert!(a.dispatch("delete", json!({"id":first["id"]})).is_err());
        a.dispatch("delete", json!({"id":returned["id"]})).unwrap();
        a.dispatch("update", json!({"id":first["id"],"amountMinor":"3000"}))
            .unwrap();
        assert!(a.dispatch("restore", json!({"id":returned["id"]})).is_err());
        let text = serde_json::to_string(&vec![draft("15"), draft("-1")]).unwrap();
        let imported = a.dispatch("importJson", json!({"text":text})).unwrap();
        assert_eq!(imported[0]["status"], "created");
        assert_eq!(imported[1]["status"], "failed");
        assert_eq!(
            a.dispatch("importJson", json!({"text":text})).unwrap()[0]["status"],
            "existing"
        );
        let export = a.dispatch("exportJson", json!({})).unwrap();
        assert_eq!(
            a.dispatch("importJson", json!({"text":export["text"]}))
                .unwrap()[0]["status"],
            "existing"
        );
        let mut source = draft("999");
        source["transactionKey"] = json!("payment:unique");
        let original = a.dispatch("create", source.clone()).unwrap();
        a.dispatch("update", json!({"id":original["id"],"note":"人工"}))
            .unwrap();
        source["note"] = json!("来源文本");
        a.dispatch("create", source).unwrap();
        let pending = a.dispatch("pending", json!({})).unwrap();
        let difference = pending
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["kind"] == "difference")
            .unwrap();
        a.dispatch(
            "resolveDifference",
            json!({"id":difference["id"],"choices":{"note":"remote"}}),
        )
        .unwrap();
        assert_eq!(a.record(s(&original, "id")).unwrap()["note"], "来源文本");
    }
    #[test]
    fn fifty_thousand_query_baseline() {
        let mut a = LedgerService::open(Path::new(":memory:"), &[1; 32]).unwrap();
        let member = a.identity().unwrap();
        let start = std::time::Instant::now();
        a.db.execute_batch("BEGIN").unwrap();
        for n in 0..50_000 {
            let mut v = draft("100");
            v["id"] = json!(format!("bench-{n}"));
            v["ownerId"] = member["memberId"].clone();
            v["deleted"] = json!(false);
            a.db.execute(
                "INSERT INTO records(id,data) VALUES(?,?)",
                params![s(&v, "id"), v.to_string()],
            )
            .unwrap();
        }
        a.db.execute_batch("COMMIT").unwrap();
        let seeded = start.elapsed();
        let start = std::time::Instant::now();
        assert_eq!(
            a.dispatch("list", json!({"pageSize":50}))
                .unwrap()
                .as_array()
                .unwrap()
                .len(),
            50
        );
        assert_eq!(
            a.dispatch("summary", json!({})).unwrap()[0]["expenseMinor"],
            "5000000"
        );
        let query = start.elapsed();
        let mut input = vec![];
        for n in 0..1000 {
            let mut v = draft("123");
            v["merchant"] = json!(format!("导入商户{n}"));
            input.push(v);
        }
        let start = std::time::Instant::now();
        let result = a
            .dispatch(
                "importJson",
                json!({"text":serde_json::to_string(&input).unwrap()}),
            )
            .unwrap();
        assert_eq!(result.as_array().unwrap().len(), 1000);
        eprintln!(
            "性能基线: 50000条填充={:?},分页+汇总={:?},1000条事务导入={:?}",
            seeded,
            query,
            start.elapsed()
        );
    }
    #[test]
    fn midnight_and_conflict_resolution() {
        let mut a = LedgerService::open(Path::new(":memory:"), &[1; 32]).unwrap();
        let mut b = LedgerService::open(Path::new(":memory:"), &[2; 32]).unwrap();
        let identity = a.identity().unwrap();
        b.dispatch(
            "adoptIdentity",
            json!({"memberId":identity["memberId"],"personalSpaceId":identity["personalSpaceId"]}),
        )
        .unwrap();
        let mut value = draft("1234");
        value["occurredAt"] = json!("2026-09-11T00:30:00+09:00");
        let created = a.dispatch("create", value).unwrap();
        let key = s(&created, "id");
        assert_eq!(a.record(key).unwrap()["occurredAt"], "2026-09-10T15:30:00Z");
        let date = chrono::DateTime::parse_from_rfc3339("2026-09-10T15:30:00Z")
            .unwrap()
            .with_timezone(&chrono::Local)
            .format("%Y-%m-%d")
            .to_string();
        assert_eq!(
            a.dispatch("list", json!({"from":date,"to":date}))
                .unwrap()
                .as_array()
                .unwrap()
                .len(),
            1
        );
        deliver(&mut a, &mut b, s(&identity, "personalSpaceId"));
        a.dispatch("update", json!({"id":key,"note":"版本A"}))
            .unwrap();
        b.dispatch("update", json!({"id":key,"note":"版本B"}))
            .unwrap();
        deliver(&mut a, &mut b, s(&identity, "personalSpaceId"));
        deliver(&mut b, &mut a, s(&identity, "personalSpaceId"));
        let pending = a.dispatch("pending", json!({})).unwrap();
        let conflict = pending
            .as_array()
            .unwrap()
            .iter()
            .find(|v| v["kind"] == "conflict")
            .unwrap();
        a.dispatch(
            "resolveConflict",
            json!({"id":conflict["id"],"choices":{"note":"remote"}}),
        )
        .unwrap();
        deliver(&mut a, &mut b, s(&identity, "personalSpaceId"));
        assert_eq!(
            a.record(key).unwrap()["note"],
            b.record(key).unwrap()["note"]
        );
        assert!(b
            .dispatch("pending", json!({}))
            .unwrap()
            .as_array()
            .unwrap()
            .is_empty());
    }
}
