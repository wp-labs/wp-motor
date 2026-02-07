//! Common parser context strings and literals for consistent error messages.

/// Group meta hint used in error messages.
pub const CTX_GROUP_META_HINT: &str = "alt,opt,some_of,seq,no";

/// Description when expecting a group meta keyword before '('.
pub const CTX_EXPECT_GROUP_META: &str = "expect group meta before '('";

/// Description for the content within a group parenthesis.
pub const CTX_GROUP_CONTENT: &str = "group '( ... )' content";
