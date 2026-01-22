use crate::core::prelude::*;
use crate::language::{Base64Decode, Base64Encode, EncodeType};
use base64::Engine;
use base64::engine::general_purpose;
use encoding_rs::{
    BIG5, EUC_JP, GB18030, IBM866, ISO_2022_JP, ISO_8859_2, ISO_8859_3, ISO_8859_4, ISO_8859_5,
    ISO_8859_6, ISO_8859_7, ISO_8859_8, ISO_8859_10, ISO_8859_13, ISO_8859_14, ISO_8859_15,
    ISO_8859_16, KOI8_R, KOI8_U, MACINTOSH, SHIFT_JIS, UTF_16BE, UTF_16LE, WINDOWS_874,
    WINDOWS_1250, WINDOWS_1251, WINDOWS_1252, WINDOWS_1253, WINDOWS_1254, WINDOWS_1255,
    WINDOWS_1256, WINDOWS_1257, WINDOWS_1258, X_MAC_CYRILLIC,
};
use imap_types::utils::escape_byte_string;
use wp_model_core::model::{DataField, Value};

impl ValueProcessor for Base64Encode {
    fn value_cacu(&self, in_val: DataField) -> DataField {
        match in_val.get_value() {
            Value::Chars(x) => {
                let encode = general_purpose::STANDARD.encode(x);
                DataField::from_chars(in_val.get_name().to_string(), encode)
            }
            _ => in_val,
        }
    }
}
impl ValueProcessor for Base64Decode {
    fn value_cacu(&self, in_val: DataField) -> DataField {
        match in_val.get_value() {
            Value::Chars(x) => {
                if let Ok(code) = general_purpose::STANDARD.decode(x) {
                    let val_str = match self.encode {
                        EncodeType::Imap => escape_byte_string(code),
                        EncodeType::Utf8 => String::from_utf8_lossy(&code).to_string(),
                        EncodeType::Utf16le => {
                            let (cow, _, _) = UTF_16LE.decode(&code);
                            cow.to_string()
                        }
                        EncodeType::Utf16be => {
                            let (cow, _, _) = UTF_16BE.decode(&code);
                            cow.to_string()
                        }
                        EncodeType::EucJp => {
                            let (cow, _, _) = EUC_JP.decode(&code);
                            cow.to_string()
                        }
                        EncodeType::Windows31j => {
                            // SHIFT_JIS is the same as Windows-31J in encoding_rs
                            let (cow, _, _) = SHIFT_JIS.decode(&code);
                            cow.to_string()
                        }
                        EncodeType::Iso2022Jp => {
                            let (cow, _, _) = ISO_2022_JP.decode(&code);
                            cow.to_string()
                        }
                        EncodeType::Gbk => {
                            // encoding_rs doesn't have GBK, use GB18030 instead
                            let (cow, _, _) = GB18030.decode(&code);
                            cow.to_string()
                        }
                        EncodeType::Gb18030 => {
                            let (cow, _, _) = GB18030.decode(&code);
                            cow.to_string()
                        }
                        EncodeType::HZ => {
                            // encoding_rs doesn't have HZ, use replacement decoder
                            String::from_utf8_lossy(&code).to_string()
                        }
                        EncodeType::Big52003 => {
                            // encoding_rs has BIG5
                            let (cow, _, _) = BIG5.decode(&code);
                            cow.to_string()
                        }
                        EncodeType::MacCyrillic => {
                            let (cow, _, _) = X_MAC_CYRILLIC.decode(&code);
                            cow.to_string()
                        }
                        EncodeType::Windows874 => {
                            let (cow, _, _) = WINDOWS_874.decode(&code);
                            cow.to_string()
                        }
                        EncodeType::Windows949 => {
                            // encoding_rs doesn't have Windows-949, use GB18030 as fallback
                            let (cow, _, _) = GB18030.decode(&code);
                            cow.to_string()
                        }
                        EncodeType::Windows1250 => {
                            let (cow, _, _) = WINDOWS_1250.decode(&code);
                            cow.to_string()
                        }
                        EncodeType::Windows1251 => {
                            let (cow, _, _) = WINDOWS_1251.decode(&code);
                            cow.to_string()
                        }
                        EncodeType::Windows1252 => {
                            let (cow, _, _) = WINDOWS_1252.decode(&code);
                            cow.to_string()
                        }
                        EncodeType::Windows1253 => {
                            let (cow, _, _) = WINDOWS_1253.decode(&code);
                            cow.to_string()
                        }
                        EncodeType::Windows1254 => {
                            let (cow, _, _) = WINDOWS_1254.decode(&code);
                            cow.to_string()
                        }
                        EncodeType::Windows1255 => {
                            let (cow, _, _) = WINDOWS_1255.decode(&code);
                            cow.to_string()
                        }
                        EncodeType::Windows1256 => {
                            let (cow, _, _) = WINDOWS_1256.decode(&code);
                            cow.to_string()
                        }
                        EncodeType::Windows1257 => {
                            let (cow, _, _) = WINDOWS_1257.decode(&code);
                            cow.to_string()
                        }
                        EncodeType::Windows1258 => {
                            let (cow, _, _) = WINDOWS_1258.decode(&code);
                            cow.to_string()
                        }
                        EncodeType::Ascii => {
                            // ASCII is a subset of UTF-8
                            String::from_utf8_lossy(&code).to_string()
                        }
                        EncodeType::Ibm866 => {
                            let (cow, _, _) = IBM866.decode(&code);
                            cow.to_string()
                        }
                        EncodeType::Iso88591 => {
                            // ISO-8859-1 not available, use WINDOWS_1252 as superset
                            let (cow, _, _) = WINDOWS_1252.decode(&code);
                            cow.to_string()
                        }
                        EncodeType::Iso88592 => {
                            let (cow, _, _) = ISO_8859_2.decode(&code);
                            cow.to_string()
                        }
                        EncodeType::Iso88593 => {
                            let (cow, _, _) = ISO_8859_3.decode(&code);
                            cow.to_string()
                        }
                        EncodeType::Iso88594 => {
                            let (cow, _, _) = ISO_8859_4.decode(&code);
                            cow.to_string()
                        }
                        EncodeType::Iso88595 => {
                            let (cow, _, _) = ISO_8859_5.decode(&code);
                            cow.to_string()
                        }
                        EncodeType::Iso88596 => {
                            let (cow, _, _) = ISO_8859_6.decode(&code);
                            cow.to_string()
                        }
                        EncodeType::Iso88597 => {
                            let (cow, _, _) = ISO_8859_7.decode(&code);
                            cow.to_string()
                        }
                        EncodeType::Iso88598 => {
                            let (cow, _, _) = ISO_8859_8.decode(&code);
                            cow.to_string()
                        }
                        EncodeType::Iso885910 => {
                            let (cow, _, _) = ISO_8859_10.decode(&code);
                            cow.to_string()
                        }
                        EncodeType::Iso885913 => {
                            let (cow, _, _) = ISO_8859_13.decode(&code);
                            cow.to_string()
                        }
                        EncodeType::Iso885914 => {
                            let (cow, _, _) = ISO_8859_14.decode(&code);
                            cow.to_string()
                        }
                        EncodeType::Iso885915 => {
                            let (cow, _, _) = ISO_8859_15.decode(&code);
                            cow.to_string()
                        }
                        EncodeType::Iso885916 => {
                            let (cow, _, _) = ISO_8859_16.decode(&code);
                            cow.to_string()
                        }
                        EncodeType::Koi8R => {
                            let (cow, _, _) = KOI8_R.decode(&code);
                            cow.to_string()
                        }
                        EncodeType::Koi8U => {
                            let (cow, _, _) = KOI8_U.decode(&code);
                            cow.to_string()
                        }
                        EncodeType::MacRoman => {
                            let (cow, _, _) = MACINTOSH.decode(&code);
                            cow.to_string()
                        }
                    };

                    DataField::from_chars(in_val.get_name().to_string(), val_str)
                } else {
                    DataField::from_chars(in_val.get_name().to_string(), String::new())
                }
            }
            _ => in_val,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::core::DataTransformer;
    use crate::parser::oml_parse_raw;
    use wp_data_model::cache::FieldQueryCache;
    use wp_model_core::model::{DataField, DataRecord};

    #[test]
    fn test_pipe_base64() {
        let cache = &mut FieldQueryCache::default();
        let data = vec![
            DataField::from_chars("A1", "hello1"),
            DataField::from_chars(
                "B2",
                "UE9TVCAvYWNjb3VudCBIVFRQLzEuMQ0KSG9zdDogZnRwLXh0by5lbmVyZ3ltb3N0LmNvbTo2MTIyMg0KVXNlci1BZ2VudDogTW96aWxsYS81LjAgKE1hY2ludG9zaDsgSW50ZWwgTWFjIE9TIFggMTBfMTVfNykgQXBwbGVXZWJLaXQvNTM3LjM2IChLSFRNTCwgbGlrZSBHZWNrbykgQ2hyb21lLzEwMS4wLjAuMCBTYWZhcmkvNTM3LjM2DQpDb250ZW50LUxlbmd0aDogMTE0DQpDb25uZWN0aW9uOiBjbG9zZQ0KQ29udGVudC1UeXBlOiBhcHBsaWNhdGlvbi94LXd3dy1mb3JtLXVybGVuY29kZWQNCkFjY2VwdC1FbmNvZGluZzogZ3ppcA0KDQo=",
            ),
            DataField::from_chars(
                "C3",
                "U1NILTIuMC1tb2Rfc2Z0cA0KAAADVAcUUhSdWEFUvYFEugJ7xA68OgAAAT1jdXJ2ZTI1NTE5LXNoYTI1NixjdXJ2ZTI1NTE5LXNoYTI1NkBsaWJzc2gub3JnLGVjZGgtc2hhMi1uaXN0cDUyMSxlY2RoLXNoYTItbmlzdHAzODQsZWNkaC1zaGEyLW5pc3RwMjU2LGRpZmZpZS1oZWxsbWFuLWdyb3VwMTgtc2hhNTEyLGRpZmZpZS1oZWxsbWFuLWdyb3VwMTYtc2hhNTEyLGRpZmZpZS1oZWxsbWFuLWdyb3VwMTQtc2hhMjU2LGRpZmZpZS1oZWxsbWFuLWdyb3VwLWV4Y2hhbmdlLXNoYTI1NixkaWZmaWUtaGVsbG1hbi1ncm91cC1leGNoYW5nZS1zaGExLGRpZmZpZS1oZWxsbWFuLWdyb3VwMTQtc2hhMSxyc2ExMDI0LXNoYTEsZXh0LWluZm8tcwAAAClyc2Etc2hhMi01MTIscnNhLXNoYTItMjU2LHNzaC1yc2Esc3NoLWRzcwAAAF9hZXMyNTYtY3RyLGFlczE5Mi1jdHIsYWVzMTI4LWN0cixhZXMyNTYtY2JjLGFlczE5Mi1jYmMsYWVzMTI4LWNiYyxjYXN0MTI4LWNiYywzZGVzLWN0ciwzZGVzLWNiYwAAAF9hZXMyNTYtY3RyLGFlczE5Mi1jdHIsYWVzMTI4LWN0cixhZXMyNTYtY2JjLGFlczE5Mi1jYmMsYWVzMTI4LWNiYyxjYXN0MTI4LWNiYywzZGVzLWN0ciwzZGVzLWNiYwAAAFtobWFjLXNoYTItMjU2LGhtYWMtc2hhMi01MTIsaG1hYy1zaGExLGhtYWMtc2hhMS05Nix1bWFjLTY0QG9wZW5zc2guY29tLHVtYWMtMTI4QG9wZW5zc2guY29tAAAAW2htYWMtc2hhMi0yNTYsaG1hYy1zaGEyLTUxMixobWFjLXNoYTEsaG1hYy1zaGExLTk2LHVtYWMtNjRAb3BlbnNzaC5jb20sdW1hYy0xMjhAb3BlbnNzaC5jb20AAAAaemxpYkBvcGVuc3NoLmNvbSx6bGliLG5vbmUAAAAaemxpYkBvcGVuc3NoLmNvbSx6bGliLG5vbmUAAAAAAAAAAAAAAAAAXuQ3JWG631Byb3RvY29sIG1pc21hdGNoLgo=",
            ),
        ];
        let src = DataRecord { items: data };

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
        assert_eq!(target.field("X"), Some(&expect));

        let expect = DataField::from_chars("Y".to_string(), r#"POST /account HTTP/1.1\r\nHost: ftp-xto.energymost.com:61222\r\nUser-Agent: Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/101.0.0.0 Safari/537.36\r\nContent-Length: 114\r\nConnection: close\r\nContent-Type: application/x-www-form-urlencoded\r\nAccept-Encoding: gzip\r\n\r\n"#.to_string());
        assert_eq!(target.field("Y"), Some(&expect));

        let expect = DataField::from_chars("Z".to_string(), "SSH-2.0-mod_sftp\\r\\n\\x00\\x00\\x03T\\x07\\x14R\\x14\\x9dXAT\\xbd\\x81D\\xba\\x02{\\xc4\\x0e\\xbc:\\x00\\x00\\x01=curve25519-sha256,curve25519-sha256@libssh.org,ecdh-sha2-nistp521,ecdh-sha2-nistp384,ecdh-sha2-nistp256,diffie-hellman-group18-sha512,diffie-hellman-group16-sha512,diffie-hellman-group14-sha256,diffie-hellman-group-exchange-sha256,diffie-hellman-group-exchange-sha1,diffie-hellman-group14-sha1,rsa1024-sha1,ext-info-s\\x00\\x00\\x00)rsa-sha2-512,rsa-sha2-256,ssh-rsa,ssh-dss\\x00\\x00\\x00_aes256-ctr,aes192-ctr,aes128-ctr,aes256-cbc,aes192-cbc,aes128-cbc,cast128-cbc,3des-ctr,3des-cbc\\x00\\x00\\x00_aes256-ctr,aes192-ctr,aes128-ctr,aes256-cbc,aes192-cbc,aes128-cbc,cast128-cbc,3des-ctr,3des-cbc\\x00\\x00\\x00[hmac-sha2-256,hmac-sha2-512,hmac-sha1,hmac-sha1-96,umac-64@openssh.com,umac-128@openssh.com\\x00\\x00\\x00[hmac-sha2-256,hmac-sha2-512,hmac-sha1,hmac-sha1-96,umac-64@openssh.com,umac-128@openssh.com\\x00\\x00\\x00\\x1azlib@openssh.com,zlib,none\\x00\\x00\\x00\\x1azlib@openssh.com,zlib,none\\x00\\x00\\x00\\x00\\x00\\x00\\x00\\x00\\x00\\x00\\x00\\x00\\x00^\\xe47%a\\xba\\xdfProtocol mismatch.\\n".to_string());
        assert_eq!(target.field("Z"), Some(&expect));
    }
}
