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
        self.queue_operation(v, space, old, None)
    }
    fn queue_operation(
        &self,
        v: &Value,
        space: &str,
        old: &Value,
        source_version: Option<&str>,
    ) -> Result<(), String> {
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
        let mut fields = v.as_object().ok_or("记录无效")?.clone();
        // 被移除的字段也要发送空值，其他设备才能清除旧共同快照。
        if let Some(previous) = old.as_object() {
            for field in previous.keys() {
                fields.entry(field.clone()).or_insert(Value::Null);
            }
        }
        for (field, value) in &fields {
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
                self.db.execute("INSERT INTO field_versions VALUES(?,?,?) ON CONFLICT(entity,field) DO UPDATE SET version=excluded.version",params![version_entity,field,source_version.unwrap_or(&operation)]).map_err(err)?;
            }
        }
        let mut payload = json!({"id":operation,"spaceId":space,"entityId":v["id"],"deviceId":identity["deviceId"],"memberId":identity["memberId"],"sequence":sequence,"payload":v,"patch":patch,"baseVersions":base,"baseVersion":0,"entityType":v["entityType"].as_str().unwrap_or("record")});
        if let Some(version) = source_version {
            payload["sourceVersion"] = json!(version);
        }
        self.db
            .execute(
                "INSERT INTO outbox(id,space,entity,payload) VALUES(?,?,?,?)",
                params![operation, space, s(v, "id"), payload.to_string()],
            )
            .map_err(err)?;
        Ok(())
    }
    pub(crate) fn apply_operation(&self, p: Value) -> Result<Value, String> {
        let duplicate = self
            .db
            .query_row(
                "SELECT 1 FROM applied WHERE id=?1 UNION ALL SELECT 1 FROM outbox WHERE id=?1 LIMIT 1",
                [s(&p, "id")], |r| r.get::<_, i32>(0),
            )
            .is_ok();
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
        if duplicate && seq <= cursor {
            return Ok(json!({"applied":false,"duplicate":true}));
        }
        if seq != cursor + 1 {
            return Err("同步序号不连续".into());
        }
        // 恢复设备可重发同一操作；跳过已有内容，仍推进新设备的连续游标。
        if duplicate {
            self.finish_operation(&p, &cursor_key, seq)?;
            return Ok(json!({"applied":false,"duplicate":true}));
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
        // 多台设备转发同一共同变更时，沿用源版本，避免生成互相冲突的个人版本。
        let version = if shared {
            s(&p, "id")
        } else {
            p["sourceVersion"].as_str().unwrap_or(s(&p, "id"))
        };

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
                conflicts.insert(field.clone(),json!({"local":old[field],"remote":value,"localVersion":current,"remoteVersion":version}));
            } else {
                merged[field] = value.clone();
                self.db.execute("INSERT INTO field_versions VALUES(?,?,?) ON CONFLICT(entity,field) DO UPDATE SET version=excluded.version",params![version_entity,field,version]).map_err(err)?;
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
        // 对方的共同变更接入个人同步链；所有者自己的保存已经生成个人操作。
        if shared
            && persisted["ownerId"] == identity["memberId"]
            && p["memberId"] != identity["memberId"]
            && persisted != stored
        {
            self.queue_operation(
                &persisted,
                s(&identity, "personalSpaceId"),
                &stored,
                Some(version),
            )?;
        }
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
            .execute("INSERT OR IGNORE INTO applied VALUES(?)", [s(p, "id")])
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

#[cfg(test)]
mod checks {
    use super::*;

    #[test]
    fn owner_changes_keep_versions_with_either_space_delivery_order() {
        for personal_first in [false, true] {
            let mut a = LedgerService::open(Path::new(":memory:"), &[1; 32]).unwrap();
            let mut b = LedgerService::open(Path::new(":memory:"), &[2; 32]).unwrap();
            let identity = a.identity().unwrap();
            let personal = s(&identity, "personalSpaceId");
            let shared = s(&identity, "sharedSpaceId");
            b.dispatch(
                "adoptIdentity",
                json!({"memberId":identity["memberId"],"personalSpaceId":personal}),
            )
            .unwrap();
            b.dispatch("adoptSharedSpace", json!({"sharedSpaceId":shared}))
                .unwrap();
            let created = a.dispatch("create", json!({"amountMinor":"100"})).unwrap();
            let key = s(&created, "id");
            a.dispatch("share", json!({"id":key})).unwrap();
            deliver(&mut a, &mut b, personal);
            deliver(&mut a, &mut b, shared);
            for (action, field) in [("updateShared", "sharedNote"), ("update", "merchant")] {
                a.dispatch(action, json!({"id":key,field:"首次编辑"}))
                    .unwrap();
                let order = if personal_first {
                    [personal, shared]
                } else {
                    [shared, personal]
                };
                for space in order {
                    deliver(&mut a, &mut b, space);
                }
                deliver(&mut b, &mut a, personal);
                deliver(&mut a, &mut b, personal);
                a.dispatch(action, json!({"id":key,field:"再次编辑"}))
                    .unwrap();
                deliver(&mut a, &mut b, personal);
                deliver(&mut a, &mut b, shared);
                assert_eq!(b.record(key).unwrap()[field], "再次编辑");
                assert!(b
                    .dispatch("pendingUploads", json!({}))
                    .unwrap()
                    .as_array()
                    .unwrap()
                    .is_empty());
            }
        }
    }

    #[test]
    fn replayed_operations_advance_new_device_cursor_once() {
        let mut a = LedgerService::open(Path::new(":memory:"), &[1; 32]).unwrap();
        let mut b = LedgerService::open(Path::new(":memory:"), &[2; 32]).unwrap();
        let created = a.dispatch("create", json!({"amountMinor":"100"})).unwrap();
        let original = a.dispatch("pendingUploads", json!({})).unwrap()[0].clone();
        assert_eq!(
            a.dispatch("applyOperation", original.clone()).unwrap()["duplicate"],
            true
        );
        b.dispatch("applyOperation", original.clone()).unwrap();
        let mut replay = original.clone();
        replay["deviceId"] = json!(id());
        assert_eq!(
            b.dispatch("applyOperation", replay.clone()).unwrap()["duplicate"],
            true
        );
        a.dispatch("update", json!({"id":created["id"],"note":"恢复后继续"}))
            .unwrap();
        let mut next = a.dispatch("pendingUploads", json!({})).unwrap()[1].clone();
        next["deviceId"] = replay["deviceId"].clone();
        assert_eq!(
            b.dispatch("applyOperation", next).unwrap()["conflict"],
            false
        );
        assert_eq!(
            b.dispatch("applyOperation", replay.clone()).unwrap()["duplicate"],
            true
        );
        replay["sequence"] = json!(4);
        assert!(b.dispatch("applyOperation", replay).is_err());
        assert_eq!(b.record(s(&created, "id")).unwrap()["note"], "恢复后继续");
    }

    fn deliver(from: &mut LedgerService, to: &mut LedgerService, space: &str) {
        let ops = from.dispatch("pendingUploads", json!({})).unwrap();
        for op in ops
            .as_array()
            .unwrap()
            .iter()
            .filter(|op| op["spaceId"] == space)
        {
            let result = to.dispatch("applyOperation", op.clone()).unwrap();
            assert_ne!(result["conflict"], true, "{op}");
            from.dispatch("markUploaded", json!({"id":op["id"]}))
                .unwrap();
        }
    }

    #[test]
    fn shared_changes_reach_personal_only_devices() {
        for deleted in [false, true] {
            let mut a1 = LedgerService::open(Path::new(":memory:"), &[1; 32]).unwrap();
            let mut a2 = LedgerService::open(Path::new(":memory:"), &[2; 32]).unwrap();
            let mut b = LedgerService::open(Path::new(":memory:"), &[3; 32]).unwrap();
            let identity = a1.identity().unwrap();
            let bi = b.identity().unwrap();
            let personal = s(&identity, "personalSpaceId");
            let shared = s(&identity, "sharedSpaceId");
            a2.dispatch(
                "adoptIdentity",
                json!({"memberId":identity["memberId"],"personalSpaceId":personal}),
            )
            .unwrap();
            b.dispatch("adoptSharedSpace", json!({"sharedSpaceId":shared}))
                .unwrap();
            a1.dispatch(
                "registerTrustedMember",
                json!({"memberId":bi["memberId"],"sharedSpaceId":shared}),
            )
            .unwrap();
            b.dispatch(
                "registerTrustedMember",
                json!({"memberId":identity["memberId"],"sharedSpaceId":shared}),
            )
            .unwrap();
            let created = a1
                .dispatch("create", json!({"amountMinor":"100","note":"私有备注"}))
                .unwrap();
            let key = s(&created, "id");
            a1.dispatch("share", json!({"id":key})).unwrap();
            if deleted {
                a1.dispatch("delete", json!({"id":key})).unwrap();
            }
            deliver(&mut a1, &mut a2, personal);
            deliver(&mut a1, &mut b, shared);
            b.dispatch("updateShared", json!({"id":key,"sharedNote":"共同备注"}))
                .unwrap();
            deliver(&mut b, &mut a1, shared);
            deliver(&mut a1, &mut a2, personal);
            assert_eq!(a2.record(key).unwrap()["sharedNote"], "共同备注");
            assert_eq!(a2.record(key).unwrap()["note"], "私有备注");
            assert!(a2
                .dispatch("pendingUploads", json!({}))
                .unwrap()
                .as_array()
                .unwrap()
                .is_empty());
            // 后续个人编辑沿个人字段版本继续，不因共同操作而出现虚假冲突。
            a2.dispatch("update", json!({"id":key,"note":"个人设备编辑"}))
                .unwrap();
            deliver(&mut a2, &mut a1, personal);
            let request = a1
                .dispatch("requestSharedDelete", json!({"id":key}))
                .unwrap();
            deliver(&mut a1, &mut b, shared);
            b.dispatch(
                "approveSharedDelete",
                json!({"id":request["pendingId"],"approve":true}),
            )
            .unwrap();
            deliver(&mut b, &mut a1, shared);
            deliver(&mut a1, &mut a2, personal);
            for service in [&mut a1, &mut a2] {
                assert_eq!(service.record(key).unwrap()["shared"], false);
                assert_eq!(service.record(key).unwrap()["deleted"], deleted);
                assert!(!service.record(key).unwrap()["sharedSnapshot"].is_object());
                assert!(service
                    .dispatch("list", json!({"shared":true}))
                    .unwrap()
                    .as_array()
                    .unwrap()
                    .is_empty());
                assert!(service
                    .dispatch("pending", json!({}))
                    .unwrap()
                    .as_array()
                    .unwrap()
                    .is_empty());
            }
        }
    }

    #[test]
    fn same_shared_change_on_two_devices_keeps_personal_versions_aligned() {
        let mut a1 = LedgerService::open(Path::new(":memory:"), &[1; 32]).unwrap();
        let mut a2 = LedgerService::open(Path::new(":memory:"), &[2; 32]).unwrap();
        let mut b = LedgerService::open(Path::new(":memory:"), &[3; 32]).unwrap();
        let identity = a1.identity().unwrap();
        let bi = b.identity().unwrap();
        let personal = s(&identity, "personalSpaceId");
        let shared = s(&identity, "sharedSpaceId");
        a2.dispatch(
            "adoptIdentity",
            json!({"memberId":identity["memberId"],"personalSpaceId":personal}),
        )
        .unwrap();
        for service in [&mut a2, &mut b] {
            service
                .dispatch("adoptSharedSpace", json!({"sharedSpaceId":shared}))
                .unwrap();
        }
        for service in [&mut a1, &mut a2] {
            service
                .dispatch(
                    "registerTrustedMember",
                    json!({"memberId":bi["memberId"],"sharedSpaceId":shared}),
                )
                .unwrap();
        }
        b.dispatch(
            "registerTrustedMember",
            json!({"memberId":identity["memberId"],"sharedSpaceId":shared}),
        )
        .unwrap();
        let created = a1.dispatch("create", json!({"amountMinor":"100"})).unwrap();
        let key = s(&created, "id");
        a1.dispatch("share", json!({"id":key})).unwrap();
        deliver(&mut a1, &mut a2, personal);
        let ops = a1.dispatch("pendingUploads", json!({})).unwrap();
        for op in ops
            .as_array()
            .unwrap()
            .iter()
            .filter(|op| op["spaceId"] == shared)
        {
            a2.dispatch("applyOperation", op.clone()).unwrap();
        }
        deliver(&mut a1, &mut b, shared);
        b.dispatch("updateShared", json!({"id":key,"sharedNote":"共同备注"}))
            .unwrap();
        let ops = b.dispatch("pendingUploads", json!({})).unwrap();
        for op in ops
            .as_array()
            .unwrap()
            .iter()
            .filter(|op| op["spaceId"] == shared)
        {
            a2.dispatch("applyOperation", op.clone()).unwrap();
        }
        deliver(&mut b, &mut a1, shared);
        deliver(&mut a1, &mut a2, personal);
        deliver(&mut a2, &mut a1, personal);
        a1.dispatch("updateShared", json!({"id":key,"sharedNote":"后续编辑"}))
            .unwrap();
        deliver(&mut a1, &mut a2, personal);
        assert_eq!(a2.record(key).unwrap()["sharedNote"], "后续编辑");
    }
}
