#[cfg(test)]
pub mod for_test {
    #[allow(dead_code)]
    pub fn once_init_log() {
        use once_cell::sync::OnceCell;
        struct TestIniter {}

        static INSTANCE: OnceCell<TestIniter> = OnceCell::new();
        INSTANCE.get_or_init(|| {
            // 使用统一日志初始化，测试默认输出到控制台，级别为 debug
            let _ = wp_log::conf::log_for_test();
            wp_log::debug_ctrl!("log inited!");
            TestIniter {}
        });
    }
}
