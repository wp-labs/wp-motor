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
        Base64Decode, Base64Encode, BuiltinFunction, Dumb, EncodeType, ExtractMainWord,
        ExtractSubjectObject, FUN_NOW_DATE, FUN_NOW_HOUR, FUN_NOW_TIME, FunOperation, Get,
        HtmlEscape, HtmlUnescape, Ip4ToInt, IpToBigUint, JsonEscape, JsonUnescape, MapTo, MapValue,
        NowDate, NowHour, NowTime, Nth, PIPE_BASE64_DECODE, PIPE_BASE64_ENCODE,
        PIPE_EXTRACT_MAIN_WORD, PIPE_EXTRACT_SUBJECT_OBJECT, PIPE_GET, PIPE_HTML_ESCAPE,
        PIPE_HTML_UNESCAPE, PIPE_IP_TO_BIGUINT, PIPE_IP4_TO_INT, PIPE_JSON_ESCAPE,
        PIPE_JSON_UNESCAPE, PIPE_MAP_TO, PIPE_NTH, PIPE_PATH, PIPE_SKIP_EMPTY, PIPE_STARTS_WITH,
        PIPE_STR_ESCAPE, PIPE_TIME_TO_TS, PIPE_TIME_TO_TS_MS, PIPE_TIME_TO_TS_US,
        PIPE_TIME_TO_TS_ZONE, PIPE_TO_JSON, PIPE_TO_STR, PIPE_URL, PathGet, PathType, PipeFun,
        SkipEmpty, StartsWith, StrEscape, TimeStampUnit, TimeToTs, TimeToTsMs, TimeToTsUs,
        TimeToTsZone, ToJson, ToStr, UrlGet, UrlType, ip_to_biguint,
    },
    //lib_prm::LookupQuery,
    operations::{
        CalcExpr, CalcFun, CalcNumber, CalcOp, CalcOperation, FmtOperation, LookupDict,
        LookupOperation, MapOperation, MatchAble, MatchCase, MatchCond, MatchCondition, MatchFun,
        MatchOperation, MatchSource, PiPeOperation, RecordOperation, RecordOperationBuilder,
        SqlKnowledgeRoute, SqlQuery,
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
