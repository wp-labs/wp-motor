use once_cell::sync::OnceCell;

#[cfg(test)]
//#[ctor::ctor]
fn init_env() {
    once_init_log();
}

struct TestIniter {}

pub fn once_init_log() {
    static INSTANCE: OnceCell<TestIniter> = OnceCell::new();
    INSTANCE.get_or_init(|| {
        // 统一使用 wp_log 的测试初始化（输出到控制台，级别=debug）
        let _ = wp_log::conf::log_for_test();
        wp_log::debug_ctrl!("log inited!");
        TestIniter {}
    });
}
