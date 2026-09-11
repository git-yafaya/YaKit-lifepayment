use super::*;
impl LedgerService {
    pub(crate) fn transfer(&mut self, action: &str, p: Value) -> Result<Value, String> {
        match action {
            "exportJson" => {
                Ok(json!({"text":serde_json::to_string_pretty(&self.export_rows()?).map_err(err)?}))
            }
            "importJson" => {
                let rows: Vec<Value> = serde_json::from_str(s(&p, "text")).map_err(err)?;
                Ok(self.import_rows(rows.into_iter().map(Ok).collect(), s(&p, "text"), "json"))
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
                let rows = reader
                    .records()
                    .map(|row| {
                        let row = row.map_err(err)?;
                        let mut value = json!({});
                        for (h, v) in headers.iter().zip(row.iter()) {
                            if !v.is_empty() {
                                value[h] = json!(v);
                            }
                        }
                        Ok(value)
                    })
                    .collect();
                Ok(self.import_rows(rows, s(&p, "text"), "csv"))
            }
            "exportBackup" | "restoreBackup" => self.backup(action, p),
            _ => Err("不支持的数据操作".into()),
        }
    }
    fn import_rows(&self, rows: Vec<Result<Value, String>>, text: &str, format: &str) -> Value {
        use sha2::Digest;
        let digest = sha2::Sha256::digest(text.as_bytes());
        let mut results = vec![Value::Null; rows.len()];
        let mut rows: Vec<_> = rows.into_iter().enumerate().collect();
        // 先导入原交易，再按原顺序处理退款；结果和来源标记仍使用文件行号。
        rows.sort_by_key(|(_, row)| row.as_ref().is_ok_and(|row| s(row, "kind") == "refund"));
        for (index, row) in rows {
            results[index] = match row {
                Ok(mut row) if row.is_object() => {
                    row["captureId"] = json!(format!("{format}:{digest:x}:{index}"));
                    self.import_row(row, index + 1)
                }
                row => {
                    json!({"status":"failed","row":index+1,"message":row.err().unwrap_or_else(|| "账单必须为对象".into())})
                }
            };
        }
        json!(results)
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

#[cfg(test)]
mod checks {
    use super::*;

    fn expense() -> Value {
        json!({"id":"expense","amountMinor":"1000","currencyCode":"CNY","kind":"expense","occurredAt":"2026-09-10T12:00:00Z","payerId":"local"})
    }

    #[test]
    fn json_non_objects_fail_without_poisoning_ledger() {
        let ledger =
            std::sync::Mutex::new(LedgerService::open(Path::new(":memory:"), &[1; 32]).unwrap());
        let text = json!([123, null, [], "text", false, expense()]).to_string();
        let result = ledger
            .lock()
            .unwrap()
            .dispatch("importJson", json!({"text":text}))
            .unwrap();
        for index in 0..5 {
            assert_eq!(result[index]["status"], "failed");
            assert_eq!(result[index]["row"], index + 1);
        }
        assert_eq!(result[5]["status"], "created");
        assert_eq!(result[5]["row"], 6);
        let mut ledger = ledger.lock().unwrap();
        assert_eq!(ledger.rows().unwrap().len(), 1);
        assert_eq!(
            ledger.dispatch("importJson", json!({"text":text})).unwrap()[5]["status"],
            "existing"
        );
    }

    #[test]
    fn exported_refunds_import_before_their_original_row() {
        let mut source = LedgerService::open(Path::new(":memory:"), &[1; 32]).unwrap();
        source.dispatch("create", expense()).unwrap();
        let mut refund = expense();
        refund["id"] = json!("refund");
        refund["kind"] = json!("refund");
        refund["amountMinor"] = json!("400");
        refund["originalTransactionId"] = json!("expense");
        refund["occurredAt"] = json!("2026-09-11T12:00:00Z");
        source.dispatch("create", refund).unwrap();
        for (export, import) in [("exportJson", "importJson"), ("exportCsv", "importCsv")] {
            let text = source.dispatch(export, json!({})).unwrap();
            let mut target = LedgerService::open(Path::new(":memory:"), &[2; 32]).unwrap();
            for status in ["created", "existing"] {
                let result = target.dispatch(import, text.clone()).unwrap();
                assert_eq!(result.as_array().unwrap().len(), 2);
                for (index, id) in ["refund", "expense"].into_iter().enumerate() {
                    assert_eq!(result[index]["status"], status);
                    assert_eq!(result[index]["id"], id);
                    assert_eq!(result[index]["row"], index + 1);
                }
            }
            assert_eq!(target.rows().unwrap().len(), 2);
        }
    }

    #[test]
    fn csv_refunds_preserve_row_order_and_partial_failures() {
        let mut ledger = LedgerService::open(Path::new(":memory:"), &[1; 32]).unwrap();
        let text = "id,amountMinor,currencyCode,kind,occurredAt,payerId,originalTransactionId\nrefund-a,600,CNY,refund,2026-09-11T12:00:00Z,local,expense\ninvalid\nrefund-b,500,CNY,refund,2026-09-11T13:00:00Z,local,expense\nexpense,1000,CNY,expense,2026-09-10T12:00:00Z,local,\n";
        for success in ["created", "existing"] {
            let result = ledger.dispatch("importCsv", json!({"text":text})).unwrap();
            for (index, status) in [success, "failed", "failed", success]
                .into_iter()
                .enumerate()
            {
                assert_eq!(result[index]["status"], status);
                assert_eq!(result[index]["row"], index + 1);
            }
            assert_eq!(result[2]["message"], "累计退款超过原始支出");
        }
        assert_eq!(ledger.rows().unwrap().len(), 2);
    }
}
