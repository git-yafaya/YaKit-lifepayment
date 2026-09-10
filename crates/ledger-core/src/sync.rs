use super::*;
impl LedgerService {
    pub(crate) fn identity(&self) -> Result<Value, String> {
        if let Ok(text) =
            self.db
                .query_row("SELECT data FROM settings WHERE id='identity'", [], |r| {
                    r.get::<_, String>(0)
                })
        {
            return serde_json::from_str(&text).map_err(err);
        }
        let value =
            json!({"deviceId":id(),"memberId":id(),"personalSpaceId":id(),"sharedSpaceId":id()});
        self.db
            .execute(
                "INSERT INTO settings VALUES('identity',?)",
                [value.to_string()],
            )
            .map_err(err)?;
        Ok(value)
    }
    pub(crate) fn queue(&self, v: &Value, space: &str, old: &Value) -> Result<(), String> {
        let identity = self.identity()?;
        let operation = id();
        self.db.execute("INSERT INTO sequences VALUES(?,1) ON CONFLICT(space) DO UPDATE SET sequence=sequence+1",[space]).map_err(err)?;
        let sequence = self
            .db
            .query_row(
                "SELECT sequence FROM sequences WHERE space=?",
                [space],
                |r| r.get::<_, i64>(0),
            )
            .map_err(err)?;
        let version_entity = format!("{space}:{}", s(v, "id"));
        let mut base = serde_json::Map::new();
        let mut patch = serde_json::Map::new();
        for (field, value) in v.as_object().ok_or("记录无效")? {
            if old[field] != *value {
                let version = self
                    .db
                    .query_row(
                        "SELECT version FROM field_versions WHERE entity=? AND field=?",
                        params![version_entity, field],
                        |r| r.get::<_, String>(0),
                    )
                    .unwrap_or_default();
                base.insert(field.clone(), json!(version));
                patch.insert(field.clone(), value.clone());
                self.db.execute("INSERT INTO field_versions VALUES(?,?,?) ON CONFLICT(entity,field) DO UPDATE SET version=excluded.version",params![version_entity,field,operation]).map_err(err)?;
            }
        }
        let payload = json!({"id":operation,"spaceId":space,"entityId":v["id"],"deviceId":identity["deviceId"],"memberId":identity["memberId"],"sequence":sequence,"payload":v,"patch":patch,"baseVersions":base,"baseVersion":0,"entityType":v["entityType"].as_str().unwrap_or("record")});
        self.db
            .execute(
                "INSERT INTO outbox(id,space,entity,payload) VALUES(?,?,?,?)",
                params![operation, space, s(v, "id"), payload.to_string()],
            )
            .map_err(err)?;
        Ok(())
    }
    pub(crate) fn apply_operation(&self, p: Value) -> Result<Value, String> {
        if self
            .db
            .query_row("SELECT 1 FROM applied WHERE id=?", [s(&p, "id")], |r| {
                r.get::<_, i32>(0)
            })
            .is_ok()
        {
            return Ok(json!({"applied":false,"duplicate":true}));
        }
        let cursor_key = format!("{}:{}", s(&p, "spaceId"), s(&p, "deviceId"));
        let cursor = self
            .db
            .query_row(
                "SELECT sequence FROM cursors WHERE device=?",
                [&cursor_key],
                |r| r.get::<_, i64>(0),
            )
            .unwrap_or(0);
        let seq = p["sequence"].as_i64().ok_or("缺少序号")?;
        if seq != cursor + 1 {
            return Err("同步序号不连续".into());
        }
        let incoming = p["payload"].clone();
        let identity = self.identity()?;
        let shared = p["spaceId"] == identity["sharedSpaceId"];
        if shared && !self.trusted(s(&p, "memberId"))? {
            return Err("同步成员未获共同空间授权".into());
        }
        let entity_type = p["entityType"].as_str().unwrap_or("record");
        if entity_type != "record" {
            self.apply_auxiliary(&p, shared)?;
            self.finish_operation(&p, &cursor_key, seq)?;
            return Ok(json!({"applied":true}));
        }

        let key = s(&p, "entityId");
        if key != s(&incoming, "id") {
            return Err("实体ID不匹配".into());
        }
        let stored = self.record(key).unwrap_or(Value::Null);
        let old = if shared && stored["sharedSnapshot"].is_object() {
            stored["sharedSnapshot"].clone()
        } else {
            stored.clone()
        };
        let canonical = old["id"].as_str().unwrap_or(key).to_string();
        let key = canonical.as_str();
        if shared && !old.is_null() && s(&p, "memberId") != s(&old, "ownerId") {
            let patch = p["patch"].as_object().ok_or("共同补丁无效")?;
            if patch
                .keys()
                .any(|k| !["shared", "sharedNote", "sharedCategory"].contains(&k.as_str()))
            {
                return Err("对方不能直接修改个人核心字段".into());
            }
        }
        let version_entity = format!("{}:{key}", s(&p, "spaceId"));

        let mut merged = if old.is_null() {
            incoming.clone()
        } else {
            old.clone()
        };
        let mut conflicts = serde_json::Map::new();
        let patch = p["patch"]
            .as_object()
            .or(incoming.as_object())
            .ok_or("补丁无效")?;
        for (field, value) in patch {
            let current = self
                .db
                .query_row(
                    "SELECT version FROM field_versions WHERE entity=? AND field=?",
                    params![version_entity, field],
                    |r| r.get::<_, String>(0),
                )
                .unwrap_or_default();
            let base = p["baseVersions"][field].as_str().unwrap_or("");
            let resolved = p["resolves"][field]
                .as_array()
                .map(|r| r.contains(&json!(current)))
                .unwrap_or(false);
            if field == "id" {
                continue;
            }
            if !old.is_null() && current != base && old[field] != *value && !resolved {
                conflicts.insert(field.clone(),json!({"local":old[field],"remote":value,"localVersion":current,"remoteVersion":p["id"]}));
            } else {
                merged[field] = value.clone();
                self.db.execute("INSERT INTO field_versions VALUES(?,?,?) ON CONFLICT(entity,field) DO UPDATE SET version=excluded.version",params![version_entity,field,s(&p,"id")]).map_err(err)?;
            }
        }
        merged["id"] = json!(key);
        self.validate(&merged)?;
        let mut persisted = merged.clone();
        if shared && stored["deleted"] == true && stored["ownerId"] == identity["memberId"] {
            persisted = stored.clone();
            persisted["shared"] = merged["shared"].clone();
            if merged["shared"] == true {
                persisted["sharedSnapshot"] = merged.clone();
            } else if let Some(fields) = persisted.as_object_mut() {
                fields.remove("sharedSnapshot");
            }
            for field in ["sharedNote", "sharedCategory"] {
                if !merged[field].is_null() {
                    persisted[field] = merged[field].clone();
                }
            }
        }
        self.db.execute("INSERT INTO records(id,data) VALUES(?,?) ON CONFLICT(id) DO UPDATE SET data=excluded.data,version=version+1",params![key,persisted.to_string()]).map_err(err)?;
        if !conflicts.is_empty() {
            self.pending(
                "conflict",
                key,
                json!({"fields":conflicts,"operation":p}),
                s(&p, "memberId"),
            )?;
        }
        if p["resolves"]
            .as_object()
            .map(|v| !v.is_empty())
            .unwrap_or(false)
        {
            self.db
                .execute(
                    "DELETE FROM pending WHERE record_id=? AND kind='conflict'",
                    [key],
                )
                .map_err(err)?;
        }
        self.finish_operation(&p, &cursor_key, seq)?;
        Ok(json!({"applied":true,"conflict":!conflicts.is_empty()}))
    }
    fn finish_operation(&self, p: &Value, cursor_key: &str, seq: i64) -> Result<(), String> {
        self.db
            .execute("INSERT INTO applied VALUES(?)", [s(p, "id")])
            .map_err(err)?;
        self.db.execute("INSERT INTO cursors VALUES(?,?) ON CONFLICT(device) DO UPDATE SET sequence=excluded.sequence",params![cursor_key,seq]).map_err(err)?;
        Ok(())
    }
    fn apply_auxiliary(&self, p: &Value, shared: bool) -> Result<(), String> {
        let v = &p["payload"];
        match s(p, "entityType") {
            "setting" => {
                if shared {
                    return Err("共同空间不接收个人设置".into());
                }
                let key = s(v, "id");
                if !["account:", "category:", "rule:", "merge:"]
                    .iter()
                    .any(|prefix| key.starts_with(prefix))
                {
                    return Err("不允许同步此设置".into());
                }
                if v["value"].is_null() {
                    self.db
                        .execute("DELETE FROM settings WHERE id=?", [key])
                        .map_err(err)?;
                } else {
                    self.db
                        .execute(
                            "INSERT OR REPLACE INTO settings VALUES(?,?)",
                            params![key, v["value"].to_string()],
                        )
                        .map_err(err)?;
                }
            }
            "alias" => {
                if shared {
                    return Err("归并操作仅用于个人空间".into());
                }
                let key = s(v, "id");
                if v["targetId"].is_null() {
                    self.db
                        .execute("DELETE FROM aliases WHERE id=?", [key])
                        .map_err(err)?;
                } else {
                    if key == s(v, "targetId") {
                        return Err("不能归并到自身".into());
                    }
                    let target = self.record(s(v, "targetId"))?;
                    self.db
                        .execute(
                            "INSERT OR REPLACE INTO aliases VALUES(?,?)",
                            params![key, s(&target, "id")],
                        )
                        .map_err(err)?;
                }
            }
            "pending" => {
                if !shared {
                    return Err("共同申请必须来自共同空间".into());
                }
                let record = self.record(s(v, "transactionId"))?;
                if record["shared"] != true && v["resolved"] != true {
                    return Err("账单尚未共享".into());
                }
                if v["resolved"] == true {
                    let requester: String = self
                        .db
                        .query_row("SELECT actor FROM pending WHERE id=?", [s(v, "id")], |r| {
                            r.get(0)
                        })
                        .map_err(err)?;
                    if requester == s(p, "memberId") {
                        return Err("同一成员不得审批自己的申请".into());
                    }
                    self.db
                        .execute("DELETE FROM pending WHERE id=?", [s(v, "id")])
                        .map_err(err)?;
                } else {
                    if s(v, "requestedBy") != s(p, "memberId") {
                        return Err("申请成员与签名成员不同".into());
                    }
                    self.db
                        .execute(
                            "INSERT OR IGNORE INTO pending VALUES(?,?,?,?,?)",
                            params![
                                s(v, "id"),
                                s(v, "kind"),
                                s(v, "transactionId"),
                                v["payload"].to_string(),
                                s(v, "requestedBy")
                            ],
                        )
                        .map_err(err)?;
                }
            }
            _ => return Err("未知同步实体类型".into()),
        }
        Ok(())
    }
}
