use super::*;
impl LedgerService {
    pub(crate) fn run(&mut self, action: &str, p: Value) -> Result<Value, String> {
        match action {
            "registerTrustedMember"
            | "revokeTrustedMember"
            | "members"
            | "capture"
            | "confirmCandidate"
            | "resolveConflict"
            | "resolveDifference"
            | "mergeTransactions"
            | "splitTransactions"
            | "sources"
            | "setRetention"
            | "purgeSources" => self.workflow(action, p),
            "create" => self.create(p, false),
            "updateShared" => {
                let old = self.record(s(&p, "id"))?;
                if old["shared"] != true || !self.trusted(s(&old, "ownerId"))? {
                    return Err("账单未获共同编辑授权".into());
                }
                let mut value = old.clone();
                for field in ["sharedNote", "sharedCategory"] {
                    if !p[field].is_null() {
                        value[field] = p[field].clone();
                    }
                }
                self.save(
                    &value,
                    "updateShared",
                    s(&self.identity()?, "memberId"),
                    old,
                )?;
                Ok(value)
            }
            "update" => {
                let old = self.record(s(&p, "id"))?;
                let mut v = old.clone();
                let identity = self.identity()?;
                let actor = s(&identity, "memberId");
                if actor != s(&old, "ownerId") {
                    if old["shared"] != true || !self.trusted(s(&old, "ownerId"))? {
                        return Err("不能修改未获准的账单".into());
                    }
                    let key = self.pending("coreModification", s(&old, "id"), p.clone(), actor)?;
                    return Ok(json!({"pendingId":key}));
                }
                let mut locks = old["locks"].as_array().cloned().unwrap_or_default();
                for (k, value) in p.as_object().ok_or("字段格式错误")? {
                    if ["id", "ownerId", "deleted", "shared", "actorId", "locks"]
                        .contains(&k.as_str())
                    {
                        continue;
                    }
                    v[k] = value.clone();
                    if !locks.contains(&json!(k)) {
                        locks.push(json!(k));
                    }
                }
                v["locks"] = json!(locks);
                self.validate(&v)?;
                self.save(&v, "update", actor, old)?;
                Ok(v)
            }
            "delete" | "restore" | "share" => {
                let old = self.record(s(&p, "id"))?;
                let mut v = old.clone();
                let identity = self.identity()?;
                if s(&identity, "memberId") != s(&v, "ownerId") {
                    return Err("只有所有者能修改个人账单".into());
                }
                v[if action == "share" {
                    "shared"
                } else {
                    "deleted"
                }] = json!(action != "restore");
                self.save(&v, action, s(&identity, "memberId"), old)?;
                Ok(v)
            }
            "list" => self.list(&p),
            "summary" => self.summary(&p),
            "analysis" => self.analysis(&p),
            "adoptIdentity" => {
                if self
                    .db
                    .query_row("SELECT count(*) FROM records", [], |r| r.get::<_, i64>(0))
                    .map_err(err)?
                    > 0
                {
                    return Err("请先备份；已有账本不能自动切换成员".into());
                }
                let mut identity = self.identity()?;
                for field in ["memberId", "personalSpaceId"] {
                    Uuid::parse_str(s(&p, field)).map_err(err)?;
                    identity[field] = p[field].clone();
                }
                self.db
                    .execute(
                        "UPDATE settings SET data=? WHERE id='identity'",
                        [identity.to_string()],
                    )
                    .map_err(err)?;
                Ok(identity)
            }
            "parseText" => Ok(self.parse_text(s(&p, "text"))),
            "importText" => {
                let parsed = self.parse_text(s(&p, "text"));
                if !parsed["missingFields"].as_array().unwrap().is_empty() {
                    return Ok(
                        json!({"status":"needsConfirmation","message":"请补充金额、方向或付款人","clarification":parsed}),
                    );
                }
                self.create(parsed["draft"].clone(), false)
            }
            "pending" => {
                let mut st = self
                    .db
                    .prepare("SELECT id,kind,record_id,payload,actor FROM pending")
                    .map_err(err)?;
                let r=st.query_map([],|r|Ok(json!({"id":r.get::<_,String>(0)?,"kind":r.get::<_,String>(1)?,"transactionId":r.get::<_,String>(2)?,"payload":serde_json::from_str::<Value>(&r.get::<_,String>(3)?).unwrap_or(Value::Null),"requestedBy":r.get::<_,String>(4)?}))).map_err(err)?;
                Ok(json!(r.collect::<Result<Vec<_>, _>>().map_err(err)?))
            }
            "resolveDuplicate" => {
                let (record, text): (String, String) = self
                    .db
                    .query_row(
                        "SELECT record_id,payload FROM pending WHERE id=? AND kind='duplicate'",
                        [s(&p, "id")],
                        |r| Ok((r.get(0)?, r.get(1)?)),
                    )
                    .map_err(err)?;
                let candidate: Value = serde_json::from_str(&text).map_err(err)?;
                let result = if p["merge"] == true {
                    if !s(&candidate, "captureId").is_empty() {
                        self.db
                            .execute(
                                "INSERT OR IGNORE INTO captures VALUES(?,?)",
                                params![s(&candidate, "captureId"), record],
                            )
                            .map_err(err)?;
                    }
                    self.db
                        .execute(
                            "INSERT INTO settings VALUES(?,?)",
                            params![format!("merged:{}", s(&p, "id")), candidate.to_string()],
                        )
                        .map_err(err)?;
                    json!({"status":"merged","id":record,"mergeId":p["id"]})
                } else {
                    self.create(candidate, true)?
                };
                self.db
                    .execute("DELETE FROM pending WHERE id=?", [s(&p, "id")])
                    .map_err(err)?;
                Ok(result)
            }
            "splitDuplicate" => {
                let archive = format!("merged:{}", s(&p, "id"));
                let text: String = self
                    .db
                    .query_row("SELECT data FROM settings WHERE id=?", [&archive], |r| {
                        r.get(0)
                    })
                    .map_err(err)?;
                let mut candidate: Value = serde_json::from_str(&text).map_err(err)?;
                self.db
                    .execute(
                        "DELETE FROM captures WHERE id=?",
                        [s(&candidate, "captureId")],
                    )
                    .map_err(err)?;
                candidate["id"] = json!(id());
                let result = self.create(candidate, true)?;
                self.db
                    .execute("DELETE FROM settings WHERE id=?", [archive])
                    .map_err(err)?;
                Ok(result)
            }
            "adoptSharedSpace" => {
                Uuid::parse_str(s(&p, "sharedSpaceId")).map_err(err)?;
                let mut identity = self.identity()?;
                if identity["sharedSpaceId"] != p["sharedSpaceId"]
                    && self
                        .db
                        .query_row(
                            "SELECT count(*) FROM records WHERE json_extract(data,'$.shared')=1",
                            [],
                            |r| r.get::<_, i64>(0),
                        )
                        .map_err(err)?
                        > 0
                {
                    return Err("现有共享账单尚未迁出，不能切换共同空间".into());
                }
                identity["sharedSpaceId"] = p["sharedSpaceId"].clone();
                self.db
                    .execute(
                        "UPDATE settings SET data=? WHERE id='identity'",
                        [identity.to_string()],
                    )
                    .map_err(err)?;
                Ok(identity)
            }
            "requestSharedDelete" => {
                let v = self.record(s(&p, "id"))?;
                if v["shared"] != true {
                    return Err("账单尚未共享".into());
                }
                let key = self.pending(
                    "sharedDelete",
                    s(&p, "id"),
                    json!({}),
                    s(&self.identity()?, "memberId"),
                )?;
                Ok(json!({"pendingId":key}))
            }
            "approveSharedDelete" | "approveModification" => {
                let (record, actor, text, kind): (String, String, String, String) = self
                    .db
                    .query_row(
                        "SELECT record_id,actor,payload,kind FROM pending WHERE id=?",
                        [s(&p, "id")],
                        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
                    )
                    .map_err(err)?;
                let identity = self.identity()?;
                let approver = s(&identity, "memberId");
                if !self.trusted(&actor)? {
                    return Err("申请人不在可信共同成员中".into());
                }
                if actor == approver {
                    return Err("审批人与发起人必须为不同成员".into());
                }
                let old = self.record(&record)?;
                let mut v = old.clone();
                if p["approve"] == true {
                    if kind == "sharedDelete" {
                        v["shared"] = json!(false);
                    } else if kind == "coreModification" && approver == s(&old, "ownerId") {
                        let patch: Value = serde_json::from_str(&text).map_err(err)?;
                        for key in ["amountMinor", "occurredAt", "payerId", "accountId"] {
                            if !patch[key].is_null() {
                                v[key] = patch[key].clone();
                            }
                        }
                        self.validate(&v)?;
                    } else {
                        return Err("修改申请必须由所有者批准".into());
                    }
                    self.save(&v, "approve", approver, old)?;
                }
                self.queue(&json!({"id":p["id"],"entityType":"pending","kind":kind,"transactionId":record,"payload":{},"requestedBy":actor,"resolved":true,"approved":p["approve"]}),s(&identity,"sharedSpaceId"),&Value::Null)?;
                self.db
                    .execute("DELETE FROM pending WHERE id=?", [s(&p, "id")])
                    .map_err(err)?;
                Ok(v)
            }
            "accounts" | "categories" | "rules" => {
                let prefix = match action {
                    "accounts" => "account:",
                    "categories" => "category:",
                    _ => "rule:",
                };
                let mut st = self
                    .db
                    .prepare("SELECT data FROM settings WHERE id LIKE ?")
                    .map_err(err)?;
                let rows = st
                    .query_map([format!("{prefix}%")], |r| r.get::<_, String>(0))
                    .map_err(err)?
                    .map(|r| serde_json::from_str::<Value>(&r.map_err(err)?).map_err(err))
                    .collect::<Result<Vec<_>, String>>()?;
                if action == "categories" {
                    let mut all = vec![
                        json!({"id":"food","name":"餐饮"}),
                        json!({"id":"transport","name":"交通"}),
                        json!({"id":"shopping","name":"购物"}),
                        json!({"id":"other","name":"其他"}),
                    ];
                    all.extend(rows);
                    return Ok(json!(all));
                }
                Ok(json!(rows))
            }
            "addAccount" | "addCategory" | "learnRule" | "setRule" => {
                let key = if action == "setRule" {
                    s(&p, "id").to_string()
                } else {
                    id()
                };
                let prefix = match action {
                    "addAccount" => "account:",
                    "addCategory" => "category:",
                    _ => "rule:",
                };
                let mut v = if action == "setRule" {
                    let text: String = self
                        .db
                        .query_row(
                            "SELECT data FROM settings WHERE id=?",
                            [format!("rule:{key}")],
                            |r| r.get(0),
                        )
                        .map_err(err)?;
                    serde_json::from_str(&text).map_err(err)?
                } else {
                    p.clone()
                };
                if action == "setRule" {
                    v["enabled"] = p["enabled"].clone();
                }
                v["id"] = json!(key);
                if action == "addAccount" {
                    if s(&p, "name").is_empty() {
                        return Err("账户名称不能为空".into());
                    }
                    v["ownerId"] = self.identity()?["memberId"].clone();
                    v["confirmed"] = json!(true);
                }
                if action == "learnRule" {
                    v["enabled"] = json!(true);
                }
                self.metadata(&format!("{prefix}{key}"), &v)?;
                Ok(v)
            }
            "revokeRule" => {
                self.metadata(&format!("rule:{}", s(&p, "id")), &Value::Null)?;
                self.db
                    .execute(
                        "DELETE FROM settings WHERE id=?",
                        [format!("rule:{}", s(&p, "id"))],
                    )
                    .map_err(err)?;
                Ok(json!({}))
            }
            "history" => {
                let mut st=self.db.prepare("SELECT actor,action,before_data,after_data,at FROM history WHERE record_id=? ORDER BY at").map_err(err)?;
                let rows=st.query_map([s(&p,"id")],|r|Ok(json!({"actor":r.get::<_,String>(0)?,"action":r.get::<_,String>(1)?,"before":r.get::<_,String>(2)?,"after":r.get::<_,String>(3)?,"at":r.get::<_,String>(4)?}))).map_err(err)?.collect::<Result<Vec<_>,_>>().map_err(err)?;
                Ok(json!(rows))
            }
            "identity" | "syncIdentity" => self.identity(),
            "syncCursor" => Ok(
                json!({"sequence":self.db.query_row("SELECT sequence FROM cursors WHERE device=?",[format!("{}:{}",s(&p,"spaceId"),s(&p,"deviceId"))],|r|r.get::<_,i64>(0)).unwrap_or(0)}),
            ),
            "pendingUploads" => {
                let mut st = self
                    .db
                    .prepare("SELECT payload FROM outbox WHERE uploaded=0 ORDER BY rowid")
                    .map_err(err)?;
                let rows = st
                    .query_map([], |r| r.get::<_, String>(0))
                    .map_err(err)?
                    .map(|r| serde_json::from_str::<Value>(&r.map_err(err)?).map_err(err))
                    .collect::<Result<Vec<_>, String>>()?;
                Ok(json!(rows))
            }
            "markUploaded" => {
                self.db
                    .execute("UPDATE outbox SET uploaded=1 WHERE id=?", [s(&p, "id")])
                    .map_err(err)?;
                Ok(json!({}))
            }
            "applyOperation" => self.apply_operation(p),
            "exportJson" | "exportCsv" | "importJson" | "importCsv" | "exportBackup"
            | "restoreBackup" => self.transfer(action, p),
            "cipherVersion" => Ok(
                json!({"version":self.db.query_row("PRAGMA cipher_version",[],|r|r.get::<_,String>(0)).map_err(err)?}),
            ),
            _ => Err(format!("未知操作: {action}")),
        }
    }
}
