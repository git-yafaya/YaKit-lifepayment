use super::*;
impl LedgerService {
    pub(crate) fn list(&self, p: &Value) -> Result<Value, String> {
        let page = p["page"].as_i64().unwrap_or(0).max(0);
        let size = p["pageSize"].as_i64().unwrap_or(50).clamp(1, 200);
        let mut st=self.db.prepare(&"SELECT data FROM records WHERE COALESCE(json_extract(data,'$.deleted'),0)=? AND ((?=0 AND json_extract(data,'$.ownerId')=?) OR (?=1 AND json_extract(data,'$.shared')=1)) AND (instr(COALESCE(json_extract(data,'$.merchant'),''),?)>0 OR instr(COALESCE(json_extract(data,'$.note'),''),?)>0 OR instr(COALESCE(json_extract(data,'$.category'),''),?)>0) AND (?='' OR date(json_extract(data,'$.occurredAt'),'localtime')>=?) AND (?='' OR date(json_extract(data,'$.occurredAt'),'localtime')<=?) ORDER BY julianday(json_extract(data,'$.occurredAt')) DESC,id LIMIT ? OFFSET ?".replace("FROM records",self.query_source(p))).map_err(err)?;
        let text = s(p, "search");
        let rows = st
            .query_map(
                params![
                    p["deleted"].as_bool().unwrap_or(false),
                    p["shared"].as_bool().unwrap_or(false),
                    s(&self.identity()?, "memberId"),
                    p["shared"].as_bool().unwrap_or(false),
                    text,
                    text,
                    text,
                    s(p, "from"),
                    s(p, "from"),
                    s(p, "to"),
                    s(p, "to"),
                    size,
                    page.saturating_mul(size)
                ],
                |r| r.get::<_, String>(0),
            )
            .map_err(err)?
            .map(|r| serde_json::from_str::<Value>(&r.map_err(err)?).map_err(err))
            .collect::<Result<Vec<_>, String>>()?;
        Ok(json!(rows))
    }
    pub(crate) fn summary(&self, p: &Value) -> Result<Value, String> {
        let mut st=self.db.prepare(&"SELECT json_extract(data,'$.currencyCode'),SUM(CASE WHEN json_extract(data,'$.kind')='expense' THEN CAST(json_extract(data,'$.amountMinor') AS INTEGER) ELSE 0 END),SUM(CASE WHEN json_extract(data,'$.kind')='income' THEN CAST(json_extract(data,'$.amountMinor') AS INTEGER) ELSE 0 END),SUM(CASE WHEN json_extract(data,'$.kind')='refund' THEN CAST(json_extract(data,'$.amountMinor') AS INTEGER) ELSE 0 END),count(*) FROM records WHERE COALESCE(json_extract(data,'$.deleted'),0)=0 AND ((?=0 AND json_extract(data,'$.ownerId')=?) OR (?=1 AND json_extract(data,'$.shared')=1)) AND (?='' OR date(json_extract(data,'$.occurredAt'),'localtime')>=?) AND (?='' OR date(json_extract(data,'$.occurredAt'),'localtime')<=?) GROUP BY json_extract(data,'$.currencyCode')".replace("FROM records",self.query_source(p))).map_err(err)?;
        let rows=st.query_map(params![p["shared"].as_bool().unwrap_or(false),s(&self.identity()?,"memberId"),p["shared"].as_bool().unwrap_or(false),s(p,"from"),s(p,"from"),s(p,"to"),s(p,"to")],|r|Ok(json!({"currencyCode":r.get::<_,String>(0)?,"expenseMinor":r.get::<_,i64>(1)?.to_string(),"incomeMinor":r.get::<_,i64>(2)?.to_string(),"refundMinor":r.get::<_,i64>(3)?.to_string(),"count":r.get::<_,i64>(4)?}))).map_err(err)?.collect::<Result<Vec<_>,_>>().map_err(err)?;
        Ok(json!(rows))
    }
    pub(crate) fn dedup_candidates(&self, v: &Value) -> Result<Vec<Value>, String> {
        let mut st=self.db.prepare("SELECT data FROM records WHERE (json_extract(data,'$.transactionKey')=? AND ?!='') OR (json_extract(data,'$.amountMinor')=? AND json_extract(data,'$.merchant')=?)").map_err(err)?;
        let rows = st
            .query_map(
                params![
                    s(v, "transactionKey"),
                    s(v, "transactionKey"),
                    s(v, "amountMinor"),
                    s(v, "merchant")
                ],
                |r| r.get::<_, String>(0),
            )
            .map_err(err)?
            .map(|r| serde_json::from_str::<Value>(&r.map_err(err)?).map_err(err))
            .collect::<Result<Vec<_>, String>>()?;
        Ok(rows)
    }
    pub(crate) fn apply_rules(&self, v: &mut Value) -> Result<(), String> {
        if s(v, "category") != "其他" && !s(v, "category").is_empty() {
            return Ok(());
        }
        let mut st = self
            .db
            .prepare("SELECT data FROM settings WHERE id LIKE 'rule:%'")
            .map_err(err)?;
        for text in st.query_map([], |r| r.get::<_, String>(0)).map_err(err)? {
            let rule: Value = serde_json::from_str(&text.map_err(err)?).map_err(err)?;
            if rule["enabled"] == true
                && !s(&rule, "merchant").is_empty()
                && s(v, "merchant").contains(s(&rule, "merchant"))
            {
                v["category"] = rule["category"].clone();
                break;
            }
        }
        Ok(())
    }
    pub(crate) fn analysis(&self, p: &Value) -> Result<Value, String> {
        let mut result = json!({});
        for (name, expr) in [
            (
                "monthly",
                "strftime('%Y-%m',json_extract(data,'$.occurredAt'),'localtime')",
            ),
            ("categories", "json_extract(data,'$.category')"),
            ("payers", "json_extract(data,'$.payerId')"),
        ] {
            let sql=format!("SELECT {expr},SUM(CASE WHEN json_extract(data,'$.kind')='refund' THEN -CAST(json_extract(data,'$.amountMinor') AS INTEGER) ELSE CAST(json_extract(data,'$.amountMinor') AS INTEGER) END),json_extract(data,'$.currencyCode') FROM records WHERE COALESCE(json_extract(data,'$.deleted'),0)=0 AND json_extract(data,'$.kind') IN ('expense','refund') AND ((?=0 AND json_extract(data,'$.ownerId')=?) OR (?=1 AND json_extract(data,'$.shared')=1)) AND (?='' OR date(json_extract(data,'$.occurredAt'),'localtime')>=?) AND (?='' OR date(json_extract(data,'$.occurredAt'),'localtime')<=?) GROUP BY {expr},json_extract(data,'$.currencyCode') ORDER BY {expr}");
            let sql = sql.replace("FROM records", self.query_source(p));
            let mut st = self.db.prepare(&sql).map_err(err)?;
            let rows=st.query_map(params![p["shared"].as_bool().unwrap_or(false),s(&self.identity()?,"memberId"),p["shared"].as_bool().unwrap_or(false),s(p,"from"),s(p,"from"),s(p,"to"),s(p,"to")],|r|Ok(json!({"label":r.get::<_,String>(0)?,"amountMinor":r.get::<_,i64>(1)?.to_string(),"currencyCode":r.get::<_,String>(2)?}))).map_err(err)?.collect::<Result<Vec<_>,_>>().map_err(err)?;
            result[name] = json!(rows);
        }
        Ok(result)
    }
    fn query_source(&self, p: &Value) -> &'static str {
        if p["shared"] == true {
            "FROM (SELECT id,COALESCE(json_extract(data,'$.sharedSnapshot'),data) AS data FROM records)"
        } else {
            "FROM records"
        }
    }
}
