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
                    let mut patch = p.clone();
                    // 共同投影不带私有字段，表单补出的空值不代表清空所有者数据。
                    for field in ["note", "accountId"] {
                        if old[field].is_null() && s(&patch, field).is_empty() {
                            patch.as_object_mut().ok_or("字段格式错误")?.remove(field);
                        }
                    }
                    let key = self.pending("coreModification", s(&old, "id"), patch, actor)?;
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
                let mut identity = self.identity()?;
                for field in ["memberId", "personalSpaceId"] {
                    Uuid::parse_str(s(&p, field)).map_err(err)?;
                }
                // 恢复后的新设备重新加入原账本，无需切换成员或个人空间。
                if identity["memberId"] == p["memberId"]
                    && identity["personalSpaceId"] == p["personalSpaceId"]
                {
                    return Ok(identity);
                }
                if self
                    .db
                    .query_row("SELECT count(*) FROM records", [], |r| r.get::<_, i64>(0))
                    .map_err(err)?
                    > 0
                {
                    if self
                        .db
                        .query_row(
                            "SELECT 1 FROM settings WHERE id='independentRecovery'",
                            [],
                            |r| r.get::<_, i32>(0),
                        )
                        .is_ok()
                    {
                        return Err("旧备份已恢复为独立账本，不能切换到原空间；请从本机导出账本信息，供空设备加入本账本".into());
                    }
                    return Err("请先备份；已有账本不能自动切换成员或个人空间".into());
                }
                for field in ["memberId", "personalSpaceId"] {
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
                        let mut locks = old["locks"].as_array().cloned().unwrap_or_default();
                        // 审批覆盖编辑器账务字段，标识、所有权和共享状态仍由专用操作维护。
                        for key in [
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
                        ] {
                            if !patch[key].is_null() {
                                v[key] = patch[key].clone();
                                if !locks.contains(&json!(key)) {
                                    locks.push(json!(key));
                                }
                            }
                        }
                        v["locks"] = json!(locks);
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

#[cfg(test)]
mod checks {
    use super::*;

    fn draft() -> Value {
        json!({"amountMinor":"2800","currencyCode":"CNY","kind":"expense","occurredAt":"2026-09-10T12:00:00Z","merchant":"午饭","payerId":"local","note":"个人备注","accountId":"个人账户"})
    }

    fn deliver(from: &mut LedgerService, to: &mut LedgerService, space: &str) {
        let operations = from.dispatch("pendingUploads", json!({})).unwrap();
        for operation in operations.as_array().unwrap() {
            if operation["spaceId"] == space {
                to.dispatch("applyOperation", operation.clone()).unwrap();
                from.dispatch("markUploaded", json!({"id":operation["id"]}))
                    .unwrap();
            }
        }
    }

    #[test]
    fn restored_ledger_rejoins_and_continues_history_without_losing_pending_edits() {
        let mut original_ledger = LedgerService::open(Path::new(":memory:"), &[1; 32]).unwrap();
        let mut peer = LedgerService::open(Path::new(":memory:"), &[2; 32]).unwrap();
        let original = original_ledger.identity().unwrap();
        let space = s(&original, "personalSpaceId");
        peer.dispatch("adoptIdentity", original.clone()).unwrap();
        let record = original_ledger.dispatch("create", draft()).unwrap();
        let key = s(&record, "id");
        original_ledger
            .dispatch("update", json!({"id":key,"note":"已同步备注"}))
            .unwrap();
        deliver(&mut original_ledger, &mut peer, space);
        peer.dispatch("update", json!({"id":key,"merchant":"已同步商户"}))
            .unwrap();
        deliver(&mut peer, &mut original_ledger, space);
        original_ledger
            .dispatch("update", json!({"id":key,"note":"待同步备注"}))
            .unwrap();
        let pending = original_ledger
            .dispatch("pendingUploads", json!({}))
            .unwrap();
        let path = std::env::temp_dir().join(format!("lightledger-pairing-{}.backup", id()));
        let backup = json!({"path":path,"password":"backup-password"});
        original_ledger
            .dispatch("exportBackup", backup.clone())
            .unwrap();
        let mut ledger = LedgerService::open(Path::new(":memory:"), &[3; 32]).unwrap();
        ledger.dispatch("restoreBackup", backup).unwrap();
        std::fs::remove_file(path).unwrap();
        let restored = ledger.identity().unwrap();
        assert_ne!(restored["deviceId"], original["deviceId"]);
        assert_eq!(
            ledger.dispatch("adoptIdentity", original.clone()).unwrap(),
            restored
        );
        for field in ["memberId", "personalSpaceId"] {
            let mut other = original.clone();
            other[field] = json!(id());
            assert!(ledger.dispatch("adoptIdentity", other).is_err());
        }
        assert_eq!(ledger.identity().unwrap(), restored);
        let requeued = ledger.dispatch("pendingUploads", json!({})).unwrap();
        assert_eq!(requeued.as_array().unwrap().len(), 1);
        for field in ["id", "baseVersions", "payload", "patch"] {
            assert_eq!(requeued[0][field], pending[0][field], "{field}");
        }
        assert_eq!(requeued[0]["deviceId"], restored["deviceId"]);
        assert_eq!(requeued[0]["sequence"], 1);
        assert_eq!(
            ledger
                .dispatch(
                    "syncCursor",
                    json!({"spaceId":space,"deviceId":original["deviceId"]})
                )
                .unwrap()["sequence"],
            3
        );

        // 原设备和恢复设备都可能上传同一待办；操作只生效一次，游标仍分别推进。
        deliver(&mut original_ledger, &mut peer, space);
        deliver(&mut ledger, &mut peer, space);
        original_ledger
            .dispatch("update", json!({"id":key,"note":"备份后的备注"}))
            .unwrap();
        deliver(&mut original_ledger, &mut ledger, space);
        peer.dispatch("update", json!({"id":key,"category":"备份后的分类"}))
            .unwrap();
        deliver(&mut peer, &mut ledger, space);
        assert_eq!(ledger.record(key).unwrap()["note"], "备份后的备注");
        assert_eq!(ledger.record(key).unwrap()["category"], "备份后的分类");
        assert_eq!(peer.record(key).unwrap()["note"], "待同步备注");
        for service in [&mut ledger, &mut peer] {
            assert!(service
                .dispatch("pending", json!({}))
                .unwrap()
                .as_array()
                .unwrap()
                .is_empty());
        }
        ledger
            .dispatch("update", json!({"id":key,"merchant":"新设备后续修改"}))
            .unwrap();
        assert_eq!(
            ledger.dispatch("pendingUploads", json!({})).unwrap()[0]["sequence"],
            2
        );
    }

    #[test]
    fn approved_editor_fields_apply_without_replacing_record_identity() {
        let mut owner = LedgerService::open(Path::new(":memory:"), &[1; 32]).unwrap();
        let mut member = LedgerService::open(Path::new(":memory:"), &[2; 32]).unwrap();
        let oi = owner.identity().unwrap();
        let mi = member.identity().unwrap();
        let space = s(&oi, "sharedSpaceId");
        member
            .dispatch("adoptSharedSpace", json!({"sharedSpaceId":space}))
            .unwrap();
        owner
            .dispatch(
                "registerTrustedMember",
                json!({"memberId":mi["memberId"],"sharedSpaceId":space}),
            )
            .unwrap();
        member
            .dispatch(
                "registerTrustedMember",
                json!({"memberId":oi["memberId"],"sharedSpaceId":space}),
            )
            .unwrap();
        let created = owner.dispatch("create", draft()).unwrap();
        let record = owner.record(s(&created, "id")).unwrap();
        let key = s(&record, "id");
        owner.dispatch("share", json!({"id":key})).unwrap();
        deliver(&mut owner, &mut member, space);

        // 未展示的私有字段由编辑器补空，批准后必须保留所有者原值。
        let request = member
            .dispatch(
                "update",
                json!({"id":key,"merchant":"晚饭","note":"","accountId":""}),
            )
            .unwrap();
        deliver(&mut member, &mut owner, space);
        owner
            .dispatch(
                "approveModification",
                json!({"id":request["pendingId"],"approve":true}),
            )
            .unwrap();
        assert_eq!(owner.record(key).unwrap()["note"], "个人备注");
        assert_eq!(owner.record(key).unwrap()["accountId"], "个人账户");
        deliver(&mut owner, &mut member, space);

        let patch = json!({"id":key,"amountMinor":"3500","currencyCode":"USD","kind":"income","occurredAt":"2026-09-11T13:00:00Z","merchant":"新商户","category":"新分类","payerId":mi["memberId"],"accountId":"新账户","note":"申请备注","originalTransactionId":"","ownerId":mi["memberId"],"deleted":true,"shared":false,"captureId":"替换来源","transactionKey":"替换交易","locks":[]});
        let request = member.dispatch("update", patch.clone()).unwrap();
        deliver(&mut member, &mut owner, space);
        owner
            .dispatch(
                "approveModification",
                json!({"id":request["pendingId"],"approve":true}),
            )
            .unwrap();
        let approved = owner.record(key).unwrap();
        for field in [
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
        ] {
            assert_eq!(approved[field], patch[field], "{field}");
            assert!(approved["locks"]
                .as_array()
                .unwrap()
                .contains(&json!(field)));
        }
        for field in ["id", "ownerId", "deleted", "captureId", "transactionKey"] {
            assert_eq!(approved[field], record[field], "{field}");
        }
        assert_eq!(approved["shared"], true);
        assert!(owner
            .dispatch("pending", json!({}))
            .unwrap()
            .as_array()
            .unwrap()
            .is_empty());
        deliver(&mut owner, &mut member, space);
        let shared = member.record(key).unwrap();
        for field in [
            "amountMinor",
            "currencyCode",
            "kind",
            "merchant",
            "category",
        ] {
            assert_eq!(shared[field], patch[field], "{field}");
        }
        assert!(shared["note"].is_null());
        assert!(shared["accountId"].is_null());
    }
}
