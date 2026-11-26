use crate::language::prelude::*;
use ipnet::IpNet;

pub const PIPE_MASK: &str = "mask";
pub const PIPE_MASK_TEXT: &str = "mask_text";
pub const PIPE_HASH: &str = "hash";
pub const PIPE_TOKENIZE: &str = "tokenize";
pub const PIPE_REMOVE: &str = "remove";

pub const DEFAULT_MASK_CHAR: char = '*';
pub const DEFAULT_PLACEHOLDER: &str = "[REDACTED]";
pub const DEFAULT_IPV4_POOL: &str = "240.0.0.0/4";
pub const DEFAULT_IPV6_POOL: &str = "2001:db8::/32";
pub const DEFAULT_SALT_NAME: &str = "pii_salt";
pub const DEFAULT_KEY_NAME: &str = "pii_key";
pub const DEFAULT_TOKEN_PREFIX: &str = "tok:v1:";

/// Mask kinds. The list is intentionally closed: unknown kinds are parse errors.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MaskKind {
    #[default]
    Text,
    Ip,
    Mobile,
    IdCard,
    Mail,
    Domain,
    Name,
    Address,
    Company,
    KeyMsg,
}

impl MaskKind {
    pub const ALL: [&'static str; 10] = [
        "ip", "mobile", "id_card", "mail", "domain", "name", "address", "company", "keymsg", "text",
    ];

    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "ip" => Some(Self::Ip),
            "mobile" => Some(Self::Mobile),
            "id_card" => Some(Self::IdCard),
            "mail" => Some(Self::Mail),
            "domain" => Some(Self::Domain),
            "name" => Some(Self::Name),
            "address" => Some(Self::Address),
            "company" => Some(Self::Company),
            "keymsg" => Some(Self::KeyMsg),
            "text" => Some(Self::Text),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Ip => "ip",
            Self::Mobile => "mobile",
            Self::IdCard => "id_card",
            Self::Mail => "mail",
            Self::Domain => "domain",
            Self::Name => "name",
            Self::Address => "address",
            Self::Company => "company",
            Self::KeyMsg => "keymsg",
            Self::Text => "text",
        }
    }
}

impl Display for MaskKind {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}

/// Name masking style for `mask(name)`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum NameStyle {
    /// Keep the first character: `张*`.
    #[default]
    Head,
    /// Keep the last character: `*三`.
    Tail,
    /// Mask every character.
    Full,
}

impl NameStyle {
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "head" => Some(Self::Head),
            "tail" => Some(Self::Tail),
            "full" => Some(Self::Full),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Head => "head",
            Self::Tail => "tail",
            Self::Full => "full",
        }
    }
}

/// What to do when the value cannot be masked according to the selected kind.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum OnError {
    /// Replace the whole value with `placeholder` and count it (default).
    #[default]
    Placeholder,
    /// Behave like `placeholder` at runtime; startup validation can reject the model.
    Fail,
}

impl OnError {
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "placeholder" => Some(Self::Placeholder),
            "fail" => Some(Self::Fail),
            _ => None,
        }
    }
}

/// Inline masking operator: `mask(kind, [params])`.
#[derive(Clone, Debug)]
pub struct Mask {
    pub kind: MaskKind,
    pub keep_first: Option<usize>,
    pub keep_last: Option<usize>,
    pub keep_levels: Option<usize>,
    pub keep_domain: Option<bool>,
    pub name_style: Option<NameStyle>,
    pub char: Option<char>,
    pub placeholder: Option<String>,
    pub on_error: OnError,
    pub pool: Option<IpNet>,
    pub allow_unsalted: bool,
}

impl Mask {
    pub fn new(kind: MaskKind) -> Self {
        Self {
            kind,
            keep_first: None,
            keep_last: None,
            keep_levels: None,
            keep_domain: None,
            name_style: None,
            char: None,
            placeholder: None,
            on_error: OnError::Placeholder,
            pool: None,
            allow_unsalted: false,
        }
    }

    pub fn mask_char(&self) -> char {
        self.char.unwrap_or(DEFAULT_MASK_CHAR)
    }

    pub fn placeholder(&self) -> &str {
        self.placeholder.as_deref().unwrap_or(DEFAULT_PLACEHOLDER)
    }
}

impl Display for Mask {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}({}", PIPE_MASK, self.kind)?;
        if let Some(v) = self.keep_first {
            write!(f, ", keep_first:{v}")?;
        }
        if let Some(v) = self.keep_last {
            write!(f, ", keep_last:{v}")?;
        }
        if let Some(v) = self.keep_levels {
            write!(f, ", keep_levels:{v}")?;
        }
        if let Some(v) = self.keep_domain {
            write!(f, ", keep_domain:{v}")?;
        }
        if let Some(v) = self.name_style {
            write!(f, ", style:{}", v.name())?;
        }
        if let Some(v) = self.char {
            write!(f, ", char:'{v}'")?;
        }
        if let Some(v) = &self.placeholder {
            write!(f, ", placeholder:'{v}'")?;
        }
        if let Some(v) = &self.pool {
            write!(f, ", pool:{v}")?;
        }
        if self.allow_unsalted {
            write!(f, ", allow_unsalted:true")?;
        }
        write!(f, ")")
    }
}

/// Kinds supported by the text-scanning operator `mask_text(kind, [params])`.
///
/// This is a separate closed set from [`MaskKind`]: `mask_text` only works on
/// values that can be located inside mixed text, so field-only kinds such as
/// `name` / `address` / `company` are intentionally absent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MaskTextKind {
    /// Aggregate preset: mobile, e-mail, ID card, credential key/value pairs
    /// and HTTP credential headers. IP is intentionally left alone.
    Pii,
    Mobile,
    IdCard,
    Mail,
    Ip,
    Domain,
    /// HTTP credential headers: `Authorization`, `Proxy-Authorization`,
    /// `*-Auth-Token`, `Cookie`, `Set-Cookie`.
    HttpAuth,
}

impl MaskTextKind {
    pub const ALL: [&'static str; 7] = [
        "pii",
        "mobile",
        "id_card",
        "mail",
        "ip",
        "domain",
        "http_auth",
    ];

    /// Human-readable candidate list used in parser error messages.
    pub const DESCRIPTION: &'static str = "pii | mobile | id_card | mail | ip | domain | http_auth";

    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "pii" => Some(Self::Pii),
            "mobile" => Some(Self::Mobile),
            "id_card" => Some(Self::IdCard),
            "mail" => Some(Self::Mail),
            "ip" => Some(Self::Ip),
            "domain" => Some(Self::Domain),
            "http_auth" => Some(Self::HttpAuth),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Pii => "pii",
            Self::Mobile => "mobile",
            Self::IdCard => "id_card",
            Self::Mail => "mail",
            Self::Ip => "ip",
            Self::Domain => "domain",
            Self::HttpAuth => "http_auth",
        }
    }
}

impl Display for MaskTextKind {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}

/// Inline text-scanning operator: `mask_text(kind, [params])`.
///
/// Contract: mask every recognized occurrence of `kind` inside the text and
/// leave the rest untouched. This is *pattern-based masking* (best effort),
/// not the fail-closed whole-value contract of `mask`.
#[derive(Clone, Debug)]
pub struct MaskText {
    pub kind: MaskTextKind,
    pub keep_first: Option<usize>,
    pub keep_last: Option<usize>,
    pub keep_levels: Option<usize>,
    pub keep_domain: Option<bool>,
    pub char: Option<char>,
    pub placeholder: Option<String>,
    pub pool: Option<IpNet>,
    pub allow_unsalted: bool,
}

impl MaskText {
    pub fn new(kind: MaskTextKind) -> Self {
        Self {
            kind,
            keep_first: None,
            keep_last: None,
            keep_levels: None,
            keep_domain: None,
            char: None,
            placeholder: None,
            pool: None,
            allow_unsalted: false,
        }
    }

    pub fn mask_char(&self) -> char {
        self.char.unwrap_or(DEFAULT_MASK_CHAR)
    }

    pub fn placeholder(&self) -> &str {
        self.placeholder.as_deref().unwrap_or(DEFAULT_PLACEHOLDER)
    }
}

impl Display for MaskText {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}({}", PIPE_MASK_TEXT, self.kind)?;
        if let Some(v) = self.keep_first {
            write!(f, ", keep_first:{v}")?;
        }
        if let Some(v) = self.keep_last {
            write!(f, ", keep_last:{v}")?;
        }
        if let Some(v) = self.keep_levels {
            write!(f, ", keep_levels:{v}")?;
        }
        if let Some(v) = self.keep_domain {
            write!(f, ", keep_domain:{v}")?;
        }
        if let Some(v) = self.char {
            write!(f, ", char:'{v}'")?;
        }
        if let Some(v) = &self.placeholder {
            write!(f, ", placeholder:'{v}'")?;
        }
        if let Some(v) = &self.pool {
            write!(f, ", pool:{v}")?;
        }
        if self.allow_unsalted {
            write!(f, ", allow_unsalted:true")?;
        }
        write!(f, ")")
    }
}

/// Hash algorithms supported by `hash`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum HashAlgo {
    #[default]
    Sha256,
    Sha1,
    Md5,
}

impl HashAlgo {
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "sha256" => Some(Self::Sha256),
            "sha1" => Some(Self::Sha1),
            "md5" => Some(Self::Md5),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Sha256 => "sha256",
            Self::Sha1 => "sha1",
            Self::Md5 => "md5",
        }
    }
}

impl Display for HashAlgo {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}

/// Output encoding for `hash`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Encoding {
    #[default]
    Hex,
    Base64Url,
}

impl Encoding {
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "hex" => Some(Self::Hex),
            "base64url" => Some(Self::Base64Url),
            _ => None,
        }
    }
}

/// Inline keyed-hash operator: `hash([algo], [params])`.
#[derive(Clone, Debug)]
pub struct Hash {
    pub algo: HashAlgo,
    pub encoding: Encoding,
    pub truncate: Option<usize>,
}

impl Default for Hash {
    fn default() -> Self {
        Self {
            algo: HashAlgo::Sha256,
            encoding: Encoding::Hex,
            truncate: None,
        }
    }
}

impl Display for Hash {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}({}", PIPE_HASH, self.algo)?;
        if self.encoding == Encoding::Base64Url {
            write!(f, ", encoding:base64url")?;
        }
        if let Some(v) = self.truncate {
            write!(f, ", truncate:{v}")?;
        }
        write!(f, ")")
    }
}

/// Reversible token algorithms.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TokenAlgo {
    #[default]
    AesGcm,
}

impl TokenAlgo {
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "aes_gcm" => Some(Self::AesGcm),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::AesGcm => "aes_gcm",
        }
    }
}

/// Inline reversible-token operator: `tokenize([params])`.
#[derive(Clone, Debug)]
pub struct Tokenize {
    pub algo: TokenAlgo,
    pub prefix: String,
}

impl Default for Tokenize {
    fn default() -> Self {
        Self {
            algo: TokenAlgo::AesGcm,
            prefix: DEFAULT_TOKEN_PREFIX.to_string(),
        }
    }
}

impl Display for Tokenize {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}(", PIPE_TOKENIZE)?;
        let mut first = true;
        if self.algo != TokenAlgo::AesGcm {
            if !first {
                write!(f, ", ")?;
            }
            write!(f, "algo:{}", self.algo.name())?;
            first = false;
        }
        if self.prefix != DEFAULT_TOKEN_PREFIX {
            if !first {
                write!(f, ", ")?;
            }
            write!(f, "prefix:'{}'", self.prefix)?;
        }
        write!(f, ")")
    }
}

/// Terminal operator: drop the target field from output.
#[derive(Clone, Copy, Debug, Default)]
pub struct Remove;

impl Display for Remove {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{PIPE_REMOVE}")
    }
}

#[cfg(test)]
mod tests {
    use super::MaskTextKind;

    #[test]
    fn mask_text_description_matches_all() {
        assert_eq!(MaskTextKind::ALL.join(" | "), MaskTextKind::DESCRIPTION);
    }

    #[test]
    fn mask_text_kind_names_round_trip() {
        for name in MaskTextKind::ALL {
            let kind = MaskTextKind::from_name(name).expect("known kind");
            assert_eq!(kind.name(), name);
        }
    }
}
