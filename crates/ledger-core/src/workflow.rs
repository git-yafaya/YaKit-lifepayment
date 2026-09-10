use super::*;
impl LedgerService {
    pub(crate) fn trusted(&self, member: &str) -> Result<bool, String> {
        let me = self.identity()?;
        if member == s(&me, "memberId") {
            return Ok(true);
        }
        let text = self
            .db
            .query_row(
                "SELECT data FROM settings WHERE id=?",
                [format!("member:{member}")],
                |r| r.get::<_, String>(0),
            )
            .ok();
        Ok(text
            .and_then(|v| serde_json::from_str::<Value>(&v).ok())
            .map(|v| v["active"] == true && v["sharedSpaceId"] == me["sharedSpaceId"])
            .unwrap_or(false))
    }
    pub(crate) fn metadata(&self, key: &str, value: &Value) -> Result<(), String> {
        self.db
            .execute(
                "INSERT OR REPLACE INTO settings VALUES(?,?)",
                params![key, value.to_string()],
            )
            .map_err(err)?;
        self.queue(
            &json!({"id":key,"entityType":"setting","value":value}),
            s(&self.identity()?, "personalSpaceId"),
            &Value::Null,
        )
    }
    pub(crate) fn source(&self, v: &Value, record: &str) -> Result<(), String> {
        let capture = s(v, "captureId");
        if capture.is_empty() {
            return Ok(());
        }
        let days = self
            .db
            .query_row("SELECT data FROM settings WHERE id='retention'", [], |r| {
                r.get::<_, String>(0)
            })
            .ok()
            .and_then(|t| serde_json::from_str::<Value>(&t).ok())
            .and_then(|v| v["days"].as_i64())
            .unwrap_or(7);
        let mut raw = v.clone();
        if days == 0 {
            raw.as_object_mut().map(|o| {
                o.remove("rawText");
                o.remove("rawImage");
            });
        }
        // 原始来源随SQLCipher数据库加密；超期只清理原文，幂等指纹继续保存。
        let expires = if days < 0 {
            String::new()
        } else {
            (chrono::Utc::now() + chrono::Duration::days(days)).to_rfc3339()
        };
        self.db
            .execute(
                "INSERT OR IGNORE INTO sources VALUES(?,?,?,?)",
                params![capture, record, raw.to_string(), expires],
            )
            .map_err(err)?;
        Ok(())
    }
    pub(crate) fn workflow(&self, action: &str, p: Value) -> Result<Value, String> {
        match action {
            "registerTrustedMember" | "revokeTrustedMember" => {
                Uuid::parse_str(s(&p, "memberId")).map_err(err)?;
                let mut value = p.clone();
                value["active"] = json!(action == "registerTrustedMember");
                self.db
                    .execute(
                        "INSERT OR REPLACE INTO settings VALUES(?,?)",
                        params![format!("member:{}", s(&p, "memberId")), value.to_string()],
                    )
                    .map_err(err)?;
                Ok(value)
            }
            "members" => {
                let mut st = self
                    .db
                    .prepare("SELECT data FROM settings WHERE id LIKE 'member:%'")
                    .map_err(err)?;
                let mut members = st
                    .query_map([], |r| r.get::<_, String>(0))
                    .map_err(err)?
                    .map(|r| serde_json::from_str::<Value>(&r.map_err(err)?).map_err(err))
                    .collect::<Result<Vec<_>, String>>()?;
                let me = self.identity()?;
                members.push(json!({"memberId":me["memberId"],"displayName":"我","active":true}));
                Ok(json!(members))
            }
            "capture" => {
                let mut v = p.clone();
                let missing = ["amountMinor", "kind", "payerId", "occurredAt", "accountId"]
                    .into_iter()
                    .filter(|f| s(&v, f).is_empty())
                    .collect::<Vec<_>>();
                if !missing.is_empty() {
                    let key = self.pending(
                        "confirmation",
                        "",
                        json!({"draft":v,"missingFields":missing}),
                        s(&self.identity()?, "memberId"),
                    )?;
                    self.source(&v, "")?;
                    return Ok(
                        json!({"status":"needsConfirmation","pendingId":key,"missingFields":missing}),
                    );
                }
                v["automatic"] = json!(true);
                self.create(v, false)
            }
            "confirmCandidate" => {
                let text: String = self
                    .db
                    .query_row(
                        "SELECT payload FROM pending WHERE id=? AND kind='confirmation'",
                        [s(&p, "id")],
                        |r| r.get(0),
                    )
                    .map_err(err)?;
                let pending: Value = serde_json::from_str(&text).map_err(err)?;
                let mut v = pending["draft"].clone();
                for (k, val) in p["draft"].as_object().ok_or("请提供确认后的账单")? {
                    v[k] = val.clone();
                }
                let result = self.create(v, false)?;
                self.db
                    .execute("DELETE FROM pending WHERE id=?", [s(&p, "id")])
                    .map_err(err)?;
                Ok(result)
            }
            "resolveConflict" | "resolveDifference" => {
                let (key,text):(String,String)=self.db.query_row("SELECT record_id,payload FROM pending WHERE id=? AND kind IN ('conflict','difference')",[s(&p,"id")],|r|Ok((r.get(0)?,r.get(1)?))).map_err(err)?;
                let info: Value = serde_json::from_str(&text).map_err(err)?;
                let old = self.record(&key)?;
                let me = self.identity()?;
                if s(&old, "ownerId") != s(&me, "memberId") {
                    return Err("只有账单所有者可确认事实差异".into());
                }
                let mut value = old.clone();
                let fields = info["fields"].as_object().ok_or("差异内容损坏")?;
                for (field, versions) in fields {
                    if p["choices"][field] == "remote" {
                        value[field] = versions["remote"].clone();
                    } else if !p["values"][field].is_null() {
                        value[field] = p["values"][field].clone();
                    }
                    let mut locks = value["locks"].as_array().cloned().unwrap_or_default();
                    if !locks.contains(&json!(field)) {
                        locks.push(json!(field));
                    }
                    value["locks"] = json!(locks);
                }
                self.save(&value, "resolveConflict", s(&me, "memberId"), old)?;
                // 解决操作带双方版本，新设备重放时可以明确取代冲突版本。
                let mut st=self.db.prepare("SELECT id,payload FROM outbox WHERE entity=? AND uploaded=0 ORDER BY rowid DESC LIMIT 2").map_err(err)?;
                let ops = st
                    .query_map([&key], |r| {
                        Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
                    })
                    .map_err(err)?
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(err)?;
                for (op, text) in ops {
                    let mut op_value: Value = serde_json::from_str(&text).map_err(err)?;
                    op_value["resolves"] = json!(fields
                        .iter()
                        .map(|(f, v)| (f.clone(), json!([v["localVersion"], v["remoteVersion"]])))
                        .collect::<serde_json::Map<String, Value>>());
                    for field in fields.keys() {
                        op_value["patch"][field] = value[field].clone();
                    }
                    for field in fields.keys() {
                        self.db.execute("INSERT INTO field_versions VALUES(?,?,?) ON CONFLICT(entity,field) DO UPDATE SET version=excluded.version",params![format!("{}:{key}",s(&op_value,"spaceId")),field,op]).map_err(err)?;
                    }
                    self.db
                        .execute(
                            "UPDATE outbox SET payload=? WHERE id=?",
                            params![op_value.to_string(), op],
                        )
                        .map_err(err)?;
                }
                self.db
                    .execute("DELETE FROM pending WHERE id=?", [s(&p, "id")])
                    .map_err(err)?;
                Ok(value)
            }
            "mergeTransactions" => {
                let canonical = self.record(s(&p, "targetId"))?;
                let old = self.record(s(&p, "sourceId"))?;
                let me = self.identity()?;
                if s(&canonical, "id") == s(&old, "id")
                    || s(&canonical, "ownerId") != s(&me, "memberId")
                    || s(&old, "ownerId") != s(&me, "memberId")
                {
                    return Err("只能归并自己的两笔不同账单".into());
                }
                let mut removed = old.clone();
                removed["deleted"] = json!(true);
                self.save(&removed, "merge", s(&me, "memberId"), old.clone())?;
                self.db
                    .execute(
                        "INSERT INTO aliases VALUES(?,?)",
                        params![s(&old, "id"), s(&canonical, "id")],
                    )
                    .map_err(err)?;
                self.metadata(&format!("merge:{}", s(&old, "id")), &old)?;
                self.queue(
                    &json!({"id":old["id"],"entityType":"alias","targetId":canonical["id"]}),
                    s(&me, "personalSpaceId"),
                    &Value::Null,
                )?;
                Ok(json!({"id":canonical["id"]}))
            }
            "splitTransactions" => {
                let key = s(&p, "id");
                let text: String = self
                    .db
                    .query_row(
                        "SELECT data FROM settings WHERE id=?",
                        [format!("merge:{key}")],
                        |r| r.get(0),
                    )
                    .map_err(err)?;
                let mut value: Value = serde_json::from_str(&text).map_err(err)?;
                self.db
                    .execute("DELETE FROM aliases WHERE id=?", [key])
                    .map_err(err)?;
                value["deleted"] = json!(false);
                let old = self.record(key)?;
                self.save(&value, "split", s(&self.identity()?, "memberId"), old)?;
                self.queue(
                    &json!({"id":key,"entityType":"alias","targetId":Value::Null}),
                    s(&self.identity()?, "personalSpaceId"),
                    &Value::Null,
                )?;
                self.db
                    .execute("DELETE FROM settings WHERE id=?", [format!("merge:{key}")])
                    .map_err(err)?;
                Ok(value)
            }
            "sources" => {
                let mut st = self
                    .db
                    .prepare("SELECT data FROM sources WHERE record_id=?")
                    .map_err(err)?;
                let values = st
                    .query_map([s(&p, "id")], |r| r.get::<_, String>(0))
                    .map_err(err)?
                    .map(|r| serde_json::from_str::<Value>(&r.map_err(err)?).map_err(err))
                    .collect::<Result<Vec<_>, String>>()?;
                Ok(json!(values))
            }
            "setRetention" => {
                let days = p["days"].as_i64().ok_or("保留天数必须为整数，-1表示永久")?;
                if ![-1, 0, 7, 30].contains(&days) {
                    return Err("保留天数仅支持0、7、30、-1".into());
                }
                self.db
                    .execute(
                        "INSERT OR REPLACE INTO settings VALUES('retention',?)",
                        [p.to_string()],
                    )
                    .map_err(err)?;
                Ok(p)
            }
            "purgeSources" => {
                let n=self.db.execute("UPDATE sources SET data=json_remove(data,'$.rawText','$.rawImage'),expires_at='' WHERE expires_at!='' AND expires_at<=?",[chrono::Utc::now().to_rfc3339()]).map_err(err)?;
                Ok(json!({"purged":n}))
            }
            _ => Err("未知流程".into()),
        }
    }
}
