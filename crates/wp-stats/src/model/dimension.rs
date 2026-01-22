use orion_overload::new::New1;
use serde_derive::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::fmt::{Display, Formatter};
use wp_model_core::model::DataField;

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct DataDim {
    item: Vec<Option<String>>,
}

impl From<&str> for DataDim {
    fn from(value: &str) -> Self {
        <DataDim as New1<String>>::new(value.to_string())
    }
}
impl DataDim {
    pub fn empty() -> Self {
        Self { item: Vec::new() }
    }
    pub fn push(&mut self, v: Option<String>) {
        self.item.push(v)
    }
    pub fn to_tdc(&self, req: &[String]) -> Vec<DataField> {
        let mut result = Vec::new();
        let mut idx = 0;
        while let (Some(k), Some(v)) = (req.get(idx), self.item.get(idx)) {
            if let Some(v) = v {
                result.push(DataField::from_chars(k.clone(), v.clone()))
            }
            idx += 1;
        }
        result
    }
}

impl New1<String> for DataDim {
    fn new(a: String) -> Self {
        Self {
            item: vec![Some(a)],
        }
    }
}
impl New1<&str> for DataDim {
    fn new(a: &str) -> Self {
        Self {
            item: vec![Some(a.into())],
        }
    }
}

impl New1<(String, String)> for DataDim {
    fn new(a: (String, String)) -> Self {
        Self {
            item: vec![Some(a.0), Some(a.1)],
        }
    }
}
impl New1<(&str, &str)> for DataDim {
    fn new(a: (&str, &str)) -> Self {
        Self {
            item: vec![Some(a.0.into()), Some(a.1.into())],
        }
    }
}
impl New1<(String, String, String)> for DataDim {
    fn new(a: (String, String, String)) -> Self {
        Self {
            item: vec![Some(a.0), Some(a.1), Some(a.2)],
        }
    }
}

impl New1<(&str, &str, &str)> for DataDim {
    fn new(a: (&str, &str, &str)) -> Self {
        Self {
            item: vec![Some(a.0.into()), Some(a.1.into()), Some(a.2.into())],
        }
    }
}

impl PartialOrd<Self> for DataDim {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for DataDim {
    fn cmp(&self, other: &Self) -> Ordering {
        let mut idx = 0;
        let mut last_ord = Ordering::Equal;
        loop {
            if let (Some(l), Some(r)) = (self.item.get(idx), other.item.get(idx)) {
                last_ord = l.cmp(r);
                if last_ord == Ordering::Equal {
                    idx += 1;
                    continue;
                }
                break;
            } else {
                if self.item.get(idx).is_some() {
                    return Ordering::Greater;
                }
                if other.item.get(idx).is_some() {
                    return Ordering::Less;
                }
                break;
            }
        }
        last_ord
    }
}
impl Display for DataDim {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        //write!(f, "{}", self.item.join(","))?;
        let mut sep = "";
        for i in &self.item {
            write!(f, "{}{}", sep, opt_string(i))?;
            sep = ",";
        }
        Ok(())
    }
}

#[derive(Debug, PartialEq, Deserialize, Serialize, Clone)]
pub enum StatTarget {
    #[serde(rename = "all")]
    All,
    #[serde(rename = "ignore")]
    Ignore,
    #[serde(untagged)]
    Item(String),
}
impl Display for StatTarget {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            StatTarget::All => {
                write!(f, "all")?;
            }
            StatTarget::Ignore => {
                write!(f, "ignore")?;
            }
            StatTarget::Item(v) => {
                write!(f, "item({})", v)?;
            }
        }
        Ok(())
    }
}

pub fn opt_string(o_v: &Option<String>) -> &str {
    if let Some(v) = o_v { v } else { "*" }
}
pub fn join_string(vs: &[String]) -> String {
    vs.join(",").to_string()
}

#[cfg(test)]
mod tests {
    use std::cmp::Ordering;

    use orion_overload::new::New1;

    use super::{join_string, opt_string};
    use crate::{DataDim, StatTarget};
    type AnyResult<T> = Result<T, anyhow::Error>;

    //test for StatRule Serialize and Deserialize
    #[test]
    fn test_stat_rule() -> AnyResult<()> {
        // 序列化示例
        let rule1 = StatTarget::All;
        let rule2 = StatTarget::Ignore;
        let rule3 = StatTarget::Item("example".to_string());

        let json1 = serde_json::to_string(&rule1)?;
        let json2 = serde_json::to_string(&rule2)?;
        let json3 = serde_json::to_string(&rule3)?;

        println!("Serialized All: {}", json1); // 输出: "all"
        println!("Serialized Ignore: {}", json2); // 输出: "ignore"
        println!("Serialized Item: {}", json3); // 输出: "example"

        // 反序列化示例
        let deserialized1: StatTarget = serde_json::from_str(&json1)?;
        let deserialized2: StatTarget = serde_json::from_str(&json2)?;
        let deserialized3: StatTarget = serde_json::from_str(&json3)?;

        println!("Deserialized All: {:?}", deserialized1); // 输出: All
        println!("Deserialized Ignore: {:?}", deserialized2); // 输出: Ignore
        println!("Deserialized Item: {:?}", deserialized3); // 输出: Item("example")

        Ok(())
    }
    #[test]
    fn test_data_dim() {
        let a = DataDim::new(("a", "b"));
        let b = DataDim::new(("a", "c"));
        let c = DataDim::new("a");
        assert_eq!(a.cmp(&b), Ordering::Less);
        assert_eq!(c.cmp(&a), Ordering::Less);
        let d = DataDim::new(("a", "c"));
        assert_eq!(b.cmp(&d), Ordering::Equal);
        assert_eq!(d.cmp(&a), Ordering::Greater);
    }

    #[test]
    fn test_data_dim_from_str() {
        let dim = DataDim::from("test");
        assert_eq!(dim.to_string(), "test");
    }

    #[test]
    fn test_data_dim_single_string() {
        let dim = DataDim::new("single".to_string());
        assert_eq!(dim.to_string(), "single");
    }

    #[test]
    fn test_data_dim_tuple2() {
        let dim1 = DataDim::new(("first", "second"));
        assert_eq!(dim1.to_string(), "first,second");

        let dim2 = DataDim::new(("first".to_string(), "second".to_string()));
        assert_eq!(dim2.to_string(), "first,second");
    }

    #[test]
    fn test_data_dim_tuple3() {
        let dim1 = DataDim::new(("a", "b", "c"));
        assert_eq!(dim1.to_string(), "a,b,c");

        let dim2 = DataDim::new(("a".to_string(), "b".to_string(), "c".to_string()));
        assert_eq!(dim2.to_string(), "a,b,c");
    }

    #[test]
    fn test_data_dim_empty() {
        let dim = DataDim::empty();
        assert_eq!(dim.to_string(), "");
    }

    #[test]
    fn test_data_dim_push() {
        let mut dim = DataDim::empty();
        dim.push(Some("first".to_string()));
        dim.push(Some("second".to_string()));
        dim.push(None);

        assert_eq!(dim.to_string(), "first,second,*");
    }

    #[test]
    fn test_data_dim_to_tdc() {
        let mut dim = DataDim::empty();
        dim.push(Some("value1".to_string()));
        dim.push(Some("value2".to_string()));

        let req = vec!["field1".to_string(), "field2".to_string()];
        let tdc = dim.to_tdc(&req);

        assert_eq!(tdc.len(), 2);
    }

    #[test]
    fn test_data_dim_to_tdc_with_none() {
        let mut dim = DataDim::empty();
        dim.push(Some("value1".to_string()));
        dim.push(None);
        dim.push(Some("value3".to_string()));

        let req = vec![
            "field1".to_string(),
            "field2".to_string(),
            "field3".to_string(),
        ];
        let tdc = dim.to_tdc(&req);

        // Should only have 2 fields (field1 and field3), field2 is None
        assert_eq!(tdc.len(), 2);
    }

    #[test]
    fn test_data_dim_ordering_different_lengths() {
        let short = DataDim::new("a");
        let long = DataDim::new(("a", "b"));

        assert_eq!(short.cmp(&long), Ordering::Less);
        assert_eq!(long.cmp(&short), Ordering::Greater);
    }

    #[test]
    fn test_data_dim_ordering_equal() {
        let dim1 = DataDim::new(("x", "y", "z"));
        let dim2 = DataDim::new(("x", "y", "z"));

        assert_eq!(dim1.cmp(&dim2), Ordering::Equal);
        assert_eq!(dim1, dim2);
    }

    #[test]
    fn test_data_dim_hash() {
        use std::collections::HashSet;

        let dim1 = DataDim::new(("a", "b"));
        let dim2 = DataDim::new(("a", "b"));
        let dim3 = DataDim::new(("a", "c"));

        let mut set = HashSet::new();
        set.insert(dim1);
        set.insert(dim2); // Should not add duplicate

        assert_eq!(set.len(), 1);

        set.insert(dim3);
        assert_eq!(set.len(), 2);
    }

    #[test]
    fn test_opt_string_helper() {
        assert_eq!(opt_string(&Some("test".to_string())), "test");
        assert_eq!(opt_string(&None), "*");
    }

    #[test]
    fn test_join_string_helper() {
        let strings = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        assert_eq!(join_string(&strings), "a,b,c");

        let empty: Vec<String> = vec![];
        assert_eq!(join_string(&empty), "");
    }
}
