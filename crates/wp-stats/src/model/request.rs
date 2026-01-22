use crate::model::dimension::StatTarget;
use crate::model::record::StatStage;
use std::fmt::Display;

/// Statistical collection requirements
///
/// Defines what and how statistics should be collected,
/// including stage, target filtering, and collection limits.
#[derive(Clone, Debug)]
pub struct StatReq {
    pub stage: StatStage,
    pub name: String,
    pub target: StatTarget,
    pub collect: Vec<String>,
    pub max: usize,
}

impl StatReq {
    /// Creates a simple test StatReq for testing purposes
    pub fn simple_test(target: StatTarget, collect: Vec<String>, max: usize) -> Self {
        Self {
            stage: StatStage::Pick,
            name: "unknown".to_string(),
            target,
            collect,
            max,
        }
    }

    /// Creates a named test StatReq for testing purposes
    pub fn simple_test2(name: &str, target: StatTarget, collect: Vec<String>, max: usize) -> Self {
        Self {
            stage: StatStage::Pick,
            name: name.to_string(),
            target,
            collect,
            max,
        }
    }

    /// Checks if the given target matches the requirements
    pub fn match_target(&self, target: &str) -> bool {
        match &self.target {
            StatTarget::All => true,
            StatTarget::Ignore => false,
            StatTarget::Item(item) => target == item,
        }
    }

    /// Returns a display string for data collection fields
    pub fn data_display(&self) -> Option<String> {
        if self.collect.is_empty() {
            None
        } else {
            Some(self.collect.join(",").to_string())
        }
    }
}

impl Display for StatReq {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(
            f,
            "stage: {} ,name: {}, match: {} , key: {}, max: {}",
            self.stage,
            self.name,
            self.target,
            self.collect.join(","),
            self.max
        )?;
        Ok(())
    }
}

/// Container for multiple statistical requirements
#[derive(Clone)]
pub struct StatRequires {
    items: Vec<StatReq>,
}

impl StatRequires {
    /// Creates a StatRequires from a vector of requirements
    pub fn from(items: Vec<StatReq>) -> Self {
        Self { items }
    }

    /// Returns all requirements for a specific stage
    pub fn get_requ_items(&self, stage: StatStage) -> Vec<StatReq> {
        let mut out = Vec::new();
        for i in &self.items {
            if i.stage == stage {
                out.push(i.clone())
            }
        }
        out
    }

    /// Returns a reference to all requirements
    pub fn get_all(&self) -> &Vec<StatReq> {
        &self.items
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stat_req_match_target_all() {
        let req = StatReq::simple_test(StatTarget::All, Vec::new(), 10);

        assert!(req.match_target("any_target"));
        assert!(req.match_target("another_target"));
    }

    #[test]
    fn test_stat_req_match_target_ignore() {
        let req = StatReq::simple_test(StatTarget::Ignore, Vec::new(), 10);

        assert!(!req.match_target("any_target"));
        assert!(!req.match_target("another_target"));
    }

    #[test]
    fn test_stat_req_match_target_item() {
        let req = StatReq::simple_test(StatTarget::Item("specific".to_string()), Vec::new(), 10);

        assert!(req.match_target("specific"));
        assert!(!req.match_target("other"));
    }

    #[test]
    fn test_stat_req_data_display_empty() {
        let req = StatReq::simple_test(StatTarget::All, Vec::new(), 10);
        assert_eq!(req.data_display(), None);
    }

    #[test]
    fn test_stat_req_data_display_with_fields() {
        let fields = vec![
            "field1".to_string(),
            "field2".to_string(),
            "field3".to_string(),
        ];
        let req = StatReq::simple_test(StatTarget::All, fields, 10);

        assert_eq!(req.data_display(), Some("field1,field2,field3".to_string()));
    }

    #[test]
    fn test_stat_req_simple_test() {
        let req = StatReq::simple_test(StatTarget::All, Vec::new(), 15);

        assert_eq!(req.stage, StatStage::Pick);
        assert_eq!(req.name, "unknown");
        assert_eq!(req.max, 15);
    }

    #[test]
    fn test_stat_req_simple_test2() {
        let req = StatReq::simple_test2("custom_name", StatTarget::All, Vec::new(), 20);

        assert_eq!(req.stage, StatStage::Pick);
        assert_eq!(req.name, "custom_name");
        assert_eq!(req.max, 20);
    }

    #[test]
    fn test_stat_requires_from() {
        let req1 = StatReq::simple_test(StatTarget::All, Vec::new(), 10);
        let req2 = StatReq::simple_test2("test", StatTarget::All, Vec::new(), 5);

        let requires = StatRequires::from(vec![req1, req2]);

        assert_eq!(requires.get_all().len(), 2);
    }

    #[test]
    fn test_stat_requires_get_stage_items() {
        let req1 = StatReq {
            stage: StatStage::Pick,
            name: "pick1".to_string(),
            target: StatTarget::All,
            collect: Vec::new(),
            max: 10,
        };
        let req2 = StatReq {
            stage: StatStage::Parse,
            name: "parse1".to_string(),
            target: StatTarget::All,
            collect: Vec::new(),
            max: 10,
        };
        let req3 = StatReq {
            stage: StatStage::Pick,
            name: "pick2".to_string(),
            target: StatTarget::All,
            collect: Vec::new(),
            max: 10,
        };

        let requires = StatRequires::from(vec![req1, req2, req3]);

        let pick_items = requires.get_requ_items(StatStage::Pick);
        assert_eq!(pick_items.len(), 2);

        let parse_items = requires.get_requ_items(StatStage::Parse);
        assert_eq!(parse_items.len(), 1);

        let sink_items = requires.get_requ_items(StatStage::Sink);
        assert_eq!(sink_items.len(), 0);
    }

    #[test]
    fn test_stat_req_display() {
        let req = StatReq::simple_test2(
            "test_req",
            StatTarget::Item("target1".to_string()),
            vec!["field1".to_string(), "field2".to_string()],
            10,
        );

        let display = format!("{}", req);
        assert!(display.contains("test_req"));
        assert!(display.contains("field1,field2"));
        assert!(display.contains("10"));
    }

    #[test]
    fn test_stat_target_display() {
        assert_eq!(format!("{}", StatTarget::All), "all");
        assert_eq!(format!("{}", StatTarget::Ignore), "ignore");
        assert_eq!(
            format!("{}", StatTarget::Item("test".to_string())),
            "item(test)"
        );
    }
}
