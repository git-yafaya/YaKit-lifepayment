use crate::crypto::Result;
use reqwest::{
    Method, Url,
    blocking::{Client, Response},
};
use std::time::Duration;
pub struct WebDav {
    client: Client,
    base: Url,
    user: String,
    password: String,
}
impl WebDav {
    pub fn new(url: &str, user: &str, password: &str) -> Result<Self> {
        let base =
            Url::parse(&format!("{}/", url.trim_end_matches('/'))).map_err(|_| "服务地址无效")?;
        if base.scheme() != "https" {
            return Err("WebDAV 必须使用 HTTPS".into());
        }
        Ok(Self {
            client: Client::builder()
                .redirect(reqwest::redirect::Policy::none())
                .timeout(Duration::from_secs(30))
                .build()
                .map_err(|e| e.to_string())?,
            base,
            user: user.into(),
            password: password.into(),
        })
    }
    fn request(
        &self,
        method: &str,
        path: &str,
        body: Option<&[u8]>,
        create: bool,
    ) -> Result<Response> {
        if path.starts_with('/') || path.contains("..") || path.contains(':') {
            return Err("无效对象路径".into());
        }
        let mut r = self
            .client
            .request(
                Method::from_bytes(method.as_bytes()).map_err(|e| e.to_string())?,
                self.base.join(path).map_err(|e| e.to_string())?,
            )
            .basic_auth(&self.user, Some(&self.password));
        if method == "PROPFIND" {
            r = r.header("Depth", "1");
        }
        if create {
            r = r.header("If-None-Match", "*");
        }
        if let Some(b) = body {
            r = r.body(b.to_vec());
        }
        r.send().map_err(|_| "WebDAV 网络错误或超时".into())
    }
    pub fn get(&self, path: &str) -> Result<Vec<u8>> {
        let r = self.request("GET", path, None, false)?;
        if !r.status().is_success() {
            return Err(format!("WebDAV GET {}", r.status()));
        }
        r.bytes().map(|v| v.to_vec()).map_err(|_| "读取失败".into())
    }
    pub fn put_immutable(&self, path: &str, bytes: &[u8]) -> Result<()> {
        let r = self.request("PUT", path, Some(bytes), true)?;
        if r.status() == 412 {
            if self.get(path)? == bytes {
                return Ok(());
            }
            return Err("远端不可变操作内容冲突".into());
        }
        if !r.status().is_success() {
            return Err(format!("WebDAV PUT {}", r.status()));
        }
        Ok(())
    }
    pub fn delete(&self, path: &str) -> Result<()> {
        let r = self.request("DELETE", path, None, false)?;
        if !r.status().is_success() && r.status() != 404 {
            return Err(format!("WebDAV DELETE {}", r.status()));
        }
        Ok(())
    }
    pub fn ensure_collection(&self, path: &str) -> Result<()> {
        let mut current = String::new();
        for part in path.trim_end_matches('/').split('/') {
            current.push_str(part);
            current.push('/');
            let r = self.request("MKCOL", &current, None, false)?;
            if !r.status().is_success() && r.status() != 405 {
                return Err(format!("WebDAV MKCOL {}", r.status()));
            }
        }
        Ok(())
    }
    pub fn list(&self, path: &str) -> Result<Vec<String>> {
        let r = self.request("PROPFIND", path, None, false)?;
        if r.status() != 207 {
            return Err(format!("WebDAV PROPFIND {}", r.status()));
        }
        let xml = r.text().map_err(|_| "目录响应失败")?;
        let mut reader = quick_xml::Reader::from_str(&xml);
        let mut out = Vec::new();
        let mut in_href = false;
        loop {
            match reader.read_event() {
                Ok(quick_xml::events::Event::Start(e)) => {
                    in_href = e.local_name().as_ref() == b"href"
                }
                Ok(quick_xml::events::Event::Text(e)) if in_href => {
                    let text = e.decode().map_err(|_| "目录编码无效")?;
                    let url = self.base.join(&text).map_err(|_| "目录路径无效")?;
                    if url.origin() == self.base.origin()
                        && url.path().starts_with(self.base.path())
                    {
                        out.push(url.path()[self.base.path().len()..].to_string());
                    }
                }
                Ok(quick_xml::events::Event::Eof) => break,
                Err(_) => return Err("目录XML无效".into()),
                _ => {}
            }
        }
        Ok(out)
    }
    pub fn probe(&self) -> Result<()> {
        self.list("")?;
        let path = format!(".lightledger-probe-{}", uuid::Uuid::new_v4());
        let bytes = crate::crypto::random_key();
        let result: Result<()> = (|| {
            self.put_immutable(&path, &bytes)?;
            if self.get(&path)? != bytes {
                return Err("WebDAV 写读内容不一致".into());
            }
            Ok(())
        })();
        let cleanup = self.delete(&path);
        result?;
        cleanup
    }
}
#[cfg(test)]
pub(crate) fn test_client(url: &str) -> WebDav {
    WebDav {
        client: Client::builder()
            .timeout(Duration::from_secs(2))
            .build()
            .unwrap(),
        base: Url::parse(url).unwrap(),
        user: "test".into(),
        password: "test".into(),
    }
}
