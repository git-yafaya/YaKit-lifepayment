use super::*;
use std::collections::BTreeMap;
use windows::UI::Notifications::{
    Management::{UserNotificationListener, UserNotificationListenerAccessStatus},
    NotificationKinds,
};
fn listener() -> Result<UserNotificationListener, String> {
    package_required()?;
    UserNotificationListener::Current().map_err(error)
}
fn allowed() -> Result<UserNotificationListener, String> {
    let listener = listener()?;
    if listener.GetAccessStatus().map_err(error)? != UserNotificationListenerAccessStatus::Allowed {
        return Err("请先授权通知访问；权限被拒绝或撤销时不会读取通知".into());
    }
    Ok(listener)
}
pub(super) fn access() -> Result<String, String> {
    Ok(match listener()?.GetAccessStatus().map_err(error)? {
        UserNotificationListenerAccessStatus::Allowed => "allowed",
        UserNotificationListenerAccessStatus::Denied => "denied",
        _ => "unspecified",
    }
    .into())
}
pub(super) fn request_access() -> Result<UiOperation, String> {
    let operation = listener()?.RequestAccessAsync().map_err(error)?;
    Ok(Box::new(move || {
        let status = operation.join().map_err(error)?;
        Ok(
            json!({"allowed":status==UserNotificationListenerAccessStatus::Allowed,"status":status.0}),
        )
    }))
}
pub(super) fn sources() -> Result<Value, String> {
    let mut values = BTreeMap::new();
    for n in allowed()?
        .GetNotificationsAsync(NotificationKinds::Toast)
        .and_then(|op| op.join())
        .map_err(error)?
    {
        let app = n.AppInfo().map_err(error)?;
        values.insert(
            app.AppUserModelId().map_err(error)?.to_string(),
            app.DisplayInfo()
                .and_then(|i| i.DisplayName())
                .map_err(error)?
                .to_string(),
        );
    }
    Ok(json!(
        values
            .into_iter()
            .map(|(id, name)| json!({"id":id,"name":name}))
            .collect::<Vec<_>>()
    ))
}
pub(super) fn read(p: &Value) -> Result<Value, String> {
    let selected = p["sources"].as_array().ok_or("请选择允许读取的通知来源")?;
    if selected.is_empty() {
        return Err("尚未选择通知来源，不读取任何通知内容".into());
    }
    let mut values = vec![];
    for n in allowed()?
        .GetNotificationsAsync(NotificationKinds::Toast)
        .and_then(|op| op.join())
        .map_err(error)?
    {
        let source = n
            .AppInfo()
            .and_then(|a| a.AppUserModelId())
            .map_err(error)?
            .to_string();
        if !selected.iter().any(|v| v.as_str() == Some(&source)) {
            continue;
        }
        let binding = match n
            .Notification()
            .and_then(|v| v.Visual())
            .and_then(|v| v.GetBinding(&windows::core::HSTRING::from("ToastGeneric")))
        {
            Ok(v) => v,
            Err(_) => continue,
        };
        let text = binding
            .GetTextElements()
            .map_err(error)?
            .into_iter()
            .filter_map(|t| t.Text().ok().map(|v| v.to_string()))
            .collect::<Vec<_>>()
            .join("\n");
        if text.trim().is_empty() {
            continue;
        }
        let created = n.CreationTime().map_err(error)?.UniversalTime;
        values.push(json!({"captureId":format!("windows-notification:{source}:{}:{created}",n.Id().map_err(error)?),"source":source,"text":text,"createdWindowsTicks":created.to_string()}));
    }
    // 仅读取选定来源，绝不清除、点击或修改系统通知。
    Ok(
        json!({"notifications":values,"access":"allowed","note":"空结果只代表当前通知中心没有可读取内容，不代表没有交易"}),
    )
}
