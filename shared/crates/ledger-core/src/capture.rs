use super::*;
impl LedgerService {
    pub(crate) fn create(&self, mut v: Value, force: bool) -> Result<Value, String> {
        if !v.is_object() {
            return Err("账单必须为对象".into());
        }
        for (key, default) in [
            ("id", json!(id())),
            ("ownerId", self.identity()?["memberId"].clone()),
            ("payerId", self.identity()?["memberId"].clone()),
            ("currencyCode", json!("CNY")),
            ("kind", json!("expense")),
            ("occurredAt", json!(chrono::Utc::now().to_rfc3339())),
            ("merchant", json!("")),
            ("category", json!("其他")),
            ("note", json!("")),
            ("accountId", json!("")),
            ("deleted", json!(false)),
            ("shared", json!(false)),
            ("locks", json!([])),
        ] {
            if v[key].is_null() {
                v[key] = default;
            }
        }
        if self.record(s(&v, "id")).is_ok() {
            return Err("账单ID已存在，不能覆盖已有账单".into());
        }
        let identity = self.identity()?;
        v["ownerId"] = identity["memberId"].clone();
        if s(&v, "payerId") == "local" {
            v["payerId"] = identity["memberId"].clone();
        }
        v["deleted"] = json!(false);
        v["shared"] = json!(false);
        v["locks"] = json!([]);
        self.apply_rules(&mut v)?;
        self.validate(&v)?;
        let time = chrono::DateTime::parse_from_rfc3339(s(&v, "occurredAt")).map_err(err)?;
        v["originalOffsetMinutes"] = json!(time.offset().local_minus_utc() / 60);
        v["occurredAt"] = json!(time
            .with_timezone(&chrono::Utc)
            .to_rfc3339_opts(chrono::SecondsFormat::Secs, true));
        if !s(&v, "captureId").is_empty() {
            if let Ok(key) = self.db.query_row(
                "SELECT record_id FROM captures WHERE id=?",
                [s(&v, "captureId")],
                |r| r.get::<_, String>(0),
            ) {
                return Ok(json!({"status":"existing","id":key,"message":"此来源已经处理"}));
            }
        }
        for old in self.dedup_candidates(&v)? {
            if !s(&v, "transactionKey").is_empty()
                && s(&v, "transactionKey") == s(&old, "transactionKey")
                && s(&v, "payerId") == s(&old, "payerId")
            {
                if !s(&v, "captureId").is_empty() {
                    self.db
                        .execute(
                            "INSERT INTO captures VALUES(?,?)",
                            params![s(&v, "captureId"), s(&old, "id")],
                        )
                        .map_err(err)?;
                }
                self.source(&v, s(&old, "id"))?;
                let mut fields = serde_json::Map::new();
                for field in [
                    "amountMinor",
                    "kind",
                    "occurredAt",
                    "payerId",
                    "accountId",
                    "merchant",
                    "category",
                    "note",
                ] {
                    if !v[field].is_null() && old[field] != v[field] {
                        fields.insert(field.into(), json!({"local":old[field],"remote":v[field]}));
                    }
                }
                if !fields.is_empty() {
                    self.pending(
                        "difference",
                        s(&old, "id"),
                        json!({"fields":fields}),
                        s(&identity, "memberId"),
                    )?;
                }
                return Ok(
                    json!({"status":"existing","id":old["id"],"message":"已关联来源，字段差异待确认"}),
                );
            }
            if !force
                && old["deleted"] != true
                && s(&old, "amountMinor") == s(&v, "amountMinor")
                && s(&old, "merchant") == s(&v, "merchant")
                && !s(&v, "merchant").is_empty()
                && s(&old, "payerId") == s(&v, "payerId")
                && s(&old, "kind") == s(&v, "kind")
                && s(&old, "currencyCode") == s(&v, "currencyCode")
            {
                let a = chrono::DateTime::parse_from_rfc3339(s(&old, "occurredAt")).map_err(err)?;
                let b = chrono::DateTime::parse_from_rfc3339(s(&v, "occurredAt")).map_err(err)?;
                if (a - b).num_seconds().abs() < 300 {
                    let key = self.pending("duplicate", s(&old, "id"), v.clone(), "local")?;
                    return Ok(
                        json!({"status":"duplicate","id":old["id"],"pendingId":key,"message":"疑似重复，确认前不计入统计"}),
                    );
                }
            }
        }
        if !s(&v, "captureId").is_empty() {
            self.db
                .execute(
                    "INSERT INTO captures VALUES(?,?)",
                    params![s(&v, "captureId"), s(&v, "id")],
                )
                .map_err(err)?;
        }
        self.source(&v, s(&v, "id"))?;
        if let Some(fields) = v.as_object_mut() {
            fields.remove("rawText");
            fields.remove("rawImage");
        }
        self.save(&v, "create", s(&identity, "memberId"), Value::Null)?;
        Ok(json!({"status":"created","id":v["id"],"message":"已入账"}))
    }
    pub(crate) fn parse_text(&self, text: &str) -> Value {
        let amount = text
            .split(|c: char| !c.is_ascii_digit() && c != '.')
            .find(|s| !s.is_empty());
        let parsed = amount.and_then(|n| {
            let mut p = n.split('.');
            let whole = p.next()?.parse::<i64>().ok()?;
            let frac = p.next().unwrap_or("");
            if frac.len() > 2 || p.next().is_some() {
                return None;
            }
            whole
                .checked_mul(100)?
                .checked_add(format!("{frac:0<2}").parse::<i64>().ok()?)
        });
        let mut missing = vec![];
        let numeric_groups = text
            .split(|c: char| !c.is_ascii_digit() && c != '.')
            .filter(|n| !n.is_empty())
            .count();
        if numeric_groups > 1 || text.contains("尾号") || text.contains("年") || text.contains("-")
        {
            missing.push("amountMinor");
        }
        if parsed.is_none() {
            missing.push("amountMinor");
        }
        let kind = if text.contains("收入") || text.contains("工资") {
            "income"
        } else if text.contains("转账") {
            "transfer"
        } else if ["买", "饭", "支出", "花", "支付", "消费"]
            .iter()
            .any(|w| text.contains(w))
        {
            "expense"
        } else {
            missing.push("kind");
            "expense"
        };
        json!({"draft":{"amountMinor":parsed.map(|a|a.to_string()),"currencyCode":"CNY","kind":kind,"occurredAt":chrono::Utc::now().to_rfc3339(),"merchant":text,"note":text,"payerId":self.identity().ok().map(|v|v["memberId"].clone()),"category":if text.contains("饭"){ "餐饮" }else{"其他"}},"missingFields":missing})
    }
}
