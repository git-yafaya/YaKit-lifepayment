use crate::{
    crypto::{Identity, Result, Space},
    dav::WebDav,
    pairing::{Grant, apply_rotation},
};
use std::collections::BTreeMap;
// 管理员消息也使用不可变连续序号，缺失任何授权或撤销都停止推进。
pub fn exchange(
    dav: &WebDav,
    identity: &Identity,
    space: &mut Space,
    pending: &BTreeMap<String, Grant>,
) -> Result<()> {
    let root = format!("LightLedger/v1/spaces/{}/control/", space.id);
    dav.ensure_collection(&root)?;
    for (path, grant) in pending.iter().filter(|(p, _)| p.starts_with(&root)) {
        dav.put_immutable(path, &serde_json::to_vec(grant).map_err(|e| e.to_string())?)?;
    }
    let files = dav.list(&root)?;
    loop {
        let expected = format!("{root}{:020}.json", space.revision + 1);
        if !files.contains(&expected) {
            if files.iter().any(|p| p > &expected) {
                return Err("缺少设备授权操作，请重试".into());
            }
            break;
        }
        let grant = serde_json::from_slice(&dav.get(&expected)?).map_err(|e| e.to_string())?;
        apply_rotation(space, identity, &grant)?;
    }
    Ok(())
}
