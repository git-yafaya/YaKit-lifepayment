use super::*;
use windows::{
    Globalization::Language,
    Graphics::Imaging::{
        BitmapAlphaMode, BitmapDecoder, BitmapPixelFormat, BitmapTransform, ColorManagementMode,
        ExifOrientationMode,
    },
    Media::Ocr::OcrEngine,
    Storage::{FileAccessMode, StorageFile},
    core::HSTRING,
};
pub(super) fn languages() -> Result<Vec<String>, String> {
    Ok(OcrEngine::AvailableRecognizerLanguages()
        .map_err(error)?
        .into_iter()
        .filter_map(|l| l.LanguageTag().ok().map(|s| s.to_string()))
        .filter(|s| s.starts_with("zh"))
        .collect())
}
pub(super) fn recognize(p: &Value) -> Result<Value, String> {
    package_required()?;
    let path = p["path"].as_str().ok_or("请选择图片")?;
    let metadata = std::fs::metadata(path).map_err(error)?;
    if metadata.len() > 20 * 1024 * 1024 {
        return Err("图片不能超过 20 MB".into());
    }
    let available = languages()?;
    let tag = p["language"]
        .as_str()
        .or_else(|| available.first().map(String::as_str))
        .ok_or("未安装中文 OCR 组件。请在 Windows 设置的语言选项中安装中文光学字符识别组件。")?;
    if !available.iter().any(|l| l == tag) {
        return Err("选择的中文 OCR 语言不可用".into());
    }
    let file = StorageFile::GetFileFromPathAsync(&HSTRING::from(path))
        .and_then(|op| op.join())
        .map_err(error)?;
    let stream = file
        .OpenAsync(FileAccessMode::Read)
        .and_then(|op| op.join())
        .map_err(error)?;
    // 由系统解码器验证真实图片格式，而不是信任扩展名。
    let decoder = BitmapDecoder::CreateAsync(&stream)
        .and_then(|op| op.join())
        .map_err(error)?;
    let (width, height) = (
        decoder.PixelWidth().map_err(error)?,
        decoder.PixelHeight().map_err(error)?,
    );
    if width == 0 || height == 0 || u64::from(width) * u64::from(height) > 100_000_000 {
        return Err("图片尺寸无效或超过一亿像素".into());
    }
    let limit = OcrEngine::MaxImageDimension().map_err(error)?;
    let scale = (f64::from(limit) / f64::from(width.max(height))).min(1.0);
    let transform = BitmapTransform::new().map_err(error)?;
    transform
        .SetScaledWidth((f64::from(width) * scale).round().max(1.0) as u32)
        .map_err(error)?;
    transform
        .SetScaledHeight((f64::from(height) * scale).round().max(1.0) as u32)
        .map_err(error)?;
    let bitmap = decoder
        .GetSoftwareBitmapTransformedAsync(
            BitmapPixelFormat::Bgra8,
            BitmapAlphaMode::Ignore,
            &transform,
            ExifOrientationMode::RespectExifOrientation,
            ColorManagementMode::DoNotColorManage,
        )
        .and_then(|op| op.join())
        .map_err(error)?;
    let engine = OcrEngine::TryCreateFromLanguage(
        &Language::CreateLanguage(&HSTRING::from(tag)).map_err(error)?,
    )
    .map_err(error)?;
    let result = engine
        .RecognizeAsync(&bitmap)
        .and_then(|op| op.join())
        .map_err(error)?;
    let text = result.Text().map_err(error)?.to_string();
    if text.trim().is_empty() {
        return Err("图片中没有识别到可用文字，请手动录入或换一张图片".into());
    }
    let mut words = vec![];
    for line in result.Lines().map_err(error)? {
        for word in line.Words().map_err(error)? {
            let rect = word.BoundingRect().map_err(error)?;
            words.push(json!({"text":word.Text().map_err(error)?.to_string(),"x":rect.X,"y":rect.Y,"width":rect.Width,"height":rect.Height}));
        }
    }
    Ok(json!({"text":text,"language":tag,"scale":scale,"words":words,"source":"windows-ocr"}))
}
