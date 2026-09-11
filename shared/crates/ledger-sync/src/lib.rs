pub mod crypto;
pub mod dav;
pub mod pairing;

pub mod engine;

pub fn new_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

#[cfg(test)]
mod checks {
    use crate::{crypto::*, dav, engine, pairing};
    use serde_json::{Value, json};
    use std::{
        collections::{BTreeMap, BTreeSet},
        io::{Read, Write},
        net::TcpListener,
        sync::{
            Arc, Mutex,
            atomic::{AtomicBool, Ordering},
        },
    };
    struct Server {
        url: String,
        objects: Arc<Mutex<BTreeMap<String, Vec<u8>>>>,
        lose: Arc<AtomicBool>,
        stop: Arc<AtomicBool>,
    }
    impl Server {
        fn new() -> Self {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let url = format!("http://{}/", listener.local_addr().unwrap());
            let objects = Arc::new(Mutex::new(BTreeMap::<String, Vec<u8>>::new()));
            let lose = Arc::new(AtomicBool::new(false));
            let stop = Arc::new(AtomicBool::new(false));
            let (o, l, s) = (objects.clone(), lose.clone(), stop.clone());
            std::thread::spawn(move || {
                for stream in listener.incoming() {
                    if s.load(Ordering::SeqCst) {
                        break;
                    }
                    let mut stream = stream.unwrap();
                    let mut bytes = Vec::new();
                    let mut one = [0; 1];
                    while !bytes.ends_with(b"\r\n\r\n") {
                        if stream.read(&mut one).unwrap_or(0) == 0 {
                            break;
                        }
                        bytes.push(one[0]);
                    }
                    let header = String::from_utf8_lossy(&bytes);
                    let parts = header
                        .lines()
                        .next()
                        .unwrap_or("")
                        .split_whitespace()
                        .collect::<Vec<_>>();
                    if parts.len() < 2 {
                        continue;
                    }
                    let method = parts[0];
                    let path = parts[1].trim_start_matches('/');
                    let length = header
                        .lines()
                        .find_map(|line| {
                            line.to_lowercase()
                                .strip_prefix("content-length:")
                                .and_then(|s| s.trim().parse::<usize>().ok())
                        })
                        .unwrap_or(0);
                    let mut body = vec![0; length];
                    stream.read_exact(&mut body).unwrap();
                    let mut objects = o.lock().unwrap();
                    let (status, output) = match method {
                        "MKCOL" => (201, vec![]),
                        "PUT" => {
                            if objects.contains_key(path) {
                                (412, vec![])
                            } else {
                                objects.insert(path.into(), body);
                                if l.swap(false, Ordering::SeqCst) {
                                    continue;
                                }
                                (201, vec![])
                            }
                        }
                        "GET" => objects
                            .get(path)
                            .map(|b| (200, b.clone()))
                            .unwrap_or((404, vec![])),
                        "DELETE" => {
                            objects.remove(path);
                            (204, vec![])
                        }
                        "PROPFIND" => {
                            let hrefs = objects
                                .keys()
                                .filter(|p| p.starts_with(path))
                                .map(|p| format!("<d:response><d:href>/{p}</d:href></d:response>"))
                                .collect::<String>();
                            (
                                207,
                                format!("<d:multistatus xmlns:d=\"DAV:\">{hrefs}</d:multistatus>")
                                    .into_bytes(),
                            )
                        }
                        _ => (405, vec![]),
                    };
                    let _ = write!(
                        stream,
                        "HTTP/1.1 {status} Result\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        output.len()
                    );
                    let _ = stream.write_all(&output);
                }
            });
            Self {
                url,
                objects,
                lose,
                stop,
            }
        }
    }
    impl Drop for Server {
        fn drop(&mut self) {
            self.stop.store(true, Ordering::SeqCst);
            let _ = std::net::TcpStream::connect(
                self.url.trim_start_matches("http://").trim_end_matches('/'),
            );
        }
    }
    #[test]
    fn encrypted_pairing_and_sync() {
        let alice = Identity::create(crate::new_id(), crate::new_id()).unwrap();
        let bob = Identity::create(alice.member_id.clone(), crate::new_id()).unwrap();
        let mut space = Space::new(crate::new_id(), "personal".into(), &alice).unwrap();
        let request = pairing::request(&bob, 100).unwrap();
        let mut used = BTreeSet::new();
        let grant = pairing::approve(
            &request,
            &alice,
            &mut space,
            101,
            &fingerprint(&bob.public_key().unwrap()).unwrap(),
            &mut used,
        )
        .unwrap();
        assert!(
            pairing::approve(
                &request,
                &alice,
                &mut space,
                102,
                &fingerprint(&bob.public_key().unwrap()).unwrap(),
                &mut used
            )
            .is_err()
        );
        let mut received = pairing::accept(
            &grant,
            &bob,
            &fingerprint(&alice.public_key().unwrap()).unwrap(),
            102,
            &mut BTreeSet::new(),
        )
        .unwrap();
        let op = |seq| json!({"id":crate::new_id(),"spaceId":space.id,"memberId":alice.member_id,"deviceId":alice.device_id,"sequence":seq,"entityId":crate::new_id(),"payload":{"amountMinor":"2800","note":"秘密午饭"},"patch":{"note":"秘密午饭"},"baseVersions":{}});
        let operation = op(1);
        let envelope = seal(
            &space,
            &alice,
            1,
            operation["id"].as_str().unwrap(),
            &operation,
        )
        .unwrap();
        let wire = serde_json::to_vec(&envelope).unwrap();
        assert!(!String::from_utf8_lossy(&wire).contains("秘密午饭"));
        assert_eq!(open(&received, &envelope).unwrap().0.version, 2);
        assert_eq!(open(&received, &envelope).unwrap().1, operation);
        let mut unsupported = envelope.clone();
        let mut header: Header = serde_json::from_slice(&unb64(&envelope.header).unwrap()).unwrap();
        header.version = 3;
        unsupported.header = b64(&serde_json::to_vec(&header).unwrap());
        assert_eq!(
            open(&received, &unsupported).err().as_deref(),
            Some("unsupported-version")
        );
        let mut tampered = envelope.clone();
        tampered.ciphertext = b64(b"tampered");
        assert!(open(&received, &tampered).is_err());
        let mut wrong = received.clone();
        wrong.id = crate::new_id();
        assert!(open(&wrong, &envelope).is_err());
        wrong = received.clone();
        wrong.devices.remove(&alice.device_id);
        assert!(open(&wrong, &envelope).is_err());
        let server = Server::new();
        let dav = dav::test_client(&server.url);
        dav.probe().unwrap();
        assert!(server.objects.lock().unwrap().is_empty());
        server.lose.store(true, Ordering::SeqCst);
        assert!(dav.put_immutable("lost.enc", &wire).is_err());
        dav.put_immutable("lost.enc", &wire).unwrap();
        assert!(dav.put_immutable("lost.enc", b"different").is_err());
        let cache = std::env::temp_dir().join(format!("lightledger-sync-{}", crate::new_id()));
        let second = op(2);
        let second_wire = serde_json::to_vec(
            &seal(&space, &alice, 2, second["id"].as_str().unwrap(), &second).unwrap(),
        )
        .unwrap();
        let root = format!(
            "LightLedger/v1/spaces/{}/ops/{}/",
            space.id, alice.device_id
        );
        dav.put_immutable(&format!("{root}{:020}.enc", 2), &second_wire)
            .unwrap();
        let mut cursor = 0;
        let mut applied = 0;
        let mut dispatch = |action: &str, p: Value| -> Result<Value> {
            match action {
                "pendingUploads" => Ok(json!([])),
                "syncCursor" => Ok(json!({"sequence":cursor})),
                "applyOperation" => {
                    assert_eq!(p["sequence"].as_u64().unwrap(), cursor + 1);
                    cursor += 1;
                    applied += 1;
                    Ok(json!({}))
                }
                _ => Err(action.into()),
            }
        };
        engine::run(&dav, &bob, &received, &cache, &mut dispatch).unwrap();
        dav.put_immutable(&format!("{root}{:020}.enc", 1), &wire)
            .unwrap();
        engine::run(&dav, &bob, &received, &cache, &mut dispatch).unwrap();
        engine::run(&dav, &bob, &received, &cache, &mut dispatch).unwrap();
        let mut mismatched = op(3);
        mismatched["deviceId"] = json!(bob.device_id);
        let invalid = seal(
            &space,
            &alice,
            3,
            mismatched["id"].as_str().unwrap(),
            &mismatched,
        )
        .unwrap();
        dav.put_immutable(
            &format!("{root}{:020}.enc", 3),
            &serde_json::to_vec(&invalid).unwrap(),
        )
        .unwrap();
        let rejected = engine::run(&dav, &bob, &received, &cache, &mut dispatch).unwrap();
        assert!(!rejected["errors"].as_array().unwrap().is_empty());
        assert_eq!(applied, 2);
        assert_eq!(cursor, 2);
        let rotation = pairing::rotate(&mut space, &alice, &bob.device_id).unwrap();
        assert!(pairing::apply_rotation(&mut received, &bob, &rotation).is_err());
        assert!(seal(&space, &bob, 1, &crate::new_id(), &json!({})).is_err());
        assert!(
            open(
                &space,
                &seal(&received, &bob, 1, &crate::new_id(), &json!({})).unwrap()
            )
            .is_err()
        );
        std::fs::remove_dir_all(cache).unwrap();
    }
}
pub mod control;

#[cfg(test)]
#[test]
fn published_interop_vector() {
    let value: serde_json::Value = serde_json::from_str(include_str!(
        "../../../contracts/examples/crypto-vector.json"
    ))
    .unwrap();
    let space: crypto::Space = serde_json::from_value(value["space"].clone()).unwrap();
    let envelope: crypto::Envelope = serde_json::from_value(value["envelope"].clone()).unwrap();
    assert_eq!(
        crypto::open(&space, &envelope).unwrap().1,
        value["plaintext"]
    );
}
