mod prelude;
pub(crate) use syntax::evaluators::precise::{
    data_field_extract_more, data_field_extract_one, data_field_support_batch, value_extract_more,
    value_extract_one, value_extract_storage, value_support_batch,
};
pub use syntax::{
    NestedBinding,
    OmlKwGet,
    VarAccess,
    accessors::{ArrOperation, FieldRead, FieldTake, FieldTakeBuilder, ReadOptionBuilder},
    accessors::{CondAccessor, DirectAccessor, GenericAccessor, NestedAccessor},
    accessors::{SqlFnArg, SqlFnExpr},
    bindings::GenericBinding,
    conditions::{ArgsTakeAble, CompareExpress, LogicalExpression},
    evaluators::{
        BatchEvalExp, BatchEvalExpBuilder, BatchEvaluation, EvalExp, PreciseEvaluator,
        SingleEvalExp, SingleEvalExpBuilder,
    },
    functions::{
        Base64Decode, Base64Encode, BuiltinFunction, DEFAULT_IPV4_POOL, DEFAULT_IPV6_POOL,
        DEFAULT_KEY_NAME, DEFAULT_MASK_CHAR, DEFAULT_PLACEHOLDER, DEFAULT_SALT_NAME,
        DEFAULT_TOKEN_PREFIX, Dumb, EncodeType, Encoding, ExtractMainWord, ExtractSubjectObject,
        FUN_NOW_DATE, FUN_NOW_HOUR, FUN_NOW_TIME, FunOperation, Get, Hash, HashAlgo, HtmlEscape,
        HtmlUnescape, IntranetIp, IntranetReplace, Ip4ToInt, IpToBigUint, JsonEscape, JsonUnescape,
        MapTo, MapValue, Mask, MaskKind, NameStyle, NowDate, NowHour, NowTime, Nth, OnError,
        OnFail, PIPE_BASE64_DECODE, PIPE_BASE64_ENCODE, PIPE_EXTRACT_MAIN_WORD,
        PIPE_EXTRACT_SUBJECT_OBJECT, PIPE_GET, PIPE_HASH, PIPE_HTML_ESCAPE, PIPE_HTML_UNESCAPE,
        PIPE_INTRANET_IP, PIPE_INTRANET_REPLACE, PIPE_IP_TO_BIGUINT, PIPE_IP4_TO_INT,
        PIPE_JSON_ESCAPE, PIPE_JSON_UNESCAPE, PIPE_MAP_TO, PIPE_MASK, PIPE_NTH, PIPE_ON_FAIL,
        PIPE_PATH, PIPE_REMOVE, PIPE_SKIP_EMPTY, PIPE_STARTS_WITH, PIPE_STR_ESCAPE,
        PIPE_TIME_FROM_TS, PIPE_TIME_FROM_TS_MS, PIPE_TIME_FROM_TS_US, PIPE_TIME_TO_TS,
        PIPE_TIME_TO_TS_MS, PIPE_TIME_TO_TS_US, PIPE_TIME_TO_TS_ZONE, PIPE_TO_JSON, PIPE_TO_STR,
        PIPE_TOKENIZE, PIPE_URL, PathGet, PathType, PipeFun, Remove, SkipEmpty, StartsWith,
        StrEscape, TimeFromTs, TimeFromTsMs, TimeFromTsUs, TimeStampUnit, TimeToTs, TimeToTsMs,
        TimeToTsUs, TimeToTsZone, ToJson, ToStr, TokenAlgo, Tokenize, UrlGet, UrlType,
        ip_to_biguint,
    },
    //lib_prm::LookupQuery,
    operations::{
        AccessDirectOperation, CalcExpr, CalcFun, CalcNumber, CalcOp, CalcOperation, FmtOperation,
        LookupDict, LookupOperation, MapOperation, MatchAble, MatchCase, MatchCond, MatchCondition,
        MatchFun, MatchOperation, MatchSource, ObjArrayOperation, PiPeOperation, PipeSource,
        RecordOperation, RecordOperationBuilder, SqlKnowledgeRoute, SqlQuery,
    },
};
pub use types::model::DataModel;
pub use types::model::ObjModel;
pub use types::model::StubModel;
pub use types::target::{BatchEvalTarget, EvaluationTarget, EvaluationTargetBuilder};
mod syntax;
mod types;
pub const DCT_GET: &str = "get";
pub const DCT_OPTION: &str = "option";
pub const OML_CRATE_IN: &str = "in";
