use serde_json::{Value, json};
use windows::{
    ApplicationModel::Package,
    Security::Credentials::UI::{
        UserConsentVerificationResult, UserConsentVerifier, UserConsentVerifierAvailability,
    },
    Win32::{
        Foundation::HWND,
        System::WinRT::{
            IUserConsentVerifierInterop, RO_INIT_MULTITHREADED, RoInitialize, RoUninitialize,
        },
    },
};
#[path = "notifications.rs"]
mod notifications;
#[path = "ocr.rs"]
mod ocr;
fn error(e: impl std::fmt::Display) -> String {
    e.to_string()
}
struct Apartment;
impl Apartment {
    fn initialize() -> Result<Self, String> {
        unsafe {
            RoInitialize(RO_INIT_MULTITHREADED).map_err(error)?;
        }
        Ok(Self)
    }
}
impl Drop for Apartment {
    fn drop(&mut self) {
        unsafe {
            RoUninitialize();
        }
    }
}
fn package_required() -> Result<(), String> {
    Package::Current()
        .map(|_| ())
        .map_err(|_| "此功能需要安装带包身份的 MSIX 版本".into())
}
pub fn capabilities() -> Value {
    let result = (|| -> Result<Value, String> {
        let _apartment = Apartment::initialize()?;
        let packaged = Package::Current().is_ok();
        let languages = ocr::languages().unwrap_or_default();
        let hello = UserConsentVerifier::CheckAvailabilityAsync()
            .and_then(|op| op.join())
            .map_err(error);
        Ok(
            json!({"packaged":packaged,"ocr":{"available":packaged&&!languages.is_empty(),"languages":languages,"reason":if !packaged{"需安装 MSIX 版本"}else{"需要中文 OCR 语言组件"}},"notifications":{"available":packaged,"access":notifications::access().unwrap_or_else(|e|e)},"hello":{"available":hello.as_ref().is_ok_and(|v|*v==UserConsentVerifierAvailability::Available),"status":hello.map(|v|v.0.to_string()).unwrap_or_else(|e|e)},"background":false,"startOnLogin":false}),
        )
    })();
    result.unwrap_or_else(|reason| json!({"available":false,"reason":reason}))
}
pub fn dispatch(action: &str, payload: &Value) -> Result<Value, String> {
    let _apartment = Apartment::initialize()?;
    match action {
        "ocrImage" => ocr::recognize(payload),
        "notificationSources" => notifications::sources(),
        "notificationRead" => notifications::read(payload),
        "helloAvailability" => Ok(
            json!({"available":UserConsentVerifier::CheckAvailabilityAsync().and_then(|op|op.join()).map_err(error)?==UserConsentVerifierAvailability::Available}),
        ),
        _ => Err(format!("尚未支持系统操作：{action}")),
    }
}
// 在窗口线程创建操作，然后把等待闭包交给后台线程，不能阻塞 UI 消息循环。
pub type UiOperation = Box<dyn FnOnce() -> Result<Value, String> + Send>;
pub fn dispatch_ui(action: &str, _payload: &Value, hwnd: isize) -> Result<UiOperation, String> {
    match action {
        "notificationRequestAccess" => notifications::request_access(),
        "helloVerify" => {
            if hwnd == 0 {
                return Err("Windows Hello 需要有效的应用窗口".into());
            }
            let factory: IUserConsentVerifierInterop =
                windows::core::factory::<UserConsentVerifier, IUserConsentVerifierInterop>()
                    .map_err(error)?;
            let operation: windows_future::IAsyncOperation<UserConsentVerificationResult> =
                unsafe {
                    factory.RequestVerificationForWindowAsync(
                        HWND(hwnd as *mut core::ffi::c_void),
                        &windows::core::HSTRING::from("验证身份以打开轻账"),
                    )
                }
                .map_err(error)?;
            Ok(Box::new(move || {
                let result = operation.join().map_err(error)?;
                if result != UserConsentVerificationResult::Verified {
                    return Err(format!("身份验证未通过（状态 {}）", result.0));
                }
                Ok(json!({"verified":true}))
            }))
        }
        _ => Err(format!("不支持的窗口操作：{action}")),
    }
}
