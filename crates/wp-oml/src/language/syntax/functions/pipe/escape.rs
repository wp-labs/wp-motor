pub const PIPE_HTML_ESCAPE: &str = "html_escape";
#[derive(Clone, Debug, Default)]
pub struct HtmlEscape {}

pub const PIPE_HTML_UNESCAPE: &str = "html_unescape";
#[derive(Clone, Debug, Default)]
pub struct HtmlUnescape {}

pub const PIPE_STR_ESCAPE: &str = "str_escape";
#[derive(Clone, Debug, Default)]
pub struct StrEscape {}

#[allow(dead_code)]
pub const PIPE_STR_UNESCAPE: &str = "str_unescape";
