use crate::resources::RuleKey;
use crate::resources::core::manager::ResManager;
use wp_conf::structure::FlexGroup;
use wp_specs::WildArray;

// 辅助函数：创建 WildArray
fn extend_matches<S: Into<String>>(rules: Vec<S>) -> WildArray {
    use wildmatch::WildMatch;
    let mut out = Vec::new();
    for item in rules {
        let x: String = item.into();
        out.push(WildMatch::new(&x));
    }
    WildArray(out)
}

// 简单的单元测试，验证 FlexGroup Rule 核心逻辑
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wildarray_matching() {
        // 测试 WildArray 的匹配逻辑
        let rule_pattern = extend_matches(vec!["/test/rule*"]);

        // WildArray 内部包含 WildMatch，我们检查它是否包含我们期望的元素
        assert!(
            !rule_pattern.as_ref().is_empty(),
            "Should contain rule pattern"
        );

        // 测试多个规则的 WildArray
        let multi_rule = extend_matches(vec!["/api/*", "/test/rule1"]);
        assert_eq!(
            multi_rule.as_ref().len(),
            2,
            "Should contain two rule patterns"
        );
    }

    #[test]
    fn test_flexgroup_creation() {
        // 测试 FlexGroup 创建和基本属性
        let flex_group = FlexGroup::test_new("test_group", "/test/rule1");

        assert_eq!(flex_group.name(), "test_group");
        assert_eq!(*flex_group.parallel(), 1);
        assert!(!flex_group.rule().as_ref().is_empty());
        assert!(flex_group.oml().as_ref().is_empty());
        assert!(flex_group.tags().is_empty());
        assert!(flex_group.filter().is_none());
    }

    #[test]
    fn test_resmanager_initialization() {
        // 测试 ResManager 基本初始化
        let res_manager = ResManager::default();

        // 验证初始状态
        assert!(res_manager.rule_sink_db().rule_sink_idx().is_empty());
        assert!(res_manager.name_mdl_res().is_empty());
        assert!(res_manager.mdl_sink_map().is_empty());
        assert!(res_manager.rule_mdl_relation().0.is_empty());
    }

    #[test]
    fn test_rulekey_creation() {
        // 测试 RuleKey 创建和使用
        let rule_key1 = RuleKey::from("/test/rule1");
        let rule_key2 = RuleKey::from("/test/rule2");

        assert_ne!(rule_key1.0, rule_key2.0);
        assert_eq!(rule_key1.0, "/test/rule1");
        assert_eq!(rule_key2.0, "/test/rule2");

        // 测试从字符串引用创建
        let rule_key3 = RuleKey::from(&String::from("/test/rule3"));
        assert_eq!(rule_key3.0, "/test/rule3");
    }
}
