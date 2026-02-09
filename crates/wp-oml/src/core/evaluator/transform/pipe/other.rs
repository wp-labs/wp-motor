use crate::core::prelude::*;
use crate::language::{Get, Nth, PathGet, PathType, PiPeOperation, SkipEmpty, UrlGet, UrlType};

use std::collections::{HashMap, VecDeque};
use std::path::Path;
use url::{Position, Url};
use wp_model_core::model::types::value::ObjectValue;
use wp_model_core::model::{DataField, DataRecord, FieldStorage, Value};
impl FieldExtractor for PiPeOperation {
    fn extract_one(
        &self,
        target: &EvaluationTarget,
        src: &mut DataRecordRef<'_>,
        dst: &DataRecord,
    ) -> Option<DataField> {
        if let Some(mut from) = self.from().extract_one(target, src, dst) {
            for pipe in self.items() {
                from = pipe.value_cacu(from);
            }
            return Some(from);
        }
        None
    }

    fn extract_storage(
        &self,
        target: &EvaluationTarget,
        src: &mut DataRecordRef<'_>,
        dst: &DataRecord,
    ) -> Option<FieldStorage> {
        // Use extract_storage to preserve zero-copy for Shared variants
        if let Some(mut from_storage) = self.from().extract_storage(target, src, dst) {
            for pipe in self.items() {
                from_storage = pipe.value_cacu_storage(from_storage);
            }
            return Some(from_storage);
        }
        None
    }
}
impl ValueProcessor for Nth {
    fn value_cacu(&self, in_val: DataField) -> DataField {
        match in_val.get_value() {
            Value::Array(arr) => {
                if let Some(found) = arr.get(self.index) {
                    return found.as_field().clone();
                }
                in_val
            }
            _ => in_val,
        }
    }
}
impl ValueProcessor for SkipEmpty {
    fn value_cacu(&self, in_val: DataField) -> DataField {
        match in_val.get_value() {
            Value::Array(x) => {
                if x.is_empty() {
                    return DataField::from_ignore(in_val.get_name());
                }
            }
            Value::Digit(x) => {
                if x.eq(&0) {
                    return DataField::from_ignore(in_val.get_name());
                }
            }
            Value::Float(x) => {
                if x.eq(&0.0) {
                    return DataField::from_ignore(in_val.get_name());
                }
            }
            Value::Chars(x) => {
                if x.is_empty() {
                    return DataField::from_ignore(in_val.get_name());
                }
            }
            Value::Obj(x) => {
                if x.is_empty() {
                    return DataField::from_ignore(in_val.get_name());
                }
            }
            _ => {}
        }
        in_val
    }
}
impl ValueProcessor for Get {
    fn value_cacu(&self, mut in_val: DataField) -> DataField {
        if let Value::Obj(obj) = in_val.get_value_mut() {
            let mut keys: VecDeque<&str> = self.name.split('/').collect();
            while let Some(key) = keys.pop_front() {
                if let Some(val) = obj.get(key) {
                    if !keys.is_empty() {
                        if let Value::Obj(o) = val.get_value() {
                            *obj = o.clone();
                        }
                    } else {
                        return val.as_field().clone();
                    }
                }
            }
        }
        in_val
    }

    fn value_cacu_storage(&self, in_val: FieldStorage) -> FieldStorage {
        // Zero-copy path: extract field from Shared object without cloning the object
        if in_val.is_shared() {
            let field = in_val.as_field(); // Returns &Field
            if let Value::Obj(obj) = field.get_value() {
                // obj is &ObjectValue
                let keys: Vec<&str> = self.name.split('/').collect();
                if let Some(result) = get_from_obj(obj, &keys) {
                    // Pass obj directly
                    // ✅ Preserve FieldStorage type (Shared or Owned)
                    return result.clone();
                }
            }
        }

        // Fallback: use default implementation
        let field = in_val.into_owned();
        let result = self.value_cacu(field);
        FieldStorage::from_owned(result)
    }
}

// Helper function to navigate nested objects
fn get_from_obj<'a>(mut obj: &'a ObjectValue, keys: &[&str]) -> Option<&'a FieldStorage> {
    for (i, key) in keys.iter().enumerate() {
        if let Some(val) = obj.get(key) {
            if i == keys.len() - 1 {
                // Last key: return the field
                return Some(val);
            } else {
                // Intermediate key: navigate deeper
                if let Value::Obj(nested) = val.get_value() {
                    obj = nested;
                } else {
                    return None;
                }
            }
        } else {
            return None;
        }
    }
    None
}

impl ValueProcessor for crate::language::StartsWith {
    fn value_cacu(&self, in_val: DataField) -> DataField {
        match in_val.get_value() {
            Value::Chars(value) => {
                if value.starts_with(&self.prefix) {
                    // 匹配成功，返回原字段
                    in_val
                } else {
                    // 不匹配，转换为 ignore 类型
                    DataField::from_ignore(in_val.get_name())
                }
            }
            _ => {
                // 非字符串类型也转换为 ignore
                DataField::from_ignore(in_val.get_name())
            }
        }
    }
}

impl ValueProcessor for crate::language::MapTo {
    fn value_cacu(&self, in_val: DataField) -> DataField {
        use crate::language::MapValue;

        // 检查字段是否为 ignore 类型
        if matches!(in_val.get_value(), Value::Ignore(_)) {
            // 如果是 ignore 类型，保持不变
            in_val
        } else {
            // 如果不是 ignore，根据参数类型创建对应的字段
            let name = in_val.get_name().to_string();
            match &self.value {
                MapValue::Chars(s) => DataField::from_chars(name, s.clone()),
                MapValue::Digit(d) => DataField::from_digit(name, *d),
                MapValue::Float(f) => DataField::from_float(name, *f),
                MapValue::Bool(b) => DataField::from_bool(name, *b),
            }
        }
    }
}

impl ValueProcessor for PathGet {
    fn value_cacu(&self, in_val: DataField) -> DataField {
        match in_val.get_value() {
            Value::Chars(x) => {
                let x = x.replace('\\', "/");
                let path = Path::new(&x);
                let val_str = match &self.key {
                    PathType::Default => x.to_string(),
                    PathType::Path => path
                        .parent()
                        .map(|f| f.to_string_lossy().into_owned())
                        .unwrap_or_else(|| x.to_string()),
                    PathType::FileName => path
                        .file_name()
                        .map(|f| f.to_string_lossy().into_owned())
                        .unwrap_or_else(|| x.to_string()),
                };
                DataField::from_chars(in_val.get_name().to_string(), val_str)
            }
            _ => in_val,
        }
    }
}

impl ValueProcessor for UrlGet {
    fn value_cacu(&self, in_val: DataField) -> DataField {
        match in_val.get_value() {
            Value::Chars(x) => {
                let origin_url = x.clone();
                let val_str = match Url::parse(&origin_url) {
                    Ok(url) => match &self.key {
                        UrlType::Domain => url.domain().unwrap_or(x).to_string(),
                        UrlType::HttpReqHost => {
                            let host = url.host_str().unwrap_or("");
                            let port = url.port().map(|p| format!(":{}", p)).unwrap_or_default();
                            format!("{}{}", host, port)
                        }
                        UrlType::HttpReqUri => url[Position::BeforePath..].to_string(),
                        UrlType::HttpReqPath => url.path().to_string(),
                        UrlType::HttpReqParams => url.query().unwrap_or("").to_string(),
                        UrlType::Default => origin_url.to_string(),
                    },
                    Err(_) => origin_url.to_string(),
                };
                DataField::from_chars(in_val.get_name().to_string(), val_str)
            }
            _ => in_val,
        }
    }
}

use lazy_static::lazy_static;

lazy_static! {
    // 中英文字段映射表
    static ref FIELD_MAPPING: HashMap<&'static str, &'static str> = {
        let mut m = HashMap::new();
        m.insert("url路径", "urlPath");
        m.insert("状态码", "statusCode");
        m.insert("用户名", "username");
        m.insert("密码", "password");
        m.insert("请求体", "requestBody");
        m.insert("请求头", "requestHeaders");
        m.insert("响应头", "responseHeaders");
        m.insert("响应体", "responseBody");
        m.insert("解密账号", "decryptedAccount");
        m.insert("解密密码", "decryptedPassword");
        m.insert("病毒名", "virusName");
        m.insert("文件路径", "filePath");
        m.insert("文件大小", "fileSize");
        m.insert("文件创建时间", "fileCreateTime");
        m.insert("文件MD5", "fileMd5");
        m.insert("referer路径", "refererPath");
        m.insert("描述", "describe");
        m.insert("描述信息", "describeInfo");
        m.insert("检测的引擎", "engine");
        m
    };
}

#[cfg(test)]
mod tests {
    use crate::core::DataTransformer;
    use crate::parser::oml_parse_raw;
    use orion_error::TestAssert;
    use wp_data_model::cache::FieldQueryCache;
    use wp_model_core::model::{DataField, DataRecord, FieldStorage};

    #[test]
    fn test_pipe_path_get() {
        let cache = &mut FieldQueryCache::default();
        let data = vec![FieldStorage::from_owned(DataField::from_chars(
            "A1",
            "C:\\Users\\wplab\\AppData\\Local\\Temp\\B8A93152-2B59-426D-BE5F-5521D4D2D957\\api-ms-win-core-file-l1-2-1.dll",
        ))];
        let src = DataRecord::from(data);

        let mut conf = r#"
        name : test
        ---
        X : chars =  pipe take(A1) | path(name);
         "#;
        let model = oml_parse_raw(&mut conf).unwrap();

        let target = model.transform(src, cache);

        let expect = DataField::from_chars(
            "X".to_string(),
            "api-ms-win-core-file-l1-2-1.dll".to_string(),
        );
        assert_eq!(target.field("X").map(|s| s.as_field()), Some(&expect));
    }

    #[test]
    fn test_pipe_url_get() {
        let cache = &mut FieldQueryCache::default();
        let data = vec![FieldStorage::from_owned(DataField::from_chars(
            "A1",
            "https://a.b.com:8888/OneCollector/1.0?cors=true&content-type=application/x-json-stream#id1",
        ))];
        let src = DataRecord::from(data);

        let mut conf = r#"
        name : test
        ---
        A : chars =  pipe read(A1) | url(domain);
        B : chars =  pipe read(A1) | url(host);
        C : chars =  pipe read(A1) | url(uri);
        D : chars =  pipe read(A1) | url(path);
        E : chars =  pipe read(A1) | url(params);
         "#;
        let model = oml_parse_raw(&mut conf).unwrap();

        let target = model.transform(src, cache);

        let expect = DataField::from_chars("A".to_string(), "a.b.com".to_string());
        assert_eq!(target.field("A").map(|s| s.as_field()), Some(&expect));
        let expect = DataField::from_chars("B".to_string(), "a.b.com:8888".to_string());
        assert_eq!(target.field("B").map(|s| s.as_field()), Some(&expect));
        let expect = DataField::from_chars(
            "C".to_string(),
            "/OneCollector/1.0?cors=true&content-type=application/x-json-stream#id1".to_string(),
        );
        assert_eq!(target.field("C").map(|s| s.as_field()), Some(&expect));
        let expect = DataField::from_chars("D".to_string(), "/OneCollector/1.0".to_string());
        assert_eq!(target.field("D").map(|s| s.as_field()), Some(&expect));
        let expect = DataField::from_chars(
            "E".to_string(),
            "cors=true&content-type=application/x-json-stream".to_string(),
        );
        assert_eq!(target.field("E").map(|s| s.as_field()), Some(&expect));
    }

    #[test]
    fn test_pipe_base64() {
        let cache = &mut FieldQueryCache::default();
        let data = vec![
            FieldStorage::from_owned(DataField::from_chars("A1", "hello1")),
            FieldStorage::from_owned(DataField::from_chars(
                "B2",
                "UE9TVCAvYWNjb3VudCBIVFRQLzEuMQ0KSG9zdDogZnRwLXh0by5lbmVyZ3ltb3N0LmNvbTo2MTIyMg0KVXNlci1BZ2VudDogTW96aWxsYS81LjAgKE1hY2ludG9zaDsgSW50ZWwgTWFjIE9TIFggMTBfMTVfNykgQXBwbGVXZWJLaXQvNTM3LjM2IChLSFRNTCwgbGlrZSBHZWNrbykgQ2hyb21lLzEwMS4wLjAuMCBTYWZhcmkvNTM3LjM2DQpDb250ZW50LUxlbmd0aDogMTE0DQpDb25uZWN0aW9uOiBjbG9zZQ0KQ29udGVudC1UeXBlOiBhcHBsaWNhdGlvbi94LXd3dy1mb3JtLXVybGVuY29kZWQNCkFjY2VwdC1FbmNvZGluZzogZ3ppcA0KDQo=",
            )),
            FieldStorage::from_owned(DataField::from_chars(
                "C3",
                "U1NILTIuMC1tb2Rfc2Z0cA0KAAADVAcUUhSdWEFUvYFEugJ7xA68OgAAAT1jdXJ2ZTI1NTE5LXNoYTI1NixjdXJ2ZTI1NTE5LXNoYTI1NkBsaWJzc2gub3JnLGVjZGgtc2hhMi1uaXN0cDUyMSxlY2RoLXNoYTItbmlzdHAzODQsZWNkaC1zaGEyLW5pc3RwMjU2LGRpZmZpZS1oZWxsbWFuLWdyb3VwMTgtc2hhNTEyLGRpZmZpZS1oZWxsbWFuLWdyb3VwMTYtc2hhNTEyLGRpZmZpZS1oZWxsbWFuLWdyb3VwMTQtc2hhMjU2LGRpZmZpZS1oZWxsbWFuLWdyb3VwLWV4Y2hhbmdlLXNoYTI1NixkaWZmaWUtaGVsbG1hbi1ncm91cC1leGNoYW5nZS1zaGExLGRpZmZpZS1oZWxsbWFuLWdyb3VwMTQtc2hhMSxyc2ExMDI0LXNoYTEsZXh0LWluZm8tcwAAAClyc2Etc2hhMi01MTIscnNhLXNoYTItMjU2LHNzaC1yc2Esc3NoLWRzcwAAAF9hZXMyNTYtY3RyLGFlczE5Mi1jdHIsYWVzMTI4LWN0cixhZXMyNTYtY2JjLGFlczE5Mi1jYmMsYWVzMTI4LWNiYyxjYXN0MTI4LWNiYywzZGVzLWN0ciwzZGVzLWNiYwAAAF9hZXMyNTYtY3RyLGFlczE5Mi1jdHIsYWVzMTI4LWN0cixhZXMyNTYtY2JjLGFlczE5Mi1jYmMsYWVzMTI4LWNiYyxjYXN0MTI4LWNiYywzZGVzLWN0ciwzZGVzLWNiYwAAAFtobWFjLXNoYTItMjU2LGhtYWMtc2hhMi01MTIsaG1hYy1zaGExLGhtYWMtc2hhMS05Nix1bWFjLTY0QG9wZW5zc2guY29tLHVtYWMtMTI4QG9wZW5zc2guY29tAAAAW2htYWMtc2hhMi0yNTYsaG1hYy1zaGEyLTUxMixobWFjLXNoYTEsaG1hYy1zaGExLTk2LHVtYWMtNjRAb3BlbnNzaC5jb20sdW1hYy0xMjhAb3BlbnNzaC5jb20AAAAaemxpYkBvcGVuc3NoLmNvbSx6bGliLG5vbmUAAAAaemxpYkBvcGVuc3NoLmNvbSx6bGliLG5vbmUAAAAAAAAAAAAAAAAAXuQ3JWG631Byb3RvY29sIG1pc21hdGNoLgo=",
            )),
        ];
        let src = DataRecord::from(data);

        let mut conf = r#"
        name : test
        ---
        X : chars =  pipe take(A1) | base64_encode | base64_decode() ;
        Y : chars =  pipe take(B2) | base64_decode(Imap) ;
        Z : chars =  pipe take(C3) | base64_decode(Imap) ;
         "#;
        let model = oml_parse_raw(&mut conf).unwrap();

        let target = model.transform(src, cache);

        let expect = DataField::from_chars("X".to_string(), "hello1".to_string());
        assert_eq!(target.field("X").map(|s| s.as_field()), Some(&expect));

        let expect = DataField::from_chars("Y".to_string(), r#"POST /account HTTP/1.1\r\nHost: ftp-xto.energymost.com:61222\r\nUser-Agent: Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/101.0.0.0 Safari/537.36\r\nContent-Length: 114\r\nConnection: close\r\nContent-Type: application/x-www-form-urlencoded\r\nAccept-Encoding: gzip\r\n\r\n"#.to_string());
        assert_eq!(target.field("Y").map(|s| s.as_field()), Some(&expect));

        let expect = DataField::from_chars("Z".to_string(), "SSH-2.0-mod_sftp\\r\\n\\x00\\x00\\x03T\\x07\\x14R\\x14\\x9dXAT\\xbd\\x81D\\xba\\x02{\\xc4\\x0e\\xbc:\\x00\\x00\\x01=curve25519-sha256,curve25519-sha256@libssh.org,ecdh-sha2-nistp521,ecdh-sha2-nistp384,ecdh-sha2-nistp256,diffie-hellman-group18-sha512,diffie-hellman-group16-sha512,diffie-hellman-group14-sha256,diffie-hellman-group-exchange-sha256,diffie-hellman-group-exchange-sha1,diffie-hellman-group14-sha1,rsa1024-sha1,ext-info-s\\x00\\x00\\x00)rsa-sha2-512,rsa-sha2-256,ssh-rsa,ssh-dss\\x00\\x00\\x00_aes256-ctr,aes192-ctr,aes128-ctr,aes256-cbc,aes192-cbc,aes128-cbc,cast128-cbc,3des-ctr,3des-cbc\\x00\\x00\\x00_aes256-ctr,aes192-ctr,aes128-ctr,aes256-cbc,aes192-cbc,aes128-cbc,cast128-cbc,3des-ctr,3des-cbc\\x00\\x00\\x00[hmac-sha2-256,hmac-sha2-512,hmac-sha1,hmac-sha1-96,umac-64@openssh.com,umac-128@openssh.com\\x00\\x00\\x00[hmac-sha2-256,hmac-sha2-512,hmac-sha1,hmac-sha1-96,umac-64@openssh.com,umac-128@openssh.com\\x00\\x00\\x00\\x1azlib@openssh.com,zlib,none\\x00\\x00\\x00\\x1azlib@openssh.com,zlib,none\\x00\\x00\\x00\\x00\\x00\\x00\\x00\\x00\\x00\\x00\\x00\\x00\\x00^\\xe47%a\\xba\\xdfProtocol mismatch.\\n".to_string());
        assert_eq!(target.field("Z").map(|s| s.as_field()), Some(&expect));
    }

    #[test]
    fn test_html_escape() {
        let cache = &mut FieldQueryCache::default();
        let data = vec![FieldStorage::from_owned(DataField::from_chars(
            "A1", "<html>",
        ))];
        let src = DataRecord::from(data);

        let mut conf = r#"
        name : test
        ---
        X : chars =  pipe take(A1) | html_escape | html_unescape;
         "#;
        let model = oml_parse_raw(&mut conf).assert();

        let target = model.transform(src, cache);

        let expect = DataField::from_chars("X".to_string(), "<html>".to_string());
        assert_eq!(target.field("X").map(|s| s.as_field()), Some(&expect));
    }

    #[test]
    fn test_str_escape() {
        let cache = &mut FieldQueryCache::default();
        let data = vec![FieldStorage::from_owned(DataField::from_chars(
            "A1", "html\"1_",
        ))];
        let src = DataRecord::from(data);

        let mut conf = r#"
        name : test
        ---
        X : chars =  pipe take(A1) | str_escape  ;
         "#;
        let model = oml_parse_raw(&mut conf).assert();

        let target = model.transform(src, cache);

        let expect = DataField::from_chars("X".to_string(), r#"html\"1_"#.to_string());
        assert_eq!(target.field("X").map(|s| s.as_field()), Some(&expect));
    }

    #[test]
    fn test_json_escape() {
        let cache = &mut FieldQueryCache::default();
        let data = vec![FieldStorage::from_owned(DataField::from_chars(
            "A1",
            "This is a crab: 🦀",
        ))];
        let src = DataRecord::from(data);

        let mut conf = r#"
        name : test
        ---
        X : chars =  pipe take(A1) | json_escape  | json_unescape ;
         "#;
        let model = oml_parse_raw(&mut conf).assert();

        let target = model.transform(src, cache);

        let expect = DataField::from_chars("X".to_string(), "This is a crab: 🦀".to_string());
        assert_eq!(target.field("X").map(|s| s.as_field()), Some(&expect));
    }

    #[test]
    fn test_pipe_time() {
        let cache = &mut FieldQueryCache::default();
        let data = vec![FieldStorage::from_owned(DataField::from_chars(
            "A1", "<html>",
        ))];
        let src = DataRecord::from(data);

        let mut conf = r#"
        name : test
        ---
        Y  =  time(2000-10-10 0:0:0);
        X  =  pipe  read(Y) | Time::to_ts ;
        Z  =  pipe  read(Y) | Time::to_ts_ms ;
        U  =  pipe  read(Y) | Time::to_ts_us ;
         "#;
        let model = oml_parse_raw(&mut conf).assert();
        let target = model.transform(src, cache);
        //let expect = TDOEnum::from_digit("X".to_string(), 971136000);
        let expect = DataField::from_digit("X".to_string(), 971107200);
        assert_eq!(target.field("X").map(|s| s.as_field()), Some(&expect));
        let expect = DataField::from_digit("Z".to_string(), 971107200000);
        assert_eq!(target.field("Z").map(|s| s.as_field()), Some(&expect));

        let expect = DataField::from_digit("U".to_string(), 971107200000000);
        assert_eq!(target.field("U").map(|s| s.as_field()), Some(&expect));
    }

    #[test]
    fn test_pipe_skip() {
        let cache = &mut FieldQueryCache::default();
        let data = vec![
            FieldStorage::from_owned(DataField::from_digit("A1", 0)),
            FieldStorage::from_owned(DataField::from_arr("A2", vec![])),
        ];
        let src = DataRecord::from(data.clone());

        let mut conf = r#"
        name : test
        ---
        X  =  collect take(keys: [A1, A2]) ;
        Y  =  pipe  read(A1) | skip_empty ;
        Z  =  pipe  read(A2) | skip_empty ;
         "#;
        let model = oml_parse_raw(&mut conf).assert();
        let target = model.transform(src, cache);
        let expect = DataField::from_arr(
            "X".to_string(),
            vec![
                DataField::from_digit("A1", 0),
                DataField::from_arr("A2", vec![]),
            ],
        );
        assert_eq!(target.field("X").map(|s| s.as_field()), Some(&expect));
        assert_eq!(
            target.field("Y").map(|s| s.as_field()),
            Some(&DataField::from_ignore("Y"))
        );
        assert_eq!(
            target.field("Z").map(|s| s.as_field()),
            Some(&DataField::from_ignore("Z"))
        );
    }

    #[test]
    fn test_pipe_obj_get() {
        let val = r#"{"id":0,"items":[{"meta":{"array":"obj"},"name":"current_process","value":{"Array":[{"meta":"obj","name":"obj","value":{"Obj":{"ctime":{"meta":"digit","name":"ctime","value":{"Digit":1676340214}},"desc":{"meta":"chars","name":"desc","value":{"Chars":""}},"md5":{"meta":"chars","name":"md5","value":{"Chars":"d4ed19a8acd9df02123f655fa1e8a8e7"}},"path":{"meta":"chars","name":"path","value":{"Chars":"c:\\\\users\\\\administrator\\\\desktop\\\\domaintool\\\\x64\\\\childproc\\\\test_le9mwv.exe"}},"sign":{"meta":"chars","name":"sign","value":{"Chars":""}},"size":{"meta":"digit","name":"size","value":{"Digit":189446}},"state":{"meta":"digit","name":"state","value":{"Digit":0}},"type":{"meta":"digit","name":"type","value":{"Digit":1}}}}}]}}]}"#;
        let src: DataRecord = serde_json::from_str(val).unwrap();
        let cache = &mut FieldQueryCache::default();

        let mut conf = r#"
        name : test
        ---
        Y  =  pipe read(current_process) | nth(0) | get(current_process/path) ;
         "#;
        let model = oml_parse_raw(&mut conf).assert();
        let target = model.transform(src, cache);
        let expect = DataField::from_chars(
            "Y",
            r#"c:\\users\\administrator\\desktop\\domaintool\\x64\\childproc\\test_le9mwv.exe"#,
        );
        assert_eq!(target.field("Y").map(|s| s.as_field()), Some(&expect));
    }

    #[test]
    fn test_pipe_start_with() {
        // 测试匹配的情况
        let cache = &mut FieldQueryCache::default();
        let data = vec![FieldStorage::from_owned(DataField::from_chars(
            "url",
            "https://example.com",
        ))];
        let src = DataRecord::from(data);

        let mut conf = r#"
        name : test
        ---
        X  =  pipe take(url) | starts_with('https://');
         "#;
        let model = oml_parse_raw(&mut conf).assert();
        let target = model.transform(src, cache);

        let expect = DataField::from_chars("X".to_string(), "https://example.com".to_string());
        assert_eq!(target.field("X").map(|s| s.as_field()), Some(&expect));

        // 测试不匹配的情况 - 使用独立的 cache 和 model
        let cache2 = &mut FieldQueryCache::default();
        let data2 = vec![FieldStorage::from_owned(DataField::from_chars(
            "url",
            "http://example.com",
        ))];
        let src2 = DataRecord::from(data2);

        let mut conf2 = r#"
        name : test
        ---
        X  =  pipe take(url) | starts_with('https://');
         "#;
        let model2 = oml_parse_raw(&mut conf2).assert();
        let target2 = model2.transform(src2, cache2);

        // 不匹配时应该返回 ignore 字段
        assert_eq!(
            target2.field("X").map(|s| s.as_field()),
            Some(&DataField::from_ignore("X"))
        );
    }

    #[test]
    fn test_pipe_map_to() {
        let cache = &mut FieldQueryCache::default();

        // 测试映射到字符串
        let data = vec![FieldStorage::from_owned(DataField::from_chars(
            "status", "200",
        ))];
        let src = DataRecord::from(data);

        let mut conf = r#"
        name : test
        ---
        A  =  pipe take(status) | map_to('success');
         "#;
        let model = oml_parse_raw(&mut conf).assert();
        let target = model.transform(src, cache);

        let expect = DataField::from_chars("A".to_string(), "success".to_string());
        assert_eq!(target.field("A").map(|s| s.as_field()), Some(&expect));

        // 测试映射到整数
        let cache2 = &mut FieldQueryCache::default();
        let data2 = vec![FieldStorage::from_owned(DataField::from_chars(
            "level", "ERROR",
        ))];
        let src2 = DataRecord::from(data2);

        let mut conf2 = r#"
        name : test
        ---
        B  =  pipe take(level) | map_to(1);
         "#;
        let model2 = oml_parse_raw(&mut conf2).assert();
        let target2 = model2.transform(src2, cache2);

        let expect2 = DataField::from_digit("B".to_string(), 1);
        assert_eq!(target2.field("B").map(|s| s.as_field()), Some(&expect2));

        // 测试映射到浮点数
        let cache3 = &mut FieldQueryCache::default();
        let data3 = vec![FieldStorage::from_owned(DataField::from_chars(
            "temp", "high",
        ))];
        let src3 = DataRecord::from(data3);

        let mut conf3 = r#"
        name : test
        ---
        C  =  pipe take(temp) | map_to(36.5);
         "#;
        let model3 = oml_parse_raw(&mut conf3).assert();
        let target3 = model3.transform(src3, cache3);

        let expect3 = DataField::from_float("C".to_string(), 36.5);
        assert_eq!(target3.field("C").map(|s| s.as_field()), Some(&expect3));

        // 测试映射到布尔值
        let cache4 = &mut FieldQueryCache::default();
        let data4 = vec![FieldStorage::from_owned(DataField::from_chars(
            "flag", "yes",
        ))];
        let src4 = DataRecord::from(data4);

        let mut conf4 = r#"
        name : test
        ---
        D  =  pipe take(flag) | map_to(true);
         "#;
        let model4 = oml_parse_raw(&mut conf4).assert();
        let target4 = model4.transform(src4, cache4);

        let expect4 = DataField::from_bool("D".to_string(), true);
        assert_eq!(target4.field("D").map(|s| s.as_field()), Some(&expect4));

        // 测试 ignore 字段保持不变
        let cache5 = &mut FieldQueryCache::default();
        let data5 = vec![FieldStorage::from_owned(DataField::from_chars(
            "url",
            "http://example.com",
        ))];
        let src5 = DataRecord::from(data5);

        let mut conf5 = r#"
        name : test
        ---
        E  =  pipe take(url) | starts_with('https://') | map_to('secure');
         "#;
        let model5 = oml_parse_raw(&mut conf5).assert();
        let target5 = model5.transform(src5, cache5);

        // 字段为 ignore 时，应该保持 ignore
        assert_eq!(
            target5.field("E").map(|s| s.as_field()),
            Some(&DataField::from_ignore("E"))
        );
    }
}
