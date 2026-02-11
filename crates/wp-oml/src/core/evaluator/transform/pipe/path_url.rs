use crate::core::prelude::*;
use crate::language::{PathGet, PathType, UrlGet, UrlType};
use std::path::Path;
use url::{Position, Url};
use wp_model_core::model::{DataField, Value};

/// 路径解析 - path(name|path)
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

/// URL 解析 - url(domain|host|uri|path|params)
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
#[cfg(test)]
mod tests {
    use crate::core::DataTransformer;
    use crate::parser::oml_parse_raw;
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
}
